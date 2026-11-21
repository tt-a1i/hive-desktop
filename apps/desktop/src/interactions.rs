mod acp;
mod attachments;
mod claude;
mod pi;
mod questions;
mod response;
mod view;

use crate::language::{english, tr};
use crate::transport::InteractionRequestError;
use crate::{
    app::{HiveApp, Status},
    protocol::{AgentInteraction, PiEvent},
};
use gpui_kit::{
    component::{
        WindowExt,
        input::{InputEvent, InputState},
    },
    *,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

#[derive(Default)]
pub struct Interactions {
    uncertain: acp::Uncertain,
    pub sessions: HashMap<String, Vec<InteractionCard>>,
    receipts: HashMap<String, QuestionReceipts>,
    receipt_requests: HashMap<String, u64>,
    receipt_revision: u64,
}

struct QuestionReceipts {
    rows: Vec<serde_json::Value>,
    has_unreadable: bool,
}
pub struct InteractionCard {
    raw: AgentInteraction,
    pi_input: Option<Entity<InputState>>,
    pi_editor: Option<Entity<gpui_kit::component::input::TextareaState>>,
    questions: Vec<QuestionDraft>,
    quick_select_question: Option<usize>,
    initialized: bool,
    submitting: bool,
    attachment_save_failed: bool,
    attachment_import_error: Option<&'static str>,
    generation: u64,
    error: Option<InteractionError>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Question {
    #[serde(default)]
    id: String,
    #[serde(default)]
    header: String,
    question: String,
    #[serde(default)]
    is_other: bool,
    #[serde(default)]
    is_secret: bool,
    options: Option<Vec<QuestionOption>>,
    #[serde(default)]
    multi_select: bool,
}
#[derive(Clone, Deserialize)]
struct QuestionOption {
    label: String,
    #[serde(default)]
    description: String,
    preview: Option<String>,
}
struct QuestionDraft {
    question: Question,
    input: Entity<InputState>,
    selected: Option<String>,
    multiple_selected: Vec<String>,
    other: bool,
    attachments: Vec<crate::protocol::AttachmentRef>,
    import_generation: u64,
    importing: bool,
    _subscription: Subscription,
}
enum InteractionError {
    Message(&'static str),
    MissingIdentity(&'static str),
    AttachmentCopyUnavailable(String),
}
impl From<&'static str> for InteractionError {
    fn from(key: &'static str) -> Self {
        Self::Message(key)
    }
}
impl InteractionError {
    fn text(&self) -> String {
        match self {
            Self::Message(key) => tr(key).to_owned(),
            Self::AttachmentCopyUnavailable(_) => tr(
                "附件副本已丢失、无法读取或已更改。回答已保留。请移除标记的附件，需要时重新选择文件。",
            ).to_owned(),
            Self::MissingIdentity(backend) => {
                if english() {
                    format!(
                        "Invalid {backend} interaction: native identity fields are missing. Use Stop in the input area to end this turn."
                    )
                } else {
                    format!(
                        "{backend} 交互格式错误：缺少原生身份字段。请使用输入区的停止按钮结束本轮。"
                    )
                }
            }
        }
    }
}
impl Interactions {
    fn retain_session_cards(&mut self, session: &str, keep: impl FnMut(&InteractionCard) -> bool) {
        if let Some(cards) = self.sessions.get_mut(session) {
            cards.retain(keep);
            if cards.is_empty() {
                self.sessions.remove(session);
            }
        }
    }

    pub(crate) fn localize_placeholders(&self, window: &mut Window, cx: &mut App) {
        for card in self.sessions.values().flatten() {
            if card.is_pi() {
                let label = card.raw.params["placeholder"]
                    .as_str()
                    .unwrap_or(tr("输入回答…"));
                if let Some(input) = &card.pi_input {
                    input.update(cx, |input, cx| input.set_placeholder(label, window, cx));
                }
                if let Some(input) = &card.pi_editor {
                    input.update(cx, |input, cx| input.set_placeholder(label, window, cx));
                }
            } else {
                for draft in &card.questions {
                    let label = if card.is_claude() {
                        tr("其他回答…")
                    } else if draft.question.is_secret {
                        tr("输入回答（隐藏内容）")
                    } else {
                        tr("输入回答…")
                    };
                    draft
                        .input
                        .update(cx, |input, cx| input.set_placeholder(label, window, cx));
                }
            }
        }
    }
}
impl InteractionCard {
    fn new(raw: AgentInteraction) -> Self {
        let error = (!raw.has_native_identity()).then(|| {
            InteractionError::MissingIdentity(if raw.backend == "claude" { "Claude" } else if raw.backend == "acp" { "ACP" } else { "Codex" })
        });
        Self {
            raw,
            pi_input: None,
            pi_editor: None,
            questions: vec![],
            quick_select_question: None,
            initialized: false,
            submitting: false,
            attachment_save_failed: false,
            attachment_import_error: None,
            generation: 0,
            error,
        }
    }
    fn is_pi(&self) -> bool {
        self.raw.backend == "pi-rpc"
    }
    fn is_claude(&self) -> bool {
        self.raw.backend == "claude"
    }
    fn backend_label(&self) -> &'static str {
        match self.raw.backend.as_str() {
            "pi-rpc" => "外部 Pi",
            "claude" => "Claude",
            "acp" => "ACP",
            _ => "Codex",
        }
    }
    fn waiting(&self) -> bool {
        self.raw.has_native_identity() && !self.submitting && self.raw.state == "pending"
    }
    fn importing_attachments(&self) -> bool {
        self.questions.iter().any(|draft| draft.importing)
    }
    fn attachment_copy_error(&self, error: InteractionRequestError) -> Option<InteractionError> {
        let InteractionRequestError::AttachmentCopyUnavailable(id) = error else {
            return None;
        };
        self.questions
            .iter()
            .flat_map(|draft| &draft.attachments)
            .any(|attachment| attachment.id == id)
            .then_some(InteractionError::AttachmentCopyUnavailable(id))
    }
    fn accepts(&self, decision: &str) -> bool {
        self.raw.params["availableDecisions"]
            .as_array()
            .is_none_or(|available| {
                available
                    .iter()
                    .any(|value| value.as_str() == Some(decision))
            })
    }
    fn decisions(&self) -> Vec<(&'static str, &'static str)> {
        let choices = [
            ("accept", "仅批准本次"),
            ("decline", "拒绝本次"),
            ("cancel", "取消请求"),
        ];
        match self.raw.params["availableDecisions"].as_array() {
            Some(available) => available
                .iter()
                .filter_map(|value| {
                    choices
                        .iter()
                        .find(|(decision, _)| value.as_str() == Some(*decision))
                        .copied()
                })
                .collect(),
            None => choices.to_vec(),
        }
    }
    fn has_diff(&self) -> bool {
        self.raw
            .item
            .as_ref()
            .and_then(|item| item["changes"].as_array())
            .is_some()
    }
}

impl HiveApp {
    pub(crate) fn inbox_interaction_ids(&self, session: &str) -> std::collections::BTreeSet<String> {
        self.interactions.sessions.get(session).into_iter().flatten()
            .filter(|card| card.waiting())
            .map(|card| card.raw.id.clone())
            .collect()
    }

    pub fn interaction_count(&self, session: &str) -> usize {
        self.interactions.sessions.get(session).map_or(0, |cards| {
            cards.iter().filter(|card| card.waiting()).count()
        })
    }
    pub fn sync_interactions(&mut self, session: &str, snapshot: Vec<AgentInteraction>) {
        self.interactions.retain_session_cards(session, |card| {
            snapshot.iter().any(|raw| raw.id == card.raw.id)
        });
        for raw in snapshot {
            self.upsert_interaction(session, raw);
        }
        if self.connected
            && !self.closing
            && self
                .session
                .as_ref()
                .is_some_and(|current| current.id == session)
            && !self.refresh_interaction_status(session)
        {
            self.status = "后台已连接".into();
        }
    }
    fn refresh_interaction_status(&mut self, session: &str) -> bool {
        if !self.connected
            || self.closing
            || self
                .session
                .as_ref()
                .is_none_or(|current| current.id != session)
        {
            return false;
        }
        let Some(cards) = self
            .interactions
            .sessions
            .get(session)
            .filter(|cards| !cards.is_empty())
        else {
            return false;
        };
        let card = cards
            .iter()
            .find(|card| card.waiting())
            .unwrap_or(&cards[0]);
        if !card.raw.has_native_identity() {
            self.status = Status::InvalidInteraction(card.backend_label());
            return true;
        }
        self.status = Status::Interaction {
            backend: card.backend_label(),
            phase: if card.waiting() {
                "等待你的回应"
            } else if card.is_pi() || card.is_claude() {
                "正在发送回应"
            } else {
                "正在处理回应"
            },
        };
        true
    }
    fn upsert_interaction(&mut self, session: &str, raw: AgentInteraction) {
        if !matches!(raw.backend.as_str(), "codex" | "pi-rpc" | "claude" | "acp") {
            return;
        }
        let cards = self
            .interactions
            .sessions
            .entry(session.into())
            .or_default();
        if let Some(card) = cards.iter_mut().find(|card| card.raw.id == raw.id) {
            if (
                card.raw.backend.as_str(),
                card.raw.method.as_str(),
                &card.raw.thread_id,
                &card.raw.turn_id,
                &card.raw.item_id,
                &card.raw.native_request_id,
            ) != (
                raw.backend.as_str(),
                raw.method.as_str(),
                &raw.thread_id,
                &raw.turn_id,
                &raw.item_id,
                &raw.native_request_id,
            ) {
                *card = InteractionCard::new(raw);
                return;
            }
            if raw.state == "responding" {
                card.submitting = true;
                card.questions.clear();
                card.quick_select_question = None;
                card.initialized = true;
            }
            if !card.raw.has_native_identity() && raw.has_native_identity() {
                card.error = None;
            }
            if !raw.has_native_identity() {
                card.error = Some(InteractionError::MissingIdentity(
                    if raw.backend == "claude" {
                        "Claude"
                    } else {
                        "Codex"
                    },
                ));
            }
            card.raw = raw;
        } else {
            cards.push(InteractionCard::new(raw));
        }
        self.refresh_interaction_status(session);
    }
    pub fn apply_interaction_event(&mut self, event: &PiEvent) {
        match event {
            PiEvent::Interaction {
                session_id,
                interaction,
                ..
            } => self.upsert_interaction(session_id, interaction.clone()),
            PiEvent::InteractionResolved {
                session_id,
                interaction_id,
                reason,
                ..
            } => {
                let claude = self
                    .interactions
                    .sessions
                    .get(session_id)
                    .and_then(|cards| cards.iter().find(|card| card.raw.id == *interaction_id))
                    .is_some_and(InteractionCard::is_claude);
                let pi = self
                    .interactions
                    .sessions
                    .get(session_id)
                    .and_then(|cards| cards.iter().find(|card| card.raw.id == *interaction_id))
                    .is_some_and(InteractionCard::is_pi);
                self.interactions.retain_session_cards(session_id, |card| card.raw.id != *interaction_id);
                if self.connected
                    && !self.closing
                    && self
                        .session
                        .as_ref()
                        .is_some_and(|session| session.id == *session_id)
                    && !self.refresh_interaction_status(session_id)
                {
                    self.status = match reason.as_str() {
                        "resolved" if claude => "Claude 回应已提交",
                        "resolved" if pi => "外部 Pi 回应已发送",
                        "resolved" => "Codex 已接收回应",
                        "expired" if claude => "Claude 交互已超时",
                        "expired" => "外部 Pi 交互已超时",
                        "aborted" => "已停止当前回复",
                        "disconnected" | "disposed" if claude => "Claude 交互已断开",
                        "disconnected" | "disposed" if pi => "外部 Pi 交互已断开",
                        "disconnected" | "disposed" => "Codex 交互已断开",
                        _ if claude => "Claude 回合已结束",
                        _ if pi => "外部 Pi 回合已结束",
                        _ => "Codex 回合已结束",
                    }
                    .into();
                }
            }
            _ => {}
        }
    }
    pub fn prepare_interactions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_question_receipts(cx);
        let Some(cards) = self
            .session
            .as_ref()
            .and_then(|session| self.interactions.sessions.get_mut(&session.id))
        else {
            return;
        };
        for card in cards {
            if card.is_claude() {
                card.prepare_claude(window, cx);
                continue;
            }
            if card.is_pi() {
                card.prepare_pi(window, cx);
                continue;
            }
            if !card.raw.has_native_identity()
                || card.initialized
                || card.raw.state != "pending"
                || card.raw.method != "item/tool/requestUserInput"
            {
                continue;
            }
            card.initialized = true;
            match serde_json::from_value::<Vec<Question>>(card.raw.params["questions"].clone()) {
                Ok(questions) => {
                    for question in questions {
                        let input = cx.new(|cx| {
                            InputState::new(window, cx)
                                .masked(question.is_secret)
                                .placeholder(if question.is_secret {
                                    tr("输入回答（隐藏内容）")
                                } else {
                                    tr("输入回答…")
                                })
                        });
                        let subscription =
                            cx.subscribe_in(&input, window, |_, _, event: &InputEvent, _, cx| {
                                if matches!(event, InputEvent::Change) {
                                    cx.notify();
                                }
                            });
                        card.questions.push(QuestionDraft {
                            other: question
                                .options
                                .as_ref()
                                .is_none_or(|options| options.is_empty())
                                || question.is_secret,
                            question,
                            input,
                            selected: None,
                            multiple_selected: vec![],
                            attachments: vec![],
                            import_generation: 0,
                            importing: false,
                            _subscription: subscription,
                        });
                    }
                }
        Err(_) => card.error = Some("无法读取 Codex 问题，请取消本轮后重试。".into()),
            }
        }
    }

    pub(super) fn focus_interaction_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let card_input_focused = self
            .session
            .as_ref()
            .and_then(|session| self.interactions.sessions.get(&session.id))
            .is_some_and(|cards| {
                cards
                    .iter()
                    .any(|card| card.has_focused_or_composing_input(window, cx))
            });
        if !card_input_focused {
            self.workspace_focus
                .region(crate::workspace_focus::Area::History)
                .focus(window, cx);
        }
    }

    pub(crate) fn handle_interaction_quick_select_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(digit_index) = quick_select_digit_index(&event.keystroke.key) else {
            return;
        };
        let has_blocked_context = window.context_stack().iter().any(|ctx| {
            ctx.contains("PopupMenu")
                || ctx.contains("Input")
                || ctx.contains("Editor")
                || ctx.contains("HiveTerminal")
                || ctx.contains("HiveGitDiffFind")
                || ctx.contains("HiveConversationFind")
                || ctx.contains("HiveSessionName")
                || ctx.contains("HiveProjectFileSearch")
        });
        let any_editor_focused_or_composing = self
            .workspace_focus
            .region(crate::workspace_focus::Area::Composer)
            .contains_focused(window, cx)
            || self.input.read(cx).focus_handle(cx).is_focused(window)
            || self
                .input
                .update(cx, |i, cx| i.marked_text_range(window, cx).is_some())
            || self
                .workspace_focus
                .region(crate::workspace_focus::Area::Terminal)
                .contains_focused(window, cx)
            || self
                .workspace_focus
                .region(crate::workspace_focus::Area::Git)
                .contains_focused(window, cx)
            || self
                .workspace_focus
                .region(crate::workspace_focus::Area::Files)
                .contains_focused(window, cx)
            || self
                .workspace_focus
                .region(crate::workspace_focus::Area::Sidebar)
                .contains_focused(window, cx)
            || self.folder.read(cx).focus_handle(cx).is_focused(window)
            || self
                .session
                .as_ref()
                .and_then(|s| self.interactions.sessions.get(&s.id))
                .is_some_and(|cards| {
                    cards
                        .iter()
                        .any(|card| card.has_focused_or_composing_input(window, cx))
                });

        let allowed = quick_select_guard_allows(QuickSelectGuard {
            is_held: event.is_held,
            modifiers_empty: event.keystroke.modifiers == Modifiers::none(),
            connected: self.connected,
            closing: self.closing,
            loading: self.loading,
            showing_welcome_guide: self.showing_welcome_guide,
            viewing_conversation: self.viewing_conversation(),
            has_active_dialog_or_sheet: window.has_active_dialog(cx)
                || window.has_active_sheet(cx),
            has_open_overlay: self.choosing_model
                || self.command_picker.open
                || self.mention_picker.open
                || self.usage.visible
                || self.add_workspace_open
                || self.session_name_target.is_some()
                || self.conversation_find.is_some(),
            has_blocked_context,
            any_editor_focused_or_composing,
        });
        if !allowed {
            return;
        }
        let Some(session_id) = self.session.as_ref().map(|s| s.id.clone()) else {
            return;
        };
        let mut pi_submit: Option<(String, String)> = None;
        let mut handled = false;
        if let Some(cards) = self.interactions.sessions.get_mut(&session_id)
            && let Some(card) = cards.iter_mut().find(|c| interaction_supports_quick_select(c))
        {
            if card.is_pi() && card.raw.method == "select" {
                if let Some(option) = card.raw.params["options"]
                    .as_array()
                    .and_then(|opts| opts.get(digit_index))
                    .and_then(|v| v.as_str())
                {
                    pi_submit = Some((card.raw.id.clone(), option.to_owned()));
                    handled = true;
                }
            } else if let Some(q_idx) = active_quick_select_question_index(card)
                && let Some(draft) = card.questions.get_mut(q_idx)
                && let Some(option) = draft
                    .question
                    .options
                    .as_ref()
                    .and_then(|opts| opts.get(digit_index))
            {
                let label = option.label.clone();
                if draft.question.multi_select {
                    if draft.multiple_selected.contains(&label) {
                        draft.multiple_selected.retain(|s| s != &label);
                    } else {
                        draft.multiple_selected.push(label);
                    }
                } else {
                    draft.selected = Some(label);
                    draft.other = false;
                }
                let meta = quick_select_question_meta(&card.questions);
                card.quick_select_question = next_quick_select_pinned_after_choice(&meta, q_idx);
                handled = true;
            }
        }
        if let Some((interaction_id, option)) = pi_submit {
            self.submit_interaction(session_id, interaction_id, json!({"value": option}), cx);
            cx.stop_propagation();
        } else if handled {
            cx.notify();
            cx.stop_propagation();
        }
    }
}

