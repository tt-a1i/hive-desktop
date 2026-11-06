use crate::conversation::message_text;
use crate::{app::HiveApp, protocol::*};
use gpui_kit::*;
use serde_json::json;
impl HiveApp {
    fn switch_draft(&mut self, next: Option<String>, cx: &mut Context<Self>) {
        if let Some(key) = self.draft_owner.clone() {
            let text = self.input.read(cx).value().to_string();
            if text.is_empty() {
                self.drafts.remove(&key);
            } else {
                self.drafts.insert(key, text);
            }
        }
        self.draft_target = next;
    }
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
        self.close_roles();
        self.switch_draft(Some(format!("workspace:{id}")), cx);
        self.loading = true;
        self.workspace = Some(id.clone());
        self.session = None;
        self.messages.clear();
        self.pending_events.clear();
        self.event_sequence = 0;
        self.sessions.clear();
        self.members.clear();
        self.member_catalog_generation += 1;
        self.member_catalog_loading = false;
        self.member_catalog.clear();
        self.member_picker_target = None;
        self.member_catalog_backend = None;
        self.member_selected_model = None;
        self.member_generation += 1;
        self.member_stopping = None;
        self.member_loading = false;
        self.editing_member = None;
        self.deleting_member = None;
        self.models.clear();
        self.model = None;
        self.open_request += 1;
        self.refresh_sessions(cx);
        self.refresh_members(cx);
        if self.member_panel {
            self.load_roles(cx);
        }
        if self.collaboration.visible {
            self.reload_collaboration(cx);
        }
        if self.memory.visible {
            self.memory_workspace_changed(cx);
        }
        cx.notify();
    }
    // Coalesce event bursts and serialize requests so older lists cannot replace newer ones.
    pub fn refresh_sessions(&mut self, cx: &mut Context<Self>) {
        self.list_dirty = true;
        if self.list_refreshing {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.list_refreshing = true;
        cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(180)).await;
                let Ok(workspace) = this.update(cx, |this, _| {
                    this.list_dirty = false;
                    this.workspace.clone()
                }) else {
                    return;
                };
                let Some(workspace) = workspace else {
                    let _ = this.update(cx, |this, _| this.list_refreshing = false);
                    return;
                };
                let result = runtime
                    .request("chat.list", json!({"workspaceId": workspace}))
                    .await;
                let Ok(again) = this.update(cx, |this, cx| {
                    if this.workspace.as_ref() == Some(&workspace) {
                        match result {
                            Ok(value) => match serde_json::from_value::<Vec<Session>>(value) {
                                Ok(sessions) => {
                                    this.sessions = sessions;
                                    if let Some(selected) = &mut this.session {
                                        if let Some(record) =
                                            this.sessions.iter().find(|s| s.id == selected.id)
                                        {
                                            *selected = record.clone();
                                        }
                                    } else {
                                        this.loading = false;
                                        if !this.memory.visible
                                            && !this.member_panel
                                            && !this.collaboration.visible
                                            && !this.settings_panel
                                            && !this.roles.visible
                                        {
                                            if let Some(first) = this.sessions.first() {
                                                this.open_session(first.id.clone(), cx);
                                            }
                                        }
                                    }
                                }
                                Err(error) => {
                                    if this.session.is_none() {
                                        this.loading = false;
                                    }
                                    this.error = Some(format!("会话列表格式错误：{error}"));
                                }
                            },
                            Err(error) => {
                                if this.session.is_none() {
                                    this.loading = false;
                                }
                                this.error = Some(error);
                            }
                        }
                    }
                    let again = this.list_dirty;
                    if !again {
                        this.list_refreshing = false;
                    }
                    cx.notify();
                    again
                }) else {
                    return;
                };
                if !again {
                    return;
                }
            }
        })
        .detach();
    }
    pub fn new_session(&mut self, backend: &'static str, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            self.error = Some("后台尚未连接".into());
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.create",
                    json!({"workspaceId":workspace,"backend":backend}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(value) => match serde_json::from_value::<Session>(value) {
                        Ok(session) => {
                            if this.workspace.as_ref() == Some(&session.workspace_id) {
                                this.sessions.insert(0, session.clone());
                                this.close_collaboration();
                                this.open_session(session.id, cx);
                            }
                        }
                        Err(error) => this.error = Some(format!("创建会话响应错误：{error}")),
                    },
                    Err(error) => this.error = Some(format!("创建 {backend} 会话失败：{error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn open_session(&mut self, id: String, cx: &mut Context<Self>) {
        self.close_memory();
        self.close_roles();
        self.switch_draft(Some(format!("session:{id}")), cx);
        self.close_settings();
        self.member_panel = false;
        self.open_request += 1;
        self.unread.remove(&id);
        self.choosing_model = false;
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
                        this.pending.insert(id.clone(), open.pending);
                        this.background_sequences
                            .insert(id.clone(), open.event_sequence);
                        this.session = Some(open.session);
                        this.models = open.models;
                        this.model = open.model;
                        this.event_sequence = open.event_sequence;
                        for (i, message) in open.history.into_iter().enumerate() {
                            this.receive_message(open.history_message_ids[i].clone(), &message, cx);
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
