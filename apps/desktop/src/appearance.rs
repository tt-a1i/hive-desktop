use gpui_kit::{
    App, Global, Window,
    component::{Theme, ThemeMode},
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Default)]
pub struct Appearance {
    pub mode: Mode,
    pub message: Option<String>,
    root: Option<PathBuf>,
}
impl Global for Appearance {}

fn load(root: &Path) -> io::Result<Mode> {
    match fs::File::open(root.join("desktop-appearance.json")) {
        Ok(file) => Ok(serde_json::from_reader(file)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Mode::System),
        Err(error) => Err(error),
    }
}

fn save(root: &Path, mode: Mode) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(file.as_file_mut(), &mode)?;
    file.persist(root.join("desktop-appearance.json"))?;
    Ok(())
}

/// Use the backend's authoritative data directory rather than duplicate its
/// platform path resolution. Reconnecting must not reload over a local choice.
pub fn restore(root: &Path, cx: &mut App) {
    if cx.global::<Appearance>().root.is_some() {
        return;
    }
    let result = load(root);
    let state = cx.global_mut::<Appearance>();
    state.root = Some(root.to_path_buf());
    match result {
        Ok(mode) => state.mode = mode,
        Err(error) => state.message = Some(format!("读取外观设置失败：{error}")),
    }
    apply(None, cx);
}

pub fn apply(window: Option<&mut Window>, cx: &mut App) {
    match cx.global::<Appearance>().mode {
        Mode::System => Theme::sync_system_appearance(window, cx),
        Mode::Light => Theme::change(ThemeMode::Light, window, cx),
        Mode::Dark => Theme::change(ThemeMode::Dark, window, cx),
    }
}

pub fn choose(mode: Mode, window: &mut Window, cx: &mut App) {
    let Some(root) = cx.global::<Appearance>().root.clone() else {
        return;
    };
    match save(&root, mode) {
        Ok(()) => {
            let state = cx.global_mut::<Appearance>();
            state.mode = mode;
            state.message = Some("外观已保存".into());
            apply(Some(window), cx);
        }
        Err(error) => {
            cx.global_mut::<Appearance>().message = Some(format!("保存外观失败：{error}"))
        }
    }
    cx.refresh_windows();
}

pub fn available(cx: &App) -> bool {
    cx.global::<Appearance>().root.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_reopen_replace_and_report_invalid_data_without_overwriting_it() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(load(root.path()).unwrap(), Mode::System);
        for mode in [Mode::Dark, Mode::Light, Mode::System] {
            save(root.path(), mode).unwrap();
            assert_eq!(load(root.path()).unwrap(), mode);
        }
        let file = root.path().join("desktop-appearance.json");
        fs::write(&file, "broken").unwrap();
        assert!(load(root.path()).is_err());
        assert_eq!(fs::read_to_string(file).unwrap(), "broken");
    }
}
