use gpui_kit::{
    App, Global, Window, WindowAppearance,
    component::{Theme, ThemeMode},
    px, rgb,
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
        use crate::language::{localize_owned_error, tr};
        self.message.as_ref().map(|message| match message {
            Notice::Saved => tr("外观已保存").to_owned(),
            Notice::ReadError(error) => format!("{}: {}", tr("读取外观设置失败"), localize_owned_error(error)),
            Notice::WriteError(error) => format!("{}: {}", tr("保存外观失败"), localize_owned_error(error)),
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
    // Component themes do not change macOS window chrome or native dialogs.
    // Clearing the override is essential when returning to the system theme.
    cx.set_window_appearance(match cx.global::<Appearance>().mode() {
        Mode::System => None,
        Mode::Light => Some(WindowAppearance::Light),
        Mode::Dark => Some(WindowAppearance::Dark),
    });
    match cx.global::<Appearance>().mode() {
        Mode::System => Theme::sync_system_appearance(window, cx),
        Mode::Light => Theme::change(ThemeMode::Light, window, cx),
        Mode::Dark => Theme::change(ThemeMode::Dark, window, cx),
    }
    let font_size = cx.global::<Appearance>().font_size();
    Theme::update(cx, |theme| {
        theme.font_size = px(f32::from(font_size));
        theme.radius = px(8.0);
        theme.radius_lg = px(14.0);
        if theme.is_dark() {
            theme.background = rgb(0x16161a).into();
            theme.foreground = rgb(0xededf0).into();
            theme.border = rgb(0x282831).into();
            theme.input = rgb(0x2d2d37).into();
            theme.popover = rgb(0x1d1d23).into();
            theme.popover_foreground = rgb(0xededf0).into();
            theme.muted = rgb(0x212128).into();
            theme.muted_foreground = rgb(0x7c7c86).into();
            theme.secondary = rgb(0x212128).into();
            theme.secondary_hover = rgb(0x2a2a33).into();
            theme.secondary_active = rgb(0x272731).into();
            theme.secondary_foreground = rgb(0xededf0).into();
            // Button::Ghost multiplies accent by 0.5 opacity in dark mode.
            theme.accent = rgb(0x363644).into();
            theme.accent_foreground = rgb(0xf4f4f7).into();
            theme.sidebar = rgb(0x111114).into();
            theme.sidebar_border = rgb(0x24242c).into();
            theme.sidebar_foreground = rgb(0xededf0).into();
            theme.sidebar_accent = rgb(0x23232c).into();
            theme.sidebar_accent_foreground = rgb(0xf4f4f7).into();
            theme.colors.list = rgb(0x16161a).into();
            theme.list_hover = rgb(0x202028).into();
            theme.list_active = rgb(0x262630).into();
            theme.list_active_border = rgb(0x3b63f6).into();
            theme.primary = rgb(0x4f6ef7).into();
            theme.primary_hover = rgb(0x6380fa).into();
            theme.primary_active = rgb(0x3f5ee0).into();
            theme.primary_foreground = rgb(0xffffff).into();
            theme.button_primary = rgb(0x4f6ef7).into();
            theme.button_primary_hover = rgb(0x6380fa).into();
            theme.button_primary_active = rgb(0x3f5ee0).into();
            theme.button_primary_foreground = rgb(0xffffff).into();
            theme.button = rgb(0x212128).into();
            theme.button_hover = rgb(0x2a2a34).into();
            theme.button_active = rgb(0x30303b).into();
            theme.button_foreground = rgb(0xededf0).into();
            theme.ring = rgb(0x5b7cfa).into();
            theme.caret = rgb(0x5b7cfa).into();
            theme.title_bar = rgb(0x16161a).into();
            theme.title_bar_border = rgb(0x282831).into();
        } else {
            theme.background = rgb(0xffffff).into();
            theme.foreground = rgb(0x18181b).into();
            theme.border = rgb(0xe5e6eb).into();
            theme.input = rgb(0xdcdfe6).into();
            theme.popover = rgb(0xffffff).into();
            theme.popover_foreground = rgb(0x18181b).into();
            theme.muted = rgb(0xf2f3f6).into();
            theme.muted_foreground = rgb(0x71717a).into();
            theme.secondary = rgb(0xf2f3f6).into();
            theme.secondary_hover = rgb(0xe8eaef).into();
            theme.secondary_active = rgb(0xe7e9ef).into();
            theme.secondary_foreground = rgb(0x18181b).into();
            theme.accent = rgb(0xecedf2).into();
            theme.accent_foreground = rgb(0x18181b).into();
            theme.sidebar = rgb(0xf6f6f8).into();
            theme.sidebar_border = rgb(0xe5e6eb).into();
            theme.sidebar_foreground = rgb(0x18181b).into();
            theme.sidebar_accent = rgb(0xe9ebf0).into();
            theme.sidebar_accent_foreground = rgb(0x18181b).into();
            theme.colors.list = rgb(0xffffff).into();
            theme.list_hover = rgb(0xf3f4f7).into();
            theme.list_active = rgb(0xeaecef).into();
            theme.list_active_border = rgb(0x3b63f6).into();
            theme.primary = rgb(0x3b63f6).into();
            theme.primary_hover = rgb(0x2f54e0).into();
            theme.primary_active = rgb(0x2547cc).into();
            theme.primary_foreground = rgb(0xffffff).into();
            theme.button_primary = rgb(0x3b63f6).into();
            theme.button_primary_hover = rgb(0x2f54e0).into();
            theme.button_primary_active = rgb(0x2547cc).into();
            theme.button_primary_foreground = rgb(0xffffff).into();
            theme.button = rgb(0xf4f4f7).into();
            theme.button_hover = rgb(0xebecef).into();
            theme.button_active = rgb(0xe2e4ea).into();
            theme.button_foreground = rgb(0x18181b).into();
            theme.ring = rgb(0x3b63f6).into();
            theme.caret = rgb(0x3b63f6).into();
            theme.title_bar = rgb(0xffffff).into();
            theme.title_bar_border = rgb(0xe5e6eb).into();
        }
    });
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
