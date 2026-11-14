use crate::language::tr;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SendMode {
    #[default]
    Enter,
    ModifierEnter,
}
impl SendMode {
    pub fn hint(self) -> &'static str {
        match self {
            Self::Enter => tr("Enter 发送，Shift+Enter 换行"),
            Self::ModifierEnter if cfg!(target_os = "macos") => tr("⌘Enter 发送，Enter 换行"),
            Self::ModifierEnter => tr("Ctrl+Enter 发送，Enter 换行"),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolActivity {
    #[default]
    Grouped,
    Individual,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct ChatSettings {
    pub mode: SendMode,
    pub tool_activity: ToolActivity,
    pub expand_thinking: bool,
}
#[derive(Default)]
pub struct ComposerPreferences {
    pub settings: ChatSettings,
    message: Option<Notice>,
    root: Option<PathBuf>,
}
impl Global for ComposerPreferences {}
enum Notice {
    ReadError(String),
    Saved,
    SaveError(String),
}
impl ComposerPreferences {
    pub(super) fn ready(&self) -> bool {
        self.root.is_some()
    }
    pub(super) fn has_error(&self) -> bool {
        matches!(
            self.message,
            Some(Notice::ReadError(_) | Notice::SaveError(_))
        )
    }
    pub(super) fn message(&self) -> Option<String> {
        self.message.as_ref().map(|notice| match notice {
            Notice::ReadError(error) => format!(
                "{}: {}",
                tr("读取聊天设置失败"),
                crate::language::localize_owned_error(error)
            ),
            Notice::Saved => tr("聊天设置已保存").to_owned(),
            Notice::SaveError(error) => format!(
                "{}: {}",
                tr("保存聊天设置失败，原设置仍生效"),
                crate::language::localize_owned_error(error)
            ),
        })
    }
    // Publish the new settings only after the atomic replacement succeeds.
    pub(super) fn change(&mut self, settings: ChatSettings) -> bool {
        let Some(root) = &self.root else {
            return false;
        };
        match save(root, settings) {
            Ok(()) => {
                self.settings = settings;
                self.message = Some(Notice::Saved);
                true
            }
            Err(error) => {
                self.message = Some(Notice::SaveError(error.to_string()));
                false
            }
        }
    }
}
fn load(root: &Path) -> io::Result<ChatSettings> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Settings(ChatSettings),
        Legacy(SendMode),
    }
    match fs::File::open(root.join("desktop-composer.json")) {
        Ok(file) => Ok(match serde_json::from_reader(file)? {
            Stored::Settings(settings) => settings,
            Stored::Legacy(mode) => ChatSettings {
                mode,
                ..ChatSettings::default()
            },
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ChatSettings::default()),
        Err(error) => Err(error),
    }
}
fn save(root: &Path, settings: ChatSettings) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(file.as_file_mut(), &settings)?;
    file.as_file().sync_all()?;
    file.persist(root.join("desktop-composer.json"))?;
    Ok(())
}
pub fn restore(root: &Path, cx: &mut App) {
    if cx.global::<ComposerPreferences>().root.is_some() {
        return;
    }
    let loaded = load(root);
    let state = cx.global_mut::<ComposerPreferences>();
    state.root = Some(root.to_path_buf());
    match loaded {
        Ok(settings) => state.settings = settings,
        Err(error) => state.message = Some(Notice::ReadError(error.to_string())),
    }
    cx.refresh_windows();
}
#[cfg(test)]
mod tests {
    use super::{ChatSettings, ComposerPreferences, Notice, SendMode, ToolActivity, load, save};
    use std::fs;
    #[test]
    fn legacy_shortcut_migrates_without_losing_the_original_choice() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("desktop-composer.json");
        for (legacy, mode) in [
            (br#""enter""#.as_slice(), SendMode::Enter),
            (br#""modifier_enter""#.as_slice(), SendMode::ModifierEnter),
        ] {
            fs::write(&path, &legacy).unwrap();
            let mut settings = load(root.path()).unwrap();
            assert_eq!(settings.mode, mode);
            assert_eq!(settings.tool_activity, ToolActivity::Grouped);
            assert!(!settings.expand_thinking);
            assert_eq!(fs::read(&path).unwrap(), legacy);
            settings.tool_activity = ToolActivity::Individual;
            settings.expand_thinking = true;
            save(root.path(), settings).unwrap();
            assert_eq!(load(root.path()).unwrap(), settings);
            assert!(
                serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap())
                    .unwrap()
                    .is_object()
            );
        }
    }
    #[test]
    fn preference_reopens_and_failed_write_retains_effective_settings_and_original_target() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(load(root.path()).unwrap(), ChatSettings::default());
        let settings = ChatSettings {
            mode: SendMode::ModifierEnter,
            tool_activity: ToolActivity::Individual,
            expand_thinking: true,
        };
        let mut state = ComposerPreferences {
            root: Some(root.path().into()),
            ..Default::default()
        };
        assert!(state.change(settings));
        assert_eq!(state.settings, settings);
        assert_eq!(load(root.path()).unwrap(), settings);
        let path = root.path().join("desktop-composer.json");
        for invalid in [
            "broken",
            r#"{"tool_activity":"unknown"}"#,
            r#"{"expand_thinking":"true"}"#,
        ] {
            fs::write(&path, invalid).unwrap();
            assert!(load(root.path()).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(!state.change(ChatSettings::default()));
        assert_eq!(state.settings, settings);
        assert!(matches!(state.message, Some(Notice::SaveError(_))));
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
