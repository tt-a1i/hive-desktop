mod claude;
mod pi;
mod questions;
mod response;
mod view;

use crate::language::{english,tr};
use crate::{
    app::{HiveApp,Status},
    protocol::{AgentInteraction, PiEvent},
};
use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Default)]
pub struct Interactions {
    pub sessions: HashMap<String, Vec<InteractionCard>>,
}
pub struct InteractionCard {
    raw: AgentInteraction,
    pi_input: Option<Entity<InputState>>,
    pi_editor: Option<Entity<gpui_kit::component::input::TextareaState>>,
    questions: Vec<QuestionDraft>,
    initialized: bool,
    submitting: bool,
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
    _subscription: Subscription,
}
enum InteractionError { Message(&'static str), MissingIdentity(&'static str) }
impl From<&'static str> for InteractionError {
    fn from(key: &'static str) -> Self { Self::Message(key) }
}
impl InteractionError {
    fn text(&self) -> String {
        match self {
            Self::Message(key) => tr(key).to_owned(),
            Self::MissingIdentity(backend) => if english() {
                format!("Invalid {backend} interaction: native identity fields are missing. Use Stop in the input area to end this turn.")
            } else { format!("{backend} 交互格式错误：缺少原生身份字段。请使用输入区的停止按钮结束本轮。") },
        }
    }
}
impl Interactions {
    pub(crate) fn localize_placeholders(&self, window: &mut Window, cx: &mut App) {
        for card in self.sessions.values().flatten() {
            if card.is_pi() {
                let label = card.raw.params["placeholder"].as_str().unwrap_or(tr("输入回答…"));
                if let Some(input) = &card.pi_input { input.update(cx, |input,cx|input.set_placeholder(label,window,cx)); }
                if let Some(input) = &card.pi_editor { input.update(cx, |input,cx|input.set_placeholder(label,window,cx)); }
            } else {
                for draft in &card.questions {
                    let label = if card.is_claude() {tr("其他回答…")} else if draft.question.is_secret {tr("输入回答（隐藏内容）")} else {tr("输入回答…")};
                    draft.input.update(cx, |input,cx|input.set_placeholder(label,window,cx));
                }
            }
        }
    }
}
impl InteractionCard {
    fn new(raw: AgentInteraction) -> Self {
        let error = (!raw.has_native_identity()).then(|| {
            InteractionError::MissingIdentity(if raw.backend == "claude" { "Claude" } else { "Codex" })
        });
        Self {
            raw,
            pi_input: None,
            pi_editor: None,
            questions: vec![],
            initialized: false,
            submitting: false,
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
            _ => "Codex",
        }
    }
    fn waiting(&self) -> bool {
        self.raw.has_native_identity() && !self.submitting && self.raw.state == "pending"
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
    pub fn interaction_count(&self, session: &str) -> usize {
        self.interactions.sessions.get(session).map_or(0, |cards| {
            cards.iter().filter(|card| card.waiting()).count()
        })
    }
    pub fn sync_interactions(&mut self, session: &str, snapshot: Vec<AgentInteraction>) {
        let cards = self
            .interactions
            .sessions
            .entry(session.into())
            .or_default();
        cards.retain(|card| snapshot.iter().any(|raw| raw.id == card.raw.id));
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
            phase: if card.waiting() { "等待你的回应" }
                else if card.is_pi() || card.is_claude() { "正在发送回应" }
                else { "正在处理回应" },
        };
        true
    }
    fn upsert_interaction(&mut self, session: &str, raw: AgentInteraction) {
        if !matches!(raw.backend.as_str(), "codex" | "pi-rpc" | "claude") {
            return;
        }
        let cards = self
            .interactions
            .sessions
            .entry(session.into())
            .or_default();
        if let Some(card) = cards.iter_mut().find(|card| card.raw.id == raw.id) {
            if raw.state == "responding" {
                card.submitting = true;
                card.questions.clear();
                card.initialized = true;
            }
            if !card.raw.has_native_identity() && raw.has_native_identity() {
                card.error = None;
            }
            if !raw.has_native_identity() {
                card.error = Some(InteractionError::MissingIdentity(if raw.backend == "claude" { "Claude" } else { "Codex" }));
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
                if let Some(cards) = self.interactions.sessions.get_mut(session_id) {
                    cards.retain(|card| card.raw.id != *interaction_id);
                }
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
                            _subscription: subscription,
                        });
                    }
                }
                Err(_) => card.error = Some("无法读取 Codex 问题，请取消本轮后重试。".into()),
            }
        }
    }
}
