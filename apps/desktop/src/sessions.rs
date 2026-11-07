use crate::conversation::message_text;
use crate::{app::HiveApp, protocol::*};
use gpui_kit::*;
use serde_json::json;
impl HiveApp {
    fn remember_conversation_position(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        if self.loading || self.messages.is_empty() {
            return;
        }
        let expanded = self
            .messages
            .iter()
            .filter(|message| {
                message.tool.as_ref().is_some_and(|tool| tool.expanded)
                    || message
                        .delivery
                        .as_ref()
                        .is_some_and(|delivery| delivery.expanded)
            })
            .map(|message| message.id.clone())
            .collect();
        let offset = self.reading_above_latest().then(|| self.scroll.offset());
        self.positions.insert(
            session.id.clone(),
            crate::app::ConversationPosition {
                offset,
                expanded,
                expanded_tool_groups: self.expanded_tool_groups.clone(),
            },
        );
    }

    fn restore_conversation_position(&mut self, id: &str) {
        let position = self.positions.get(id);
        if let Some(position) = position {
            self.expanded_tool_groups
                .clone_from(&position.expanded_tool_groups);
            for message in self
                .messages
                .iter_mut()
                .filter(|m| position.expanded.contains(&m.id))
            {
                if let Some(tool) = &mut message.tool {
                    tool.expanded = true;
                }
                if let Some(delivery) = &mut message.delivery {
                    delivery.expanded = true;
                }
            }
        }
        self.scroll = ScrollHandle::new();
        if let Some(offset) = position.and_then(|p| p.offset) {
            self.scroll.set_offset(offset);
        } else {
            self.scroll.scroll_to_bottom();
        }
    }