impl InteractionCard {
    pub(super) fn has_focused_or_composing_input(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.questions.iter().any(|draft| {
            draft.input.read(cx).focus_handle(cx).is_focused(window)
                || draft
                    .input
                    .update(cx, |i, cx| i.marked_text_range(window, cx).is_some())
        }) || self.pi_input.as_ref().is_some_and(|input| {
            input.read(cx).focus_handle(cx).is_focused(window)
                || input.update(cx, |i, cx| i.marked_text_range(window, cx).is_some())
        }) || self.pi_editor.as_ref().is_some_and(|editor| {
            editor.read(cx).focus_handle(cx).is_focused(window)
                || editor.update(cx, |i, cx| i.marked_text_range(window, cx).is_some())
        })
    }
}

pub(crate) fn quick_select_digit_index(key: &str) -> Option<usize> {
    match key {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        "4" => Some(3),
        "5" => Some(4),
        "6" => Some(5),
        "7" => Some(6),
        "8" => Some(7),
        "9" => Some(8),
        _ => None,
    }
}

pub(crate) struct QuickSelectGuard {
    pub is_held: bool,
    pub modifiers_empty: bool,
    pub connected: bool,
    pub closing: bool,
    pub loading: bool,
    pub showing_welcome_guide: bool,
    pub viewing_conversation: bool,
    pub has_active_dialog_or_sheet: bool,
    pub has_open_overlay: bool,
    pub has_blocked_context: bool,
    pub any_editor_focused_or_composing: bool,
}

