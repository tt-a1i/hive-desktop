use crate::{
    app::{HiveApp, Message},
    protocol::*,
};
use gpui_kit::{component::text::TextViewState, *};
use serde_json::{Value, json};
impl HiveApp {
    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.session.as_ref().map(|s| s.id.clone()) else {
            return;
        };
        let text = self.input.read(cx).value().to_string();
        if text.trim().is_empty() || self.busy.contains(&id) {
            return;
        }
        if self.closing {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.busy.insert(id.clone());
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.push_message(
            format!("local-{}", self.messages.len()),
            "user".into(),
            text.clone(),
            cx,
        );
        self.scroll.scroll_to_bottom();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.prompt", json!({"sessionId":id,"text":text}))
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Err(error) => {
                        this.busy.remove(&id);
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
    pub fn set_model(&mut self, model: Model, cx: &mut Context<Self>) {
        if let Some(session) = &self.session {
            let id = session.id.clone();
            self.request(
                "chat.setModel",
                json!({"sessionId":id,"provider":model.provider,"modelId":model.id}),
                cx,
                move |this, _, _| {
                    if this.session.as_ref().map(|s| &s.id) == Some(&id) {
                        this.choosing_model = false;
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
        if !matches!(role.as_str(), "user" | "assistant" | "tool" | "toolResult") {
            return;
        }
        let markdown = cx.new(|cx| TextViewState::markdown(&text, cx));
        self.messages.push(Message {
            id,
            role,
            text,
            markdown,
        });
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
            message
                .markdown
                .update(cx, |state, cx| state.set_text(&message.text, cx));
        } else {
            self.push_message(id, role.into(), text, cx);
        }
    }
    pub(crate) fn agent_event(&mut self, event: PiEvent, cx: &mut Context<Self>) {
        match &event {
            PiEvent::Settled { session_id, .. } => {
                self.busy.remove(session_id);
            }
            _ => {}
        }
        if self.session.as_ref().map(|s| s.id.as_str()) != Some(event.session_id()) {
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
        // GPUI stores a positive max_offset and a negative current offset.
        let follow = self.scroll.max_offset().y + self.scroll.offset().y <= px(4.);
        match event {
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
            } => self.upsert(message_id, "assistant", message_text(&message), false, cx),
            PiEvent::ToolStart {
                tool_call_id,
                tool_name,
                args,
                ..
            } => self.upsert(
                tool_call_id,
                "tool",
                format!("{tool_name} · 执行中\n{args}"),
                false,
                cx,
            ),
            PiEvent::ToolUpdate {
                tool_call_id,
                tool_name,
                result,
                ..
            } => self.upsert(
                tool_call_id,
                "tool",
                format!("{tool_name}\n{result}"),
                false,
                cx,
            ),
            PiEvent::ToolEnd {
                tool_call_id,
                tool_name,
                result,
                is_error,
                ..
            } => self.upsert(
                tool_call_id,
                "tool",
                format!(
                    "{tool_name} · {}\n{result}",
                    if is_error { "失败" } else { "完成" }
                ),
                false,
                cx,
            ),
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
                    Some("toolCall") => Some(format!(
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
