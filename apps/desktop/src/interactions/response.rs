use super::*;
use serde_json::json;

fn acp_offers_option(params: &serde_json::Value, option: &str) -> bool {
    params["options"].as_array().is_some_and(|options| options.iter().any(|entry| entry["optionId"].as_str() == Some(option)))
}
impl HiveApp {
    pub(super) fn answer_acp_permission(&mut self, session: String, id: String, option: String, cx: &mut Context<Self>) {
        let valid = self.interactions.sessions.get(&session).and_then(|cards| cards.iter().find(|card| card.raw.id == id)).is_some_and(|card| card.raw.backend == "acp" && card.raw.method == "session/request_permission" && card.waiting() && acp_offers_option(&card.raw.params, &option));
        if valid { self.submit_interaction(session, id, json!({"optionId":option}), cx); }
    }

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
            if card.attachment_save_failed
                || card.questions.is_empty()
                || card.questions.iter().any(|draft| draft.importing)
            {
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
        let attachments = card
            .questions
            .iter()
            .filter(|draft| !draft.attachments.is_empty())
            .map(|draft| {
                (
                    draft.question.id.clone(),
                    json!(
                        draft
                            .attachments
                            .iter()
                            .map(|item| &item.id)
                            .collect::<Vec<_>>()
                    ),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        self.refresh_interaction_status(&session);
        cx.notify();
        let connection = self.connection_generation;
        let workspace = self.workspace.clone();
        cx.spawn(async move |this, cx| {
            let has_attachments = !attachments.is_empty();
            let mut params = json!({"sessionId":session,"interactionId":id,"response":response});
            if !attachments.is_empty() {
                params["attachmentsByQuestionId"] = json!(attachments);
            }
            let result = runtime.request("chat.respondInteraction", params).await;
            let safe_retry = if result.is_err() && has_attachments {
                match runtime
                    .request(
                        "chat.interactionAttachments.list",
                        json!({"sessionId":session}),
                    )
                    .await
                {
                    Ok(value) => value["receipts"]
                        .as_array()
                        .map(|receipts| {
                            receipts
                                .iter()
                                .find(|receipt| {
                                    receipt["interactionId"] == id
                                        && receipt["threadId"].as_str() == identity.0.as_deref()
                                        && receipt["turnId"].as_str() == identity.1.as_deref()
                                        && receipt["itemId"].as_str() == identity.2.as_deref()
                                        && receipt["nativeRequestId"] == identity.3
                                })
                                .is_none_or(|receipt| receipt["status"] == "draft")
                        })
                        .unwrap_or(false),
                    Err(_) => false,
                }
            } else {
                false
            };
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                if has_attachments {
                    if let Some(workspace) = &workspace {
                        this.interactions
                            .receipt_requests
                            .remove(&format!("{}:{}:{}", connection, workspace, session));
                        cx.notify();
                    }
                }
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
                    Err(_) if !has_attachments || safe_retry => {
                        card.error = Some("提交失败，输入已保留。请检查连接后重试。".into());
                    }
                    Err(_) => {
                        card.submitting = true;
                        card.error = Some("提交结果未知，请勿重复提交；请检查会话状态。".into());
                    }
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
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.abort", json!({"sessionId":session}))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
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

#[cfg(test)]
mod acp_tests {
    #[test]
    fn permission_accepts_only_native_option_id() {
        let params=serde_json::json!({"options":[{"optionId":"deny-id","name":"Deny","kind":"reject_once"}]});
        assert!(super::acp_offers_option(&params,"deny-id"));
        assert!(!super::acp_offers_option(&params,"accept"));
    }
}
