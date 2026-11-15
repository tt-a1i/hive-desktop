use super::*;
use serde_json::json;

impl HiveApp {
    pub(super) fn load_question_receipts(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(session), Some(runtime)) = (
            self.workspace.clone(),
            self.session.as_ref().map(|s| s.id.clone()),
            self.runtime.clone(),
        ) else {
            return;
        };
        if !self.connected {
            return;
        }
        let key = format!("{}:{}:{}", self.connection_generation, workspace, session);
        if !self.interactions.receipt_requests.insert(key.clone()) {
            return;
        }
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.interactionAttachments.list",
                    json!({"sessionId":session}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.workspace.as_deref() != Some(&workspace)
                    || this
                        .session
                        .as_ref()
                        .is_none_or(|current| current.id != session)
                {
                    this.interactions.receipt_requests.remove(&key);
                    return;
                }
                if let Ok(value) = result {
                    this.interactions.receipts.insert(
                        key,
                        value["receipts"].as_array().cloned().unwrap_or_default(),
                    );
                } else {
                    this.interactions.receipt_requests.remove(&key);
                    return;
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn question_target(
        &self,
        workspace: &str,
        session: &str,
        id: &str,
        question: &str,
        identity: &(
            Option<String>,
            Option<String>,
            Option<String>,
            serde_json::Value,
        ),
        connection: u64,
    ) -> bool {
        self.connection_generation == connection
            && self.connected
            && !self.closing
            && self.workspace.as_deref() == Some(workspace)
            && self.workspace_writes_allowed()
            && self.session.as_ref().is_some_and(|s| s.id == session)
            && self
                .interactions
                .sessions
                .get(session)
                .and_then(|cards| cards.iter().find(|c| c.raw.id == id))
                .is_some_and(|c| {
                    c.waiting()
                        && c.raw.backend == "codex"
                        && c.raw.method == "item/tool/requestUserInput"
                        && (
                            c.raw.thread_id.clone(),
                            c.raw.turn_id.clone(),
                            c.raw.item_id.clone(),
                            c.raw.native_request_id.clone(),
                        ) == *identity
                        && c.questions
                            .iter()
                            .any(|q| q.question.id == question && !q.question.is_secret)
                })
    }
    pub(super) fn choose_question_attachments(
        &mut self,
        session: String,
        id: String,
        question: String,
        cx: &mut Context<Self>,
    ) {
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let Some(card) = self
            .interactions
            .sessions
            .get_mut(&session)
            .and_then(|cards| cards.iter_mut().find(|c| c.raw.id == id))
        else {
            return;
        };
        let identity = (
            card.raw.thread_id.clone(),
            card.raw.turn_id.clone(),
            card.raw.item_id.clone(),
            card.raw.native_request_id.clone(),
        );
        let Some(draft) = card
            .questions
            .iter_mut()
            .find(|q| q.question.id == question && !q.question.is_secret)
        else {
            return;
        };
        if draft.importing {
            return;
        }
        draft.import_generation += 1;
        let generation = draft.import_generation;
        let connection = self.connection_generation;
        if !self.question_target(&workspace, &session, &id, &question, &identity, connection) {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr("选择图片、文档或其他文件").into()),
        });
        cx.spawn(async move |this, cx| {
            let paths = match receiver.await {
                Ok(Ok(Some(paths))) => paths,
                _ => return,
            };
            let valid = this
                .update(cx, |this, cx| {
                    if !this.question_target(
                        &workspace, &session, &id, &question, &identity, connection,
                    ) {
                        return false;
                    }
                    let draft = this
                        .interactions
                        .sessions
                        .get_mut(&session)
                        .unwrap()
                        .iter_mut()
                        .find(|c| c.raw.id == id)
                        .unwrap()
                        .questions
                        .iter_mut()
                        .find(|q| q.question.id == question)
                        .unwrap();
                    if draft.import_generation != generation
                        || draft.importing
                        || draft.attachments.len() + paths.len() > 10
                    {
                        return false;
                    }
                    draft.importing = true;
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !valid {
                return;
            }
            let mut ready = Vec::new();
            let mut failed = false;
            for path in paths {
                let (method, mut params) =
                    match crate::image_input::ImageSource::File(path).prepare() {
                        Ok(value) => value,
                        Err(_) => {
                            failed = true;
                            continue;
                        }
                    };
                params["workspaceId"] = json!(workspace);
                match runtime.request(method, params).await.ok().and_then(|v| {
                    serde_json::from_value::<crate::protocol::AttachmentRef>(
                        v["attachment"].clone(),
                    )
                    .ok()
                }) {
                    Some(item) => ready.push(item),
                    None => failed = true,
                }
            }
            let _ = this.update(cx, |this, cx| {
                if !this
                    .question_target(&workspace, &session, &id, &question, &identity, connection)
                {
                    if let Some(card) =
                        this.interactions
                            .sessions
                            .get_mut(&session)
                            .and_then(|cards| {
                                cards.iter_mut().find(|card| {
                                    card.raw.id == id
                                        && (
                                            card.raw.thread_id.clone(),
                                            card.raw.turn_id.clone(),
                                            card.raw.item_id.clone(),
                                            card.raw.native_request_id.clone(),
                                        ) == identity
                                })
                            })
                    {
                        if let Some(draft) = card.questions.iter_mut().find(|draft| {
                            draft.question.id == question && draft.import_generation == generation
                        }) {
                            draft.importing = false;
                        }
                    }
                    cx.notify();
                    return;
                }
                let card = this
                    .interactions
                    .sessions
                    .get_mut(&session)
                    .unwrap()
                    .iter_mut()
                    .find(|c| c.raw.id == id)
                    .unwrap();
                let draft = card
                    .questions
                    .iter_mut()
                    .find(|q| q.question.id == question)
                    .unwrap();
                if draft.import_generation != generation {
                    return;
                }
                draft.importing = false;
                for item in ready {
                    if !draft
                        .attachments
                        .iter()
                        .any(|existing| existing.id == item.id)
                    {
                        draft.attachments.push(item);
                    }
                }
                if failed {
                    card.error = Some("部分附件导入失败，请重新选择。".into());
                }
                this.save_question_attachments(session.clone(), id.clone(), cx);
            });
        })
        .detach();
    }
    pub(super) fn save_question_attachments(
        &mut self,
        session: String,
        id: String,
        cx: &mut Context<Self>,
    ) {
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let Some(card) = self
            .interactions
            .sessions
            .get_mut(&session)
            .and_then(|cards| cards.iter_mut().find(|c| c.raw.id == id))
        else {
            return;
        };
        let mapping = card
            .questions
            .iter()
            .filter(|q| !q.attachments.is_empty())
            .map(|q| {
                (
                    q.question.id.clone(),
                    json!(q.attachments.iter().map(|a| &a.id).collect::<Vec<_>>()),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        card.attachment_save_failed = false;
        card.submitting = true;
        card.generation += 1;
        let generation = card.generation;
        let identity = (
            card.raw.thread_id.clone(),
            card.raw.turn_id.clone(),
            card.raw.item_id.clone(),
            card.raw.native_request_id.clone(),
        );
        let workspace = self.workspace.clone();
        let connection = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request("chat.interactionAttachments.save", json!({"sessionId": session, "interactionId": id, "attachmentsByQuestionId": mapping})).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection { return; }
                if this.workspace != workspace || this.session.as_ref().is_none_or(|current| current.id != session) {
                    if let Some(card) = this.interactions.sessions.get_mut(&session).and_then(|cards| cards.iter_mut().find(|card| card.raw.id == id && card.generation == generation && (card.raw.thread_id.clone(), card.raw.turn_id.clone(), card.raw.item_id.clone(), card.raw.native_request_id.clone()) == identity)) {
                        card.submitting = false;
                        card.attachment_save_failed = true;
                        card.error = Some("附件状态保存失败，请重试保存。".into());
                    }
                    return;
                }
                let Some(card) = this.interactions.sessions.get_mut(&session).and_then(|cards| cards.iter_mut().find(|c| c.raw.id == id)) else { return; };
                if card.generation != generation || (card.raw.thread_id.clone(), card.raw.turn_id.clone(), card.raw.item_id.clone(), card.raw.native_request_id.clone()) != identity { return; }
                card.submitting = false;
                if result.is_err() { card.attachment_save_failed = true; card.error = Some("附件状态保存失败，请重试保存。".into()); } else { card.error = None; if let Some(workspace) = &workspace { this.interactions.receipt_requests.remove(&format!("{}:{}:{}", connection, workspace, session)); } }
                cx.notify();
            });
        }).detach();
    }
}
