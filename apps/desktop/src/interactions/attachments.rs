use super::*;
use serde_json::json;

const MAX_QUESTION_ATTACHMENTS: usize = 10;
const ATTACHMENT_LIMIT: &str = "每次问题回复最多添加 10 个附件。请减少选择后再试。";

struct NativeQuestion {
    thread: Option<String>,
    turn: Option<String>,
    item: Option<String>,
    request: serde_json::Value,
}

impl From<&AgentInteraction> for NativeQuestion {
    fn from(raw: &AgentInteraction) -> Self {
        Self {
            thread: raw.thread_id.clone(),
            turn: raw.turn_id.clone(),
            item: raw.item_id.clone(),
            request: raw.native_request_id.clone(),
        }
    }
}

impl NativeQuestion {
    fn matches(&self, raw: &AgentInteraction) -> bool {
        raw.backend == "codex"
            && raw.method == "item/tool/requestUserInput"
            && raw.thread_id == self.thread
            && raw.turn_id == self.turn
            && raw.item_id == self.item
            && raw.native_request_id == self.request
    }
}

struct QuestionImport {
    workspace: String,
    session: String,
    interaction: String,
    question: String,
    native: NativeQuestion,
    connection: u64,
    generation: u64,
}

impl InteractionCard {
    fn remaining_attachment_slots(&self) -> usize {
        MAX_QUESTION_ATTACHMENTS.saturating_sub(
            self.questions
                .iter()
                .map(|draft| draft.attachments.len())
                .sum(),
        )
    }
}

