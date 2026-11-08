use gpui_kit::{
    App, Global, Window,
    component::{Theme, ThemeMode},
    px,
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

pub const DEFAULT_FONT_SIZE: u8 = 16;
pub const MIN_FONT_SIZE: u8 = 12;
pub const MAX_FONT_SIZE: u8 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
struct Preferences {
    mode: Mode,
    font_size: u8,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            mode: Mode::System,
            font_size: DEFAULT_FONT_SIZE,
        }
    }
}

#[derive(Default)]
pub struct Appearance {
    preferences: Preferences,
    message: Option<Notice>,
    root: Option<PathBuf>,
}
impl Global for Appearance {}
enum Notice { Saved, ReadError(String), WriteError(String) }

impl Appearance {
    pub fn message(&self) -> Option<String> {
        use crate::language::tr;
        self.message.as_ref().map(|message| match message {
            Notice::Saved => tr("外观已保存").to_owned(),
            Notice::ReadError(error) => format!("{}: {error}", tr("读取外观设置失败")),
            Notice::WriteError(error) => format!("{}: {error}", tr("保存外观失败")),
        })
    }
    pub fn mode(&self) -> Mode {
        self.preferences.mode
    }
    pub fn font_size(&self) -> u8 {
        self.preferences.font_size
    }
}

fn validate(preferences: Preferences) -> io::Result<Preferences> {
    if !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&preferences.font_size) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "界面字号须在 12–20 之间",
        ));
    }
    Ok(preferences)
}

fn load(root: &Path) -> io::Result<Preferences> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Saved {
        Legacy(Mode),
        Current(Preferences),
    }
    match fs::File::open(root.join("desktop-appearance.json")) {
        Ok(file) => validate(match serde_json::from_reader(file)? {
            Saved::Legacy(mode) => Preferences {
                mode,
                ..Default::default()
            },
            Saved::Current(preferences) => preferences,
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(error) => Err(error),
    }
}

fn save(root: &Path, preferences: Preferences) -> io::Result<()> {
    validate(preferences)?;
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(file.as_file_mut(), &preferences)?;
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
        Ok(preferences) => state.preferences = preferences,
        Err(error) => state.message = Some(Notice::ReadError(error.to_string())),
    }
    apply(None, cx);
}

pub fn apply(window: Option<&mut Window>, cx: &mut App) {
    match cx.global::<Appearance>().mode() {
        Mode::System => Theme::sync_system_appearance(window, cx),
        Mode::Light => Theme::change(ThemeMode::Light, window, cx),
        Mode::Dark => Theme::change(ThemeMode::Dark, window, cx),
    }
    let font_size = cx.global::<Appearance>().font_size();
    Theme::update(cx, |theme| theme.font_size = px(f32::from(font_size)));
}

pub fn choose(mode: Mode, window: &mut Window, cx: &mut App) {
    let preferences = Preferences {
        mode,
        ..cx.global::<Appearance>().preferences
    };
    choose_preferences(preferences, window, cx);
}

pub fn choose_font_size(font_size: u8, window: &mut Window, cx: &mut App) {
    let preferences = Preferences {
        font_size,
        ..cx.global::<Appearance>().preferences
    };
    choose_preferences(preferences, window, cx);
}

fn choose_preferences(preferences: Preferences, window: &mut Window, cx: &mut App) {
    let Some(root) = cx.global::<Appearance>().root.clone() else {
        return;
    };
    match save(&root, preferences) {
        Ok(()) => {
            let state = cx.global_mut::<Appearance>();
            state.preferences = preferences;
            state.message = Some(Notice::Saved);
            apply(Some(window), cx);
        }
        Err(error) => {
            cx.global_mut::<Appearance>().message = Some(Notice::WriteError(error.to_string()))
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
        assert_eq!(load(root.path()).unwrap(), Preferences::default());
        for mode in [Mode::Dark, Mode::Light, Mode::System] {
            // Legacy theme-only files retain their chosen mode on upgrade.
            fs::write(
                root.path().join("desktop-appearance.json"),
                serde_json::to_vec(&mode).unwrap(),
            )
            .unwrap();
            assert_eq!(
                load(root.path()).unwrap(),
                Preferences {
                    mode,
                    ..Default::default()
                }
            );
            let preferences = Preferences {
                mode,
                font_size: 20,
            };
            save(root.path(), preferences).unwrap();
            assert_eq!(load(root.path()).unwrap(), preferences);
        }
        let file = root.path().join("desktop-appearance.json");
        fs::write(&file, "broken").unwrap();
        assert!(load(root.path()).is_err());
        assert_eq!(fs::read_to_string(file).unwrap(), "broken");
    }

    #[test]
    fn invalid_size_and_write_failure_preserve_the_saved_preferences() {
        let root = tempfile::tempdir().unwrap();
        let expected = Preferences {
            mode: Mode::Dark,
            font_size: MIN_FONT_SIZE,
        };
        save(root.path(), expected).unwrap();
        assert!(
            save(
                root.path(),
                Preferences {
                    font_size: 0,
                    ..expected
                }
            )
            .is_err()
        );
        assert_eq!(load(root.path()).unwrap(), expected);
        let file = root.path().join("desktop-appearance.json");
        fs::write(&file, r#"{"mode":"dark","font_size":255}"#).unwrap();
        assert_eq!(
            load(root.path()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();
        assert!(save(root.path(), expected).is_err());
        assert!(file.is_dir());
    }
}
