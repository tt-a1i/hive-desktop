use crate::{
    app::{DeliveryCard, HiveApp, Message, ToolPresentation},
    protocol::*,
};
use gpui_kit::{component::text::TextViewState, *};
use serde_json::{Value, json};
impl HiveApp {
    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.conversation_ready || !self.workspace_writes_allowed() {
            return;
        }
        let Some(id) = self.session.as_ref().map(|s| s.id.clone()) else {
            return;
        };
        let text = self.input.read(cx).value().to_string();
        let attachment_key = format!("session:{id}");
        let refs = self.attachments.refs(Some(&attachment_key));
        if (text.trim().is_empty() && refs.is_empty())
            || !self.attachments.ready(Some(&attachment_key))
            || self.inflight_inputs.contains_key(&id)
            || (!self.busy.contains(&id) && self.pending.get(&id).copied().unwrap_or(0) > 0)
            || self.loading
            || !self.connected
        {
            return;
        }
        if self.closing {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let was_busy = self.busy.contains(&id);
        self.busy.insert(id.clone());
        self.inflight_inputs.insert(id.clone(), text.clone());
        self.drafts.remove(&format!("session:{id}"));
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.schedule_draft_save(cx);
        let window = window.window_handle();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.prompt", json!({"sessionId":id,"text":text,"attachmentIds":refs.iter().map(|item|&item.id).collect::<Vec<_>>()}))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.inflight_inputs.remove(&id);
                if this.removed_sessions.contains(&id) {
                    return;
                }
                if result.is_ok() {
                    if let Some(items)=this.attachments.drafts.get_mut(&attachment_key) {items.retain(|item| !refs.iter().any(|sent| sent.id==item.id));}
                    this.attachments.prune();
                    this.schedule_draft_save(cx);
                }
                match result {
                    Err(error) => {
                        if !was_busy { this.busy.remove(&id); }
                        if this.session.as_ref().is_some_and(|s| s.id == id)
                            && this.input.read(cx).value().is_empty()
                        {
                            let input = this.input.clone();
                            if window
                                .update(cx, |_, window, cx| {
                                    input.update(cx, |state, cx| {
                                        state.set_value(text.clone(), window, cx)
                                    });
                                })
                                .is_err()
                            {
                                this.failed_drafts
                                    .entry(id.clone())
                                    .or_default()
                                    .push(text.clone());
                            }
                        } else if !this.session.as_ref().is_some_and(|s| s.id == id)
                            && !this.drafts.contains_key(&format!("session:{id}"))
                        {
                            this.drafts.insert(format!("session:{id}"), text.clone());
                        } else {
                            this.failed_drafts
                                .entry(id.clone())
                                .or_default()
                                .push(text.clone());
                        }
                        this.schedule_draft_save(cx);
                        this.error = Some(error.into());
                    }
                    Ok(value) => {
                        match serde_json::from_value::<Session>(value["session"].clone()) {
                            Ok(session) if session.id == id => {
                                if let Some(item) =
                                    this.sessions.iter_mut().find(|item| item.id == id)
                                {
                                    *item = session.clone();
                                }
                                if this.session.as_ref().is_some_and(|item| item.id == id) {
                                    this.session = Some(session);
                                }
                            }
                            Ok(_) => this.error = Some("发送响应中的会话身份不匹配".into()),
                            Err(error) => this.error = Some(crate::app::ErrorNotice::Detail("发送响应格式错误",error.to_string())),
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn resume(&mut self, cx: &mut Context<Self>) {
        if !self.conversation_ready || !self.workspace_writes_allowed() {
            return;
        }
        let Some(id) = self.session.as_ref().map(|s| s.id.clone()) else {
            return;
        };
        if self.loading
            || self.busy.contains(&id)
            || self.pending.get(&id).copied().unwrap_or(0) == 0
        {
            return;
        }
        self.request("chat.resume", json!({"sessionId":id}), cx, |this, _, cx| {
            this.refresh_sessions(cx)
        });
    }
    fn apply_activity(&mut self, event: &PiEvent) {
        let id = event.session_id().to_string();
        match event {
            PiEvent::State {
                busy,
                pending,
                pending_reports,
                pending_user_inputs,
                completed_report_id,
                delivery_failure,
                ..
            } => {
                if self.session.as_ref().is_some_and(|s| s.id == id) {
                    self.pending_reports.clone_from(pending_reports);
                    self.pending_user_inputs.clone_from(pending_user_inputs);
                    let was_busy = self.busy.contains(&id);
                    if *busy
                        && matches!(self.status, crate::app::Status::Message("已停止当前回复"))
                    {
                        self.status = "后台已连接".into();
                    }
                    if (*busy && !was_busy)
                        || (!*busy
                            && was_busy
                            && delivery_failure.is_some()
                            && !self
                                .agent_diagnostic
                                .as_ref()
                                .is_some_and(|d| d.diagnostic_write_failed))
                    {
                        self.agent_diagnostic = None;
                    }
                    self.set_delivery_failure(delivery_failure.clone());
                    if let Some(id) = completed_report_id {
                        self.completed_deliveries.insert(id.clone());
                    }
                }
                if *busy {
                    self.busy.insert(id.clone());
                } else {
                    self.busy.remove(&id);
                }
                self.pending.insert(id, *pending);
            }
            PiEvent::Settled { .. } => {
                self.busy.remove(&id);
            }
            PiEvent::AssistantDelta { .. } | PiEvent::ToolStart { .. } => {
                self.busy.insert(id);
            }
            _ => {}
        }
        // Background deliveries can start a turn while the model menu is open.
        // Clear its controlled state before the busy trigger replaces the popover.
        if self.session.as_ref().is_some_and(|session| self.busy.contains(&session.id)) {
            self.choosing_model = false;
        }
    }
    pub fn abort(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &self.session {
            self.request(
                "chat.abort",
                json!({"sessionId":session.id}),
                cx,
                |_, _, _| {},
            );
        }
    }
    pub fn refresh_pi_models(&mut self, cx: &mut Context<Self>) {
        if self.refreshing_models || !self.connected {
            return;
        }
        let (Some(session), Some(runtime)) = (&self.session, self.runtime.clone()) else {
            return;
        };
        let id = session.id.clone();
        let open = self.open_request;
        self.refreshing_models = true;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.refreshPiModels", json!({"sessionId":id}))
                .await
                .and_then(|v| serde_json::from_value::<Vec<Model>>(v).map_err(|e| e.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                if this.open_request != open {
                    return;
                }
                this.refreshing_models = false;
                match result {
                    Ok(models) => {
                        let selected = this
                            .model
                            .as_ref()
                            .or(this.unavailable_model.as_ref())
                            .cloned();
                        if let Some(selected) = selected {
                            if let Some(current) = models.iter().find(|model| {
                                model.available
                                    && model.provider == selected.provider
                                    && model.id == selected.id
                            }) {
                                this.model = Some(ModelIdentity {
                                    provider: current.provider.clone(),
                                    id: current.id.clone(),
                                });
                                this.unavailable_model = None;
                            } else {
                                this.unavailable_model = Some(selected);
                                this.model = None;
                            }
                        }
                        this.choosing_model = models.iter().any(|m| m.available);
                        this.models = models;
                    }
                    Err(e) => {
                        if let Some(selected) = this.model.take() {
                            this.unavailable_model = Some(selected);
                        }
                        this.error = Some(crate::app::ErrorNotice::Detail("刷新 Pi 模型失败",e.to_string()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn set_model(&mut self, model: Model, cx: &mut Context<Self>) {
        if !self.workspace_writes_allowed() {
            return;
        }
        if let Some(session) = &self.session {
            let id = session.id.clone();
            self.request(
                "chat.setModel",
                json!({"sessionId":id,"provider":model.provider,"modelId":model.id}),
                cx,
                move |this, _, _| {
                    if this.session.as_ref().map(|s| &s.id) == Some(&id) {
                        this.choosing_model = false;
                        this.reasoning_picker = Default::default();
                        this.reset_usage();
                        this.unavailable_model = None;
                        this.model = Some(ModelIdentity {
                            provider: model.provider,
                            id: model.id,
                        });
                    }
                },
            );
        }
    }
    pub(crate) fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        self.conversation_find = None;
        // Update the header action now, then pin to the final offset after layout.
        self.conversation_list
            .state
            .set_follow_mode(FollowMode::Tail);
        self.conversation_list.state.scroll_to_end();
        cx.notify();
    }

    pub(crate) fn reading_above_latest(&self) -> bool {
        !self.conversation_list.state.is_following_tail()
    }

    pub fn push_message(&mut self, id: String, role: String, text: String, cx: &mut Context<Self>) {
        if !matches!(
            role.as_str(),
            "user" | "assistant" | "tool" | "toolResult" | "notice"
        ) {
            return;
        }
        let markdown =
            (role == "assistant").then(|| cx.new(|cx| TextViewState::markdown(&text, cx)));
        let markdown_subscription = markdown.as_ref().map(|state| {
            let id = id.clone();
            let mut rendered = state.read(cx).rendered_text();
            cx.observe(state, move |this, state, cx| {
                let next = state.read(cx).rendered_text();
                // Parse completion can change an offscreen row's height. Ignore
                // selection/fade notifications that keep the same document.
                if std::ptr::eq(next.source(), rendered.source()) {
                    return;
                }
                rendered = next;
                if let Some(message) = this.messages.iter_mut().find(|m| m.id == id) {
                    message.layout_revision += 1;
                    cx.notify();
                }
            })
        });
        self.messages.push(Message {
            layout_revision: 0,
            _markdown_subscription: markdown_subscription,
            id,
            role,
            text,
            markdown,
            tool: None,
            thinking: vec![],
            delivery: None,
            explicit_user: false,
            failure: None,
            images: vec![],
            files: vec![],
            image_sources: vec![],
            images_loading: false,
            images_error: None,
        });
    }
    // Both snapshots and live events use the same provenance contract. Never infer
    // a source from message text: users may paste a complete Hive envelope.
    pub(crate) fn receive_message(
        &mut self,
        mut id: String,
        message: &Value,
        cx: &mut Context<Self>,
    ) {
        let role = message["role"].as_str().unwrap_or("assistant");
        if role == "assistant" {
            for block in message["content"].as_array().into_iter().flatten() {
                if block["type"] == "toolCall" {
                    if let (Some(id), Some(args)) = (block["id"].as_str(), block.get("arguments")) {
                        self.tool_inputs.insert(id.to_owned(), args.clone());
                    }
                }
            }
        }
        if role == "assistant" {
            self.sync_thinking(&id, message, false, cx);
        }
        let text = message_text(message);
        if (role == "custom" && message["customType"] == "hive-delivery")
            || (role == "user" && delivery_presentation(message).is_some())
        {
            let presentation = delivery_presentation(message);
            id = presentation
                .as_ref()
                .map(|p| p.delivery_id.clone())
                .unwrap_or(id);
            let explicit_user = presentation
                .as_ref()
                .is_some_and(|p| p.origin == DeliveryOrigin::User);
            let display = presentation
                .as_ref()
                .map(|p| {
                    // A system delivery without a separate display summary is
                    // internal context. Keep it in the existing raw-message
                    // disclosure rather than repeating the envelope in chat.
                    if p.origin == DeliveryOrigin::System && p.display_text == text {
                        String::new()
                    } else {
                        p.display_text.clone()
                    }
                })
                .unwrap_or_else(|| "此消息缺少有效的来源记录。展开查看原文。".into());
            let heading = presentation.as_ref().map(|p| p.origin.heading()).unwrap_or("来源未记录");
            let worker_name = presentation.as_ref().filter(|p|p.origin==DeliveryOrigin::Report).and_then(|p|p.worker_name.clone());
            let expanded = self
                .messages
                .iter()
                .find(|m| m.id == id)
                .and_then(|m| m.delivery.as_ref())
                .is_some_and(|d| d.expanded);
            // Notice is a rendered role, never an engine role.
            self.upsert(
                id.clone(),
                if explicit_user { "user" } else { "notice" },
                display,
                false,
                cx,
            );
            self.load_message_images(id.clone(), message, cx);
            if let Some(rendered) = self.messages.iter_mut().find(|m| m.id == id) {
                rendered.explicit_user = explicit_user;
                let files = presentation
                    .as_ref()
                    .map(|p| {
                        p.attachments
                            .iter()
                            .filter(|item| !item.is_image())
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if rendered.files != files {
                    rendered.files = files;
                    rendered.layout_revision += 1;
                }

                rendered.delivery = if explicit_user {
                    None
                } else {
                    Some(DeliveryCard {
                        dispatch_id: presentation.as_ref().and_then(|p| p.dispatch_id.clone()),
                        report: presentation
                            .as_ref()
                            .is_some_and(|p| p.origin == DeliveryOrigin::Report),
                        heading,
                        worker_name,
                        worker_id: presentation.as_ref().and_then(|p| p.worker_id.clone()),
                        artifacts: presentation.map(|p| p.artifacts).unwrap_or_default(),
                        raw: text,
                        expanded,
                    })
                };
            }
        } else if role == "toolResult" {
            if let (Some(call), Some(args)) =
                (message["toolCallId"].as_str(), message.get("toolInput"))
            {
                self.tool_inputs.insert(call.to_owned(), args.clone());
            }
            self.upsert_tool(
                message["toolCallId"].as_str().unwrap_or(&id).into(),
                message["toolName"].as_str().unwrap_or("工具").into(),
                crate::tool_message::result_status(message, message["isError"] == true),
                text,
                cx,
            );
            self.retain_tool_patch(message["toolCallId"].as_str().unwrap_or(&id), message, cx);
        } else {
            self.upsert(id.clone(), role, text, false, cx);
            if role == "assistant" {
                let failure = (message["stopReason"] == "error").then(|| {
                    message["errorMessage"]
                        .as_str()
                        .unwrap_or("Agent 回复未完成。")
                        .to_owned()
                });
                if failure.is_some() && !self.messages.iter().any(|m| m.id == id) {
                    self.push_message(id.clone(), role.into(), String::new(), cx);
                }
                if let Some(rendered) = self.messages.iter_mut().find(|m| m.id == id) {
                    if rendered.failure.as_ref().map(|f| &f.message) != failure.as_ref() {
                        rendered.failure = failure.map(|message| crate::failures::Diagnostic {
                            message,
                            expanded: false,
                            diagnostic_write_failed: false,
                        });
                        rendered.layout_revision += 1;
                    }
                }
            }
            self.load_message_images(id.clone(), message, cx);
        }
        self.refresh_conversation_find(&id, cx);
    }
    pub(crate) fn upsert_tool(
        &mut self,
        id: String,
        name: String,
        status: &'static str,
        output: String,
        cx: &mut Context<Self>,
    ) {
        self.upsert(id.clone(), "toolResult", output, false, cx);
        if let Some(message) = self.messages.iter_mut().find(|m| m.id == id) {
            if let Some(tool) = &mut message.tool {
                tool.name = name;
                tool.status = status;
            } else {
                message.tool = Some(ToolPresentation {
                    patch: None,
                    name,
                    status,
                    expanded: false,
                    details_scroll: ScrollHandle::new(),
                });
            }
        }
        self.refresh_conversation_find(&id, cx);
    }
    pub(crate) fn upsert(
        &mut self,
        id: String,
        role: &str,
        text: String,
        append: bool,
        cx: &mut Context<Self>,
    ) {
        if !append
            && role == "assistant"
            && text.trim().is_empty()
            && !self
                .messages
                .iter()
                .any(|message| message.id == id && !message.thinking.is_empty())
        {
            self.messages.retain(|message| message.id != id);
            self.refresh_conversation_find(&id, cx);
            return;
        }
        if let Some(message) = self.messages.iter_mut().find(|m| m.id == id) {
            message.layout_revision += 1;
            if append {
                message.text.push_str(&text);
            } else {
                message.text = text;
            }
            if let Some(markdown) = &message.markdown {
                markdown.update(cx, |state, cx| state.set_text(&message.text, cx));
            }
        } else {
            self.push_message(id.clone(), role.into(), text, cx);
        }
        self.refresh_conversation_find(&id, cx);
    }
    pub(crate) fn agent_event(&mut self, event: PiEvent, cx: &mut Context<Self>) {
        if self.removed_sessions.contains(event.session_id()) {
            return;
        }
        // Keep current-session navigation in the same snapshot replay as chat
        // events, so an in-flight open cannot replace newer metadata.
        if self.loading && self.session.as_ref().map(|s| s.id.as_str()) == Some(event.session_id())
        {
            self.pending_events.push(event);
            return;
        }
        if let PiEvent::SessionNavigation {
            session_id,
            sequence,
            archived,
            pinned,
            updated_at,
        } = &event
        {
            let seen = self
                .background_sequences
                .get(session_id)
                .copied()
                .unwrap_or(0)
                .max(
                    if self
                        .session
                        .as_ref()
                        .is_some_and(|session| session.id == *session_id)
                    {
                        self.event_sequence
                    } else {
                        0
                    },
                );
            if *sequence > seen {
                self.background_sequences
                    .insert(session_id.clone(), *sequence);
                self.apply_archive_navigation(session_id, *archived, *pinned, *updated_at, cx);
            }
            return;
        }
        if self.session.as_ref().map(|s| s.id.as_str()) != Some(event.session_id()) {
            let previous = self
                .background_sequences
                .entry(event.session_id().into())
                .or_default();
            if event.sequence() <= *previous {
                return;
            }
            *previous = event.sequence();
            self.apply_activity(&event);
            self.apply_interaction_event(&event);
            if !matches!(&event, PiEvent::State { .. } | PiEvent::Settled { .. }) {
                self.unread.insert(event.session_id().into());
            }
            self.refresh_sessions(cx);
            return;
        }
        if event.sequence() <= self.event_sequence {
            return;
        }
        self.event_sequence = event.sequence();
        self.background_sequences
            .entry(event.session_id().into())
            .and_modify(|seen| *seen = (*seen).max(event.sequence()))
            .or_insert(event.sequence());
        self.apply_activity(&event);
        self.apply_interaction_event(&event);
        if matches!(&event, PiEvent::State { .. } | PiEvent::Settled { .. }) {
            self.refresh_sessions(cx);
        }
        // GPUI stores a positive max_offset and a negative current offset.
        let follow = self.conversation_find.is_none() && !self.reading_above_latest();
        match event {
            PiEvent::SessionNavigation { .. }
            | PiEvent::State { .. }
            | PiEvent::Interaction { .. }
            | PiEvent::InteractionResolved { .. } => {}
            PiEvent::AssistantDelta {
                message_id,
                kind,
                content_index,
                delta,
                ..
            } => {
                if kind == "thinking" {
                    self.append_thinking(message_id, content_index, delta, cx);
                } else if kind == "text" {
                    self.finish_thinking_before(&message_id, content_index);
                    self.upsert(message_id, "assistant", delta, true, cx);
                }
            }
            PiEvent::MessageEnd {
                message_id,
                message,
                ..
            } => {
                self.receive_message(message_id, &message, cx);
            }
            PiEvent::ToolStart {
                tool_call_id,
                tool_name,
                args,
                ..
            } => {
                self.tool_inputs.insert(tool_call_id.clone(), args);
                self.upsert_tool(tool_call_id, tool_name, "执行中", String::new(), cx);
            }
            PiEvent::ToolUpdate {
                tool_call_id,
                tool_name,
                result,
                ..
            } => {
                self.upsert_tool(tool_call_id, tool_name, "执行中", message_text(&result), cx);
            }
            PiEvent::ToolEnd {
                tool_call_id,
                tool_name,
                result,
                is_error,
                ..
            } => {
                let call_id = tool_call_id.clone();
                self.upsert_tool(
                    tool_call_id,
                    tool_name,
                    crate::tool_message::result_status(&result, is_error),
                    message_text(&result),
                    cx,
                );
                self.retain_tool_patch(&call_id, &result, cx);
            }
            PiEvent::Settled { aborted, .. } => {
                self.load_usage(cx);
                self.finish_thinking(aborted);
                if aborted {
                    self.status = "已停止当前回复".into();
                }
            }
            PiEvent::Error {
                message,
                diagnostic_write_failed,
                ..
            } => {
                self.failure_scroll.set_offset(point(px(0.),px(0.)));
                self.agent_diagnostic = Some(crate::failures::Diagnostic {
                    message,
                    expanded: false,
                    diagnostic_write_failed,
                })
            }
        }
        if follow {
            self.conversation_list
                .state
                .set_follow_mode(FollowMode::Tail);
        }
    }
}
pub(crate) fn message_text(message: &Value) -> String {
    let text = if let Some(text) = message["content"].as_str() {
        text.to_owned()
    } else {
        message["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| match block["type"].as_str() {
                        Some("text") => block["text"].as_str().map(String::from),
                        Some("toolCall") if message["role"] != "assistant" => Some(format!(
                            "{}\n{}",
                            block["name"].as_str().unwrap_or("工具"),
                            block["arguments"]
                        )),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            })
            .unwrap_or_else(|| message.to_string())
    };
    text
}

#[derive(serde::Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum DeliveryOrigin {
    User,
    Dispatch,
    Report,
    Cancel,
    Recovery,
    System,
    Unknown,
}
impl DeliveryOrigin {
    fn heading(&self) -> &'static str {
        match self {
            Self::User => "你",
            Self::Dispatch => "派单",
            Self::Report => "成员汇报",
            Self::Cancel => "取消",
            Self::Recovery => "会话恢复",
            Self::System => "团队通知",
            Self::Unknown => "来源未记录",
        }
    }
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryPresentation {
    dispatch_id: Option<String>,
    version: u32,
    delivery_id: String,
    origin: DeliveryOrigin,
    display_text: String,
    worker_name: Option<String>,
    worker_id: Option<String>,
    #[serde(default)]
    artifacts: Vec<String>,
    #[serde(default)]
    attachments: Vec<crate::attachments::AttachmentRef>,
}
fn delivery_presentation(message: &Value) -> Option<DeliveryPresentation> {
    let presentation: DeliveryPresentation =
        serde_json::from_value(message["presentation"].clone()).ok()?;
    (presentation.version == 1 && !presentation.delivery_id.is_empty()).then_some(presentation)
}
