mod archiving;
mod naming;
mod pinning;
pub(crate) use pinning::PinnedSessions;
mod search;
use crate::{app::HiveApp, protocol::*};
use gpui_kit::*;
use serde_json::json;
impl HiveApp {
    fn remember_conversation_position(&mut self, cx: &App) {
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
        let anchor = self
            .reading_above_latest()
            .then(|| self.conversation_list.anchor())
            .flatten();
        self.positions.insert(
            session.id.clone(),
            crate::app::ConversationPosition {
                anchor,
                expanded,
                expanded_tool_groups: self.expanded_tool_groups.clone(),
                thinking: crate::thinking::ThinkingPosition::new(
                    cx.global::<crate::composer::ComposerPreferences>().settings.expand_thinking,
                    self
                    .messages
                    .iter()
                    .flat_map(|message| {
                        message
                            .thinking
                            .iter()
                            .map(|block| ((message.id.clone(), block.index), block.expanded))
                    })
                ),
            },
        );
    }

    fn restore_conversation_position(&mut self, id: &str, cx: &App) {
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
        if let Some(position) = position {
            for message in &mut self.messages {
                for block in &mut message.thinking {
                    block.expanded = position.thinking.expanded(
                        cx.global::<crate::composer::ComposerPreferences>().settings.expand_thinking,
                        &message.id, block.index, block.redacted,
                    );
                }
            }
        }
        self.conversation_list
            .reset(position.and_then(|p| p.anchor.clone()));
    }

