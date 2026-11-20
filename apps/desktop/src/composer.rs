mod preferences;
mod settings;
use crate::app::HiveApp;
use gpui_kit::{component::{WindowExt, input::Enter}, *};
pub use preferences::{ComposerPreferences, SendMode, ToolActivity, restore};

#[derive(Clone, Copy)]
pub(crate) enum SendIntent {
    Configured,
    AlternateWhenBusy,
    AndStartDraft,
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
        // Other fields keep their native Enter behavior.
        if action.secondary && !action.shift {
            cx.stop_propagation();
            self.submit_composer_key(SendIntent::AlternateWhenBusy, window, cx);
        }
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
