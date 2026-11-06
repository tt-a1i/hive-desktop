use crate::conversation::message_text;
use crate::{app::HiveApp, protocol::*};
use gpui_kit::*;
use serde_json::json;
impl HiveApp {
    pub fn load_workspaces(&mut self, cx: &mut Context<Self>) {
        self.request(
            "hive.request",
            json!({"method":"GET","path":"/api/workspaces"}),
            cx,
            |this, value, cx| match serde_json::from_value::<Vec<Workspace>>(value) {
                Ok(workspaces) => {
                    this.workspaces = workspaces;
                    if this.workspace.is_none() {
                        if let Some(first) = this.workspaces.first() {
                            this.select_workspace(first.id.clone(), cx);
                        }
                    }
                }
                Err(error) => this.error = Some(format!("工作区响应格式错误：{error}")),
            },
        );
    }
    pub fn add_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self.folder.read(cx).value().trim().to_string();
        if path.is_empty() {
            return;
        }
        let name = std::path::Path::new(&path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("项目")
            .to_string();
        let window = window.window_handle();
        self.request("hive.request", json!({"method":"POST","path":"/api/workspaces","body":{"path":path,"name":name,"autostart_orchestrator":false}}), cx, move |this, value, cx| {
            match serde_json::from_value::<Workspace>(value) {
                Ok(workspace) => {
                    let folder = this.folder.clone();
                    if folder.read(cx).value().trim() == path {
                        let _ = window.update(cx, |_, window, cx| folder.update(cx, |state, cx| state.set_value("", window, cx)));
                    }
                    let id = workspace.id.clone(); this.workspaces.push(workspace); this.select_workspace(id, cx); },
                Err(error) => this.error = Some(format!("添加工作区响应错误：{error}")),
            }
        });
    }
    pub fn select_workspace(&mut self, id: String, cx: &mut Context<Self>) {
        self.loading = true;
        self.workspace = Some(id.clone());
        self.session = None;
        self.messages.clear();
        self.pending_events.clear();
        self.event_sequence = 0;
        self.sessions.clear();
        self.models.clear();
        self.model = None;
        self.request(
            "chat.list",
            json!({"workspaceId":id}),
            cx,
            move |this, value, cx| {
                if this.workspace.as_ref() != Some(&id) {
                    return;
                }
                this.loading = false;
                match serde_json::from_value::<Vec<Session>>(value) {
                    Ok(sessions) => {
                        this.sessions = sessions;
                        if let Some(first) = this.sessions.first() {
                            this.open_session(first.id.clone(), cx);
                        }
                    }
                    Err(error) => this.error = Some(format!("会话列表格式错误：{error}")),
                }
            },
        );
        cx.notify();
    }
    pub fn new_session(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        self.request(
            "chat.create",
            json!({"workspaceId":workspace}),
            cx,
            move |this, value, cx| match serde_json::from_value::<Session>(value) {
                Ok(session) => {
                    if this.workspace.as_ref() == Some(&session.workspace_id) {
                        this.sessions.insert(0, session.clone());
                        this.open_session(session.id, cx);
                    }
                }
                Err(error) => this.error = Some(format!("创建会话响应错误：{error}")),
            },
        );
    }
    pub fn open_session(&mut self, id: String, cx: &mut Context<Self>) {
        self.open_request += 1;
        let request = self.open_request;
        if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
            self.session = Some(session.clone());
        }
        self.messages.clear();
        self.pending_events.clear();
        self.event_sequence = 0;
        self.models.clear();
        self.model = None;
        self.loading = true;
        self.request(
            "chat.open",
            json!({"sessionId":id}),
            cx,
            move |this, value, cx| {
                if this.open_request != request || this.session.as_ref().map(|s| &s.id) != Some(&id)
                {
                    return;
                }
                this.loading = false;
                match serde_json::from_value::<OpenSession>(value) {
                    Ok(open) => {
                        if open.history_message_ids.len() != open.history.len() {
                            this.error = Some("会话快照中的消息身份数量不匹配".into());
                            return;
                        }
                        if open.busy {
                            this.busy.insert(id.clone());
                        } else {
                            this.busy.remove(&id);
                        }
                        this.session = Some(open.session);
                        this.models = open.models;
                        this.model = open.model;
                        this.event_sequence = open.event_sequence;
                        for (i, message) in open.history.into_iter().enumerate() {
                            let text = message_text(&message);
                            if text.trim().is_empty() {
                                continue;
                            }
                            let message_id = if message["role"] == "toolResult" {
                                message["toolCallId"].as_str().map(String::from)
                            } else {
                                None
                            }
                            .unwrap_or_else(|| open.history_message_ids[i].clone());
                            this.push_message(
                                message_id,
                                message["role"].as_str().unwrap_or("message").into(),
                                text,
                                cx,
                            );
                        }
                        if let Some(active) = open.active_message {
                            this.upsert(
                                active.message_id,
                                "assistant",
                                message_text(&active.message),
                                false,
                                cx,
                            );
                        }
                        for event in std::mem::take(&mut this.pending_events) {
                            this.agent_event(event, cx);
                        }
                        this.scroll.scroll_to_bottom();
                    }
                    Err(error) => this.error = Some(format!("打开会话响应错误：{error}")),
                }
            },
        );
        cx.notify();
    }
}
