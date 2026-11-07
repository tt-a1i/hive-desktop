mod connection;
mod events;
mod view;

use crate::{
    app::HiveApp,
    protocol::{TerminalFrame, TerminalRun, TerminalWireEvent},
};
use gpui_kit::*;
use hive_terminal::{TerminalConfig, TerminalEvent, TerminalView};
use serde_json::json;

pub struct WorkspaceTerminal {
    pub visible: bool,
    workspace: Option<String>,
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
    status: String,
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
    pub fn show_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        self.terminal.visible = false;
        self.detach_terminal(cx);
        cx.notify();
    }
    pub fn prepare_terminal_view(&mut self, cx: &mut Context<Self>) {
        self.sync_terminal_workspace(cx);
        self.set_terminal_input_enabled(self.can_type_in_terminal(), cx);
    }
    fn can_type_in_terminal(&self) -> bool {
        self.connected
            && self.terminal.visible
            && !self.roles.visible
            && !self.settings_panel
            && self.terminal.ready
            && self.terminal.workspace == self.workspace
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
        let anchor = self.terminal.focus_anchor.clone();
        let weak = cx.weak_entity();
        let _ = window.update(cx, |_, window, _| {
            window.on_next_frame(move |window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if !this.terminal_identity(&attachment, &workspace, &run)
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
        let visible = self.terminal.visible
            && !self.memory.visible
            && !self.collaboration.visible
            && !self.member_panel;
        let changed = self.terminal.workspace != self.workspace;
        if changed || !visible || !self.connected {
            if self.terminal.attachment.is_some() || self.terminal.loading {
                self.detach_terminal(cx);
            }
        }
        if changed {
            self.terminal.workspace = self.workspace.clone();
            self.terminal.runs.clear();
            self.terminal.run_id = None;
            self.terminal.title.clear();
            self.terminal.error = None;
            self.terminal.view.update(cx, |view, cx| view.reset(cx));
            self.terminal.status = "尚未连接 Shell".into();
        }
        if visible
            && self.connected
            && self.workspace.is_some()
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
        if let (Some(attachment), Some(runtime)) =
            (self.terminal.attachment.take(), self.runtime.clone())
        {
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
    fn terminal_identity(&self, attachment: &str, workspace: &str, run: &str) -> bool {
        self.connected
            && self.terminal.visible
            && self.terminal.attachment.as_deref() == Some(attachment)
            && self.workspace.as_deref() == Some(workspace)
            && self.terminal.workspace.as_deref() == Some(workspace)
            && self.terminal.run_id.as_deref() == Some(run)
    }
    fn terminal_fault(&mut self, error: String, cx: &mut Context<Self>) {
        self.detach_terminal(cx);
        self.terminal.status = "终端视图已断开；Shell 状态需重新确认".into();
        self.terminal.error = Some(error);
    }
}