pub(crate) fn quick_select_guard_allows(guard: QuickSelectGuard) -> bool {
    !guard.is_held
        && guard.modifiers_empty
        && guard.connected
        && !guard.closing
        && !guard.loading
        && !guard.showing_welcome_guide
        && guard.viewing_conversation
        && !guard.has_active_dialog_or_sheet
        && !guard.has_open_overlay
        && !guard.has_blocked_context
        && !guard.any_editor_focused_or_composing
}

pub(crate) fn interaction_supports_quick_select(card: &InteractionCard) -> bool {
    card.waiting()
        && ((card.raw.backend == "codex" && card.raw.method == "item/tool/requestUserInput")
            || (card.is_claude() && card.raw.method == "ask-user-question")
            || (card.is_pi() && card.raw.method == "select"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QuickSelectQuestionMeta {
    is_secret: bool,
    options_len: usize,
    other: bool,
    has_selected: bool,
    has_multi: bool,
    multi_select: bool,
}

fn quick_select_question_meta(questions: &[QuestionDraft]) -> Vec<QuickSelectQuestionMeta> {
    questions
        .iter()
        .map(|draft| QuickSelectQuestionMeta {
            is_secret: draft.question.is_secret,
            options_len: draft.question.options.as_ref().map_or(0, Vec::len),
            other: draft.other,
            has_selected: draft.selected.is_some(),
            has_multi: !draft.multiple_selected.is_empty(),
            multi_select: draft.question.multi_select,
        })
        .collect()
}

fn next_quick_select_pinned_after_choice(
    questions: &[QuickSelectQuestionMeta],
    chosen_idx: usize,
) -> Option<usize> {
    let chosen = questions.get(chosen_idx)?;
    if chosen.multi_select {
        return Some(chosen_idx);
    }
    let has_other_unanswered = questions.iter().enumerate().any(|(idx, item)| {
        idx != chosen_idx
            && !item.is_secret
            && item.options_len > 0
            && !item.other
            && !item.has_selected
            && !item.has_multi
    });
    if has_other_unanswered {
        None
    } else {
        Some(chosen_idx)
    }
}

fn active_quick_select_question_index(card: &InteractionCard) -> Option<usize> {
    let meta = quick_select_question_meta(&card.questions);
    active_quick_select_question_index_from_meta(meta.into_iter(), card.quick_select_question)
}

fn active_quick_select_question_index_from_meta<I>(
    items: I,
    pinned: Option<usize>,
) -> Option<usize>
where
    I: Iterator<Item = QuickSelectQuestionMeta> + Clone,
{
    if let Some(idx) = pinned
        && let Some(item) = items.clone().nth(idx)
        && !item.is_secret
        && item.options_len > 0
        && (item.multi_select || !item.other)
    {
        return Some(idx);
    }
    items
        .clone()
        .position(|item| {
            !item.is_secret
                && item.options_len > 0
                && !item.other
                && !item.has_selected
                && !item.has_multi
        })
        .or_else(|| {
            items
                .clone()
                .position(|item| !item.is_secret && item.options_len > 0)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        InteractionCard, QuickSelectGuard, QuickSelectQuestionMeta,
        active_quick_select_question_index_from_meta, interaction_supports_quick_select,
        next_quick_select_pinned_after_choice, quick_select_digit_index, quick_select_guard_allows,
    };
    use crate::protocol::AgentInteraction;

    fn sample_interaction(backend: &str, method: &str, state: &str) -> AgentInteraction {
        let native_request_id = if backend == "claude" {
            serde_json::json!("req-1")
        } else {
            serde_json::json!(1)
        };
        let value = serde_json::json!({
            "id": "int-1",
            "backend": backend,
            "method": method,
            "state": state,
            "threadId": "thread-1",
            "turnId": "turn-1",
            "itemId": "item-1",
            "nativeRequestId": native_request_id,
            "params": {},
        });
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn quick_select_targets_first_unanswered_non_secret_question_and_parses_1_to_9() {
        for digit in 1..=9 {
            assert_eq!(quick_select_digit_index(&digit.to_string()), Some(digit - 1));
        }
        for invalid in ["0", "a", "10", "space", "enter"] {
            assert_eq!(quick_select_digit_index(invalid), None);
        }

        // Skips secret & already-answered questions, targets first unanswered non-secret.
        let meta = [
            QuickSelectQuestionMeta { is_secret: true, options_len: 3, other: false, has_selected: false, has_multi: false, multi_select: false },
            QuickSelectQuestionMeta { is_secret: false, options_len: 2, other: false, has_selected: true, has_multi: false, multi_select: false },
            QuickSelectQuestionMeta { is_secret: false, options_len: 4, other: false, has_selected: false, has_multi: false, multi_select: false },
            QuickSelectQuestionMeta { is_secret: false, options_len: 2, other: false, has_selected: false, has_multi: false, multi_select: false },
        ];
        assert_eq!(
            active_quick_select_question_index_from_meta(meta.into_iter(), None),
            Some(2)
        );

        // Falls back to first non-secret optioned question when all are answered without pin.
        let all_answered = [
            QuickSelectQuestionMeta { is_secret: true, options_len: 2, other: false, has_selected: false, has_multi: false, multi_select: false },
            QuickSelectQuestionMeta { is_secret: false, options_len: 3, other: false, has_selected: true, has_multi: false, multi_select: false },
            QuickSelectQuestionMeta { is_secret: false, options_len: 2, other: false, has_selected: false, has_multi: true, multi_select: true },
        ];
        assert_eq!(
            active_quick_select_question_index_from_meta(all_answered.into_iter(), None),
            Some(1)
        );

        // Multi-question Defect D1 regression: Q0(single-select) + Q1(multi-select).
        let mut flow = [
            QuickSelectQuestionMeta { is_secret: false, options_len: 3, other: false, has_selected: false, has_multi: false, multi_select: false },
            QuickSelectQuestionMeta { is_secret: false, options_len: 4, other: false, has_selected: false, has_multi: false, multi_select: true },
        ];
        let mut pinned = None;
        assert_eq!(
            active_quick_select_question_index_from_meta(flow.iter().copied(), pinned),
            Some(0)
        );
        // Step 1: User presses `1` to answer Q0 (single-select) -> advances cleanly to Q1.
        flow[0].has_selected = true;
        pinned = next_quick_select_pinned_after_choice(&flow, 0);
        assert_eq!(pinned, None);
        assert_eq!(
            active_quick_select_question_index_from_meta(flow.iter().copied(), pinned),
            Some(1)
        );
        // Step 2: User presses `1` to toggle Q1 option 1 (multi-select) -> must STAY on Q1!
        flow[1].has_multi = true;
        pinned = next_quick_select_pinned_after_choice(&flow, 1);
        assert_eq!(pinned, Some(1));
        assert_eq!(
            active_quick_select_question_index_from_meta(flow.iter().copied(), pinned),
            Some(1)
        );
        // Step 3: User continues pressing `2` -> still targets Q1, never jumps back to overwrite Q0!
        pinned = next_quick_select_pinned_after_choice(&flow, 1);
        assert_eq!(
            active_quick_select_question_index_from_meta(flow.iter().copied(), pinned),
            Some(1)
        );
        // Step 4: Even if user toggles all options off on Q1, focus stays pinned on Q1.
        flow[1].has_multi = false;
        pinned = next_quick_select_pinned_after_choice(&flow, 1);
        assert_eq!(
            active_quick_select_question_index_from_meta(flow.iter().copied(), pinned),
            Some(1)
        );

        // Approval cards must never be quick-selected.
        let codex_q = InteractionCard::new(sample_interaction(
            "codex",
            "item/tool/requestUserInput",
            "pending",
        ));
        let codex_approval = InteractionCard::new(sample_interaction(
            "codex",
            "item/commandExecution/requestApproval",
            "pending",
        ));
        let claude_q =
            InteractionCard::new(sample_interaction("claude", "ask-user-question", "pending"));
        let claude_tool =
            InteractionCard::new(sample_interaction("claude", "can-use-tool", "pending"));
        let pi_select = InteractionCard::new(sample_interaction("pi-rpc", "select", "pending"));
        let pi_confirm = InteractionCard::new(sample_interaction("pi-rpc", "confirm", "pending"));
        assert!(interaction_supports_quick_select(&codex_q));
        assert!(!interaction_supports_quick_select(&codex_approval));
        assert!(interaction_supports_quick_select(&claude_q));
        assert!(!interaction_supports_quick_select(&claude_tool));
        assert!(interaction_supports_quick_select(&pi_select));
        assert!(!interaction_supports_quick_select(&pi_confirm));

        // Multi-card queue: only the first waiting quick-selectable card receives key routing/badges.
        let cards = [codex_approval, pi_select, claude_q];
        assert_eq!(cards.iter().position(interaction_supports_quick_select), Some(1));
    }

    #[test]
    fn quick_select_guard_blocks_all_inputs_dialogs_and_popovers() {
        let base = QuickSelectGuard {
            is_held: false,
            modifiers_empty: true,
            connected: true,
            closing: false,
            loading: false,
            showing_welcome_guide: false,
            viewing_conversation: true,
            has_active_dialog_or_sheet: false,
            has_open_overlay: false,
            has_blocked_context: false,
            any_editor_focused_or_composing: false,
        };
        assert!(quick_select_guard_allows(base));

        let cases = [
            QuickSelectGuard { is_held: true, ..QuickSelectGuard { is_held: false, modifiers_empty: true, connected: true, closing: false, loading: false, showing_welcome_guide: false, viewing_conversation: true, has_active_dialog_or_sheet: false, has_open_overlay: false, has_blocked_context: false, any_editor_focused_or_composing: false } },
            QuickSelectGuard { modifiers_empty: false, is_held: false, connected: true, closing: false, loading: false, showing_welcome_guide: false, viewing_conversation: true, has_active_dialog_or_sheet: false, has_open_overlay: false, has_blocked_context: false, any_editor_focused_or_composing: false },
            QuickSelectGuard { has_active_dialog_or_sheet: true, is_held: false, modifiers_empty: true, connected: true, closing: false, loading: false, showing_welcome_guide: false, viewing_conversation: true, has_open_overlay: false, has_blocked_context: false, any_editor_focused_or_composing: false },
            QuickSelectGuard { has_open_overlay: true, is_held: false, modifiers_empty: true, connected: true, closing: false, loading: false, showing_welcome_guide: false, viewing_conversation: true, has_active_dialog_or_sheet: false, has_blocked_context: false, any_editor_focused_or_composing: false },
            QuickSelectGuard { has_blocked_context: true, is_held: false, modifiers_empty: true, connected: true, closing: false, loading: false, showing_welcome_guide: false, viewing_conversation: true, has_active_dialog_or_sheet: false, has_open_overlay: false, any_editor_focused_or_composing: false },
            QuickSelectGuard { any_editor_focused_or_composing: true, is_held: false, modifiers_empty: true, connected: true, closing: false, loading: false, showing_welcome_guide: false, viewing_conversation: true, has_active_dialog_or_sheet: false, has_open_overlay: false, has_blocked_context: false },
            QuickSelectGuard { viewing_conversation: false, is_held: false, modifiers_empty: true, connected: true, closing: false, loading: false, showing_welcome_guide: false, has_active_dialog_or_sheet: false, has_open_overlay: false, has_blocked_context: false, any_editor_focused_or_composing: false },
        ];
        for guard in cases {
            assert!(!quick_select_guard_allows(guard));
        }
    }
}