    fn switch_draft(&mut self, next: Option<String>, cx: &mut Context<Self>) {
        self.capture_editor(cx);
        self.schedule_draft_save(cx);
        if self.draft_target != next {
            self.attachments.scroll.set_offset(point(px(0.), px(0.)));
        }
        self.draft_target = next;
    }
    pub(crate) fn save_navigation(&self, cx: &mut Context<Self>) {
        let Some(runtime) = &self.runtime else { return };
        let write = runtime.begin_request("desktop.navigation.save", json!(self.navigation));
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            if let Err(error) = write.await {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return;
                    }
                    this.error = Some(crate::app::ErrorNotice::Detail("保存会话位置失败",error.to_string()));
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
                    this.pinned_sessions.records.retain(|hit| {
                        this.workspaces
                            .iter()
                            .any(|workspace| workspace.id == hit.session.workspace_id)
                    });
                    this.refresh_pinned_sessions(cx);
                    if this.workspace.as_ref().is_some_and(|id| {
                        !this.workspaces.iter().any(|workspace| &workspace.id == id)
                    }) {
                        this.capture_editor(cx);
                        this.workspace = None;
                        this.reset_workspace_view();
                    }
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
                Err(error) => this.error = Some(crate::app::ErrorNotice::Detail("工作区响应格式错误",error.to_string())),
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
            .unwrap_or_else(|| crate::language::tr("项目"))
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
                    let id = workspace.id.clone(); this.workspaces.push(workspace); this.select_workspace(id, cx);
                    this.focus_project_input = true;
                    let _ = window.update(cx, |_, window, _cx| {
                        if window.viewport_size().width < px(820.) { this.sidebar_open = false; }
                    });
                },
                Err(error) => this.error = Some(crate::app::ErrorNotice::Detail("添加工作区响应错误",error.to_string())),
            }
        });
    }
    pub fn select_workspace(&mut self, id: String, cx: &mut Context<Self>) {
        if self.workspace_removing.as_ref() == Some(&id) {
            return;
        }
        self.remember_conversation_position(cx);
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
        if self.workflows.visible { self.refresh_workflows(cx); }
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
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let sessions = runtime
                .request("chat.list", json!({"workspaceId": id}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<Vec<Session>>(value)
                        .map_err(|error| format!("会话列表格式错误：{error}"))
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
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
                        this.error = Some(error.into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn reset_workspace_view(&mut self) {
        self.collaboration.task_expanded = false;
        self.focus_project_input = false;
        self.reset_workflows_workspace();
        self.conversation_sources.clear();
        self.session = None;
        self.conversation_ready = false;
        self.conversation_find = None;
        self.conversation_list.reset(None);
        self.messages.clear();
        self.tool_inputs.clear();
        self.expanded_tool_groups.clear();
        self.pending_reports.clear();
        self.pending_user_inputs.clear();
        self.reset_reply_failures();
        self.completed_deliveries.clear();
        self.pending_events.clear();
        self.event_sequence = 0;
        self.sessions.clear();
        self.members.clear();
        self.member_details_open.clear();
        self.member_catalog_generation += 1;
        self.member_catalog_loading = false;
        self.member_catalog.clear();
        self.member_picker_target = None;
        self.member_catalog_backend = None;
        self.member_generation += 1;
        self.member_stopping = None;
        self.member_loading = false;
        self.member_error = None;
        self.editing_member = None;
        self.deleting_member = None;
        self.models.clear();
        self.refreshing_models = false;
        self.model = None;
        self.unavailable_model = None;
        self.choosing_model = false;
        self.reasoning_picker = Default::default();
        self.command_picker = Default::default();
        self.reset_usage();
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
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(180)).await;
                let Ok(workspace) = this.update(cx, |this, _| {
                    if this.connection_generation != connection {
                        return None;
                    }
                    this.list_dirty = false;
                    this.workspace.clone()
                }) else {
                    return;
                };
                let Some(workspace) = workspace else {
                    let _ = this.update(cx, |this, _| {
                        if this.connection_generation == connection {
                            this.list_refreshing = false;
                        }
                    });
                    return;
                };
                let result = runtime
                    .request("chat.list", json!({"workspaceId": workspace}))
                    .await;
                let Ok(again) = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return false;
                    }
                    if this.workspace.as_ref() == Some(&workspace) {
                        match result {
                            Ok(value) => match serde_json::from_value::<Vec<Session>>(value) {
                                Ok(sessions) => {
                                    this.sessions = sessions;
                                    this.sync_pinned_project();
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
                                            this.status = if this
                                                .sessions
                                                .iter()
                                                .all(|session| session.archived)
                                            {
                                                "工作区已就绪，新建会话开始工作"
                                            } else {
                                                "工作区已就绪"
                                            }
                                            .into();
                                        }
                                        if !this.memory.visible
                                            && !this.workflows.visible
                                            && !this.member_panel
                                            && !this.collaboration.visible
                                            && !this.settings_panel
                                            && !this.roles.visible
                                        {
                                            let saved =
                                                this.navigation.sessions.get(&workspace).and_then(
                                                    |id| this.sessions.iter().find(|s| &s.id == id),
                                                );
                                            if let Some(first) = saved.or_else(|| {
                                                this.sessions
                                                    .iter()
                                                    .find(|session| !session.archived)
                                            }) {
                                                this.open_session(first.id.clone(), cx);
                                                this.focus_project_input = true;
                                            }
                                        }
                                    }
                                }
                                Err(error) => {
                                    if this.session.is_none() {
                                        this.loading = false;
                                    }
                                    this.error = Some(crate::app::ErrorNotice::Detail("会话列表格式错误",error.to_string()));
                                }
                            },
                            Err(error) => {
                                if this.session.is_none() {
                                    this.loading = false;
                                }
                                this.error = Some(error.into());
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
    pub(crate) fn new_session_in_project(
        &mut self,
        project: String,
        backend: &'static str,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self.closing
            || self.workspace_removing.as_ref() == Some(&project)
            || !self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == project)
        {
            return;
        }
        if self.workspace.as_ref() != Some(&project) {
            self.select_workspace(project, cx);
        }
        self.new_session(backend, cx);
    }

    pub fn new_session(&mut self, backend: &'static str, cx: &mut Context<Self>) {
        if !self.workspace_writes_allowed() || !self.connected {
            return;
        }
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        if self
            .attachments
            .loading
            .contains(&format!("workspace:{workspace}"))
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            self.error = Some("后台尚未连接".into());
            return;
        };
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.createRecord",
                    json!({"workspaceId":workspace,"backend":backend}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                match result {
                    Ok(value) => match serde_json::from_value::<Session>(value) {
                        Ok(session) => {
                            if this.workspace.as_ref() == Some(&session.workspace_id)
                                && this.workspace_writes_allowed()
                            {
                                this.sessions.retain(|existing| existing.id != session.id);
                                this.sessions.insert(0, session.clone());
                                this.close_collaboration();
                                let workspace_draft = format!("workspace:{}", session.workspace_id);
                                let transfer = this.session.is_none()
                                    && this.draft_owner.as_ref() == Some(&workspace_draft);
                                let session_key = format!("session:{}", session.id);
                                this.open_session(session.id, cx);
                                this.focus_project_input = true;
                                if transfer {
                                    this.attachments
                                        .transfer(&workspace_draft, session_key.clone());
                                    if let Some(text) = this.drafts.remove(&workspace_draft) {
                                        this.drafts.insert(session_key, text);
                                    }
                                }
                            }
                        }
                        Err(error) => this.error = Some(crate::app::ErrorNotice::Detail("创建会话响应错误",error.to_string())),
                    },
                    Err(error) => this.error = Some(crate::app::ErrorNotice::Detail("创建会话失败",format!("{backend}: {error}"))),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn navigate_to_session(
        &mut self,
        session: Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.connected
            || self.closing
            || self.workspace_removing.as_ref() == Some(&session.workspace_id)
            || !self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == session.workspace_id)
        {
            return false;
        }
        if window.viewport_size().width < px(820.) {
            self.sidebar_open = false;
        }
        self.hide_terminal(cx);
        self.close_collaboration();
        if self.workspace.as_ref() != Some(&session.workspace_id) {
            let cached = self
                .project_sessions
                .entry(session.workspace_id.clone())
                .or_default();
            if !cached.iter().any(|item| item.id == session.id) {
                cached.push(session.clone());
            }
            self.select_workspace(session.workspace_id.clone(), cx);
        } else if !self.sessions.iter().any(|item| item.id == session.id) {
            self.sessions.push(session.clone());
        }
        self.open_session(session.id, cx);
        true
    }

    pub fn open_session(&mut self, id: String, cx: &mut Context<Self>) {
        self.close_collaboration();
        self.conversation_sources.clear();
        self.show_session(id, cx);
    }

    pub(crate) fn open_member_session(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        use crate::app::ConversationSource;
        if !self.workspace_writes_allowed() || !self.sessions.iter().any(|s| s.id == id) {
            return;
        }
        if self.collaboration.visible {
            let Some(dispatch) = &self.collaboration.selected else {
                return;
            };
            self.conversation_sources.clear();
            self.conversation_sources
                .push(ConversationSource::Dispatch(dispatch.id.clone()));
            self.close_collaboration();
        } else if self.workflows.visible {
            self.conversation_sources.clear();
            self.conversation_sources.push(ConversationSource::Workflow);
        } else if self.member_panel {
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
        let focus = self.input.read(cx).focus_handle(cx);
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
    }

    pub(crate) fn show_session(&mut self, id: String, cx: &mut Context<Self>) {
        self.focus_project_input = false;
        self.close_workflows();
        if self.removed_sessions.contains(&id) || !self.workspace_writes_allowed() {
            return;
        }
        self.remember_conversation_position(cx);
        self.close_memory();
        self.close_roles();
        self.switch_draft(Some(format!("session:{id}")), cx);
        self.close_settings();
        self.member_panel = false;
        self.open_request += 1;
        self.conversation_ready = false;
        self.unread.remove(&id);
        self.choosing_model = false;
        self.reasoning_picker = Default::default();
        self.command_picker = Default::default();
        self.reset_usage();
        let request = self.open_request;
        if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
            self.session = Some(session.clone());
        }
        self.conversation_find = None;
        self.conversation_list.reset(None);
        self.messages.clear();
        self.tool_inputs.clear();
        self.expanded_tool_groups.clear();
        self.pending_reports.clear();
        self.pending_user_inputs.clear();
        self.reset_reply_failures();
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
                this.status = "未能打开会话".into();
                match serde_json::from_value::<OpenSession>(value) {
                    Ok(open) => {
                        if open.session.id != id {
                            this.error = Some(crate::app::ErrorNotice::Conversation(Box::new("打开响应中的会话身份不匹配".into())));
                            return;
                        }
                        if open.history_message_ids.len() != open.history.len() {
                            this.error = Some(crate::app::ErrorNotice::Conversation(Box::new("会话快照中的消息身份数量不匹配".into())));
                            return;
                        }
                        this.conversation_ready = true;
                        this.status = "后台已连接".into();
                        if open.busy {
                            this.busy.insert(id.clone());
                        } else {
                            this.busy.remove(&id);
                        }
                        this.sync_interactions(&id, open.interactions);
                        this.pending.insert(id.clone(), open.pending);
                        this.pending_reports = open.pending_reports;
                        this.pending_user_inputs = open.pending_user_inputs;
                        this.set_delivery_failure(open.delivery_failure);
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
                            this.receive_message(active.message_id.clone(), &active.message, cx);
                            this.sync_thinking(&active.message_id, &active.message, true, cx);
                        }
                        for event in std::mem::take(&mut this.pending_events) {
                            this.agent_event(event, cx);
                        }
                        this.restore_conversation_position(&id, cx);
                    }
                    Err(error) => this.error = Some(crate::app::ErrorNotice::Conversation(Box::new(crate::app::ErrorNotice::Detail("打开会话响应错误",error.to_string())))),
                }
            },
        );
        cx.notify();
    }
}
