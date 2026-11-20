mod archiving;
mod naming;
mod pinning;
pub(crate) mod reading_position;
pub(crate) use pinning::PinnedSessions;
pub(crate) mod search;
use crate::{app::HiveApp, history_navigation::HistoryDirection, protocol::*};
use gpui_kit::{component::WindowExt, *};
use serde_json::json;
use std::path::PathBuf;

enum ConversationOpen {
    Resume,
    AttachFiles(Vec<PathBuf>),
    Focus(String),
    Find(String),
    Latest,
    Connect(NativeSessionPanel),
}

#[derive(serde::Deserialize)]
struct ScratchConversation {
    workspace: Workspace,
    session: Session,
}

struct FreshDraftAfterSend {
    source: Session,
    window: AnyWindowHandle,
    narrow: bool,
    worktree_request: Option<String>,
}

pub(crate) struct FreshWorktreeDraftRetry {
    source: Session,
    request: String,
}

fn matches_fresh_draft(source: &Session, request: Option<&str>, draft: &Session) -> bool {
    if draft.id == source.id || draft.workspace_id != source.workspace_id
        || draft.backend != source.backend || draft.profile != source.profile
        || draft.identity_state.as_deref() != Some("new") || draft.agent_id.is_some()
        || draft.origin_agent_id.is_some() || draft.scratch || draft.archived
        || draft.worktree_released
    {
        return false;
    }
    match (source.execution.as_ref(), request, draft.execution.as_ref()) {
        (None, None, None) => true,
        (Some(original), Some(request), Some(execution)) => {
            draft.id == request
                && original.kind == "worktree"
                && original.owner_session_id.is_none()
                && execution.kind == "worktree"
                && execution.owner_session_id.is_none()
                && std::path::Path::new(&execution.path).is_absolute()
                && execution.path != original.path
                && !execution.branch.is_empty()
                && execution.branch != original.branch
        }
        _ => false,
    }
}

pub(crate) enum NativeSessionPanel {
    Models,
    Commands,
}

fn anchor_message(anchor: crate::conversation_list::ReadingAnchor) -> Option<String> {
    match anchor.key {
        crate::conversation_list::RowKey::Message(id) | crate::conversation_list::RowKey::Group(id) => Some(id),
        _ => None,
    }
}

