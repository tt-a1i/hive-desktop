use crate::{
    app::{DeliveryCard, HiveApp, Message, ToolPresentation},
    protocol::*,
};
use gpui_kit::{component::text::TextViewState, *};
use serde_json::{Value, json};
impl HiveApp {
    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.workspace_writes_allowed() {
            return;
        }
        let Some(id) = self.session.as_ref().map(|s| s.id.clone()) else {
            return;
        };
        let text = self.input.read(cx).value().to_string();
        if text.trim().is_empty()
            || self.busy.contains(&id)
            || self.pending.get(&id).copied().unwrap_or(0) > 0
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
        self.busy.insert(id.clone());
        self.drafts.remove(&format!("session:{id}"));
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.schedule_draft_save(cx);
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.prompt", json!({"sessionId":id,"text":text}))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.removed_sessions.contains(&id) {
                    return;
                }
                match result {
                    Err(error) => {
                        this.busy.remove(&id);
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
                        this.error = Some(error);
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
                            Err(error) => this.error = Some(format!("发送响应格式错误：{error}")),
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
        if !self.workspace_writes_allowed() {
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
                completed_report_id,
                ..
            } => {
                if self.session.as_ref().is_some_and(|s| s.id == id) {
                    self.pending_reports.clone_from(pending_reports);
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
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.refreshPiModels", json!({"sessionId":id}))
                .await
                .and_then(|v| serde_json::from_value::<Vec<Model>>(v).map_err(|e| e.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.open_request != open {
                    return;
                }
                this.refreshing_models = false;
                match result {
                    Ok(models) => {
                        this.choosing_model = models.iter().any(|m| m.available);
                        this.models = models;
                    }
                    Err(e) => this.error = Some(format!("刷新 Pi 模型失败：{e}")),
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
    pub fn push_message(&mut self, id: String, role: String, text: String, cx: &mut Context<Self>) {
        if !matches!(
            role.as_str(),
            "user" | "assistant" | "tool" | "toolResult" | "notice"
        ) {
            return;
        }
        let markdown =
            (role == "assistant").then(|| cx.new(|cx| TextViewState::markdown(&text, cx)));
        self.messages.push(Message {
            id,
            role,
            text,
            markdown,
            tool: None,
            delivery: None,
            explicit_user: false,
        });
    }
    // Both snapshots and live events use the same provenance contract. Never infer
    // a source from message text: users may paste a complete Hive envelope.
    pub(crate) fn receive_message(&mut self, id: String, message: &Value, cx: &mut Context<Self>) {
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
        let text = message_text(message);
        if role == "custom" && message["customType"] == "hive-delivery" {
            let presentation = delivery_presentation(message);
            let id = presentation
                .as_ref()
                .map(|p| p.delivery_id.clone())
                .unwrap_or(id);
            let explicit_user = presentation
                .as_ref()
                .is_some_and(|p| p.origin == DeliveryOrigin::User);
            let display = presentation
                .as_ref()
                .map(|p| p.display_text.clone())
                .unwrap_or_else(|| "此消息缺少有效的来源记录。展开查看原文。".into());
            let heading = presentation
                .as_ref()
                .map(|p| match (&p.worker_name, &p.origin) {
                    (Some(worker), DeliveryOrigin::Report) => format!("{worker} · 成员汇报"),
                    _ => p.origin.heading().to_string(),
                })
                .unwrap_or_else(|| "来源未记录".into());
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
            if let Some(rendered) = self.messages.iter_mut().find(|m| m.id == id) {
                rendered.explicit_user = explicit_user;
                rendered.delivery = if explicit_user {
                    None
                } else {
                    Some(DeliveryCard {
                        report: presentation
                            .as_ref()
                            .is_some_and(|p| p.origin == DeliveryOrigin::Report),
                        heading,
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
                match message["toolStatus"].as_str() {
                    Some("declined") => "已拒绝",
                    Some("inProgress") => "执行中",
                    _ if message["isError"] == true => "失败",
                    _ => "完成",
                },
                text,
                cx,
            );
        } else {
            self.upsert(id, role, text, false, cx);
        }
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
            let expanded = message.tool.as_ref().is_some_and(|tool| tool.expanded);
            message.tool = Some(ToolPresentation {
                name,
                status,
                expanded,
            });
        }
    }
    pub(crate) fn upsert(
        &mut self,
        id: String,
        role: &str,
        text: String,
        append: bool,
        cx: &mut Context<Self>,
    ) {
        if !append && role == "assistant" && text.trim().is_empty() {
            self.messages.retain(|message| message.id != id);
            return;
        }
        if let Some(message) = self.messages.iter_mut().find(|m| m.id == id) {
            if append {
                message.text.push_str(&text);
            } else {
                message.text = text;
            }
            if let Some(markdown) = &message.markdown {
                markdown.update(cx, |state, cx| state.set_text(&message.text, cx));
            }
        } else {
            self.push_message(id, role.into(), text, cx);
        }
    }
    pub(crate) fn agent_event(&mut self, event: PiEvent, cx: &mut Context<Self>) {
        if self.removed_sessions.contains(event.session_id()) {
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
        if self.loading {
            self.pending_events.push(event);
            return;
        }
        if event.sequence() <= self.event_sequence {
            return;
        }
        self.event_sequence = event.sequence();
        self.background_sequences
            .insert(event.session_id().into(), event.sequence());
        self.apply_activity(&event);
        self.apply_interaction_event(&event);
        if matches!(&event, PiEvent::State { .. } | PiEvent::Settled { .. }) {
            self.refresh_sessions(cx);
        }
        // GPUI stores a positive max_offset and a negative current offset.
        let follow = self.scroll.max_offset().y + self.scroll.offset().y <= px(4.);
        match event {
            PiEvent::State { .. }
            | PiEvent::Interaction { .. }
            | PiEvent::InteractionResolved { .. } => {}
            PiEvent::AssistantDelta {
                message_id,
                kind,
                delta,
                ..
            } => {
                if kind == "text" {
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
                self.upsert_tool(
                    tool_call_id,
                    tool_name,
                    if is_error { "失败" } else { "完成" },
                    message_text(&result),
                    cx,
                );
            }
            PiEvent::Settled { aborted, .. } => {
                if aborted {
                    self.status = "已停止当前回复".into();
                }
            }
            PiEvent::Error { message, .. } => self.error = Some(message),
        }
        if follow {
            self.scroll.scroll_to_bottom();
        }
    }
}
pub(crate) fn message_text(message: &Value) -> String {
    if let Some(text) = message["content"].as_str() {
        return text.into();
    }
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
    version: u32,
    delivery_id: String,
    origin: DeliveryOrigin,
    display_text: String,
    worker_name: Option<String>,
    worker_id: Option<String>,
    #[serde(default)]
    artifacts: Vec<String>,
}
fn delivery_presentation(message: &Value) -> Option<DeliveryPresentation> {
    let presentation: DeliveryPresentation =
        serde_json::from_value(message["presentation"].clone()).ok()?;
    (presentation.version == 1 && !presentation.delivery_id.is_empty()).then_some(presentation)
}
