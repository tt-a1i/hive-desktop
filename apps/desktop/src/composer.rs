use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    component::{button::Button, input::Enter, *},
    *,
};
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
            Self::Enter => "Enter 发送，Shift+Enter 换行",
            Self::ModifierEnter if cfg!(target_os = "macos") => "⌘Enter 发送，Enter 换行",
            Self::ModifierEnter => "Ctrl+Enter 发送，Enter 换行",
        }
    }
}
#[derive(Default)]
pub struct ComposerPreferences {
    pub mode: SendMode,
    pub message: Option<String>,
    root: Option<PathBuf>,
}
impl Global for ComposerPreferences {}
fn load(root: &Path) -> io::Result<SendMode> {
    match fs::File::open(root.join("desktop-composer.json")) {
        Ok(file) => Ok(serde_json::from_reader(file)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SendMode::Enter),
        Err(error) => Err(error),
    }
}
fn save(root: &Path, mode: SendMode) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(file.as_file_mut(), &mode)?;
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
        Ok(mode) => state.mode = mode,
        Err(error) => state.message = Some(format!("读取聊天设置失败：{error}")),
    }
    cx.refresh_windows();
}
fn choose(mode: SendMode, cx: &mut App) {
    let Some(root) = cx.global::<ComposerPreferences>().root.clone() else {
        return;
    };
    match save(&root, mode) {
        Ok(()) => {
            let state = cx.global_mut::<ComposerPreferences>();
            state.mode = mode;
            state.message = Some("发送快捷键已保存".into());
        }
        Err(error) => {
            cx.global_mut::<ComposerPreferences>().message =
                Some(format!("保存聊天设置失败，原快捷键仍生效：{error}"))
        }
    }
    cx.refresh_windows();
}
impl HiveApp {
    pub(crate) fn composer_enter(
        &mut self,
        action: &Enter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !action.secondary && !action.shift && self.command_picker.open {
            if self.accept_selected_command(window, cx) {
                cx.stop_propagation();
                return;
            }
            self.command_picker.open = false;
        }
        // Capture only the composer's modified Enter, before the text component
        // would insert a newline in ModifierEnter mode. Other fields keep their keys.
        if action.secondary && !action.shift {
            cx.stop_propagation();
            self.submit_composer_key(window, cx);
        }
    }
    pub(crate) fn submit_composer_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.composer_can_submit {
            return;
        }
        let composing = self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        });
        if !composing {
            self.send(window, cx);
        }
    }
    pub(crate) fn render_chat_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let saved = cx.global::<ComposerPreferences>();
        let modifier = if cfg!(target_os = "macos") {
            "⌘Enter 发送"
        } else {
            "Ctrl+Enter 发送"
        };
        v_flex().gap_4()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("发送消息"))
            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child("选择习惯的发送方式。Shift+Enter 始终换行，点击发送按钮也可发送。适用于所有 Agent。"))
            .child(h_flex().gap_2().flex_wrap().children([("send-mode-enter","Enter 发送",SendMode::Enter),("send-mode-modifier",modifier,SendMode::ModifierEnter)].into_iter().map(|(id,label,mode)|Button::new(id).rounded_full().label(label).accessibility_label(format!("发送快捷键：{label}")).selected(saved.mode==mode).toggled(saved.mode==mode).disabled(saved.root.is_none()).on_click(move|_,_,cx|choose(mode,cx)))))
            .child(div().text_sm().text_color(Color::Muted.get(cx)).child(saved.mode.hint()))
            .children(saved.message.clone().map(|message|div().text_sm().child(message)))
    }
}
#[cfg(test)]
mod tests {
    use super::{SendMode, load, save};
    use std::fs;
    #[test]
    fn send_preference_reopens_and_preserves_invalid_or_unwritable_files() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(load(root.path()).unwrap(), SendMode::Enter);
        save(root.path(), SendMode::ModifierEnter).unwrap();
        assert_eq!(load(root.path()).unwrap(), SendMode::ModifierEnter);
        save(root.path(), SendMode::Enter).unwrap();
        assert_eq!(load(root.path()).unwrap(), SendMode::Enter);
        let file = root.path().join("desktop-composer.json");
        fs::write(&file, "broken").unwrap();
        assert!(load(root.path()).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "broken");
        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();
        assert!(save(root.path(), SendMode::ModifierEnter).is_err());
        assert!(file.is_dir());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