    fn switch_draft(&mut self, next: Option<String>, cx: &mut Context<Self>) {
        self.capture_editor(cx);
        self.schedule_draft_save(cx);
        self.draft_target = next;
    }
    pub(crate) fn save_navigation(&self, cx: &mut Context<Self>) {
        let Some(runtime) = &self.runtime else { return };
        let write = runtime.begin_request("desktop.navigation.save", json!(self.navigation));
        cx.spawn(async move |this, cx| {
            if let Err(error) = write.await {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("保存会话位置失败：{error}"));
                    cx.notify();
                });
            }
        })
        .detach();
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
                        let saved = this
                            .workspaces
                            .iter()
                            .find(|w| Some(&w.id) == this.navigation.workspace.as_ref());
                        if let Some(first) = saved.or_else(|| this.workspaces.first()) {
                            this.select_workspace(first.id.clone(), cx);
                        }
                    }
                }
                Err(error) => this.error = Some(format!("工作区响应格式错误：{error}")),
            },
        );
    }
    pub fn add_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected {
            return;
        }
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
                        this.add_workspace_open = false;
                        let _ = window.update(cx, |_, window, cx| folder.update(cx, |state, cx| state.set_value("", window, cx)));
                    }
                    let id = workspace.id.clone(); this.workspaces.push(workspace); this.select_workspace(id, cx); },
                Err(error) => this.error = Some(format!("添加工作区响应错误：{error}")),
            }
        });
    }
    pub fn select_workspace(&mut self, id: String, cx: &mut Context<Self>) {
        if self.workspace_removing.as_ref() == Some(&id) {
            return;
        }
        self.remember_conversation_position();
        self.close_roles();
        self.switch_draft(Some(format!("workspace:{id}")), cx);
        if let Some(previous) = &self.workspace {
            self.project_sessions
                .insert(previous.clone(), std::mem::take(&mut self.sessions));
        }
        self.expanded_projects.insert(id.clone());
        self.workspace = Some(id.clone());
        if self.workspace != self.navigation.workspace {
            self.navigation.workspace = self.workspace.clone();
            self.save_navigation(cx);
        }
        self.reset_workspace_view();
        self.sessions = self.project_sessions.remove(&id).unwrap_or_default();
        self.loading = true;
        if self.connected && !self.closing {
            self.status = "正在读取工作区…".into();
        }
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
    pub fn toggle_project(&mut self, id: String, cx: &mut Context<Self>) {
        if self.expanded_projects.remove(&id) {
            cx.notify();
            return;
        }
        self.expanded_projects.insert(id.clone());
        cx.notify();
        if self.workspace.as_ref() == Some(&id) {
            return;
        }
        let Some(runtime) = self.runtime.clone().filter(|_| self.connected) else {
            self.expanded_projects.remove(&id);
            self.error = Some("后台未连接，无法读取项目会话".into());
            return;
        };
        cx.spawn(async move |this, cx| {
            let sessions = runtime
                .request("chat.list", json!({"workspaceId": id}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<Vec<Session>>(value)
                        .map_err(|error| format!("会话列表格式错误：{error}"))
                });
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() == Some(&id)
                    || !this.expanded_projects.contains(&id)
                    || !this.workspaces.iter().any(|workspace| workspace.id == id)
                {
                    return;
                }
                match sessions {
                    Ok(sessions) => {
                        this.project_sessions.insert(id, sessions);
                    }
                    Err(error) => {
                        this.expanded_projects.remove(&id);
                        this.error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn reset_workspace_view(&mut self) {
        self.conversation_sources.clear();
        self.session = None;
        self.conversation_ready = false;
        self.messages.clear();
        self.tool_inputs.clear();
        self.expanded_tool_groups.clear();
        self.pending_reports.clear();
        self.completed_deliveries.clear();
        self.pending_events.clear();
        self.event_sequence = 0;
        self.sessions.clear();
        self.members.clear();
        self.member_catalog_generation += 1;
        self.member_catalog_loading = false;
        self.member_catalog.clear();
        self.member_picker_target = None;
        self.member_catalog_backend = None;
        self.member_generation += 1;
        self.member_stopping = None;
        self.member_loading = false;
        self.editing_member = None;
        self.deleting_member = None;
        self.models.clear();
        self.refreshing_models = false;
        self.model = None;
        self.unavailable_model = None;
        self.choosing_model = false;
        self.open_request += 1;
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
                                        if this.connected
                                            && !this.closing
                                            && this.workspace_writes_allowed()
                                        {
                                            this.status = if this.sessions.is_empty() {
                                                "工作区已就绪，新建会话开始工作"
                                            } else {
                                                "工作区已就绪"
                                            }
                                            .into();
                                        }
                                        if !this.memory.visible
                                            && !this.member_panel
                                            && !this.collaboration.visible
                                            && !this.settings_panel
                                            && !this.roles.visible
                                        {
                                            let saved =
                                                this.navigation.sessions.get(&workspace).and_then(
                                                    |id| this.sessions.iter().find(|s| &s.id == id),
                                                );
                                            if let Some(first) =
                                                saved.or_else(|| this.sessions.first())
                                            {
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
        if !self.workspace_writes_allowed() || !self.connected {
            return;
        }
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
                            if this.workspace.as_ref() == Some(&session.workspace_id)
                                && this.workspace_writes_allowed()
                            {
                                this.sessions.insert(0, session.clone());
                                this.close_collaboration();
                                let workspace_draft = format!("workspace:{}", session.workspace_id);
                                let transfer = this.session.is_none()
                                    && this.draft_owner.as_ref() == Some(&workspace_draft);
                                let session_key = format!("session:{}", session.id);
                                this.open_session(session.id, cx);
                                if transfer {
                                    if let Some(text) = this.drafts.remove(&workspace_draft) {
                                        this.drafts.insert(session_key, text);
                                    }
                                }
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
        self.conversation_sources.clear();
        self.show_session(id, cx);
    }

    pub(crate) fn open_member_session(&mut self, id: String, cx: &mut Context<Self>) {
        use crate::app::ConversationSource;
        if !self.workspace_writes_allowed() || !self.sessions.iter().any(|s| s.id == id) {
            return;
        }
        if self.member_panel {
            self.conversation_sources.clear();
            self.conversation_sources.push(ConversationSource::Team);
        } else if let Some(current) = &self.session {
            if current.id == id {
                return;
            }
            self.conversation_sources
                .push(ConversationSource::Session(current.id.clone()));
        }
        self.show_session(id, cx);
    }

    pub(crate) fn show_session(&mut self, id: String, cx: &mut Context<Self>) {
        if self.removed_sessions.contains(&id) || !self.workspace_writes_allowed() {
            return;
        }
        self.remember_conversation_position();
        self.close_memory();
        self.close_roles();
        self.switch_draft(Some(format!("session:{id}")), cx);
        self.close_settings();
        self.member_panel = false;
        self.open_request += 1;
        self.conversation_ready = false;
        self.unread.remove(&id);
        self.choosing_model = false;
        let request = self.open_request;
        if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
            self.session = Some(session.clone());
        }
        self.messages.clear();
        self.tool_inputs.clear();
        self.expanded_tool_groups.clear();
        self.pending_reports.clear();
        self.completed_deliveries.clear();
        self.pending_events.clear();
        self.event_sequence = 0;
        self.models.clear();
        self.refreshing_models = false;
        self.model = None;
        self.unavailable_model = None;
        self.loading = true;
        if self.connected && !self.closing {
            self.status = "正在打开会话…".into();
        }
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
                        if open.session.id != id {
                            this.error = Some("打开响应中的会话身份不匹配".into());
                            return;
                        }
                        if open.history_message_ids.len() != open.history.len() {
                            this.error = Some("会话快照中的消息身份数量不匹配".into());
                            return;
                        }
                        this.conversation_ready = true;
                        if open.busy {
                            this.busy.insert(id.clone());
                        } else {
                            this.busy.remove(&id);
                        }
                        this.sync_interactions(&id, open.interactions);
                        this.pending.insert(id.clone(), open.pending);
                        this.pending_reports = open.pending_reports;
                        this.completed_deliveries =
                            open.completed_delivery_ids.into_iter().collect();
                        this.background_sequences
                            .insert(id.clone(), open.event_sequence);
                        if this.navigation.sessions.get(&open.session.workspace_id) != Some(&id) {
                            this.navigation
                                .sessions
                                .insert(open.session.workspace_id.clone(), id.clone());
                            this.save_navigation(cx);
                        }
                        this.session = Some(open.session);
                        this.models = open.models;
                        this.model = open.model;
                        this.unavailable_model = open.unavailable_model;
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
                        this.restore_conversation_position(&id);
                    }
                    Err(error) => this.error = Some(format!("打开会话响应错误：{error}")),
                }
            },
        );
        cx.notify();
    }
}
