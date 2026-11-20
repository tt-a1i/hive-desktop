mod list_continuation;
mod preferences;
mod settings;
use crate::app::HiveApp;
use gpui_kit::{component::{WindowExt, input::Enter}, *};
pub use preferences::{ComposerPreferences, SendMode, ToolActivity, restore};

gpui_kit::actions!(hive_desktop, [AlternateMultilineSend]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SendIntent {
    Configured,
    AlternateWhenBusy,
    AndStartDraft,
}

fn modifier_send_intent(mode: SendMode, text: &str, shift: bool, busy: bool) -> Option<SendIntent> {
    if mode.uses_shift_for_alternate(text) {
        if shift {
            busy.then_some(SendIntent::AlternateWhenBusy)
        } else {
            Some(SendIntent::Configured)
        }
    } else {
        (!shift).then_some(SendIntent::AlternateWhenBusy)
    }
}

impl HiveApp {
    pub(crate) fn composer_enter(
        &mut self,
        action: &Enter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.composer_key_is_focused(window, cx) {
            return;
        }
        if action.shift && !action.secondary {
            if self.continue_composer_list(window, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if !action.secondary && !action.shift && self.command_picker.open {
            if self.accept_selected_command(window, cx) {
                cx.stop_propagation();
                return;
            }
            self.command_picker.open = false;
        }
        if action.secondary && !action.shift {
            cx.stop_propagation();
            if let Some(intent) = self.composer_modifier_intent(false, cx) {
                self.submit_composer_key(intent, window, cx);
            }
        } else if !action.shift {
            // Capturing the action precedes GPUI's decision to insert a newline.
            // Consult the live draft even if the last render saw another line count.
            self.sync_composer_enter_mode(cx);
        }
    }

    pub(crate) fn alternate_multiline_send(
        &mut self,
        _: &AlternateMultilineSend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.composer_key_is_focused(window, cx)
            && !self.command_picker.open
            && let Some(intent) = self.composer_modifier_intent(true, cx)
        {
            cx.stop_propagation();
            self.submit_composer_key(intent, window, cx);
        }
    }

    pub(crate) fn composer_key_is_focused(&self, window: &mut Window, cx: &mut App) -> bool {
        self.draft_owner == self.draft_target
            && !window.has_active_dialog(cx)
            && self.input.read(cx).focus_handle(cx).is_focused(window)
    }

    fn composer_modifier_intent(&self, shift: bool, cx: &App) -> Option<SendIntent> {
        let mode = cx.global::<ComposerPreferences>().settings.mode;
        let busy = self
            .session
            .as_ref()
            .is_some_and(|session| self.busy.contains(&session.id));
        modifier_send_intent(mode, &self.input.read(cx).value(), shift, busy)
    }

    pub(crate) fn sync_composer_enter_mode(&mut self, cx: &mut Context<Self>) -> bool {
        let mode = cx.global::<ComposerPreferences>().settings.mode;
        let submit_on_enter =
            self.composer_can_submit && !mode.requires_modifier(&self.input.read(cx).value());
        if submit_on_enter == self.composer_submit_on_enter {
            return false;
        }
        self.composer_submit_on_enter = submit_on_enter;
        self.input.update(cx, |input, cx| {
            input.set_submit_on_enter(submit_on_enter, cx)
        });
        true
    }

    pub(crate) fn send_and_start_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !window.has_active_dialog(cx) && self.can_send_and_start_draft(cx) {
            self.submit_composer_key(SendIntent::AndStartDraft, window, cx);
        }
    }

    pub(crate) fn can_send_and_start_draft(&self, cx: &Context<Self>) -> bool {
        let Some(source) = self.session.as_ref() else { return false };
        let attachment_key = format!("session:{}", source.id);
        let lead = format!("{}:orchestrator", source.workspace_id);
        let text = self.input.read(cx).value();
        self.can_start_new_conversation()
            && self.conversation_ready
            && self.viewing_conversation()
            && !self.command_picker.open
            && !self.conversation_is_read_only()
            && !source.scratch
            && (source.execution.is_none() || source.can_start_fresh_worktree_draft)
            && !source.archived
            && source.profile.is_some()
            && source.display_agent_id().is_none_or(|agent| agent == lead)
            && self.workspace.as_ref() == Some(&source.workspace_id)
            && self.draft_target.as_deref() == Some(attachment_key.as_str())
            && self.draft_owner == self.draft_target
            && !self.busy.contains(&source.id)
            && !self.inflight_inputs.contains_key(&source.id)
            && self.pending.get(&source.id).copied().unwrap_or(0) == 0
            && self.pending_user_inputs.is_empty()
            && self.delivery_failure.is_none()
            && self.interactions.sessions.get(&source.id).is_none_or(Vec::is_empty)
            && !self.current_history_access().is_some_and(|access| access.handoff_unconsumed() || access.handoff_blocked())
            && !text.trim_start().starts_with('/')
            && (!text.trim().is_empty() || !self.attachments.refs(Some(&attachment_key)).is_empty())
            && self.attachments.ready(Some(&attachment_key))
    }

    pub(crate) fn submit_composer_key(
        &mut self,
        intent: SendIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
            self.send(intent, window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SendIntent, SendMode, modifier_send_intent};

    #[test]
    fn native_modifier_routes_follow_up_only_in_the_selected_mode() {
        let configured = Some(SendIntent::Configured);
        let alternate = Some(SendIntent::AlternateWhenBusy);
        let multiline = SendMode::ModifierEnterMultiline;
        assert_eq!(modifier_send_intent(multiline, "one\ntwo", false, true), configured);
        assert_eq!(modifier_send_intent(multiline, "one\ntwo", true, true), alternate);
        assert_eq!(modifier_send_intent(multiline, "one\ntwo", false, false), configured);
        assert_eq!(modifier_send_intent(multiline, "one\ntwo", true, false), None);
        assert_eq!(modifier_send_intent(multiline, "one", false, true), alternate);
        assert_eq!(modifier_send_intent(multiline, "one", true, true), None);
        for mode in [SendMode::Enter, SendMode::ModifierEnter] {
            assert_eq!(modifier_send_intent(mode, "one\ntwo", false, true), alternate);
            assert_eq!(modifier_send_intent(mode, "one\ntwo", true, true), None);
        }
    }
}
