use crate::language::tr;
use gpui_kit::{App, BorrowAppContext, Global};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const FILE: &str = "desktop-general.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenTarget {
    #[default]
    Finder,
    Vscode,
    VscodeInsiders,
    Cursor,
    Zed,
    Terminal,
    Ghostty,
}

impl OpenTarget {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Finder => "finder",
            Self::Vscode => "vscode",
            Self::VscodeInsiders => "vscode-insiders",
            Self::Cursor => "cursor",
            Self::Zed => "zed",
            Self::Terminal => "terminal",
            Self::Ghostty => "ghostty",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Finder => {
                if cfg!(target_os = "macos") {
                    tr("访达")
                } else {
                    tr("文件管理器")
                }
            }
            Self::Vscode => "VS Code",
            Self::VscodeInsiders => "VS Code Insiders",
            Self::Cursor => "Cursor",
            Self::Zed => "Zed",
            Self::Terminal => tr("系统终端"),
            Self::Ghostty => "Ghostty",
        }
    }

    pub fn for_platform() -> &'static [Self] {
        if cfg!(target_os = "macos") {
            &[
                Self::Finder,
                Self::Vscode,
                Self::VscodeInsiders,
                Self::Cursor,
                Self::Zed,
                Self::Terminal,
                Self::Ghostty,
            ]
        } else {
            &[
                Self::Finder,
                Self::Vscode,
                Self::VscodeInsiders,
                Self::Cursor,
                Self::Zed,
            ]
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSortOrder {
    #[default]
    AddedOrder,
    RecentActivity,
    Alphabetical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionTimeFormat {
    #[default]
    Relative,
    Clock24h,
    Clock12h,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusySendMode {
    #[default]
    Queue,
    SteerWhenSupported,
    BlockUntilIdle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuitConfirmation {
    #[default]
    Never,
    WhenBusy,
    Always,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    pub default_open_target: OpenTarget,
    pub project_sort_order: ProjectSortOrder,
    pub pin_active_sessions_top: bool,
    pub session_time_format: SessionTimeFormat,
    pub busy_send_mode: BusySendMode,
    pub confirm_archive_session: bool,
    pub confirm_unpin_session: bool,
    pub confirm_cancel_queued_input: bool,
    pub quit_confirmation: QuitConfirmation,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            default_open_target: OpenTarget::default(),
            project_sort_order: ProjectSortOrder::default(),
            pin_active_sessions_top: false,
            session_time_format: SessionTimeFormat::default(),
            busy_send_mode: BusySendMode::default(),
            confirm_archive_session: false,
            confirm_unpin_session: false,
            confirm_cancel_queued_input: true,
            quit_confirmation: QuitConfirmation::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Notice {
    Saved,
    ReadError(String),
    WriteError(String),
}

pub struct GeneralPreferences {
    root: Option<PathBuf>,
    pub settings: GeneralSettings,
    loading: bool,
    notice: Option<Notice>,
    preview: bool,
}

impl Global for GeneralPreferences {}

impl GeneralPreferences {
    fn new() -> Self {
        Self {
            root: None,
            settings: GeneralSettings::default(),
            loading: true,
            notice: None,
            preview: cfg!(feature = "ui-preview")
                && std::env::var("HIVE_DESKTOP_PREVIEW_SCREEN")
                    .is_ok_and(|value| !value.trim().is_empty()),
        }
    }
}

fn validate(settings: GeneralSettings) -> GeneralSettings {
    let mut normalized = settings;
    if !OpenTarget::for_platform().contains(&normalized.default_open_target) {
        normalized.default_open_target = OpenTarget::Finder;
    }
    normalized
}

fn load(root: &Path) -> io::Result<GeneralSettings> {
    match fs::read_to_string(root.join(FILE)) {
        Ok(content) => serde_json::from_str::<GeneralSettings>(&content)
            .map(validate)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(GeneralSettings::default()),
        Err(error) => Err(error),
    }
}

fn save(root: &Path, settings: &GeneralSettings) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let validated = validate(settings.clone());
    let mut bytes = serde_json::to_vec_pretty(&validated)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    bytes.push(b'\n');

    use io::Write;
    let mut temp = tempfile::NamedTempFile::new_in(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o600))?;
    }
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(root.join(FILE)).map_err(|error| error.error)?;
    Ok(())
}

pub fn init(cx: &mut App) {
    cx.set_global(GeneralPreferences::new());
}

pub fn restore(root: &Path, cx: &mut App) {
    let preview = cx.global::<GeneralPreferences>().preview;
    let root_buf = root.to_path_buf();
    let result = if preview {
        Ok(GeneralSettings::default())
    } else {
        load(root)
    };
    cx.update_global::<GeneralPreferences, _>(|state, _| {
        state.root = Some(root_buf);
        state.loading = false;
        match result {
            Ok(settings) => {
                state.settings = settings;
                state.notice = None;
            }
            Err(error) => {
                state.settings = GeneralSettings::default();
                state.notice = Some(Notice::ReadError(error.to_string()));
            }
        }
    });
    cx.refresh_windows();
}

#[allow(dead_code)]
pub fn reload(cx: &mut App) {
    let (root, preview) = {
        let state = cx.global::<GeneralPreferences>();
        (state.root.clone(), state.preview)
    };
    let Some(root) = root else {
        return;
    };
    let result = if preview {
        Ok(GeneralSettings::default())
    } else {
        load(&root)
    };
    cx.update_global::<GeneralPreferences, _>(|state, _| {
        state.loading = false;
        match result {
            Ok(settings) => {
                state.settings = settings;
                state.notice = None;
            }
            Err(error) => {
                state.notice = Some(Notice::ReadError(error.to_string()));
            }
        }
    });
    cx.refresh_windows();
}

pub fn set(settings: GeneralSettings, cx: &mut App) -> bool {
    let settings = validate(settings);
    let (root, preview) = {
        let state = cx.global::<GeneralPreferences>();
        if state.loading {
            return false;
        }
        (state.root.clone(), state.preview)
    };
    if preview {
        cx.update_global::<GeneralPreferences, _>(|state, _| {
            state.settings = settings;
            state.notice = Some(Notice::Saved);
        });
        cx.refresh_windows();
        return true;
    }
    let Some(root) = root else {
        return false;
    };
    match save(&root, &settings) {
        Ok(()) => {
            cx.update_global::<GeneralPreferences, _>(|state, _| {
                state.settings = settings;
                state.notice = Some(Notice::Saved);
            });
            cx.refresh_windows();
            true
        }
        Err(error) => {
            cx.update_global::<GeneralPreferences, _>(|state, _| {
                state.notice = Some(Notice::WriteError(error.to_string()));
            });
            cx.refresh_windows();
            false
        }
    }
}

pub fn reset(cx: &mut App) -> bool {
    set(GeneralSettings::default(), cx)
}

pub fn current(cx: &App) -> GeneralSettings {
    cx.global::<GeneralPreferences>().settings.clone()
}

pub fn notice(cx: &App) -> Option<(bool, String)> {
    match cx.global::<GeneralPreferences>().notice.as_ref()? {
        Notice::Saved => Some((false, tr("通用设置已保存。").to_owned())),
        Notice::ReadError(error) => Some((true, format!("{}: {error}", tr("读取通用设置失败")))),
        Notice::WriteError(error) => Some((true, format!("{}: {error}", tr("保存通用设置失败")))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_malformed_file_protection() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(load(root.path()).unwrap(), GeneralSettings::default());

        let custom = GeneralSettings {
            default_open_target: OpenTarget::Cursor,
            project_sort_order: ProjectSortOrder::RecentActivity,
            pin_active_sessions_top: true,
            session_time_format: SessionTimeFormat::Clock24h,
            busy_send_mode: BusySendMode::SteerWhenSupported,
            confirm_archive_session: true,
            confirm_unpin_session: false,
            confirm_cancel_queued_input: true,
            quit_confirmation: QuitConfirmation::WhenBusy,
        };
        save(root.path(), &custom).unwrap();
        assert_eq!(load(root.path()).unwrap(), custom);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(root.path().join(FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let broken = r#"{ not valid json"#;
        fs::write(root.path().join(FILE), broken).unwrap();
        assert_eq!(
            load(root.path()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read_to_string(root.path().join(FILE)).unwrap(), broken);

        fs::remove_file(root.path().join(FILE)).unwrap();
        fs::create_dir(root.path().join(FILE)).unwrap();
        assert!(save(root.path(), &custom).is_err());
        assert!(root.path().join(FILE).is_dir());
    }
}