impl HiveApp {
    fn remember_conversation_position(&mut self, cx: &App) {
        self.flush_reading_positions();
        let Some(session) = self.session.as_ref() else {
            return;
        };
        if self.loading || !self.conversation_ready || self.messages.is_empty() {
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
        self.positions.insert(
            session.id.clone(),
            crate::app::ConversationPosition {
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

    fn restore_conversation_position(&mut self, id: &str, restore_anchor: bool, cx: &App) {
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
        let anchor = self.workspace.as_deref().filter(|_| restore_anchor)
            .and_then(|workspace| self.reading_positions.anchor(workspace, id))
            .filter(|anchor| match &anchor.key {
                crate::conversation_list::RowKey::Message(id) | crate::conversation_list::RowKey::Group(id) => self.message_indices.contains_key(id),
                _ => false,
            });
        if let Some(crate::conversation_list::ReadingAnchor {
            key: crate::conversation_list::RowKey::Message(id),
            ..
        }) = &anchor {
            self.reveal_conversation_tool_group(id);
        }
        self.conversation_list.reset(anchor);
    }

    fn switch_draft(&mut self, next: Option<String>, cx: &mut Context<Self>) {
        self.capture_editor(cx);
        self.schedule_draft_save(cx);
        if self.draft_target != next {
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
                    this.mark_global_inbox_workspaces_loaded();
                    this.recover_draft_copies(cx);
                    this.refresh_scratch_projects(cx);
                    this.maybe_open_first_run_welcome(cx);
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
        let window = window.window_handle();
        self.request("hive.request", crate::workspace_actions::workspace_add_params(&path), cx, move |this, value, cx| {
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
        self.history_page = None;
        self.conversation_find = None;
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
        self.open_request += 1;
        if self.connected && !self.closing {
            self.request(
                "chat.clearView",
                json!({"viewRevision": self.open_request}),
                cx,
                |_, _, _| {},
            );
        }
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
                    self.remember_conversation_position(cx);
                    self.clear_active_session_only(cx);
                }
            } else if self.session.is_some() {
                self.remember_conversation_position(cx);
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
        if self.workspace.as_ref() != Some(&id) {
            self.refresh_project_sessions(id, cx);
        }
    }

    pub(crate) fn refresh_project_sessions(&mut self, id: String, cx: &mut Context<Self>) {
        if self.workspace.as_ref() == Some(&id) {
            self.refresh_sessions(cx);
            return;
        }
        if !self.expanded_projects.contains(&id) {
            self.project_sessions.remove(&id);
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
                                            && !this.showing_welcome_guide
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
        self.create_new_session(Some(project), backend, None, None, cx);
    }

    pub fn new_session(&mut self, backend: &'static str, cx: &mut Context<Self>) {
        self.new_session_with_profile(backend, None, cx);
    }

    pub(crate) fn new_session_with_profile(
        &mut self,
        backend: &'static str,
        profile: Option<crate::agent_profiles::ProfileRef>,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self.workspace.clone() {
            self.create_new_session(Some(project), backend, profile, None, cx);
        }
    }

    pub(crate) fn create_new_session(
        &mut self,
        project: Option<String>,
        backend: &'static str,
        profile: Option<crate::agent_profiles::ProfileRef>,
        window: Option<&Window>,
        cx: &mut Context<Self>,
    ) {
        self.create_new_session_with_open(project, backend, profile, window, None, cx);
    }

    pub(crate) fn create_fresh_draft_after_send(
        &mut self,
        source: &Session,
        window: AnyWindowHandle,
        narrow: bool,
        cx: &mut Context<Self>,
    ) {
        let worktree_request = source.execution.as_ref().map(|_| uuid::Uuid::new_v4().to_string());
        self.create_after_send(source.clone(), window, narrow, worktree_request, cx);
    }

    pub(crate) fn can_retry_fresh_worktree_draft(&self) -> bool {
        self.can_start_new_conversation()
            && self.viewing_conversation()
            && self.conversation_ready
            && self.fresh_worktree_draft_retry.as_ref().is_some_and(|retry| {
                self.workspace.as_ref() == Some(&retry.source.workspace_id)
                    && self.session.as_ref().is_some_and(|current| {
                        current.id == retry.source.id
                            && current.workspace_id == retry.source.workspace_id
                            && current.execution == retry.source.execution
                            && current.profile == retry.source.profile
                            && current.backend == retry.source.backend
                            && current.can_start_fresh_worktree_draft
                    })
            })
    }

    pub(crate) fn retry_fresh_worktree_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_retry_fresh_worktree_draft() || window.has_active_dialog(cx) {
            return;
        }
        if let Some(retry) = self.fresh_worktree_draft_retry.as_ref() {
            self.create_after_send(
                retry.source.clone(),
                window.window_handle(),
                window.viewport_size().width < px(820.),
                Some(retry.request.clone()),
                cx,
            );
        }
    }

    fn create_after_send(
        &mut self,
        source: Session,
        window: AnyWindowHandle,
        narrow: bool,
        worktree_request: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.create_new_session_with_open(
            Some(source.workspace_id.clone()), source.backend.wire(), source.profile.clone(),
            None, Some(FreshDraftAfterSend { source, window, narrow, worktree_request }), cx,
        );
    }

    fn create_new_session_with_open(
        &mut self,
        project: Option<String>,
        backend: &'static str,
        profile: Option<crate::agent_profiles::ProfileRef>,
        window: Option<&Window>,
        fresh_after: Option<FreshDraftAfterSend>,
        cx: &mut Context<Self>,
    ) {
        use crate::app::ErrorNotice;

        if !self.connected
            || self.closing
            || self.new_session_creation == Some(self.connection_generation)
            || !self.workspace_writes_allowed()
            || project.as_ref().is_some_and(|id| {
                self.workspace_removing.as_ref() == Some(id)
                    || !self.workspaces.iter().any(|workspace| &workspace.id == id)
                    || self.attachments.loading.contains(&format!("workspace:{id}"))
            })
            || (project.is_none() && self.workspace_removing.is_some())
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let scratch_id = if project.is_none() {
            let request = self.scratch_session_retry.get_or_insert_with(|| {
                (backend, uuid::Uuid::new_v4().to_string())
            });
            if request.0 != backend {
                *request = (backend, uuid::Uuid::new_v4().to_string());
            }
            Some(request.1.clone())
        } else {
            None
        };

        let connection = self.connection_generation;
        let open_request = self.open_request;
        let welcome = self.showing_welcome_guide;
        let narrow = fresh_after.as_ref().map_or_else(
            || window.is_some_and(|window| window.viewport_size().width < px(820.)),
            |target| target.narrow,
        );
        let hide_terminal = window.is_some();
        self.new_session_creation = Some(connection);
        let fresh_worktree = fresh_after.as_ref().and_then(|target| {
            target.worktree_request.as_ref().map(|request| (&target.source, request))
        });
        let (method, params) = if let Some((source, request)) = fresh_worktree {
            ("chat.createFreshWorktreeDraftRecord", json!({
                "workspaceId": source.workspace_id, "sourceSessionId": source.id, "requestId": request,
            }))
        } else {
            let (method, mut params) = if let Some(project) = &project {
                ("chat.createRecord", json!({"workspaceId": project, "backend": backend}))
            } else {
                ("chat.createScratchRecord", json!({"requestId": scratch_id, "backend": backend}))
            };
            if let Some(reference) = &profile {
                params["profile"] = json!(reference);
            }
            if fresh_after.is_some() {
                params["deferFirstConnection"] = json!(true);
            }
            (method, params)
        };
        if fresh_after.is_some() {
            self.error = None;
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.new_session_creation = None;
                let result = result
                    .map_err(|error| ErrorNotice::Detail("创建会话失败", format!("{backend}: {error}")))
                    .and_then(|value| {
                        let parsed = if scratch_id.is_some() {
                            serde_json::from_value::<ScratchConversation>(value)
                                .map(|response| (Some(response.workspace), response.session))
                        } else {
                            serde_json::from_value::<Session>(value).map(|session| (None, session))
                        };
                        let (workspace, session) = parsed.map_err(|error| {
                            ErrorNotice::Detail("创建会话响应错误", error.to_string())
                        })?;
                        let expected = project.as_deref()
                            .or_else(|| workspace.as_ref().map(|item| item.id.as_str()));
                        if session.id.is_empty()
                            || expected != Some(session.workspace_id.as_str())
                            || session.backend.wire() != backend
                            || profile.as_ref().is_some_and(|reference| session.profile.as_ref() != Some(reference))
                            || fresh_after.as_ref().is_some_and(|target| {
                                !matches_fresh_draft(&target.source, target.worktree_request.as_deref(), &session)
                            })
                            || scratch_id.as_ref().is_some_and(|id| id != &session.id || !session.scratch)
                        {
                            return Err(ErrorNotice::Message("创建会话返回的会话身份不匹配"));
                        }
                        if let Some(workspace) = workspace {
                            if workspace.id.is_empty()
                                || !std::path::Path::new(&workspace.path).is_absolute()
                                || this.workspaces.iter().any(|item| {
                                    (item.id == workspace.id) != (item.path == workspace.path)
                                })
                            {
                                return Err(ErrorNotice::Message("返回的临时工作区与现有项目冲突"));
                            }
                            this.scratch_session_retry = None;
                            if !this.workspaces.iter().any(|item| item.id == workspace.id) {
                                this.workspaces.push(workspace);
                            }
                        }
                        Ok(session)
                    });

                let fresh_origin_current = fresh_after.as_ref().is_none_or(|target| {
                    this.session.as_ref().is_some_and(|session| {
                        session.id == target.source.id && session.execution == target.source.execution
                    })
                        && !this.inflight_inputs.contains_key(&target.source.id)
                        && this.viewing_conversation()
                        && target.window.update(cx, |_, window, cx| !window.has_active_dialog(cx)).unwrap_or(false)
                });
                if let Some(target) = fresh_after.as_ref()
                    && let Some(request) = target.worktree_request.as_ref()
                {
                    if result.is_err() {
                        this.fresh_worktree_draft_retry = Some(FreshWorktreeDraftRetry {
                            source: target.source.clone(),
                            request: request.clone(),
                        });
                    } else if this.fresh_worktree_draft_retry.as_ref().is_some_and(|retry| {
                        retry.source.id == target.source.id && &retry.request == request
                    }) {
                        this.fresh_worktree_draft_retry = None;
                    }
                }
                match result {
                    Ok(session) if fresh_origin_current
                        && this.open_request == open_request
                        && this.showing_welcome_guide == welcome
                        && this.connected
                        && !this.closing
                        && !this.settings_panel
                        && !this.roles.visible
                        && this.workspace_writes_allowed()
                        && !this.removed_sessions.contains(&session.id)
                        && this.workspace_removing.as_ref() != Some(&session.workspace_id)
                        && this.workspaces.iter().any(|item| item.id == session.workspace_id) =>
                    {
                        if hide_terminal {
                            this.hide_terminal(cx);
                        }
                        let opened = if this.workspace.as_ref() == Some(&session.workspace_id) {
                            if narrow {
                                this.sidebar_open = false;
                            }
                            this.accept_created_session(session, cx)
                        } else {
                            // The first save for the new project must already identify the new session.
                            this.navigation.sessions.insert(session.workspace_id.clone(), session.id.clone());
                            this.navigate_to_session_at(session, ConversationOpen::Resume, narrow, cx)
                        };
                        if opened {
                            this.focus_project_input = true;
                            if welcome {
                                this.mark_welcome_started(cx);
                            }
                        }
                    }
                    Ok(session) => {
                        this.refresh_project_sessions(session.workspace_id, cx);
                    }
                    Err(error) if fresh_origin_current && this.open_request == open_request => {
                        this.error = Some(if fresh_after.is_some() {
                            let guidance = if fresh_after.as_ref().is_some_and(|target| target.worktree_request.is_some()) {
                                "重试只会准备新的工作树草稿。"
                            } else {
                                "请从侧边栏手动打开新会话"
                            };
                            ErrorNotice::Context("消息已发送，但未能打开新草稿", Box::new(error), guidance)
                        } else {
                            error
                        });
                    }
                    Err(_) => (),
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
        self.navigate_to_session_at(
            session,
            ConversationOpen::Resume,
            window.viewport_size().width < px(820.),
            cx,
        )
    }

    pub(crate) fn can_drop_files_on_session(&self, session: &Session) -> bool {
        let key = format!("session:{}", session.id);
        let selected = self.workspace.as_ref() == Some(&session.workspace_id)
            && self.session.as_ref().is_some_and(|current| current.id == session.id);
        self.connected
            && !self.closing
            && !session.archived
            && !session.worktree_released
            && !self.worktree_release_loading.contains(&session.id)
            && !self.removed_sessions.contains(&session.id)
            && !self.session_archive_loading.contains(&session.id)
            && self.workspace_removing.as_ref() != Some(&session.workspace_id)
            && self.workspaces.iter().any(|workspace| workspace.id == session.workspace_id)
            && !self.inflight_inputs.contains_key(&session.id)
            && !self.attachments.loading.contains(&key)
            && !(selected && (self.loading || self.conversation_is_read_only()))
    }

    pub(crate) fn drop_files_on_session(
        &mut self,
        session: Session,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() || !self.can_drop_files_on_session(&session) {
            return;
        }
        if self.viewing_conversation()
            && self.conversation_ready
            && self.workspace.as_ref() == Some(&session.workspace_id)
            && self.session.as_ref().is_some_and(|current| current.id == session.id)
        {
            if window.viewport_size().width < px(820.) {
                self.sidebar_open = false;
            }
            self.attach_open_conversation_files(&session.workspace_id, &session.id, paths, cx);
            return;
        }
        self.navigate_to_session_at(
            session,
            ConversationOpen::AttachFiles(paths),
            window.viewport_size().width < px(820.),
            cx,
        );
    }

    fn attach_open_conversation_files(
        &mut self,
        workspace: &str,
        session: &str,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if !self.conversation_ready
            || self.workspace.as_deref() != Some(workspace)
            || !self.session.as_ref().is_some_and(|current| {
                current.id == session
                    && current.workspace_id == workspace
                    && self.can_drop_files_on_session(current)
            })
            // Navigation events can update this cache before an older open reply
            // replaces the active session snapshot.
            || self.sessions.iter().any(|row| row.id == session && row.archived)
            || self.draft_target.as_deref() != Some(format!("session:{session}").as_str())
        {
            self.error = Some("当前会话暂不能添加附件，请稍后再试。".into());
            cx.notify();
            return;
        }
        self.import_image_paths(paths, cx);
        self.focus_project_input = self.viewing_conversation();
        cx.notify();
    }

    fn navigate_to_session_at(
        &mut self,
        session: Session,
        target: ConversationOpen,
        narrow: bool,
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
        if narrow {
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
        self.conversation_sources.clear();
        self.show_session_at(session.id, target, cx);
        self.nav_suppress_record = prev_suppress;
        true
    }

    pub(crate) fn navigate_to_search_hit(
        &mut self,
        hit: crate::protocol::SessionHit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let target = hit.body_match.map_or(ConversationOpen::Resume, |body| {
            ConversationOpen::Focus(body.message_id)
        });
        self.navigate_to_session_at(hit.session, target, window.viewport_size().width < px(820.), cx)
    }

    pub fn open_session(&mut self, id: String, cx: &mut Context<Self>) {
        self.close_collaboration();
        self.conversation_sources.clear();
        self.show_session(id, cx);
    }

    pub(crate) fn lead_session_id(&self) -> Option<String> {
        let workspace = self.workspace.as_deref()?;
        let orchestrator = format!("{workspace}:orchestrator");
        self.sessions
            .iter()
            .find(|s| !s.archived && s.agent_id.as_deref() == Some(orchestrator.as_str()))
            .or_else(|| {
                self.sessions
                    .iter()
                    .find(|s| s.agent_id.as_deref() == Some(orchestrator.as_str()))
            })
            .map(|s| s.id.clone())
    }

    pub(crate) fn member_session_id(&self, agent_id: &str) -> Option<String> {
        if let Some(member) = self.members.iter().find(|m| m.id == agent_id) {
            if let Some(id) = member
                .desktop_activity
                .as_ref()
                .and_then(|a| a.session_id.as_ref())
            {
                if self.sessions.iter().any(|s| &s.id == id) {
                    return Some(id.clone());
                }
            }
        }
        self.sessions
            .iter()
            .find(|s| !s.archived && s.agent_id.as_deref() == Some(agent_id))
            .or_else(|| {
                self.sessions
                    .iter()
                    .find(|s| !s.archived && s.origin_agent_id.as_deref() == Some(agent_id))
            })
            .or_else(|| {
                self.sessions.iter().find(|s| {
                    s.agent_id.as_deref() == Some(agent_id)
                        || s.origin_agent_id.as_deref() == Some(agent_id)
                })
            })
            .map(|s| s.id.clone())
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
            self.conversation_sources.clear();
            if let Some(dispatch) = &self.collaboration.selected {
                self.conversation_sources
                    .push(ConversationSource::Dispatch(dispatch.id.clone()));
            } else if let Some(lead_id) = self.lead_session_id().filter(|lead| lead != &id) {
                self.conversation_sources
                    .push(ConversationSource::Session(lead_id));
            }
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
        self.show_session_at(id, ConversationOpen::Resume, cx);
    }

    pub(crate) fn show_latest_session(&mut self, id: String, cx: &mut Context<Self>) {
        self.show_session_at(id, ConversationOpen::Latest, cx);
    }

    pub(crate) fn open_conversation_find_match(&mut self, message: String, cx: &mut Context<Self>) {
        if let Some(id) = self.session.as_ref().map(|session| session.id.clone()) {
            self.show_session_at(id, ConversationOpen::Find(message), cx);
        }
    }

    pub(crate) fn connect_saved_session(&mut self, panel: NativeSessionPanel, cx: &mut Context<Self>) -> bool {
        if self.loading || !self.connected || self.closing || self.conversation_is_read_only()
            || !self.current_history_access().is_some_and(|access| access.saved_history_passive)
        {
            return false;
        }
        let Some(id) = self.session.as_ref().map(|session| session.id.clone()) else { return false };
        self.show_session_at(id, ConversationOpen::Connect(panel), cx);
        true
    }

    fn show_session_at(&mut self, id: String, target: ConversationOpen, cx: &mut Context<Self>) {
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
        let opened_around = self.history_navigation.opened_around.clone();
        let retained_find = matches!(&target, ConversationOpen::Find(_))
            .then(|| self.conversation_find.take()).flatten();
        self.open_request += 1;
        self.reset_conversation_view(cx);
        self.conversation_find = retained_find;
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
        let around_message_id = match &target {
            ConversationOpen::Focus(id) | ConversationOpen::Find(id) => Some(id.clone()),
            ConversationOpen::Latest => None,
            ConversationOpen::Resume | ConversationOpen::AttachFiles(_) | ConversationOpen::Connect(_) => self.reading_positions.anchor(&request_workspace, &id)
                .and_then(anchor_message)
                .or_else(|| matches!(&target, ConversationOpen::Connect(_)).then_some(opened_around).flatten()),
        };
        self.history_navigation.opened_around.clone_from(&around_message_id);
        let mut open_params = json!({
            "sessionId": id,
            "deferHistoryImages": true,
            "viewRevision": request,
            "historyLimit": 120
        });
        if let Some(around) = around_message_id {
            open_params["historyAroundMessageId"] = json!(around);
        }
        if matches!(&target, ConversationOpen::Connect(_)) {
            open_params["preferSavedHistory"] = json!(false);
        }
        self.request(
            "chat.open",
            open_params,
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
                            this.error = Some(crate::app::ErrorNotice::Conversation(Box::new("打开响应中的会话身份不匹配".into())));
                            return;
                        }
                        if open.history_message_ids.len() != open.history.len() {
                            this.error = Some(crate::app::ErrorNotice::Conversation(Box::new("会话快照中的消息身份数量不匹配".into())));
                            return;
                        }
                        this.read_inbox_chat(&open.session, cx);
                        if let (Some(draft), Some(operation)) = (open.fork_draft.clone(), open.lineage.as_ref().and_then(|v| v["operationId"].as_str()).map(str::to_owned)) {
                            this.restore_fork_draft(id.clone(), request_workspace.clone(), operation, draft, cx);
                        }
                        if let Some(review) = &open.pr_review {
                            this.restore_pr_review_draft(&id, review.prompt.clone(), cx);
                        }
                        this.history_navigation.access = crate::history_navigation::HistoryAccess::snapshot(&open);
                        this.conversation_ready = true;
                        this.history_navigation.initialization = open.initialization.clone();
                        // A short script can exit before the open response reaches this window.
                        this.reconcile_active_initialization(cx);
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
                        this.history_page = open.history_page;
                        let retained_find = this.conversation_find.take();
                        this.restore_conversation_history(open.history_message_ids, open.history, cx);
                        if this.history_page.as_ref().is_some_and(|page| page.has_newer) {
                            this.history_navigation.live_start = Some(this.messages.len());
                        }
                        if let Some(active) = open.active_message {
                            this.receive_message(active.message_id.clone(), &active.message, cx);
                            this.sync_thinking(&active.message_id, &active.message, true, cx);
                        }
                        for event in std::mem::take(&mut this.pending_events) {
                            this.agent_event(event, cx);
                        }
                        this.restore_conversation_position(&id, matches!(&target, ConversationOpen::Resume | ConversationOpen::AttachFiles(_) | ConversationOpen::Connect(_)), cx);
                        this.conversation_find = retained_find;
                        match target {
                            ConversationOpen::AttachFiles(paths) => {
                                this.attach_open_conversation_files(&request_workspace, &id, paths, cx);
                            }
                            ConversationOpen::Find(message) => this.finish_conversation_find_jump(&message, cx),
                            ConversationOpen::Focus(message_id) => {
                                if let Some(message) = this.message_mut(&message_id)
                                    && let Some(failure) = &mut message.failure
                                    && !failure.expanded
                                {
                                    failure.expanded = true;
                                    message.layout_revision += 1;
                                }
                                if this.scroll_to_conversation_message(&message_id, cx) {
                                    this.schedule_reading_position_save(cx);
                                } else {
                                    this.error = Some(crate::app::ErrorNotice::Conversation(Box::new(
                                        if crate::language::english() {
                                            "The matched message is no longer available. Search again.".to_owned()
                                        } else {
                                            "匹配的消息已不可用，请重新搜索。".to_owned()
                                        }.into(),
                                    )));
                                }
                            }
                            ConversationOpen::Latest => this.schedule_reading_position_save(cx),
                            ConversationOpen::Connect(panel) if !this.conversation_is_read_only() => match panel {
                                NativeSessionPanel::Models => {
                                    this.choosing_model = true;
                                    this.load_reasoning(None, cx);
                                }
                                NativeSessionPanel::Commands => {
                                    this.command_picker.open = true;
                                    this.load_commands(cx);
                                }
                            },
                            ConversationOpen::Connect(_) => {}
                            ConversationOpen::Resume => {}
                        }
                    }
                    Err(error) => {
                        this.error = Some(crate::app::ErrorNotice::Conversation(Box::new(crate::app::ErrorNotice::Detail("打开会话响应错误",error.to_string()))));
                    }
                }
            },
        );
        cx.notify();
    }

    pub(crate) fn load_history_page(&mut self, direction: HistoryDirection, cx: &mut Context<Self>) {
        use crate::transport::HistoryPageRequestError;
        if self.history_navigation.loading_page.is_some() || !self.connected || self.closing {
            return;
        }
        let (Some(session), Some(page)) = (self.session.as_ref(), self.history_page.as_ref()) else {
            return;
        };
        if self.history_navigation.snapshot_stale {
            let (id, workspace) = (session.id.clone(), session.workspace_id.clone());
            self.flush_reading_positions();
            let anchor = self.reading_positions.anchor(&workspace, &id).and_then(anchor_message)
                .or_else(|| self.history_navigation.opened_around.clone());
            self.error = None;
            self.show_session_at(id, anchor.map_or(ConversationOpen::Resume, ConversationOpen::Focus), cx);
            return;
        }
        let (index_key, id_key, index, anchor) = match direction {
            HistoryDirection::Newer => (
                "afterIndex", "afterMessageId", page.next_after_index, &page.next_after_message_id,
            ),
            HistoryDirection::Older => (
                "beforeIndex", "beforeMessageId", page.next_before_index, &page.next_before_message_id,
            ),
        };
        let (Some(runtime), Some(index)) = (self.runtime.clone(), index) else {
            return;
        };
        let session_id = session.id.clone();
        let request = self.open_request;
        let connection = self.connection_generation;
        let mut params = json!({
            "sessionId": session_id,
            "viewRevision": request,
            "limit": 120,
            "deferHistoryImages": true
        });
        params[index_key] = json!(index);
        if let Some(anchor) = anchor {
            params[id_key] = json!(anchor);
        }
        let snapshot = page.snapshot.clone();
        if let Some(snapshot) = &snapshot {
            params["snapshot"] = json!(snapshot);
        }
        self.history_navigation.loading_page = Some(direction);
        self.sync_conversation_list(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.history_page(params).await
                .and_then(|value| serde_json::from_value::<HistoryPageResponse>(value).map_err(|error| HistoryPageRequestError::Other(error.to_string())))
                .and_then(|page| {
                    if !page.history_page.matches_snapshot(snapshot.as_deref()) {
                        return Err(HistoryPageRequestError::StaleSnapshot);
                    }
                    if page.history.len() != page.history_message_ids.len() {
                        return Err(HistoryPageRequestError::Other(crate::language::tr("会话快照中的消息身份数量不匹配").to_owned()));
                    }
                    Ok(page)
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection || this.open_request != request
                    || this.session.as_ref().map(|s| &s.id) != Some(&session_id)
                {
                    return;
                }
                this.history_navigation.loading_page = None;
                let newer = direction == HistoryDirection::Newer;
                match result {
                    Ok(resp) => {
                        if let Some(current) = &mut this.history_page {
                            current.extend(resp.history_page, newer);
                            let has_newer = current.has_newer;
                            let find = this.conversation_find.take();
                            if newer {
                                this.append_conversation_history(
                                    resp.history_message_ids, resp.history, has_newer, cx,
                                );
                            } else {
                                this.prepend_conversation_history(resp.history_message_ids, resp.history, cx);
                            }
                            this.conversation_find = find;
                            this.refresh_loaded_conversation_find();
                        }
                        this.schedule_reading_position_save(cx);
                    }
                    Err(HistoryPageRequestError::StaleSnapshot) => {
                        this.history_navigation.snapshot_stale = true;
                        this.error = Some("已保存的历史发生变化，请重新读取会话历史。".into());
                    }
                    Err(HistoryPageRequestError::Other(error)) => {
                        this.error = Some(crate::app::ErrorNotice::Detail(
                            if newer { "读取更新历史失败" } else { "读取更早历史失败" },
                            error,
                        ));
                    }
                }
                this.sync_conversation_list(cx);
                cx.notify();
            });
        }).detach();
    }

    pub(crate) fn refresh_handoff_target_state(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.connected
            || self.closing
            || self.loading
            || !self.conversation_ready
            || self.removed_sessions.contains(&id)
            || self.session.as_ref().map(|s| &s.id) != Some(&id)
        {
            return;
        }
        let Some(request_workspace) = self.workspace.clone() else {
            return;
        };
        let request = self.open_request;
        let state_request = self.history_navigation.next_handoff_state_request();
        self.request(
            "chat.handoffTargetState",
            json!({"sessionId": id, "viewRevision": request}),
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
                    let latest_event = this.event_sequence.max(
                        this.background_sequences.get(&id).copied().unwrap_or(0),
                    );
                    if !this.history_navigation.apply_handoff_state(&open, state_request, latest_event)
                    {
                        return;
                    }
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

    pub(crate) fn step_session(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing {
            return;
        }
        let Some(target) = self.sidebar_session_step(delta, cx) else {
            return;
        };
        self.navigate_to_session(target, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{Session, matches_fresh_draft};
    use serde_json::json;

    #[test]
    fn background_draft_response_cannot_reuse_the_source_worktree() {
        let path = std::env::temp_dir();
        let source = json!({
            "id": "source", "title": "Source", "workspaceId": "project", "backend": "codex",
            "profile": {"id": "profile", "revision": 3},
            "execution": {"kind": "worktree", "path": path.join("original"), "branch": "original"},
        });
        let mut draft = source.clone();
        draft["id"] = json!("request");
        draft["identityState"] = json!("new");
        let source = serde_json::from_value::<Session>(source).unwrap();
        let matches = |value: serde_json::Value| {
            let target = serde_json::from_value::<Session>(value).unwrap();
            matches_fresh_draft(&source, Some("request"), &target)
        };
        assert!(!matches(draft.clone()));
        draft["execution"]["path"] = json!(path.join("separate"));
        assert!(!matches(draft.clone()));
        draft["execution"]["branch"] = json!("separate");
        assert!(matches(draft.clone()));
        draft["execution"]["ownerSessionId"] = json!("source");
        assert!(!matches(draft));
    }
}
