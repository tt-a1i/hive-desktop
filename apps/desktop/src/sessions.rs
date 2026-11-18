mod archiving;
mod naming;
mod pinning;
pub(crate) use pinning::PinnedSessions;
pub(crate) mod search;
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
                    cx.global::<crate::composer::ComposerPreferences>()
                        .settings
                        .expand_thinking,
                    self.messages.iter().flat_map(|message| {
                        message
                            .thinking
                            .iter()
                            .map(|block| ((message.id.clone(), block.index), block.expanded))
                    }),
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
                        cx.global::<crate::composer::ComposerPreferences>()
                            .settings
                            .expand_thinking,
                        &message.id,
                        block.index,
                        block.redacted,
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
            let _ = self.voice.cancel();
            self.attachments.scroll.set_offset(point(px(0.), px(0.)));
            self.draft_target = next;
            self.release_attachment_previews(true, cx);
        }
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
                    this.error = Some(crate::app::ErrorNotice::Detail(
                        "保存会话位置失败",
                        error.to_string(),
                    ));
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
                    if let Err(error) = crate::desktop_update::report_startup() {
                        eprintln!("Could not report updated desktop startup: {error}");
                    }
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
                        this.reset_workspace_view(cx);
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
                Err(error) => {
                    this.error = Some(crate::app::ErrorNotice::Detail(
                        "工作区响应格式错误",
                        error.to_string(),
                    ))
                }
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
    pub(crate) fn record_nav_snapshot(&mut self) {
        if self.nav_suppress_record || self.workspace.is_none() {
            return;
        }
        let current = (
            self.workspace.clone(),
            self.session.as_ref().map(|s| s.id.clone()),
        );
        if self.nav_back.last() != Some(&current) {
            self.nav_back.push(current);
            if self.nav_back.len() > 50 {
                self.nav_back.remove(0);
            }
        }
        self.nav_forward.clear();
    }

    fn reset_conversation_view(&mut self, cx: &mut Context<Self>) {
        self.conversation_ready = false;
        self.history_navigation.invalidate();
        self.conversation_find = None;
        self.search_target = None;
        self.conversation_list.reset(None);
        self.clear_conversation_messages(cx);
        self.tool_inputs = Default::default();
        self.expanded_tool_groups = Default::default();
        self.pending_reports = Vec::new();
        self.pending_user_inputs = Vec::new();
        self.reset_reply_failures();
        self.completed_deliveries = Default::default();
        self.pending_events = Vec::new();
        self.event_sequence = 0;
        self.models = Vec::new();
        self.refreshing_models = false;
        self.model = None;
        self.unavailable_model = None;
    }

    fn clear_active_session_only(&mut self, cx: &mut Context<Self>) {
        self.collaboration.task_expanded = false;
        self.focus_project_input = false;
        self.conversation_sources.clear();
        self.session = None;
        self.reset_conversation_view(cx);
    }

    fn apply_history_entry(
        &mut self,
        target_ws: Option<String>,
        target_sess: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.nav_suppress_record = true;
        if let Some(ws) = target_ws {
            if self.workspace.as_ref() != Some(&ws) {
                self.select_workspace(ws.clone(), cx);
            }
            if let Some(sess) = target_sess {
                if self.sessions.iter().any(|s| s.id == sess) {
                    self.open_session(sess, cx);
                } else {
                    self.clear_active_session_only(cx);
                }
            } else if self.session.is_some() {
                self.clear_active_session_only(cx);
                self.switch_draft(Some(format!("workspace:{ws}")), cx);
                self.sync_git_workspace(cx);
            }
        }
        self.nav_suppress_record = false;
        cx.notify();
    }

    pub fn navigate_history_back(&mut self, cx: &mut Context<Self>) {
        let current = (
            self.workspace.clone(),
            self.session.as_ref().map(|s| s.id.clone()),
        );
        while let Some((ws, sess)) = self.nav_back.pop() {
            let valid_ws = ws
                .as_ref()
                .is_some_and(|id| self.workspaces.iter().any(|w| &w.id == id));
            if !valid_ws || (&ws, &sess) == (&current.0, &current.1) {
                continue;
            }
            if current.0.is_some() && self.nav_forward.last() != Some(&current) {
                self.nav_forward.push(current);
            }
            self.apply_history_entry(ws, sess, cx);
            return;
        }
    }

    pub fn navigate_history_forward(&mut self, cx: &mut Context<Self>) {
        let current = (
            self.workspace.clone(),
            self.session.as_ref().map(|s| s.id.clone()),
        );
        while let Some((ws, sess)) = self.nav_forward.pop() {
            let valid_ws = ws
                .as_ref()
                .is_some_and(|id| self.workspaces.iter().any(|w| &w.id == id));
            if !valid_ws || (&ws, &sess) == (&current.0, &current.1) {
                continue;
            }
            if current.0.is_some() && self.nav_back.last() != Some(&current) {
                self.nav_back.push(current);
            }
            self.apply_history_entry(ws, sess, cx);
            return;
        }
    }

    pub fn select_workspace(&mut self, id: String, cx: &mut Context<Self>) {
        if self.workspace_removing.as_ref() == Some(&id) {
            return;
        }
        if self.workspace.as_ref() != Some(&id) || self.session.is_some() {
            self.record_nav_snapshot();
        }
        self.remember_conversation_position(cx);
        self.showing_welcome_guide = false;
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
        self.reset_workspace_view(cx);
        self.sync_git_workspace(cx);
        self.sessions = self.project_sessions.remove(&id).unwrap_or_default();
        self.loading = true;
        if self.connected && !self.closing {
            self.status = "正在读取工作区…".into();
        }
        self.refresh_sessions(cx);
        self.refresh_members(cx);
        if self.workflows.visible {
            self.refresh_workflows(cx);
        }
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
    pub(crate) fn reset_workspace_view(&mut self, cx: &mut Context<Self>) {
        self.reset_workflows_workspace();
        self.clear_active_session_only(cx);
        self.sessions = Vec::new();
        self.members = Vec::new();
        self.member_details_open = Default::default();
        self.member_catalog_generation += 1;
        self.member_catalog_loading = false;
        self.member_catalog = Vec::new();
        self.member_picker_target = None;
        self.member_catalog_backend = None;
        self.member_generation += 1;
        self.member_stopping = None;
        self.member_loading = false;
        self.member_error = None;
        self.editing_member = None;
        self.deleting_member = None;
        self.choosing_model = false;
        self.reasoning_picker = Default::default();
        self.mode_picker = Default::default();
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
                                    this.error = Some(crate::app::ErrorNotice::Detail(
                                        "会话列表格式错误",
                                        error.to_string(),
                                    ));
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
        self.new_session_with_profile(backend, None, cx);
    }

    pub(crate) fn new_session_with_profile(&mut self, backend: &'static str, profile: Option<crate::agent_profiles::ProfileRef>, cx: &mut Context<Self>) {
        if !self.workspace_writes_allowed() || !self.connected {
            return;
        }
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        self.showing_welcome_guide = false;
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
        let open_request = self.open_request;
        let mut params = json!({"workspaceId":workspace,"backend":backend});
        if let Some(profile) = &profile { params["profile"] = json!(profile); }
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.createRecord",
                    params,
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                match result {
                    Ok(value) => match serde_json::from_value::<Session>(value) {
                        Ok(session) => {
                            if profile.as_ref().is_some_and(|reference| session.profile.as_ref() != Some(reference)) {
                                this.error = Some("创建会话返回的配置引用不匹配".into());
                                cx.notify();
                                return;
                            }
                            if this.workspace.as_ref() == Some(&session.workspace_id)
                                && this.open_request == open_request
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
                        Err(error) => {
                            this.error = Some(crate::app::ErrorNotice::Detail(
                                "创建会话响应错误",
                                error.to_string(),
                            ))
                        }
                    },
                    Err(error) => {
                        this.error = Some(crate::app::ErrorNotice::Detail(
                            "创建会话失败",
                            format!("{backend}: {error}"),
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn accept_created_session(
        &mut self,
        session: Session,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.workspace.as_ref() != Some(&session.workspace_id)
            || !self.workspace_writes_allowed()
        {
            return false;
        }
        self.sessions.retain(|existing| existing.id != session.id);
        self.sessions.insert(0, session.clone());
        self.close_collaboration();
        let workspace_draft = format!("workspace:{}", session.workspace_id);
        let transfer =
            self.session.is_none() && self.draft_owner.as_ref() == Some(&workspace_draft);
        let session_key = format!("session:{}", session.id);
        self.open_session(session.id, cx);
        self.focus_project_input = true;
        if transfer {
            self.attachments
                .transfer(&workspace_draft, session_key.clone());
            if let Some(text) = self.drafts.remove(&workspace_draft) {
                self.drafts.insert(session_key, text);
            }
        }
        true
    }
    pub(crate) fn navigate_to_session(
        &mut self,
        session: Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.connected
            || self.closing
            || self.removed_sessions.contains(&session.id)
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
        self.close_collaboration();
        if self.workspace.as_ref() != Some(&session.workspace_id)
            || self.session.as_ref().map(|s| &s.id) != Some(&session.id)
        {
            self.record_nav_snapshot();
        }
        let prev_suppress = self.nav_suppress_record;
        self.nav_suppress_record = true;
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
        self.nav_suppress_record = prev_suppress;
        true
    }

    pub(crate) fn navigate_to_search_hit(
        &mut self,
        hit: crate::protocol::SessionHit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let session_id = hit.session.id.clone();
        let message_id = hit.body_match.map(|body| body.message_id);
        if !self.navigate_to_session(hit.session, window, cx) {
            return false;
        }
        if let Some(message_id) = message_id {
            self.search_target = Some((self.open_request, session_id, message_id));
        }
        true
    }

    pub fn open_session(&mut self, id: String, cx: &mut Context<Self>) {
        self.close_collaboration();
        self.conversation_sources.clear();
        self.show_session(id, cx);
    }

    pub(crate) fn open_member_session(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        let Some(request_workspace) = self.workspace.clone() else {
            return;
        };
        self.focus_project_input = false;
        self.showing_welcome_guide = false;
        self.close_workflows();
        if self.removed_sessions.contains(&id) || !self.workspace_writes_allowed() {
            return;
        }
        if self.session.as_ref().map(|s| &s.id) != Some(&id) {
            self.record_nav_snapshot();
        }
        self.remember_conversation_position(cx);
        self.close_memory();
        self.close_roles();
        self.switch_draft(Some(format!("session:{id}")), cx);
        self.close_settings();
        self.member_panel = false;
        self.open_request += 1;
        self.reset_conversation_view(cx);
        self.unread.remove(&id);
        self.choosing_model = false;
        self.reasoning_picker = Default::default();
        self.mode_picker = Default::default();
        self.command_picker = Default::default();
        self.reset_usage();
        let request = self.open_request;
        if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
            self.session = Some(session.clone());
        }
        self.sync_git_workspace(cx);
        self.loading = true;
        if self.connected && !self.closing {
            self.status = "正在打开会话…".into();
        }
        self.request(
            "chat.open",
            json!({"sessionId":id,"deferHistoryImages":true}),
            cx,
            move |this, value, cx| {
                if this.open_request != request || this.workspace.as_ref() != Some(&request_workspace) || this.session.as_ref().map(|s| &s.id) != Some(&id)
                {
                    return;
                }
                this.loading = false;
                this.status = "未能打开会话".into();
                match serde_json::from_value::<OpenSession>(value) {
                    Ok(open) => {
                        if !crate::history_navigation::open_session_identity_matches(&open.session, &id, &request_workspace) {
                            this.search_target = None;
                            this.error = Some(crate::app::ErrorNotice::Conversation(Box::new("打开响应中的会话身份不匹配".into())));
                            return;
                        }
                        if open.history_message_ids.len() != open.history.len() {
                            this.search_target = None;
                            this.error = Some(crate::app::ErrorNotice::Conversation(Box::new("会话快照中的消息身份数量不匹配".into())));
                            return;
                        }
                        if let (Some(draft), Some(operation)) = (open.fork_draft.clone(), open.lineage.as_ref().and_then(|v| v["operationId"].as_str()).map(str::to_owned)) {
                            this.restore_fork_draft(id.clone(), request_workspace.clone(), operation, draft, cx);
                        }
                        this.history_navigation.access = crate::history_navigation::HistoryAccess::snapshot(&open);
                        this.conversation_ready = true;
                        this.history_navigation.initialization = open.initialization.clone();
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
                        this.load_mode(None, cx);
                        this.load_permission_mode(None, cx);
                        this.sync_git_workspace(cx);
                        this.event_sequence = open.event_sequence;
                        for (i, message) in open.history.into_iter().enumerate() {
                            let message_id = open.history_message_ids[i].clone();
                            let handoff_id = crate::session_handoff::historical_item_id(&message);
                            this.receive_message(message_id.clone(), &message, cx);
                            if let Some(rendered) = this.messages.iter_mut().find(|m| m.id == message_id && matches!(m.role.as_str(), "user" | "assistant")) { rendered.handoff_item_id = handoff_id; }
                        }
                        if let Some(active) = open.active_message {
                            this.receive_message(active.message_id.clone(), &active.message, cx);
                            this.sync_thinking(&active.message_id, &active.message, true, cx);
                        }
                        for event in std::mem::take(&mut this.pending_events) {
                            this.agent_event(event, cx);
                        }
                        this.restore_conversation_position(&id, cx);
                        if let Some((target_request, target_session, message_id)) = this.search_target.take() {
                            if target_request == request && target_session == id {
                                if !this.scroll_to_conversation_message(&message_id, cx) {
                                    this.error = Some(crate::app::ErrorNotice::Conversation(Box::new(
                                        if crate::language::english() {
                                            "The matched message is no longer available. Search again.".to_owned()
                                        } else {
                                            "匹配的消息已不可用，请重新搜索。".to_owned()
                                        }.into(),
                                    )));
                                }
                            }
                        }
                    }
                    Err(error) => {
                        this.search_target = None;
                        this.error = Some(crate::app::ErrorNotice::Conversation(Box::new(crate::app::ErrorNotice::Detail("打开会话响应错误",error.to_string()))));
                    }
                }
            },
        );
        cx.notify();
    }

    pub(crate) fn refresh_handoff_target_state(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.connected
            || self.closing
            || self.removed_sessions.contains(&id)
            || self.session.as_ref().map(|s| &s.id) != Some(&id)
        {
            return;
        }
        let Some(request_workspace) = self.workspace.clone() else {
            return;
        };
        let request = self.open_request;
        self.request(
            "chat.open",
            json!({"sessionId": id, "includeLiveHistory": false, "deferHistoryImages": true}),
            cx,
            move |this, value, cx| {
                if this.open_request != request
                    || this.workspace.as_ref() != Some(&request_workspace)
                    || this.session.as_ref().map(|s| &s.id) != Some(&id)
                {
                    return;
                }
                if let Ok(open) = serde_json::from_value::<OpenSession>(value) {
                    if !crate::history_navigation::open_session_identity_matches(
                        &open.session,
                        &id,
                        &request_workspace,
                    ) {
                        return;
                    }
                    this.history_navigation.access =
                        crate::history_navigation::HistoryAccess::snapshot(&open);
                    this.history_navigation.initialization = open.initialization.clone();
                    if open.busy {
                        this.busy.insert(id.clone());
                    } else {
                        this.busy.remove(&id);
                    }
                    if let Some(item) = this.sessions.iter_mut().find(|item| item.id == id) {
                        *item = open.session.clone();
                    }
                    this.session = Some(open.session);
                    this.models = open.models;
                    this.model = open.model;
                    this.unavailable_model = open.unavailable_model;
                    this.sync_interactions(&id, open.interactions);
                    this.pending.insert(id.clone(), open.pending);
                    this.pending_reports = open.pending_reports;
                    this.pending_user_inputs = open.pending_user_inputs;
                    this.set_delivery_failure(open.delivery_failure);
                    this.completed_deliveries =
                        open.completed_delivery_ids.into_iter().collect();
                    cx.notify();
                }
            },
        );
    }

    pub(crate) fn is_session_active(&self, id: &str) -> bool {
        self.interaction_count(id) > 0
            || self.busy.contains(id)
            || self.pending.get(id).copied().unwrap_or(0) > 0
    }

    pub(crate) fn sorted_projects(&self, cx: &App) -> Vec<&Workspace> {
        let mut list: Vec<&Workspace> = self.workspaces.iter().collect();
        match crate::general::current(cx).project_sort_order {
            crate::general::ProjectSortOrder::AddedOrder => {}
            crate::general::ProjectSortOrder::RecentActivity => {
                list.sort_by(|a, b| {
                    let latest = |ws: &Workspace| -> i64 {
                        let slice = if self.workspace.as_ref() == Some(&ws.id)
                            && !self.sessions.is_empty()
                        {
                            self.sessions.as_slice()
                        } else {
                            self.project_sessions
                                .get(&ws.id)
                                .map(Vec::as_slice)
                                .unwrap_or(&[])
                        };
                        slice.iter().map(|s| s.updated_at).max().unwrap_or(i64::MIN)
                    };
                    latest(b).cmp(&latest(a))
                });
            }
            crate::general::ProjectSortOrder::Alphabetical => {
                list.sort_by(|a, b| {
                    a.name
                        .to_lowercase()
                        .cmp(&b.name.to_lowercase())
                        .then_with(|| a.name.cmp(&b.name))
                });
            }
        }
        list
    }

    pub(crate) fn ordered_sidebar_sessions(&self, cx: &App) -> Vec<Session> {
        let general = crate::general::current(cx);
        let mut seen = std::collections::HashSet::new();
        let mut pinned = Vec::new();
        for hit in &self.pinned_sessions.records {
            if !self.removed_sessions.contains(&hit.session.id)
                && !hit.session.archived
                && seen.insert(hit.session.id.clone())
            {
                pinned.push(hit.session.clone());
            }
        }
        let mut list = Vec::new();
        for ws in self.sorted_projects(cx) {
            let sessions = if self.workspace.as_ref() == Some(&ws.id) && !self.sessions.is_empty() {
                &self.sessions
            } else if let Some(cached) = self.project_sessions.get(&ws.id) {
                cached
            } else {
                continue;
            };
            for s in sessions
                .iter()
                .filter(|s| !self.removed_sessions.contains(&s.id) && !s.archived)
            {
                if seen.insert(s.id.clone()) {
                    list.push(s.clone());
                }
            }
        }
        if self.workspaces.len() > 1
            && general.project_sort_order == crate::general::ProjectSortOrder::RecentActivity
        {
            list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        }
        if general.pin_active_sessions_top {
            let mut active = Vec::new();
            let mut inactive = Vec::new();
            for s in list {
                if self.is_session_active(&s.id) {
                    active.push(s);
                } else {
                    inactive.push(s);
                }
            }
            active.extend(inactive);
            list = active;
        }
        pinned.extend(list);
        pinned
    }

    pub(crate) fn step_session(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing {
            return;
        }
        let list = self.ordered_sidebar_sessions(cx);
        let Some(target) = (match self
            .session
            .as_ref()
            .and_then(|current| list.iter().position(|item| item.id == current.id))
        {
            Some(pos) if list.len() > 1 => {
                let next = (pos as isize + delta).rem_euclid(list.len() as isize) as usize;
                Some(list[next].clone())
            }
            Some(_) => None,
            None => {
                if delta < 0 {
                    list.last().cloned()
                } else {
                    list.first().cloned()
                }
            }
        }) else {
            return;
        };
        self.navigate_to_session(target, window, cx);
    }
}
