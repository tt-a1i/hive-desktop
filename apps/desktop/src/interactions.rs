mod questions;
mod response;
mod view;

use crate::{
    app::HiveApp,
    protocol::{CodexInteraction, PiEvent},
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
    raw: CodexInteraction,
    questions: Vec<QuestionDraft>,
    initialized: bool,
    submitting: bool,
    generation: u64,
    error: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Question {
    id: String,
    header: String,
    question: String,
    #[serde(default)]
    is_other: bool,
    #[serde(default)]
    is_secret: bool,
    options: Option<Vec<QuestionOption>>,
}
#[derive(Clone, Deserialize)]
struct QuestionOption {
    label: String,
    description: String,
}
struct QuestionDraft {
    question: Question,
    input: Entity<InputState>,
    selected: Option<String>,
    other: bool,
    _subscription: Subscription,
}
impl InteractionCard {
    fn new(raw: CodexInteraction) -> Self {
        Self {
            raw,
            questions: vec![],
            initialized: false,
            submitting: false,
            generation: 0,
            error: None,
        }
    }
    fn waiting(&self) -> bool {
        !self.submitting && self.raw.state == "pending"
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
    pub fn sync_interactions(&mut self, session: &str, snapshot: Vec<CodexInteraction>) {
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
        self.status = if cards.iter().any(InteractionCard::waiting) {
            "Codex 等待你的回应"
        } else {
            "Codex 正在处理回应"
        }
        .into();
        true
    }
    fn upsert_interaction(&mut self, session: &str, raw: CodexInteraction) {
        if raw.backend != "codex" {
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
                        "resolved" => "Codex 已接收回应",
                        "aborted" => "已停止当前回复",
                        "disconnected" | "disposed" => "Codex 交互已断开",
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
            if card.initialized
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
                                    "输入回答（隐藏内容）"
                                } else {
                                    "输入回答…"
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
                            _subscription: subscription,
                        });
                    }
                }
                Err(_) => card.error = Some("无法读取 Codex 问题，请取消本轮后重试。".into()),
            }
        }
    }
}
