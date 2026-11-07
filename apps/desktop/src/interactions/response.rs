use super::*;
use serde_json::json;

impl HiveApp {
    pub(super) fn answer_interaction(
        &mut self,
        session: String,
        id: String,
        decision: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
        if !self.workspace_writes_allowed()
            || !self.connected
            || self
                .session
                .as_ref()
                .is_none_or(|current| current.id != session)
        {
            return;
        }

        let Some(card) = self
            .interactions
            .sessions
            .get_mut(&session)
            .and_then(|cards| cards.iter_mut().find(|card| card.raw.id == id))
        else {
            return;
        };
        if !card.waiting() {
            return;
        }
        let response = if let Some(decision) = decision {
            if !card.accepts(decision)
                || (decision == "accept"
                    && card.raw.method == "item/fileChange/requestApproval"
                    && !card.has_diff())
            {
                return;
            }
            json!({"decision":decision})
        } else {
            if card.questions.is_empty() {
                return;
            }
            let mut answers = serde_json::Map::new();
            for draft in &card.questions {
                let answer = if draft.other {
                    draft.input.read(cx).value().to_string()
                } else {
                    draft.selected.clone().unwrap_or_default()
                };
                if answer.trim().is_empty() {
                    card.error = Some("请回答每个问题；取消本轮请使用取消按钮。".into());
                    cx.notify();
                    return;
                }
                answers.insert(draft.question.id.clone(), json!({"answers":[answer]}));
            }
            json!({"answers":answers})
        };
        self.submit_interaction(session, id, response, cx);
    }
    pub(super) fn submit_interaction(
        &mut self,
        session: String,
        id: String,
        response: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if !self.workspace_writes_allowed()
            || !self.connected
            || self
                .session
                .as_ref()
                .is_none_or(|current| current.id != session)
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let Some(card) = self
            .interactions
            .sessions
            .get_mut(&session)
            .and_then(|cards| cards.iter_mut().find(|card| card.raw.id == id))
        else {
            return;
        };
        if !card.waiting() {
            return;
        }
        card.submitting = true;
        card.error = None;
        card.generation += 1;
        let generation = card.generation;
        // The native identity stays bound to this card even while navigating between sessions.
        let identity = (
            card.raw.thread_id.clone(),
            card.raw.turn_id.clone(),
            card.raw.item_id.clone(),
            card.raw.native_request_id.clone(),
        );
        self.refresh_interaction_status(&session);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.respondInteraction",
                    json!({"sessionId":session,"interactionId":id,"response":response}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                let Some(card) = this
                    .interactions
                    .sessions
                    .get_mut(&session)
                    .and_then(|cards| cards.iter_mut().find(|card| card.raw.id == id))
                else {
                    return;
                };
                if card.generation != generation
                    || (
                        card.raw.thread_id.clone(),
                        card.raw.turn_id.clone(),
                        card.raw.item_id.clone(),
                        card.raw.native_request_id.clone(),
                    ) != identity
                {
                    return;
                }
                card.submitting = false;
                match result {
                    Ok(value) if value["accepted"] == true => {
                        card.submitting = true;
                        card.raw.state = "responding".into();
                        card.questions.clear();
                        card.initialized = true;
                    }
                    Ok(_) => {
                        card.error = Some(
                            if card.is_pi() || card.is_claude() {
                                "回应尚未发送，请重试。"
                            } else {
                                "Codex 尚未确认提交，请重试。"
                            }
                            .into(),
                        )
                    }
                    // Do not display provider error bodies: they may contain submitted secret values.
                    Err(_) if card.raw.state == "responding" => {}
                    Err(_) => card.error = Some("提交失败，输入已保留。请检查连接后重试。".into()),
                }
                this.refresh_interaction_status(&session);
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn cancel_interaction_turn(
        &mut self,
        session: String,
        id: String,
        cx: &mut Context<Self>,
    ) {
        if !self.workspace_writes_allowed()
            || !self.connected
            || self
                .session
                .as_ref()
                .is_none_or(|current| current.id != session)
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let Some(card) = self
            .interactions
            .sessions
            .get_mut(&session)
            .and_then(|cards| cards.iter_mut().find(|card| card.raw.id == id))
        else {
            return;
        };
        if !card.waiting() {
            return;
        }
        card.submitting = true;
        card.generation += 1;
        card.error = None;
        let generation = card.generation;
        self.refresh_interaction_status(&session);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.abort", json!({"sessionId":session}))
                .await;
            let _ = this.update(cx, |this, cx| {
                let Some(card) = this
                    .interactions
                    .sessions
                    .get_mut(&session)
                    .and_then(|cards| cards.iter_mut().find(|card| card.raw.id == id))
                else {
                    return;
                };
                if card.generation != generation {
                    return;
                }
                card.submitting = false;
                match result {
                    Ok(_) => {
                        card.submitting = true;
                        card.raw.state = "responding".into();
                        card.questions.clear();
                        card.initialized = true;
                    }
                    Err(_) => card.error = Some("取消失败，输入已保留，请重试。".into()),
                }
                this.refresh_interaction_status(&session);
                cx.notify();
            });
        })
        .detach();
    }
}
