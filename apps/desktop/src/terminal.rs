use crate::language::{english, tr};
mod connection;
mod events;
mod view;

use crate::{
    app::HiveApp,
    protocol::{TerminalFrame, TerminalRun, TerminalWireEvent},
};
use gpui_kit::*;
use hive_terminal::{TerminalConfig, TerminalEvent, TerminalView};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;

enum TerminalStatus {
    Message(&'static str),
    Exited(Option<i32>),
    Detached(String),
}
impl From<&'static str> for TerminalStatus {
    fn from(key: &'static str) -> Self {
        Self::Message(key)
    }
}
impl TerminalStatus {
    fn text(&self) -> String {
        match self {
            Self::Message(key) => tr(key).to_owned(),
            Self::Exited(code) => match code {
                Some(code) => {
                    if english() {
                        format!("Shell exited ({code})")
                    } else {
                        format!("Shell 已退出（{code}）")
                    }
                }
                None => tr("Shell 已退出").to_owned(),
            },
            Self::Detached(reason) => {
                if english() {
                    format!(
                        "Terminal view disconnected: {reason}. Reconnect to confirm Shell status."
                    )
                } else {
                    format!("终端视图已断开：{reason}；请重新连接确认 Shell 状态")
                }
            }
        }
    }
}
fn terminal_error_text(error: &str) -> String {
    [
        "终端恢复顺序错误，请重新连接",
        "终端快照未完整消费，未确认恢复",
        "终端输出顺序错误，请重新连接",
        "终端输出未完整消费，未发送确认",
        "终端操作未获确认，请重新连接",
        "终端连接响应身份不匹配",
        "未能确认 Shell 已停止，请刷新后查看状态",
        "Shell 列表格式错误",
        "Shell 启动响应格式错误",
        "后台未确认终端的工作区与会话身份，已拒绝连接。",
        "请先刷新并确认当前会话终端身份，再启动 Shell。",
    ]
    .into_iter()
    .find(|key| *key == error)
    .map(|key| tr(key).to_owned())
    .unwrap_or_else(|| crate::language::localize_owned_error(error))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminalMode {
    Auto,
    Workspace,
    Session,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalTarget {
    workspace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}
impl TerminalTarget {
    fn matches_scope(&self, workspace: &str, session: Option<&str>) -> bool {
        self.workspace_id == workspace && self.session_id.as_deref() == session
    }
}
fn selected_target(
    mode: TerminalMode,
    workspace: Option<&str>,
    session: Option<&crate::protocol::Session>,
) -> Option<TerminalTarget> {
    let workspace = workspace?;
    let session = session.filter(|session| session.workspace_id == workspace);
    let session_id = match mode {
        TerminalMode::Workspace => None,
        TerminalMode::Session => Some(session?.id.clone()),
        TerminalMode::Auto => session
            .filter(|session| {
                session.scratch || session.execution.as_ref()
                    .is_some_and(|execution| execution.kind == "worktree")
            })
            .map(|session| session.id.clone()),
    };
    Some(TerminalTarget {
        workspace_id: workspace.into(),
        session_id,
    })
}
#[derive(Clone)]
struct TerminalRequestTarget {
    target: TerminalTarget,
    runtime: Arc<crate::transport::Runtime>,
    connection: u64,
    generation: u64,
}
pub struct WorkspaceTerminal {
    pub visible: bool,
    workspace: Option<String>,
    session_id: Option<String>,
    mode: TerminalMode,
    runtime: Option<Arc<crate::transport::Runtime>>,
    connection: u64,
    scope_confirmed: bool,
    source_unavailable: Option<String>,
    runs: Vec<TerminalRun>,
    run_id: Option<String>,
    attachment: Option<String>,
    generation: u64,
    sequence: u64,
    ready: bool,
    input_enabled: bool,
    window: Option<AnyWindowHandle>,
    focus_anchor: Option<FocusHandle>,
    loading: bool,
    cols: usize,
    rows: usize,
    status: TerminalStatus,
    error: Option<String>,
    title: String,
    view: Entity<TerminalView>,
    _subscription: Subscription,
}
impl WorkspaceTerminal {
    pub fn new(cx: &mut Context<HiveApp>) -> Self {
        let view = cx.new(|cx| TerminalView::new(TerminalConfig::default(), cx));
        let subscription = cx.subscribe(&view, |this, source, event: &TerminalEvent, cx| {
            if source.entity_id() == this.terminal.view.entity_id() {
                this.terminal_component_event(event, cx);
            }
        });
        Self {
            visible: false,
            workspace: None,
            session_id: None,
            mode: TerminalMode::Auto,
            runtime: None,
            connection: 0,
            scope_confirmed: false,
            source_unavailable: None,
            runs: vec![],
            run_id: None,
            attachment: None,
            generation: 0,
            sequence: 0,
            ready: false,
            input_enabled: false,
            window: None,
            focus_anchor: None,
            loading: false,
            cols: 80,
            rows: 24,
            status: "尚未连接 Shell".into(),
            error: None,
            title: String::new(),
            view,
            _subscription: subscription,
        }
    }
}
impl HiveApp {
    fn desired_terminal_target(&self) -> Option<TerminalTarget> {
        if self.current_worktree_released() {
            return None;
        }
        selected_target(
            self.terminal.mode,
            self.workspace.as_deref(),
            self.session.as_ref(),
        )
    }
    fn terminal_scope_current(&self) -> bool {
        self.connected
            && self.terminal.connection == self.connection_generation
            && self
                .terminal
                .runtime
                .as_ref()
                .zip(self.runtime.as_ref())
                .is_some_and(|(old, now)| Arc::ptr_eq(old, now))
            && self.desired_terminal_target().is_some_and(|target| {
                self.terminal.workspace.as_ref() == Some(&target.workspace_id)
                    && self.terminal.session_id == target.session_id
            })
    }
    fn terminal_request_target(&self) -> Option<TerminalRequestTarget> {
        if !self.terminal_scope_current() || !self.terminal.visible {
            return None;
        }
        Some(TerminalRequestTarget {
            target: self.desired_terminal_target()?,
            runtime: self.runtime.clone()?,
            connection: self.connection_generation,
            generation: self.terminal.generation,
        })
    }
    fn terminal_request_current(&self, request: &TerminalRequestTarget) -> bool {
        self.terminal.visible
            && self.terminal_scope_current()
            && self.connection_generation == request.connection
            && self.terminal.generation == request.generation
            && self.desired_terminal_target().as_ref() == Some(&request.target)
            && self
                .runtime
                .as_ref()
                .is_some_and(|runtime| Arc::ptr_eq(runtime, &request.runtime))
    }
    fn select_terminal_mode(&mut self, mode: TerminalMode, cx: &mut Context<Self>) {
        if self.terminal.mode == mode {
            return;
        }
        self.terminal.mode = mode;
        self.sync_terminal_workspace(cx);
        if self.terminal.visible
            && !self.terminal.loading
            && self.terminal.attachment.is_none()
            && self.desired_terminal_target().is_some()
        {
            self.load_terminals(true, cx);
        }
        cx.notify();
    }
    pub fn show_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.current_worktree_released() {
            return;
        }
        self.close_workflows();
        self.close_memory();
        self.close_roles();
        self.close_settings();
        self.close_collaboration();
        self.member_panel = false;
        self.terminal.visible = true;
        if window.viewport_size().width < px(820.) {
            self.sidebar_open = false;
        }
        self.remember_terminal_focus(window, cx);
        self.sync_terminal_workspace(cx);
        if self.workspace.is_some() && self.terminal.attachment.is_none() && !self.terminal.loading
        {
            self.load_terminals(true, cx);
        }
        if self.terminal.ready {
            self.terminal
                .view
                .read(cx)
                .focus_handle()
                .clone()
                .focus(window, cx);
        }
        cx.notify();
    }
    pub fn hide_terminal(&mut self, cx: &mut Context<Self>) {
        let was_visible = self.terminal.visible;
        self.terminal.visible = false;
        self.detach_terminal(cx);
        if was_visible {
            // Reopening restores the running Shell from the backend. Release the
            // hidden emulator's scrollback even if its attachment already ended.
            self.terminal.view.update(cx, |view, cx| view.reset(cx));
        }
        cx.notify();
    }
    pub fn prepare_terminal_view(&mut self, cx: &mut Context<Self>) {
        self.sync_terminal_typography(cx);
        self.sync_terminal_workspace(cx);
        self.set_terminal_input_enabled(self.can_type_in_terminal(), cx);
    }
    pub(crate) fn sync_terminal_typography(&mut self, cx: &mut Context<Self>) {
        let view = self.terminal.view.clone();
        crate::appearance::sync_terminal(&view, cx);
    }
    fn can_type_in_terminal(&self) -> bool {
        self.connected
            && self.terminal.visible
            && !self.roles.visible
            && !self.settings_panel
            && self.terminal.ready
            && self.terminal.source_unavailable.is_none()
            && self.terminal_scope_current()
            && self.workspace_writes_allowed()
    }
    pub(crate) fn remember_terminal_focus(&mut self, window: &mut Window, cx: &Context<Self>) {
        self.terminal.window = Some(window.window_handle());
        self.terminal.focus_anchor = window.focused(cx);
    }
    fn focus_ready_terminal(&mut self, cx: &mut Context<Self>) {
        let (Some(window), Some(attachment), Some(workspace), Some(run)) = (
            self.terminal.window,
            self.terminal.attachment.clone(),
            self.terminal.workspace.clone(),
            self.terminal.run_id.clone(),
        ) else {
            return;
        };
        let session = self.terminal.session_id.clone();
        let generation = self.terminal.generation;
        let anchor = self.terminal.focus_anchor.clone();
        let weak = cx.weak_entity();
        let _ = window.update(cx, |_, window, _| {
            window.on_next_frame(move |window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if this.terminal.generation != generation
                        || !this.terminal_identity(
                            &attachment,
                            &workspace,
                            session.as_deref(),
                            &run,
                        )
                        || !this.terminal.ready
                        || !this.workspace_writes_allowed()
                        || this.memory.visible
                        || this.roles.visible
                        || this.settings_panel
                        || this.collaboration.visible
                        || this.member_panel
                        || (window.viewport_size().width < px(820.) && this.sidebar_open)
                    {
                        return;
                    }
                    let target = this.terminal.view.read(cx).focus_handle().clone();
                    let focused = window.focused(cx);
                    if focused == anchor || focused.as_ref() == Some(&target) {
                        target.focus(window, cx);
                        this.terminal.focus_anchor = Some(target);
                    }
                });
            });
        });
    }
    // Idempotent view reconciliation catches workspace switches/removal through every existing caller.
    pub fn sync_terminal_workspace(&mut self, cx: &mut Context<Self>) {
        if self.current_worktree_released() {
            self.terminal.visible = false;
        }
        let visible = self.terminal.visible
            && !self.memory.visible
            && !self.collaboration.visible
            && !self.member_panel;
        let desired = self.desired_terminal_target();
        let desired_workspace = desired.as_ref().map(|target| target.workspace_id.clone());
        let desired_session = desired
            .as_ref()
            .and_then(|target| target.session_id.clone());
        let runtime_changed = match (&self.terminal.runtime, &self.runtime) {
            (Some(old), Some(now)) => !Arc::ptr_eq(old, now),
            (None, None) => false,
            _ => true,
        };
        let changed = self.terminal.workspace != desired_workspace
            || self.terminal.session_id != desired_session
            || self.terminal.connection != self.connection_generation
            || runtime_changed;
        if changed || !visible || !self.connected {
            if self.terminal.attachment.is_some() || self.terminal.loading {
                self.detach_terminal(cx);
            }
        }
        if changed {
            self.terminal.workspace = desired_workspace;
            self.terminal.session_id = desired_session;
            self.terminal.runtime = self.runtime.clone();
            self.terminal.connection = self.connection_generation;
            self.terminal.scope_confirmed = false;
            self.terminal.source_unavailable = None;
            self.terminal.runs.clear();
            self.terminal.run_id = None;
            self.terminal.title.clear();
            self.terminal.error = None;
            self.terminal.view.update(cx, |view, cx| view.reset(cx));
            self.terminal.status = "尚未连接 Shell".into();
        }
        if visible
            && self.connected
            && desired.is_some()
            && self.terminal.attachment.is_none()
            && !self.terminal.loading
            && changed
        {
            self.load_terminals(true, cx);
        }
    }
    fn detach_terminal(&mut self, cx: &mut Context<Self>) {
        self.terminal.generation += 1;
        self.terminal.ready = false;
        self.terminal.loading = false;
        self.set_terminal_input_enabled(false, cx);
        self.terminal.status = "已断开终端视图；未发送停止请求".into();
        if let (Some(attachment), Some(runtime)) = (
            self.terminal.attachment.take(),
            self.terminal.runtime.clone(),
        ) {
            let reply =
                runtime.begin_request("terminal.detach", json!({"attachmentId":attachment}));
            cx.spawn(async move |_, _| {
                let _ = reply.await;
            })
            .detach();
        }
    }
    pub fn terminal_disconnected(&mut self, cx: &mut Context<Self>) {
        self.terminal.generation += 1;
        self.terminal.attachment = None;
        self.terminal.scope_confirmed = false;
        self.terminal.source_unavailable = None;
        self.terminal.ready = false;
        self.terminal.loading = false;
        self.terminal.status = "后台已断开；Shell 状态未确认，终端输入已禁用".into();
        self.set_terminal_input_enabled(false, cx);
    }
    fn set_terminal_input_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.terminal.input_enabled == enabled {
            return;
        }
        self.terminal.input_enabled = enabled;
        self.terminal
            .view
            .update(cx, |view, cx| view.set_input_enabled(enabled, cx));
    }
    fn bind_terminal_view(&mut self, attachment: String, cx: &mut Context<Self>) {
        let config = TerminalConfig {
            cols: self.terminal.cols,
            rows: self.terminal.rows,
            ..Default::default()
        };
        let view = cx.new(|cx| TerminalView::new(config, cx));
        let subscription = cx.subscribe(&view, move |this, source, event: &TerminalEvent, cx| {
            if this.terminal.attachment.as_deref() == Some(&attachment)
                && source.entity_id() == this.terminal.view.entity_id()
            {
                this.terminal_component_event(event, cx);
            }
        });
        self.terminal.view = view;
        self.terminal.input_enabled = false;
        self.terminal._subscription = subscription;
    }
    fn terminal_identity(
        &self,
        attachment: &str,
        workspace: &str,
        session: Option<&str>,
        run: &str,
    ) -> bool {
        self.connected
            && self.terminal.visible
            && self.terminal.attachment.as_deref() == Some(attachment)
            && self.workspace.as_deref() == Some(workspace)
            && self.terminal.workspace.as_deref() == Some(workspace)
            && self
                .desired_terminal_target()
                .is_some_and(|target| target.matches_scope(workspace, session))
            && self.terminal_scope_current()
            && self.terminal.run_id.as_deref() == Some(run)
    }
    fn terminal_fault(&mut self, error: String, cx: &mut Context<Self>) {
        self.detach_terminal(cx);
        self.terminal.status = "终端视图已断开；Shell 状态需重新确认".into();
        self.terminal.error = Some(error);
    }
}

