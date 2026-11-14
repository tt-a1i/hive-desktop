mod preferences;
mod settings;
use crate::app::HiveApp;
use gpui_kit::{component::input::Enter, *};
pub use preferences::{ComposerPreferences, SendMode, ToolActivity, restore};

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
        // Other fields keep their native Enter behavior.
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
            if self.workspace.is_none() {
                if self.composer_has_text && self.connected && !self.loading && !self.closing {
                    self.choose_workspace_folder(window, cx);
                }
                return;
            }
            self.send(window, cx);
        }
    }
}