impl HiveApp {
    pub(super) fn load_question_receipts(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(session), Some(runtime)) = (
            self.workspace.clone(),
            self.session.as_ref().map(|s| s.id.clone()),
            self.runtime.clone(),
        ) else {
            self.interactions.receipts.clear();
            self.interactions.receipt_requests.clear();
            return;
        };
        let key = format!("{}:{}:{}", self.connection_generation, workspace, session);
        // Only the visible conversation renders receipts. Release the previous
        // view's metadata and reject its response even if the user returns to it.
        self.interactions.receipts.retain(|scope, _| scope == &key);
        self.interactions
            .receipt_requests
            .retain(|scope, _| scope == &key);
        if !self.connected || self.interactions.receipt_requests.contains_key(&key) {
            return;
        }
        self.interactions.receipt_revision += 1;
        let revision = self.interactions.receipt_revision;
        self.interactions
            .receipt_requests
            .insert(key.clone(), revision);
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.interactionAttachments.list",
                    json!({"sessionId":session}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.interactions.receipt_requests.get(&key) != Some(&revision) {
                    return;
                }
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

    fn question_attachment_card(
        &mut self,
        session: &str,
        id: &str,
        native: &NativeQuestion,
    ) -> Option<&mut InteractionCard> {
        self.interactions
            .sessions
            .get_mut(session)?
            .iter_mut()
            .find(|card| card.raw.id == id && native.matches(&card.raw))
    }

    fn active_question_import(&mut self, import: &QuestionImport) -> Option<&mut InteractionCard> {
        if self.connection_generation != import.connection || !self.connected || self.closing {
            return None;
        }
        let card =
            self.question_attachment_card(&import.session, &import.interaction, &import.native)?;
        if !card.waiting()
            || !card.questions.iter().any(|draft| {
                draft.question.id == import.question
                    && !draft.question.is_secret
                    && draft.importing
                    && draft.import_generation == import.generation
            })
        {
            return None;
        }
        Some(card)
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
        if !self.connected
            || self.closing
            || !self.workspace_writes_allowed()
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
            .and_then(|cards| cards.iter_mut().find(|c| c.raw.id == id))
        else {
            return;
        };
        if !card.waiting()
            || card.importing_attachments()
            || card.raw.backend != "codex"
            || card.raw.method != "item/tool/requestUserInput"
        {
            return;
        }
        if card.remaining_attachment_slots() == 0 {
            card.attachment_import_error = Some(ATTACHMENT_LIMIT);
            cx.notify();
            return;
        }
        let Some(draft) = card
            .questions
            .iter_mut()
            .find(|q| q.question.id == question && !q.question.is_secret)
        else {
            return;
        };
        // One chooser, upload or draft write at a time for the whole card.
        draft.importing = true;
        draft.import_generation += 1;
        let import = QuestionImport {
            workspace,
            session,
            interaction: id,
            question,
            native: NativeQuestion::from(&card.raw),
            connection: self.connection_generation,
            generation: draft.import_generation,
        };
        cx.notify();
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr("选择图片、文档或其他文件").into()),
        });
        cx.spawn(async move |this, cx| {
            let paths = match receiver.await {
                Ok(Ok(Some(paths))) if !paths.is_empty() => paths,
                _ => {
                    let _ = this.update(cx, |this, cx| {
                        this.finish_question_import(&import, Vec::new(), None, cx);
                    });
                    return;
                }
            };
            let valid = this
                .update(cx, |this, cx| {
                    let Some(card) = this.active_question_import(&import) else {
                        return false;
                    };
                    if paths.len() > card.remaining_attachment_slots() {
                        this.finish_question_import(
                            &import,
                            Vec::new(),
                            Some(ATTACHMENT_LIMIT),
                            cx,
                        );
                        return false;
                    }
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
                params["workspaceId"] = json!(import.workspace);
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
                this.finish_question_import(
                    &import,
                    ready,
                    failed.then_some("部分附件导入失败，请重新选择。"),
                    cx,
                );
            });
        })
        .detach();
    }

    fn finish_question_import(
        &mut self,
        import: &QuestionImport,
        ready: Vec<crate::protocol::AttachmentRef>,
        error: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.active_question_import(import) else {
            return;
        };
        let draft = card
            .questions
            .iter_mut()
            .find(|draft| draft.question.id == import.question)
            .unwrap();
        draft.importing = false;
        let changed = !ready.is_empty();
        if changed || error.is_some() {
            card.attachment_import_error = error;
        }
        for item in ready {
            if !draft
                .attachments
                .iter()
                .any(|existing| existing.id == item.id)
            {
                draft.attachments.push(item);
            }
        }
        if changed {
            self.save_question_attachment_draft(
                import.workspace.clone(),
                import.session.clone(),
                import.interaction.clone(),
                cx,
            );
        } else {
            cx.notify();
        }
    }

    pub(super) fn save_question_attachments(
        &mut self,
        session: String,
        id: String,
        cx: &mut Context<Self>,
    ) {
        if self
            .session
            .as_ref()
            .is_none_or(|current| current.id != session)
            || !self.workspace_writes_allowed()
        {
            return;
        }
        if let Some(workspace) = self.workspace.clone() {
            self.save_question_attachment_draft(workspace, session, id, cx);
        }
    }

    fn save_question_attachment_draft(
        &mut self,
        workspace: String,
        session: String,
        id: String,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing {
            return;
        }
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
        if !card.waiting() || card.importing_attachments() {
            return;
        }
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
        if card.attachment_save_failed {
            card.error = None;
        }
        card.attachment_save_failed = false;
        card.submitting = true;
        card.generation += 1;
        let generation = card.generation;
        let identity = NativeQuestion::from(&card.raw);
        let connection = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request("chat.interactionAttachments.save", json!({"sessionId": session, "interactionId": id, "attachmentsByQuestionId": mapping})).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection { return; }
                if let Some(card) = this.question_attachment_card(&session, &id, &identity)
                    .filter(|card| card.generation == generation && card.raw.state == "pending")
                {
                    card.submitting = false;
                    if result.is_err() {
                        card.attachment_save_failed = true;
                        card.error = Some("附件状态保存失败，请重试保存。".into());
                    }
                }
                if result.is_ok() {
                    this.interactions.receipt_requests.remove(&format!("{connection}:{workspace}:{session}"));
                }
                cx.notify();
            });
        }).detach();
    }
}
