use gpui_kit::{App, Global, component};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[derive(Clone, Copy, Default, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    Zh,
    En,
}
impl Locale {
    fn component_locale(self) -> &'static str {
        match self {
            Self::Zh => "zh-CN",
            Self::En => "en",
        }
    }
}
enum Notice {
    LoadedError(String),
    Saved,
    SaveError(String),
}
#[derive(Default)]
pub struct Language {
    pub locale: Locale,
    root: Option<PathBuf>,
    notice: Option<Notice>,
}
impl Global for Language {}

pub fn english() -> bool {
    component::locale().starts_with("en")
}

// Only explicitly marked application-owned static text is translated. Runtime
// messages, user content, tools and diagnostics never pass through this lookup.
pub fn tr(chinese: &'static str) -> &'static str {
    if !english() {
        return chinese;
    }
    static ENGLISH: OnceLock<HashMap<String, String>> = OnceLock::new();
    ENGLISH
        .get_or_init(|| {
            serde_json::from_str(include_str!("locales/en.json"))
                .expect("embedded English catalogue")
        })
        .get(chinese)
        .map(String::as_str)
        .unwrap_or(chinese)
}
fn load(root: &Path) -> Result<Locale, String> {
    match fs::File::open(root.join("desktop-language.json")) {
        Ok(file) => serde_json::from_reader(file).map_err(|e| e.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Locale::default()),
        Err(error) => Err(error.to_string()),
    }
}
fn save(root: &Path, locale: Locale) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    crate::file_save::save_file(
        &root.join("desktop-language.json"),
        &serde_json::to_vec(&locale).map_err(|e| e.to_string())?,
    )
}
pub fn restore(root: &Path, cx: &mut App) {
    if cx.global::<Language>().root.is_some() {
        return;
    }
    let result = load(root);
    let state = cx.global_mut::<Language>();
    state.root = Some(root.to_owned());
    match result {
        Ok(locale) => state.locale = locale,
        Err(error) => state.notice = Some(Notice::LoadedError(error)),
    }
    component::set_locale(state.locale.component_locale());
    cx.refresh_windows();
}
pub fn choose(locale: Locale, cx: &mut App) -> bool {
    let Some(root) = cx.global::<Language>().root.clone() else {
        return false;
    };
    match save(&root, locale) {
        Ok(()) => {
            let state = cx.global_mut::<Language>();
            state.locale = locale;
            state.notice = Some(Notice::Saved);
            component::set_locale(locale.component_locale());
            cx.refresh_windows();
            true
        }
        Err(error) => {
            cx.global_mut::<Language>().notice = Some(Notice::SaveError(error));
            cx.refresh_windows();
            false
        }
    }
}
pub fn available(cx: &App) -> bool {
    cx.global::<Language>().root.is_some()
}
pub fn message(cx: &App) -> Option<String> {
    cx.global::<Language>()
        .notice
        .as_ref()
        .map(|notice| match notice {
            Notice::Saved => tr("语言已保存").to_owned(),
            Notice::LoadedError(error) => format!("{}：{error}", tr("读取语言设置失败")),
            Notice::SaveError(error) => format!("{}：{error}", tr("保存语言设置失败")),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_preferences_reopen_and_preserve_invalid_or_failed_targets() {
        let previous = component::locale().to_string();
        component::set_locale(Locale::En.component_locale());
        assert_eq!(tr("设置"), "Settings");
        component::set_locale(Locale::Zh.component_locale());
        assert_eq!(tr("设置"), "设置");
        component::set_locale(&previous);
        let root = tempfile::tempdir().unwrap();
        assert_eq!(load(root.path()).unwrap(), Locale::Zh);
        for locale in [Locale::En, Locale::Zh, Locale::En] {
            save(root.path(), locale).unwrap();
            assert_eq!(load(root.path()).unwrap(), locale);
        }
        let file = root.path().join("desktop-language.json");
        fs::write(&file, b"\"unsupported\"").unwrap();
        assert!(load(root.path()).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"\"unsupported\"");
        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();
        fs::write(file.join("preserved"), b"owned target").unwrap();
        assert!(save(root.path(), Locale::En).is_err());
        assert_eq!(fs::read(file.join("preserved")).unwrap(), b"owned target");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