impl HiveApp {
    pub fn refresh_terminal_after_reconnect(&mut self, cx: &mut Context<Self>) {
        self.terminal.runs.clear();
        self.terminal.run_id = None;
        self.terminal.scope_confirmed = false;
        self.terminal.source_unavailable = None;
        self.sync_terminal_workspace(cx);
        self.terminal.status = "后台已恢复，正在核对 Shell 状态…".into();
        if self.terminal.visible {
            self.load_terminals(true, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{selected_target, TerminalMode, TerminalTarget, TerminalWireEvent};
    use serde_json::json;
    fn session(worktree: bool) -> crate::protocol::Session {
        serde_json::from_value(json!({"id":"s", "workspaceId":"w", "title":"session", "execution": if worktree { json!({"kind":"worktree","path":"/worktrees/s","branch":"test"}) } else { serde_json::Value::Null }})).unwrap()
    }
    #[test]
    fn auto_follows_private_session_directories_and_explicit_scope_overrides_it() {
        let isolated = session(true);
        let shared = session(false);
        let scratch: crate::protocol::Session = serde_json::from_value(json!({
            "id":"scratch", "workspaceId":"w", "title":"Scratch conversation", "scratch":true,
        })).unwrap();
        for session in [&isolated, &scratch] {
            let auto = selected_target(TerminalMode::Auto, Some("w"), Some(session)).unwrap();
            assert_eq!(auto.session_id.as_deref(), Some(session.id.as_str()));
            assert!(selected_target(TerminalMode::Workspace, Some("w"), Some(session))
                .unwrap()
                .session_id
                .is_none());
        }
        assert!(
            selected_target(TerminalMode::Auto, Some("w"), Some(&shared))
                .unwrap()
                .session_id
                .is_none()
        );
        assert_eq!(
            selected_target(TerminalMode::Session, Some("w"), Some(&shared))
                .unwrap()
                .session_id
                .as_deref(),
            Some("s")
        );
        assert!(selected_target(TerminalMode::Session, Some("w"), None).is_none());
        assert!(selected_target(TerminalMode::Session, Some("other"), Some(&isolated)).is_none());
    }
    #[test]
    fn wire_scope_never_accepts_missing_or_foreign_session_identity() {
        let target = TerminalTarget {
            workspace_id: "w".into(),
            session_id: Some("s".into()),
        };
        for (workspace, session, accepted) in [
            ("w", Some("s"), true),
            ("w", None, false),
            ("w", Some("other"), false),
            ("other", Some("s"), false),
        ] {
            let mut value = json!({"workspaceId":workspace,"attachmentId":"attachment","runId":"run","type":"output","sequence":2,"bytes":[65]});
            if let Some(session) = session {
                value["sessionId"] = json!(session);
            }
            let event: TerminalWireEvent = serde_json::from_value(value).unwrap();
            assert_eq!(
                target.matches_scope(&event.workspace_id, event.session_id.as_deref()),
                accepted
            );
        }
    }
}
