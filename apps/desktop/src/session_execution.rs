use crate::{
    app::HiveApp,
    language::tr,
    palette::Color,
    protocol::{Backend, Session},
    transport::Runtime,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputState},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingWorktree {
    session_id: String,
    branch: String,
    path: String,
    state: String,
    backend: Option<String>,
    origin_agent_id: Option<String>,
    identity_recorded: Option<bool>,
    recovery_kind: Option<String>,
}
impl PendingWorktree {
    fn recoverable(&self) -> bool {
        if self.session_id.is_empty()
            || self.path.is_empty()
            || self.branch.is_empty()
            || !matches!(self.state.as_str(), "creating" | "ready")
            || self
                .origin_agent_id
                .as_ref()
                .is_some_and(|id| id.is_empty())
        {
            return false;
        }
        match self.recovery_kind.as_deref() {
            Some("new-orchestrator") => {
                self.origin_agent_id.is_none()
                    && self.identity_recorded == Some(false)
                    && self.backend.as_deref().is_none_or(valid_backend)
            }
            Some(_) => false,
            None => {
                self.identity_recorded != Some(false)
                    && self.backend.as_deref().is_some_and(valid_backend)
            }
        }
    }
    fn origin_label(&self) -> &str {
        self.origin_agent_id.as_deref().unwrap_or_else(|| {
            if self.recovery_kind.as_deref() == Some("new-orchestrator") {
                tr("新主控")
            } else if self.identity_recorded == Some(false) {
                tr("原归属未记录")
            } else {
                tr("尚未绑定的会话")
            }
        })
    }
    fn action_key(&self) -> &'static str {
        if self.recovery_kind.as_deref() == Some("new-orchestrator") {
            "作为新主控会话恢复（不恢复旧对话）"
        } else if self.origin_agent_id.is_some() {
            "恢复成员独立会话"
        } else {
            "恢复此工作树会话"
        }
    }
}
#[derive(Default, Deserialize)]
struct PendingPage {
    pending: Vec<PendingWorktree>,
    unreadable: usize,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberHistory {
    #[serde(flatten)]
    session: Session,
    execution_available: bool,
    execution_error: Option<ExecutionError>,
}
#[derive(Clone, Deserialize)]
struct ExecutionError {
    code: String,
    message: String,
}
#[derive(Default, Deserialize)]
struct MemberHistoryPage {
    sessions: Vec<MemberHistory>,
}
fn member_history_matches(page: &MemberHistoryPage, workspace: &str, member: Option<&str>) -> bool {
    member.is_some()
        && page.sessions.iter().all(|item| {
            !item.session.id.is_empty()
                && item.session.workspace_id == workspace
                && item.session.origin_agent_id.as_deref() == member
                && item.session.identity_state.as_deref() == Some("known")
                && item.session.execution.as_ref().is_some_and(|execution| {
                    execution.kind == "worktree"
                        && !execution.path.is_empty()
                        && !execution.branch.is_empty()
                })
        })
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionResult {
    #[serde(flatten)]
    session: Session,
    recovery_kind: Option<String>,
}
#[derive(Clone)]
struct MemberTarget {
    id: String,
    name: String,
    command: Option<String>,
    model: Option<(String, String)>,
    role: String,
    description: String,
    profile: Option<crate::agent_profiles::ProfileRef>,
    backend_label: String,
    model_label: String,
    open_request: u64,
    session_id: Option<String>,
}
fn valid_backend(backend: &str) -> bool {
    matches!(backend, "pi" | "codex" | "pi-rpc" | "claude" | "acp")
}
fn backend_key(backend: Backend) -> &'static str {
    match backend {
        Backend::Pi => "pi",
        Backend::Codex => "codex",
        Backend::PiRpc => "pi-rpc",
        Backend::Claude => "claude",
        Backend::Acp => "acp",
    }
}
fn request_params(
    workspace: &str,
    backend: &str,
    branch: &str,
    pending: Option<&PendingWorktree>,
    member: Option<&str>,
) -> Result<(&'static str, Value), String> {
    if let Some(item) = pending {
        if !item.recoverable() {
            return Err(tr("此记录缺少可信的创建身份，不能猜测成员或后端；代码仍保留。").into());
        }
        let selected_backend = item.backend.as_deref().unwrap_or(
            if item.recovery_kind.as_deref() == Some("new-orchestrator") {
                backend
            } else {
                tr("原后端未记录")
            },
        );
        if !valid_backend(selected_backend) {
            return Err(tr("原工作树后端不受支持，未替换其身份。").into());
        }
        let mut params =
            json!({"workspaceId":workspace,"sessionId":item.session_id,"backend":selected_backend});
        if let Some(origin) = &item.origin_agent_id {
            params["memberId"] = json!(origin);
        }
        Ok(("chat.worktrees.recover", params))
    } else {
        let (method, mut params) = if let Some(member) = member {
            (
                "member.worktree.create",
                json!({"workspaceId":workspace,"memberId":member}),
            )
        } else {
            (
                "chat.createRecord",
                json!({"workspaceId":workspace,"backend":backend,"isolation":"worktree"}),
            )
        };
        if !branch.trim().is_empty() {
            params["branch"] = json!(branch.trim());
        }
        Ok((method, params))
    }
}
fn response_session(
    value: Value,
    workspace: &str,
    expected: Option<&str>,
    member: Option<&str>,
    backend: Option<&str>,
    recovery_kind: Option<&str>,
) -> Result<Session, String> {
    let result: SessionResult = serde_json::from_value(value).map_err(|error| error.to_string())?;
    let session = result.session;
    if session.workspace_id != workspace
        || expected.is_some_and(|id| session.id != id)
        || session.id.is_empty()
        || member.is_some_and(|id| {
            session.origin_agent_id.as_deref() != Some(id)
                || session.identity_state.as_deref() != Some("known")
        })
        || backend.is_some_and(|backend| backend_key(session.backend) != backend)
        || recovery_kind.is_some_and(|kind| result.recovery_kind.as_deref() != Some(kind))
        || !session.execution.as_ref().is_some_and(|execution| {
            execution.kind == "worktree"
                && !execution.path.is_empty()
                && !execution.branch.is_empty()
        })
    {
        return Err(tr("工作树会话响应与原项目或会话身份不匹配，未切换当前会话。").into());
    }
    Ok(session)
}
fn member_context_matches(
    target: &MemberTarget,
    member: Option<&crate::protocol::Member>,
    open_request: u64,
    session_id: Option<&str>,
) -> bool {
    target.open_request == open_request
        && target.session_id.as_deref() == session_id
        // Roster poll generations identify reads, not changes to this Worker.
        // Status/workload may change while a new conversation is being created.
        && member.is_some_and(|member| {
            member.id == target.id
                && member.role == target.role
                && member.description == target.description
                && member.configured_command == target.command
                && member.desktop_profile == target.profile
                && member.desktop_model.as_ref().map(|model| (&model.provider, &model.id))
                    == target.model.as_ref().map(|(provider, id)| (provider, id))
        })
}

struct WorktreeDialog {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    workspace: String,
    connection: u64,
    branch: Entity<InputState>,
    backend: &'static str,
    profiles: Vec<crate::agent_profiles::Profile>,
    profile: Option<crate::agent_profiles::ProfileRef>,
    skip_setup: bool,
    setup: Option<Result<(String, Option<(String, String)>), String>>,
    setup_loading: bool,
    member: Option<MemberTarget>,
    recovering: bool,
    member_history: bool,
    page: PendingPage,
    history: MemberHistoryPage,
    generation: u64,
    busy: bool,
    loading: bool,
    error: Option<String>,
    notice: Option<&'static str>,
    scroll: ScrollHandle,
}
impl HiveApp {
    pub(crate) fn open_session_execution(
        &mut self,
        project: Option<String>,
        recovering: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing || window.has_active_dialog(cx) {
            return;
        }
        let Some(workspace) = project.or_else(|| self.workspace.clone()) else {
            return;
        };
        if self.workspace_removing.as_ref() == Some(&workspace)
            || !self.workspaces.iter().any(|item| item.id == workspace)
        {
            return;
        }
        if self.workspace.as_ref() != Some(&workspace) {
            self.select_workspace(workspace.clone(), cx);
        }
        self.show_worktree_dialog(workspace, None, recovering, false, window, cx);
    }
    pub(crate) fn open_member_worktree(
        &mut self,
        workspace: String,
        member_id: String,
        history: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace.as_ref() != Some(&workspace) || !self.workspace_writes_allowed() {
            return;
        }
        let Some(member) = self.members.iter().find(|member| member.id == member_id) else {
            return;
        };
        if !history
            && (crate::members::command_backend(member.configured_command.as_deref()).is_none() || member.desktop_profile.is_none() || member.desktop_launch_error.is_some())
        {
            self.error = Some("成员配置引用不可用，无法创建工作树会话".into());
            cx.notify();
            return;
        }
        let target = MemberTarget {
            id: member.id.clone(),
            name: member.name.clone(),
            command: member.configured_command.clone(),
            model: member
                .desktop_model
                .as_ref()
                .map(|model| (model.provider.clone(), model.id.clone())),
            profile: member.desktop_profile.clone(),
            backend_label: member.backend_label().into(),
            model_label: member
                .desktop_model
                .as_ref()
                .map(|model| format!("{} / {}", model.provider, model.id))
                .unwrap_or_else(|| tr("成员默认模型").into()),
            role: member.role.clone(),
            description: member.description.clone(),
            open_request: self.open_request,
            session_id: self.session.as_ref().map(|session| session.id.clone()),
        };
        self.show_worktree_dialog(workspace, Some(target), false, history, window, cx);
    }
    fn show_worktree_dialog(
        &mut self,
        workspace: String,
        member: Option<MemberTarget>,
        recovering: bool,
        history: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing || window.has_active_dialog(cx) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        let backend = self
            .session
            .as_ref()
            .map(|session| backend_key(session.backend))
            .unwrap_or("pi");
        let profiles = self.profiles.snapshot.as_ref().map(|s| s.profiles.clone()).unwrap_or_default();
        let app = cx.weak_entity();
        let editor = cx.new(|cx: &mut Context<WorktreeDialog>| {
            let branch =
                cx.new(|cx| InputState::new(window, cx).placeholder(tr("留空自动分配分支")));
            if let Some(parent) = app.upgrade() {
                cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            }
            let editor = WorktreeDialog {
                app,
                runtime,
                workspace,
                connection,
                branch,
                backend,
                profiles,
                profile: None,
                skip_setup: false,
                setup: None,
                setup_loading: false,
                member,
                recovering,
                member_history: history,
                page: PendingPage::default(),
                history: MemberHistoryPage::default(),
                generation: 0,
                busy: false,
                loading: false,
                error: None,
                notice: None,
                scroll: ScrollHandle::new(),
            };
            if !recovering && !history {
                let pending = cx.weak_entity();
                cx.defer(move |cx| {
                    let _ = pending.update(cx, |editor, cx| editor.load_setup(cx));
                });
            }
            if recovering || history {
                let pending = cx.weak_entity();
                cx.defer(move |cx| {
                    let _ = pending.update(cx, |editor, cx| editor.load_list(history, cx));
                });
            }
            editor
        });
        let cancel = editor.clone();
        let focus = editor.read(cx).branch.read(cx).focus_handle(cx);
        window.open_dialog(cx, move |dialog, _, cx| {
            let cancel = cancel.clone();
            let busy = editor.read(cx).busy;
            dialog
                .title(tr("独立工作树会话"))
                .w(px(620.))
                .overlay_closable(false)
                .keyboard(!busy)
                .close_button(!busy)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !cancel.read(cx).busy)
                .child(editor.clone())
        });
        if !recovering && !history {
            let dialog_focus = window.focused(cx);
            window.on_next_frame(move |window, cx| {
                if dialog_focus.is_some_and(|root| root.contains_focused(window, cx)) {
                    focus.focus(window, cx);
                }
            });
        }
    }
}
impl WorktreeDialog {
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|entity| {
            let app = entity.read(cx);
            app.connected
                && !app.closing
                && app.connection_generation == self.connection
                && app.workspace.as_ref() == Some(&self.workspace)
                && app.workspace_removing.as_ref() != Some(&self.workspace)
                && app
                    .runtime
                    .as_ref()
                    .is_some_and(|runtime| Arc::ptr_eq(runtime, &self.runtime))
                && self.member.as_ref().is_none_or(|target| {
                    app.member_panel
                        && member_context_matches(
                            target,
                            app.members.iter().find(|member| member.id == target.id),
                            app.open_request,
                            app.session.as_ref().map(|session| session.id.as_str()),
                        )
                })
        })
    }
    fn load_setup(&mut self, cx: &mut Context<Self>) {
        if self.setup_loading || self.busy || !self.current(cx) {
            return;
        }
        self.setup_loading = true;
        self.setup = None;
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("script.settings.list", json!({"workspaceId":workspace}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<crate::project_scripts::model::Snapshot>(value)
                        .map_err(|e| e.to_string())
                })
                .and_then(|snapshot| snapshot.validate())
                .map(|snapshot| {
                    (
                        snapshot.revision,
                        snapshot
                            .scripts
                            .into_iter()
                            .find(|script| script.run_on_worktree_create)
                            .map(|script| (script.name, script.version)),
                    )
                });
            let _ = this.update(cx, |this, cx| {
                this.setup_loading = false;
                if this.current(cx) {
                    this.setup = Some(result);
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn load_list(&mut self, history: bool, cx: &mut Context<Self>) {
        if self.busy || self.loading || !self.current(cx) {
            return;
        }
        self.member_history = history;
        self.recovering = !history;
        self.loading = true;
        self.error = None;
        self.notice = None;
        self.generation += 1;
        let generation = self.generation;
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        let member = self.member.as_ref().map(|member| member.id.clone());
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| {
                    if this.generation == generation && this.current(cx) {
                        true
                    } else {
                        if this.generation == generation {
                            this.loading = false;
                            cx.notify();
                        }
                        false
                    }
                })
                .ok()
                != Some(true)
            {
                return;
            }
            let (method, params) = if history {
                (
                    "member.worktrees",
                    json!({"workspaceId":workspace,"memberId":member}),
                )
            } else {
                ("chat.worktrees.pending", json!({"workspaceId":workspace}))
            };
            let result = runtime.request(method, params).await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading = false;
                if !this.current(cx) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(value) if history => {
                        match serde_json::from_value::<MemberHistoryPage>(value) {
                            Ok(page)
                                if member_history_matches(&page, &workspace, member.as_deref()) =>
                            {
                                this.history = page
                            }
                            Ok(_) => {
                                this.error = Some(
                                    tr("工作树会话响应与原项目或会话身份不匹配，未切换当前会话。")
                                        .into(),
                                )
                            }
                            Err(error) => this.error = Some(error.to_string()),
                        }
                    }
                    Ok(value) => match serde_json::from_value::<PendingPage>(value) {
                        Ok(page) => this.page = page,
                        Err(error) => this.error = Some(error.to_string()),
                    },
                    Err(error) => this.error = Some(crate::language::localize_owned_error(&error)),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn submit(&mut self, pending: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy
            || self.loading
            || !self.current(cx)
            || (pending.is_none() && (self.setup_loading || !matches!(self.setup, Some(Ok(_)))))
        {
            return;
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let open_request = app.read(cx).open_request;
        let item = pending
            .as_ref()
            .and_then(|id| self.page.pending.iter().find(|item| &item.session_id == id))
            .cloned();
        if pending.is_some() && item.is_none() {
            self.error = Some(tr("记录已变化，请重新读取。").into());
            cx.notify();
            return;
        }
        let member = item
            .as_ref()
            .and_then(|item| item.origin_agent_id.clone())
            .or_else(|| self.member.as_ref().map(|member| member.id.clone()));
        let expected_backend = item
            .as_ref()
            .map(|item| item.backend.as_deref().unwrap_or(self.backend).to_owned())
            .or_else(|| {
                self.member.as_ref().and_then(|member| {
                    crate::members::command_backend(member.command.as_deref()).map(str::to_owned)
                })
            })
            .or_else(|| Some(self.backend.to_owned()));
        let recovery_kind = item.as_ref().and_then(|item| item.recovery_kind.clone());
        let (method, mut params) = match request_params(
            &self.workspace,
            self.backend,
            &self.branch.read(cx).value(),
            item.as_ref(),
            self.member.as_ref().map(|member| member.id.as_str()),
        ) {
            Ok(request) => request,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        if item.is_none() && self.member.is_none() && self.backend == "acp" && self.profile.is_none() { self.error=Some("请选择已启用的 ACP 配置".into()); cx.notify(); return; }
        if item.is_none() {
            if let Some(profile) = self.member.as_ref().and_then(|m| m.profile.as_ref()).or(self.profile.as_ref()) { params["profile"] = json!(profile); }
            let Some(Ok((revision, _))) = &self.setup else {
                return;
            };
            params["expectedSetupRevision"] = json!(revision);
            if self.skip_setup {
                params["skipSetup"] = json!(true);
            }
        }
        let expected_profile = if item.is_none() { self.member.as_ref().and_then(|m| m.profile.clone()).or(self.profile.clone()) } else { None };
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        let window = window.window_handle();
        self.busy = true;
        self.error = None;
        self.notice = None;
        self.generation += 1;
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| {
                    if this.generation == generation
                        && this.current(cx)
                        && this
                            .app
                            .upgrade()
                            .is_some_and(|app| app.read(cx).open_request == open_request)
                    {
                        true
                    } else {
                        if this.generation == generation {
                            this.busy = false;
                            cx.notify();
                        }
                        false
                    }
                })
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime.request(method, params).await.and_then(|value| {
                let session = response_session(
                    value,
                    &workspace,
                    pending.as_deref(),
                    member.as_deref(),
                    expected_backend.as_deref(),
                    recovery_kind.as_deref(),
                )?;
                if expected_profile.as_ref().is_some_and(|reference| session.profile.as_ref() != Some(reference)) { return Err(tr("创建会话返回的配置引用不匹配").into()); }
                Ok(session)
            });
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                if !this.current(cx)
                    || this
                        .app
                        .upgrade()
                        .is_none_or(|app| app.read(cx).open_request != open_request)
                {
                    this.notice =
                        Some("操作已结束，当前视图已变化；请在原项目的会话或待恢复列表查看结果。");
                    cx.notify();
                    return;
                }
                match result {
                    Ok(session) => {
                        let accepted = this
                            .app
                            .update(cx, |app, cx| app.accept_created_session(session, cx))
                            .unwrap_or(false);
                        if accepted {
                            let _ = window.update(cx, |_, window, cx| window.close_dialog(cx));
                        }
                    }
                    Err(error) => this.error = Some(crate::language::localize_owned_error(&error)),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn open_history(&mut self, session_id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.loading || !self.current(cx) {
            return;
        }
        let Some(item) = self
            .history
            .sessions
            .iter()
            .find(|item| item.session.id == session_id && item.execution_available)
            .cloned()
        else {
            return;
        };
        // Delay the parent update until the child's click update has released its lease.
        self.busy = true;
        self.generation += 1;
        let generation = self.generation;
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                if !this.current(cx) {
                    cx.notify();
                    return;
                }
                let accepted = window
                    .update(cx, |_, window, cx| {
                        this.app.update(cx, |app, cx| {
                            app.sessions.retain(|session| session.id != item.session.id);
                            app.sessions.insert(0, item.session.clone());
                            app.open_member_session(item.session.id.clone(), window, cx);
                        })
                    })
                    .is_ok_and(|result| result.is_ok());
                if accepted {
                    let _ = window.update(cx, |_, window, cx| window.close_dialog(cx));
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
impl Render for WorktreeDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current(cx);
        let disabled = !current || self.busy || self.loading;
        let setup_unavailable = self.setup_loading || !matches!(self.setup, Some(Ok(_)));
        let backend = self.backend;
        let recovering = self.recovering;
        let history = self.member_history;
        let show_backend = self.member.is_none()
            && (!recovering
                || self
                    .page
                    .pending
                    .iter()
                    .any(|item| item.recovery_kind.as_deref() == Some("new-orchestrator")));
        v_flex().gap_3().min_w_0()
            .child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr("从当前 HEAD 创建独立代码目录；团队工作区保持不变。已有原生会话不会迁移。")))
            .when_some(self.member.as_ref(),|view,member| view
                .child(div().text_sm().child(format!("{} · {} · {}",member.name,member.backend_label,member.model_label)))
                .child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr("继承此成员的后端与模型。创建后会打开并初始化原生会话，但不会切换此成员的当前会话。成员空闲后发送或继续才切换，后续派单才会到这里。"))))
            .when(!current,|view| view.child(div().id("worktree-target-changed").role(Role::Status).text_sm().child(tr("项目、成员、会话或后台连接已变化，请关闭后重新打开。"))))
            .when(show_backend,|view| view.when(recovering, |view| view.child(div().text_xs().child(tr("已记录的后端保持不变；此选择仅用于旧记录的新主控会话。")))).child(h_flex().gap_2().flex_wrap().children([("pi","内置 Pi"),("codex","Codex"),("pi-rpc","外部 Pi"),("claude","Claude Code"),("acp","ACP")].into_iter().enumerate().map(|(index,(value,label))|
                Button::new(("worktree-backend",index)).outline().small().label(tr(label)).selected(backend == value).disabled(disabled)
                    .on_click(cx.listener(move |this,_,_,cx| {this.backend = value;this.profile = None;cx.notify();}))))))
            .when(show_backend && !recovering, |view| view.child(h_flex().flex_wrap().gap_1().children(self.profiles.iter().filter(|p|p.enabled && p.driver == backend).enumerate().map(|(index,p)| {
                let reference=crate::agent_profiles::ProfileRef{id:p.id.clone(),revision:p.current_revision};
                let selected=self.profile.as_ref()==Some(&reference);
                Button::new(("worktree-profile",index)).small().label(format!("{} · r{}",p.label,p.current_revision)).selected(selected).disabled(disabled).on_click(cx.listener(move |this,_,_,cx|{this.profile=Some(reference.clone());cx.notify();}))
            }))))
            .when(!recovering && !history,|view| view
                 .child(div().id("worktree-setup-selection").role(Role::Status).text_sm().text_color(Color::Muted.get(cx)).child(match &self.setup {
                    Some(Ok((_, Some((name, version))))) => format!("{}: {} · {}: {}. {}", tr("工作树初始化脚本"), name, tr("版本"), version, tr(if self.skip_setup { "本次创建将跳过此脚本。" } else { "新会话会等待脚本完成后才能发送消息。" })),
                    Some(Ok((_, None))) => tr("此项目没有启用工作树初始化脚本。").to_owned(),
                    Some(Err(_)) => tr("无法读取初始化设置，请重试后创建。").to_owned(),
                    None => tr("正在读取初始化设置…").to_owned(),
                }))
                .child(Button::new("reload-worktree-setup").outline().small().label(tr("刷新初始化设置")).disabled(disabled || self.setup_loading).on_click(cx.listener(|this,_,_,cx| this.load_setup(cx))))
                .child(Button::new("skip-new-worktree-setup").outline().small().selected(self.skip_setup).disabled(disabled).label(tr(if self.skip_setup {"本次跳过初始化：是"} else {"本次跳过初始化：否"})).on_click(cx.listener(|this,_,_,cx| {this.skip_setup = !this.skip_setup; cx.notify();})))
                .child(div().text_sm().child(tr("分支名（可选）"))).child(Input::new(&self.branch).disabled(disabled))
                .child(Button::new("create-isolated-session").primary().label(tr(if self.busy {"正在创建工作树…"} else if self.member.is_some() {"新建独立会话"} else {"新建独立工作树会话"}))
                    .disabled(disabled || setup_unavailable).on_click(cx.listener(|this,_,window,cx| this.submit(None,window,cx)))))
            .when(recovering,|view| view
                .child(div().text_sm().child(tr("按已保存的原成员与后端恢复目录；缺失或损坏的创建身份不会被替换，代码仍保留。")))
                .when(!self.loading && self.page.pending.is_empty(),|view| view.child(div().text_sm().child(tr("没有待恢复的工作树。"))))
                .when(self.page.unreadable > 0,|view| view.child(div().text_sm().text_color(Color::Error.get(cx)).child(format!("{} ({})",tr("部分工作树记录暂时无法读取，可重新读取或在设置中复制诊断信息。"),self.page.unreadable))))
                .child(v_flex().id("pending-worktrees").gap_2().min_w_0().max_h(px(300.).min(window.viewport_size().height * 0.4)).overflow_y_scroll().track_scroll(&self.scroll).vertical_scrollbar(&self.scroll)
                    .children(self.page.pending.clone().into_iter().enumerate().map(|(index,item)| {
                        let id = item.session_id.clone();
                        v_flex().gap_1().p_2().min_w_0()
                            .child(div().text_sm().child(item.branch.clone())).child(div().text_xs().text_color(Color::Muted.get(cx)).child(item.path.clone()))
                            .child(div().text_xs().child(format!("{} · {}",item.origin_label(),item.backend.as_deref().unwrap_or(if item.recovery_kind.as_deref() == Some("new-orchestrator") {backend} else {tr("原后端未记录")}))))
                            .child(div().text_xs().child(tr(if item.state == "creating" {"创建中断，等待核验恢复"} else {"等待领取"})))
                            .when(!item.recoverable(),|view| view.child(div().text_xs().text_color(Color::Error.get(cx)).child(tr("此记录缺少可信的创建身份，不能猜测成员或后端；代码仍保留。"))))
                            .child(Button::new(("recover-worktree",index)).outline().small().label(tr(item.action_key())).disabled(disabled || !item.recoverable())
                                .on_click(cx.listener(move |this,_,window,cx| this.submit(Some(id.clone()),window,cx))))
                    }))))
            .when(history,|view| view
                .when(!self.loading && self.history.sessions.is_empty(),|view| view.child(div().text_sm().child(tr("此成员还没有独立会话。"))))
                .child(v_flex().id("member-worktree-history").gap_2().min_w_0().max_h(px(300.).min(window.viewport_size().height * 0.4)).overflow_y_scroll().track_scroll(&self.scroll).vertical_scrollbar(&self.scroll)
                    .children(self.history.sessions.clone().into_iter().enumerate().map(|(index,item)| {
                        let id = item.session.id.clone();let current_binding = item.session.agent_id.as_ref().is_some_and(|id| self.member.as_ref().is_some_and(|member| &member.id == id));
                        v_flex().gap_1().p_2().min_w_0()
                            .child(div().text_sm().child(format!("{} · {} · {}",item.session.title,tr(item.session.backend.label()),tr(if current_binding {"当前绑定"} else {"历史会话"}))))
                            .when_some(item.session.execution.as_ref(),|view,execution| view.child(div().text_xs().text_color(Color::Muted.get(cx)).child(format!("{} · {}",execution.branch,execution.path))))
                            .when(!item.execution_available,|view| view.child(div().text_xs().text_color(Color::Error.get(cx)).child(item.execution_error.as_ref().map(|error| format!("[{}] {}",error.code,error.message)).unwrap_or_else(|| tr("原工作树不可用；恢复原目录后再打开，不会改用工作区目录。").into()))))
                            .child(Button::new(("open-member-worktree",index)).outline().small().label(tr("打开原会话")).disabled(disabled || !item.execution_available)
                                .on_click(cx.listener(move |this,_,window,cx| this.open_history(id.clone(),window,cx))))
                    }))))
            .when(self.loading,|view| view.child(div().id("worktree-list-loading").role(Role::Status).child(tr("正在读取工作树会话…"))))
            .when_some(self.error.clone(),|view,error| view.child(div().id("worktree-error").role(Role::Alert).text_sm().text_color(Color::Error.get(cx)).child(error)))
            .when_some(self.notice,|view,notice| view.child(div().id("worktree-notice").role(Role::Status).text_sm().child(tr(notice))))
            .child(h_flex().gap_2().flex_wrap()
                .child(Button::new("show-worktree-list").outline().small().label(tr(if self.member.is_some() {if history {"重新读取独立会话"} else {"独立会话历史"}} else if recovering {"重新读取待恢复工作树"} else {"查看待恢复工作树…"}))
                    .disabled(disabled).on_click(cx.listener(|this,_,_,cx| this.load_list(this.member.is_some(),cx))))
                .when(recovering || history,|view| view.child(Button::new("worktree-create-mode").outline().small().label(tr(if self.member.is_some() {"新建独立会话"} else {"新建独立工作树会话"})).disabled(disabled)
                    .on_click(cx.listener(|this,_,_,cx| {this.recovering = false;this.member_history = false;this.error = None;this.notice = None;cx.notify();})))))
    }
}

#[cfg(test)]
mod tests {
    use super::{MemberHistoryPage, member_history_matches};
    use super::{
        MemberTarget, PendingWorktree, member_context_matches, request_params, response_session,
    };
    use crate::protocol::Session;
    use serde_json::json;
    #[test]
    fn creation_and_recovery_bind_member_or_saved_backend_without_caller_cwd() {
        let (method, params) =
            request_params("workspace", "codex", " feature/example ", None, None).unwrap();
        assert_eq!(method, "chat.createRecord");
        assert_eq!(
            params,
            json!({"workspaceId":"workspace","backend":"codex","isolation":"worktree","branch":"feature/example"})
        );
        let (method, params) = request_params(
            "workspace",
            "claude",
            " worker/topic ",
            None,
            Some("worker"),
        )
        .unwrap();
        assert_eq!(method, "member.worktree.create");
        assert_eq!(
            params,
            json!({"workspaceId":"workspace","memberId":"worker","branch":"worker/topic"})
        );
        assert!(params.get("backend").is_none());
        assert!(params.get("cwd").is_none());
        let saved:PendingWorktree=serde_json::from_value(json!({"sessionId":"s","branch":"b","path":"/saved","state":"ready","backend":"pi-rpc","originAgentId":"worker"})).unwrap();
        assert_eq!(
            request_params("workspace", "codex", "ignored", Some(&saved), None)
                .unwrap()
                .1,
            json!({"workspaceId":"workspace","sessionId":"s","backend":"pi-rpc","memberId":"worker"})
        );
        let unknown:PendingWorktree=serde_json::from_value(json!({"sessionId":"s","branch":"b","path":"/saved","state":"ready","identityRecorded":false})).unwrap();
        assert!(!unknown.recoverable());
        assert!(request_params("workspace", "pi", "", Some(&unknown), None).is_err());
        let legacy:PendingWorktree=serde_json::from_value(json!({"sessionId":"s","branch":"b","path":"/saved","state":"ready","identityRecorded":false,"recoveryKind":"new-orchestrator"})).unwrap();
        assert!(legacy.recoverable());
        assert_eq!(legacy.action_key(), "作为新主控会话恢复（不恢复旧对话）");
    }
    #[test]
    fn member_response_requires_original_identity_and_worktree() {
        let value = json!({"id":"s","title":"new","workspaceId":"w","agentId":null,"originAgentId":"worker","identityState":"known","backend":"codex","execution":{"kind":"worktree","path":"/owned","branch":"hive/s","ownerToken":"private"},"executionPath":"/owned"});
        let parsed = response_session(
            value.clone(),
            "w",
            Some("s"),
            Some("worker"),
            Some("codex"),
            None,
        )
        .unwrap();
        assert!(parsed.agent_id.is_none());
        assert_eq!(parsed.display_role_key(), "成员");
        assert_eq!(
            serde_json::to_value(parsed.execution.unwrap()).unwrap(),
            json!({"kind":"worktree","path":"/owned","branch":"hive/s"})
        );
        assert!(
            response_session(
                value.clone(),
                "other",
                Some("s"),
                Some("worker"),
                None,
                None
            )
            .is_err()
        );
        assert!(
            response_session(
                value.clone(),
                "w",
                Some("replacement"),
                Some("worker"),
                None,
                None
            )
            .is_err()
        );
        assert!(
            response_session(value.clone(), "w", None, Some("replacement"), None, None).is_err()
        );
        assert!(response_session(value, "w", None, Some("worker"), Some("pi"), None).is_err());
        let legacy: Session =
            serde_json::from_value(json!({"id":"old","title":"old","workspaceId":"w"})).unwrap();
        assert!(legacy.execution.is_none());
        assert_eq!(legacy.display_role_key(), "会话");
    }
    fn member_fixture() -> crate::protocol::Member {
        serde_json::from_value(json!({"id":"worker","name":"Worker","role":"coder","description":"Original responsibility","status":"idle","pending_task_count":0,"configured_command":"hive:codex","desktop_model":{"provider":"openai","id":"gpt-6.1-terra"}})).unwrap()
    }
    fn member_target() -> MemberTarget {
        MemberTarget {
            id: "worker".into(),
            name: "Worker".into(),
            command: Some("hive:codex".into()),
            model: Some(("openai".into(), "gpt-6.1-terra".into())),
            role: "coder".into(),
            description: "Original responsibility".into(),
            profile: None,
            backend_label: "Codex".into(),
            model_label: "Terra".into(),
            open_request: 5,
            session_id: Some("B".into()),
        }
    }
    #[test]
    fn normal_roster_polls_and_workload_updates_keep_member_dialog_current() {
        let target = member_target();
        for pending in 0..4 {
            // Each successful poll replaces the roster with fresh Member values.
            let mut refreshed = member_fixture();
            refreshed.status = if pending == 0 { "idle" } else { "working" }.into();
            refreshed.pending_task_count = pending;
            assert!(member_context_matches(
                &target,
                Some(&refreshed),
                5,
                Some("B")
            ));
        }
    }
    #[test]
    fn member_context_rejects_removal_role_launch_config_and_navigation_changes() {
        let target = member_target();
        let member = member_fixture();
        assert!(!member_context_matches(&target, None, 5, Some("B")));
        assert!(!member_context_matches(
            &target,
            Some(&member),
            6,
            Some("B")
        ));
        assert!(!member_context_matches(
            &target,
            Some(&member),
            5,
            Some("A")
        ));
        let mutations: [fn(&mut crate::protocol::Member); 7] = [
            |member| member.id = "replacement-worker".into(),
            |member| member.role = "reviewer".into(),
            |member| member.description = "Changed responsibility".into(),
            |member| member.configured_command = Some("hive:pi".into()),
            |member| member.desktop_model.as_mut().unwrap().provider = "other-provider".into(),
            |member| member.desktop_model.as_mut().unwrap().id = "other-model".into(),
            |member| member.desktop_model = None,
        ];
        for mutate in mutations {
            let mut changed = member.clone();
            mutate(&mut changed);
            assert!(!member_context_matches(
                &target,
                Some(&changed),
                5,
                Some("B")
            ));
        }
    }
    #[test]
    fn history_requires_capability_and_keeps_unavailable_source_as_an_error() {
        let value = json!({"sessions":[{"id":"A","title":"Worker history","workspaceId":"w","originAgentId":"worker","identityState":"known","execution":{"kind":"worktree","path":"/original-A","branch":"A"},"executionAvailable":false,"executionError":{"code":"WORKTREE_UNAVAILABLE","message":"Original directory missing"}}]});
        let page: MemberHistoryPage = serde_json::from_value(value.clone()).unwrap();
        assert!(member_history_matches(&page, "w", Some("worker")));
        assert!(!page.sessions[0].execution_available);
        assert_eq!(
            page.sessions[0].execution_error.as_ref().unwrap().code,
            "WORKTREE_UNAVAILABLE"
        );
        assert!(!member_history_matches(&page, "other", Some("worker")));
        assert!(!member_history_matches(&page, "w", Some("other")));
        let mut old_backend = value;
        old_backend["sessions"][0]
            .as_object_mut()
            .unwrap()
            .remove("executionAvailable");
        assert!(serde_json::from_value::<MemberHistoryPage>(old_backend).is_err());
        let unsupported:PendingWorktree=serde_json::from_value(json!({"sessionId":"s","branch":"b","path":"/saved","state":"ready","backend":"pi","recoveryKind":"future-recovery"})).unwrap();
        assert!(!unsupported.recoverable());
    }
    #[test]
    fn presentation_origin_never_becomes_a_current_binding() {
        for (origin, identity, role) in [
            ("w:worker", "known", "成员"),
            ("w:orchestrator", "known", "主控"),
            ("w:__workflow__", "known", "工作流"),
            ("w:worker", "unknown", "会话"),
        ] {
            let session:Session=serde_json::from_value(json!({"id":"s","workspaceId":"w","title":"新会话","originAgentId":origin,"identityState":identity})).unwrap();
            assert_eq!(session.display_role_key(), role);
            assert!(session.agent_id.is_none());
        }
    }
    #[test]
    fn unnamed_worker_title_and_legacy_recovery_warning_remain_explicit() {
        let mut worker:Session=serde_json::from_value(json!({"id":"A","workspaceId":"w","title":"新会话","originAgentId":"worker","identityState":"known"})).unwrap();
        assert_eq!(
            crate::view::untitled_session_label(&worker),
            Some(crate::language::tr("成员会话"))
        );
        worker.custom_title = true;
        assert!(crate::view::untitled_session_label(&worker).is_none());
        let value = json!({"id":"legacy","workspaceId":"w","title":"新会话","backend":"pi","identityState":"new","recoveryKind":"new-orchestrator","execution":{"kind":"worktree","path":"/original","branch":"legacy"}});
        assert!(
            response_session(
                value.clone(),
                "w",
                Some("legacy"),
                None,
                Some("pi"),
                Some("new-orchestrator")
            )
            .is_ok()
        );
        let mut missing = value;
        missing.as_object_mut().unwrap().remove("recoveryKind");
        assert!(
            response_session(
                missing,
                "w",
                Some("legacy"),
                None,
                Some("pi"),
                Some("new-orchestrator")
            )
            .is_err()
        );
    }
}
