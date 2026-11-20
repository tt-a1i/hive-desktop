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

pub const DEFAULT_CODE_FONT_SIZE: u8 = 12;
pub const MIN_CODE_FONT_SIZE: u8 = 11;
pub const MAX_CODE_FONT_SIZE: u8 = 18;

pub const DEFAULT_TERMINAL_FONT_SIZE: u8 = 14;
pub const MIN_TERMINAL_FONT_SIZE: u8 = 11;
pub const MAX_TERMINAL_FONT_SIZE: u8 = 18;

fn default_font_size() -> u8 {
    DEFAULT_FONT_SIZE
}

fn default_code_font_size() -> u8 {
    DEFAULT_CODE_FONT_SIZE
}

fn default_terminal_font_size() -> u8 {
    DEFAULT_TERMINAL_FONT_SIZE
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatWidth {
    #[default]
    Standard,
    Comfortable,
    Wide,
}

impl ChatWidth {
    pub fn px(self) -> f32 {
        match self {
            Self::Standard => crate::conversation_list::CHAT_COLUMN_WIDTH,
            Self::Comfortable => 896.,
            Self::Wide => 1080.,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => crate::language::tr("标准"),
            Self::Comfortable => crate::language::tr("宽松"),
            Self::Wide => crate::language::tr("宽屏"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MonoFontPreset {
    #[default]
    System,
    SfMono,
    Menlo,
    Consolas,
    CascadiaMono,
    JetBrainsMono,
    FiraCode,
}

impl MonoFontPreset {
    pub fn font_family_name(self) -> Option<&'static str> {
        match self {
            Self::System => None,
            Self::SfMono => Some("SF Mono"),
            Self::Menlo => Some("Menlo"),
            Self::Consolas => Some("Consolas"),
            Self::CascadiaMono => Some("Cascadia Mono"),
            Self::JetBrainsMono => Some("JetBrains Mono"),
            Self::FiraCode => Some("Fira Code"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => crate::language::tr("跟随系统"),
            Self::SfMono => "SF Mono",
            Self::Menlo => "Menlo",
            Self::Consolas => "Consolas",
            Self::CascadiaMono => "Cascadia Mono",
            Self::JetBrainsMono => "JetBrains Mono",
            Self::FiraCode => "Fira Code",
        }
    }

    pub fn for_platform() -> &'static [Self] {
        if cfg!(target_os = "macos") {
            &[
                Self::System,
                Self::SfMono,
                Self::Menlo,
                Self::JetBrainsMono,
                Self::FiraCode,
            ]
        } else if cfg!(target_os = "windows") {
            &[
                Self::System,
                Self::Consolas,
                Self::CascadiaMono,
                Self::JetBrainsMono,
                Self::FiraCode,
            ]
        } else {
            &[Self::System, Self::JetBrainsMono, Self::FiraCode]
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
struct Preferences {
    mode: Mode,
    #[serde(default = "default_font_size")]
    font_size: u8,
    chat_width: ChatWidth,
    code_font_family: MonoFontPreset,
    #[serde(default = "default_code_font_size")]
    code_font_size: u8,
    code_word_wrap: bool,
    terminal_font_family: MonoFontPreset,
    #[serde(default = "default_terminal_font_size")]
    terminal_font_size: u8,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            mode: Mode::System,
            font_size: DEFAULT_FONT_SIZE,
            chat_width: ChatWidth::Standard,
            code_font_family: MonoFontPreset::System,
            code_font_size: DEFAULT_CODE_FONT_SIZE,
            code_word_wrap: false,
            terminal_font_family: MonoFontPreset::System,
            terminal_font_size: DEFAULT_TERMINAL_FONT_SIZE,
        }
    }
}

pub struct Appearance {
    preferences: Preferences,
    message: Option<Notice>,
    root: Option<PathBuf>,
    preview: bool,
    default_mono_font: Option<gpui_kit::SharedString>,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            preferences: Preferences::default(),
            message: None,
            root: None,
            preview: cfg!(feature = "ui-preview")
                && std::env::var("HIVE_DESKTOP_PREVIEW_SCREEN")
                    .is_ok_and(|value| !value.trim().is_empty()),
            default_mono_font: None,
        }
    }
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
    pub fn chat_width(&self) -> ChatWidth {
        self.preferences.chat_width
    }
    pub fn chat_width_px(&self) -> f32 {
        self.preferences.chat_width.px()
    }
    pub fn code_font_family(&self) -> MonoFontPreset {
        self.preferences.code_font_family
    }
    pub fn code_font_size(&self) -> u8 {
        self.preferences.code_font_size
    }
    pub fn code_word_wrap(&self) -> bool {
        self.preferences.code_word_wrap
    }
    pub fn terminal_font_family(&self) -> MonoFontPreset {
        self.preferences.terminal_font_family
    }
    pub fn terminal_font_size(&self) -> u8 {
        self.preferences.terminal_font_size
    }
    pub fn is_default(&self) -> bool {
        self.preferences == Preferences::default()
    }
}

fn validate(preferences: Preferences) -> io::Result<Preferences> {
    if !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&preferences.font_size) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "界面字号须在 12–20 之间",
        ));
    }
    if !(MIN_CODE_FONT_SIZE..=MAX_CODE_FONT_SIZE).contains(&preferences.code_font_size) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "代码字号须在 11–18 之间",
        ));
    }
    if !(MIN_TERMINAL_FONT_SIZE..=MAX_TERMINAL_FONT_SIZE).contains(&preferences.terminal_font_size) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "终端字号须在 11–18 之间",
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
    let preview = cx.global::<Appearance>().preview;
    let result = if preview {
        Ok(Preferences::default())
    } else {
        load(root)
    };
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
    let default_mono = cx.global::<Appearance>().default_mono_font.clone();
    if default_mono.is_none() {
        let initial_mono = Theme::global(cx).mono_font_family.clone();
        cx.global_mut::<Appearance>().default_mono_font = Some(initial_mono);
    }
    let (font_size, code_font_family, code_font_size, base_mono) = {
        let state = cx.global::<Appearance>();
        (
            state.font_size(),
            state.code_font_family(),
            state.code_font_size(),
            state
                .default_mono_font
                .clone()
                .unwrap_or_else(|| Theme::global(cx).mono_font_family.clone()),
        )
    };
    Theme::update(cx, |theme| {
        theme.font_size = px(f32::from(font_size));
        theme.mono_font_size = px(f32::from(code_font_size));
        theme.mono_font_family = code_font_family
            .font_family_name()
            .map(Into::into)
            .unwrap_or(base_mono);
        theme.radius = px(8.0);
        theme.radius_lg = px(16.0);
        if theme.is_dark() {
            theme.background = rgb(0x18181b).into();
            theme.foreground = rgb(0xf4f4f5).into();
            theme.border = gpui_kit::rgba(0xffffff14).into();
            theme.input = gpui_kit::rgba(0xffffff1c).into();
            theme.popover = rgb(0x1f1f23).into();
            theme.popover_foreground = rgb(0xf4f4f5).into();
            theme.muted = rgb(0x222227).into();
            theme.muted_foreground = rgb(0x8e8e98).into();
            theme.secondary = rgb(0x222227).into();
            theme.secondary_hover = rgb(0x2a2a31).into();
            theme.secondary_active = rgb(0x32323a).into();
            theme.secondary_foreground = rgb(0xf4f4f5).into();
            // Button::Ghost multiplies accent by 0.5 opacity in dark mode.
            theme.accent = rgb(0x2c2c34).into();
            theme.accent_foreground = rgb(0xf4f4f5).into();
            theme.sidebar = rgb(0x131316).into();
            theme.sidebar_border = gpui_kit::rgba(0xffffff14).into();
            theme.sidebar_foreground = rgb(0xf4f4f5).into();
            theme.sidebar_accent = rgb(0x26262c).into();
            theme.sidebar_accent_foreground = rgb(0xf4f4f5).into();
            theme.colors.list = rgb(0x18181b).into();
            theme.list_hover = rgb(0x222227).into();
            theme.list_active = rgb(0x28282f).into();
            theme.list_active_border = rgb(0x52525e).into();
            theme.primary = rgb(0xf4f4f5).into();
            theme.primary_hover = rgb(0xffffff).into();
            theme.primary_active = rgb(0xd4d4dc).into();
            theme.primary_foreground = rgb(0x18181b).into();
            theme.button_primary = rgb(0xf4f4f5).into();
            theme.button_primary_hover = rgb(0xffffff).into();
            theme.button_primary_active = rgb(0xd4d4dc).into();
            theme.button_primary_foreground = rgb(0x18181b).into();
            theme.button = rgb(0x222227).into();
            theme.button_hover = rgb(0x2a2a31).into();
            theme.button_active = rgb(0x32323a).into();
            theme.button_foreground = rgb(0xf4f4f5).into();
            theme.ring = rgb(0x71717a).into();
            theme.caret = rgb(0x4d7ef5).into();
            theme.title_bar = rgb(0x18181b).into();
            theme.title_bar_border = gpui_kit::rgba(0xffffff14).into();
        } else {
            theme.background = rgb(0xfcfcfc).into();
            theme.foreground = rgb(0x18181b).into();
            theme.border = gpui_kit::rgba(0x00000012).into();
            theme.input = rgb(0xd4d4d8).into();
            theme.popover = rgb(0xffffff).into();
            theme.popover_foreground = rgb(0x18181b).into();
            theme.muted = rgb(0xf4f4f5).into();
            theme.muted_foreground = rgb(0x71717b).into();
            theme.secondary = rgb(0xf4f4f5).into();
            theme.secondary_hover = rgb(0xebebee).into();
            theme.secondary_active = rgb(0xe4e4e7).into();
            theme.secondary_foreground = rgb(0x18181b).into();
            theme.accent = rgb(0xf4f4f5).into();
            theme.accent_foreground = rgb(0x18181b).into();
            theme.sidebar = rgb(0xfafafa).into();
            theme.sidebar_border = gpui_kit::rgba(0x00000012).into();
            theme.sidebar_foreground = rgb(0x18181b).into();
            theme.sidebar_accent = rgb(0xebebee).into();
            theme.sidebar_accent_foreground = rgb(0x18181b).into();
            theme.colors.list = rgb(0xfcfcfc).into();
            theme.list_hover = rgb(0xf4f4f5).into();
            theme.list_active = rgb(0xebebee).into();
            theme.list_active_border = rgb(0x18181b).into();
            theme.primary = rgb(0x18181b).into();
            theme.primary_hover = rgb(0x27272a).into();
            theme.primary_active = rgb(0x3f3f46).into();
            theme.primary_foreground = rgb(0xffffff).into();
            theme.button_primary = rgb(0x18181b).into();
            theme.button_primary_hover = rgb(0x27272a).into();
            theme.button_primary_active = rgb(0x3f3f46).into();
            theme.button_primary_foreground = rgb(0xffffff).into();
            theme.button = rgb(0xf4f4f5).into();
            theme.button_hover = rgb(0xebebee).into();
            theme.button_active = rgb(0xe4e4e7).into();
            theme.button_foreground = rgb(0x18181b).into();
            theme.ring = rgb(0x71717a).into();
            theme.caret = rgb(0x1b4ed8).into();
            theme.title_bar = rgb(0xfcfcfc).into();
            theme.title_bar_border = gpui_kit::rgba(0x00000012).into();
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

pub fn choose_chat_width(chat_width: ChatWidth, window: &mut Window, cx: &mut App) {
    let preferences = Preferences {
        chat_width,
        ..cx.global::<Appearance>().preferences
    };
    choose_preferences(preferences, window, cx);
}

pub fn choose_code_font_family(family: MonoFontPreset, window: &mut Window, cx: &mut App) {
    let preferences = Preferences {
        code_font_family: family,
        ..cx.global::<Appearance>().preferences
    };
    choose_preferences(preferences, window, cx);
}

pub fn choose_code_font_size(size: u8, window: &mut Window, cx: &mut App) {
    let preferences = Preferences {
        code_font_size: size,
        ..cx.global::<Appearance>().preferences
    };
    choose_preferences(preferences, window, cx);
}

pub fn choose_code_word_wrap(wrap: bool, window: &mut Window, cx: &mut App) {
    let preferences = Preferences {
        code_word_wrap: wrap,
        ..cx.global::<Appearance>().preferences
    };
    choose_preferences(preferences, window, cx);
}

pub fn choose_terminal_font_family(family: MonoFontPreset, window: &mut Window, cx: &mut App) {
    let preferences = Preferences {
        terminal_font_family: family,
        ..cx.global::<Appearance>().preferences
    };
    choose_preferences(preferences, window, cx);
}

pub fn choose_terminal_font_size(size: u8, window: &mut Window, cx: &mut App) {
    let preferences = Preferences {
        terminal_font_size: size,
        ..cx.global::<Appearance>().preferences
    };
    choose_preferences(preferences, window, cx);
}

pub fn reset_all(window: &mut Window, cx: &mut App) {
    choose_preferences(Preferences::default(), window, cx);
}

fn choose_preferences(preferences: Preferences, window: &mut Window, cx: &mut App) {
    if cx.global::<Appearance>().preview {
        let state = cx.global_mut::<Appearance>();
        state.preferences = preferences;
        state.message = Some(Notice::Saved);
        apply(Some(window), cx);
        cx.refresh_windows();
        return;
    }
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

static TERMINAL_SF_MONO_AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Register the local SF Mono faces from Terminal.app when the system font list
/// does not already include them.
pub fn prepare_terminal_fonts(cx: &mut App) {
    #[cfg(target_os = "macos")]
    TERMINAL_SF_MONO_AVAILABLE.get_or_init(|| {
        let text_system = cx.text_system();
        let available = || text_system.all_font_names().iter().any(|name| name == "SF Mono");
        if available() {
            return true;
        }
        let directory = Path::new(
            "/System/Applications/Utilities/Terminal.app/Contents/Resources/Fonts",
        );
        let fonts: Option<Vec<_>> = [
            "SF-Mono-Regular.otf",
            "SF-Mono-Bold.otf",
            "SF-Mono-RegularItalic.otf",
            "SF-Mono-BoldItalic.otf",
        ]
        .into_iter()
        .map(|name| {
            fs::read(directory.join(name))
                .ok()
                .map(std::borrow::Cow::Owned)
        })
        .collect();
        if let Some(fonts) = fonts {
            let _ = text_system.add_fonts(fonts);
        }
        available()
    });
    #[cfg(not(target_os = "macos"))]
    let _ = cx;
}

pub fn sync_terminal(view: &gpui_kit::Entity<hive_terminal::TerminalView>, cx: &mut App) {
    let (family, size) = {
        let state = cx.global::<Appearance>();
        (
            state
                .terminal_font_family()
                .font_family_name()
                .map(str::to_owned),
            px(f32::from(state.terminal_font_size())),
        )
    };
    let family = family.or_else(|| {
        TERMINAL_SF_MONO_AVAILABLE
            .get()
            .copied()
            .unwrap_or(false)
            .then(|| "SF Mono".to_owned())
    });
    view.update(cx, |view, cx| view.set_typography(family, size, cx));
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
                chat_width: ChatWidth::Wide,
                code_font_family: MonoFontPreset::JetBrainsMono,
                code_font_size: 14,
                code_word_wrap: true,
                terminal_font_family: MonoFontPreset::FiraCode,
                terminal_font_size: 15,
            };
            save(root.path(), preferences).unwrap();
            assert_eq!(load(root.path()).unwrap(), preferences);
        }
        // Older 2-field files load with new defaults intact.
        fs::write(
            root.path().join("desktop-appearance.json"),
            r#"{"mode":"dark","font_size":14}"#,
        )
        .unwrap();
        assert_eq!(
            load(root.path()).unwrap(),
            Preferences {
                mode: Mode::Dark,
                font_size: 14,
                ..Default::default()
            }
        );
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
            ..Default::default()
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
        assert!(
            save(
                root.path(),
                Preferences {
                    code_font_size: 0,
                    ..expected
                }
            )
            .is_err()
        );
        assert!(
            save(
                root.path(),
                Preferences {
                    terminal_font_size: 99,
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
        fs::write(&file, r#"{"mode":"dark","font_size":14,"code_font_size":99}"#).unwrap();
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
