mod details;
mod settings;
mod types;
mod view;
use crate::{app::HiveApp, language::tr};
use gpui_kit::*;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use types::*;

pub struct Workflows {
    pub visible: bool,
    pub loading: bool,
    pub acting: bool,
    pub generation: u64,
    pub error: Option<WorkflowError>,
    pub action_error: Option<WorkflowError>,
    pub enabled: Option<bool>,
    pub policy: Option<Policy>,
    pub policy_dirty: bool,
    pub settings_open: bool,
    pub runs: Vec<Run>,
    pub schedules: Vec<Schedule>,
    pub selected: Option<String>,
    pub detail: Option<Run>,
    pub dispatches: Vec<Dispatch>,
    pub logs: Vec<Log>,
    pub filter: &'static str,
    pub list_scroll: ScrollHandle,
    pub detail_scroll: ScrollHandle,
    pub deleting: Option<String>,
}
pub enum WorkflowError { Refresh(String), Action(String), Unconfirmed }
impl WorkflowError {
    fn text(&self) -> String {
        match self {
            Self::Refresh(error) => format!("{}: {error}", tr("工作流未能更新，保留上次结果")),
            Self::Action(error) => format!("{}: {error}", tr("工作流操作失败，请刷新核对")),
            Self::Unconfirmed => tr("操作结果尚未确认，请刷新后核对；执行器草稿已保留。").to_owned(),
        }
    }
}
impl Default for Workflows {
    fn default() -> Self {
        Self {
            visible: false,
            loading: false,
            acting: false,
            generation: 0,
            error: None,
            action_error: None,
            enabled: None,
            policy: None,
            policy_dirty: false,
            settings_open: false,
            runs: vec![],
            schedules: vec![],
            selected: None,
            detail: None,
            dispatches: vec![],
            logs: vec![],
            filter: "all",
            list_scroll: ScrollHandle::new(),
            detail_scroll: ScrollHandle::new(),
            deleting: None,
        }
    }
}
async fn read<T: DeserializeOwned>(
    runtime: &crate::transport::Runtime,
    path: String,
) -> Result<T, String> {
    let value = runtime
        .request("hive.request", json!({"method":"GET","path":path}))
        .await?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}
struct Snapshot {
    runs: Vec<Run>,
    schedules: Vec<Schedule>,
    enabled: bool,
    policy: Policy,
    detail: Option<(Run, Vec<Dispatch>, Vec<Log>)>,
}
impl HiveApp {
    pub(crate) fn close_workflows(&mut self) {
        self.workflows.visible = false;
        self.invalidate_workflows();
    }
    pub(crate) fn invalidate_workflows(&mut self) {
        let state = &mut self.workflows;
        state.generation += 1;
        state.loading = false;
        if state.acting {
            state.action_error = Some(WorkflowError::Unconfirmed);
        }
        state.acting = false;
    }
    pub(crate) fn reset_workflows_workspace(&mut self) {
        self.invalidate_workflows();
        let state = &mut self.workflows;
        state.runs.clear();
        state.schedules.clear();
        state.selected = None;
        state.detail = None;
        state.dispatches.clear();
        state.logs.clear();
        state.deleting = None;
        state.filter = "all";
        state.list_scroll = ScrollHandle::new();
        state.detail_scroll = ScrollHandle::new();
        state.error = None;
        state.action_error = None;
    }
    pub(crate) fn show_workflows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hide_terminal(cx);
        self.close_memory();
        self.close_roles();
        self.close_settings();
        self.close_collaboration();
        self.member_panel = false;
        self.workflows.visible = true;
        if window.viewport_size().width < px(820.) {
            self.sidebar_open = false;
        }
        self.refresh_workflows(cx);
        cx.notify();
    }
    pub(crate) fn select_workflow(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.invalidate_workflows();
        let state = &mut self.workflows;
        state.selected = id;
        state.detail_scroll = ScrollHandle::new();
        state.detail = None;
        state.dispatches.clear();
        state.logs.clear();
        state.error = None;
        self.refresh_workflows(cx);
        cx.notify();
    }
    pub(crate) fn refresh_workflows(&mut self, cx: &mut Context<Self>) {
        if !self.workflows.visible
            || !self.connected
            || self.workflows.loading
            || self.workflows.acting
        {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let state = &mut self.workflows;
        state.loading = true;
        state.generation += 1;
        let generation = state.generation;
        let selected = state.selected.clone();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result: Result<Snapshot, String> = async {
                let runs: Runs = read(
                    &runtime,
                    format!("/api/workspaces/{workspace}/workflows/runs"),
                )
                .await?;
                let schedules: Schedules = read(
                    &runtime,
                    format!("/api/workspaces/{workspace}/workflow-schedules"),
                )
                .await?;
                let feature: Feature =
                    read(&runtime, "/api/settings/workflow-feature".into()).await?;
                let policy: Policy =
                    read(&runtime, "/api/settings/workflow-cli-policy".into()).await?;
                let detail = if let Some(id) = selected {
                    let run: RunDetail =
                        read(&runtime, format!("/api/workflows/runs/{id}")).await?;
                    let dispatches: Dispatches =
                        read(&runtime, format!("/api/workflows/runs/{id}/dispatches")).await?;
                    let logs: Logs =
                        read(&runtime, format!("/api/workflows/runs/{id}/logs")).await?;
                    Some((run.run, dispatches.dispatches, logs.logs))
                } else {
                    None
                };
                Ok(Snapshot {
                    runs: runs.runs,
                    schedules: schedules.schedules,
                    enabled: feature.enabled,
                    policy,
                    detail,
                })
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.workspace.as_ref() != Some(&workspace)
                    || this.workflows.generation != generation
                    || !this.workflows.visible
                {
                    return;
                }
                let state = &mut this.workflows;
                state.loading = false;
                match result {
                    Ok(next) => {
                        state.runs = next.runs;
                        state.schedules = next.schedules;
                        state.enabled = Some(next.enabled);
                        if !state.policy_dirty {
                            state.policy = Some(next.policy);
                        }
                        if let Some((run, dispatches, logs)) = next.detail {
                            state.detail = Some(run);
                            state.dispatches = dispatches;
                            state.logs = logs;
                        }
                        state.error = None;
                    }
                    Err(error) => {
                        state.error = Some(WorkflowError::Refresh(error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn workflow_action(
        &mut self,
        method: &'static str,
        path: String,
        body: Option<Value>,
        policy: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || !self.workspace_writes_allowed() || self.workflows.acting {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        self.invalidate_workflows();
        self.workflows.acting = true;
        self.workflows.action_error = None;
        let generation = self.workflows.generation;
        let connection = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "hive.request",
                    json!({"method":method,"path":path,"body":body}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.workspace.as_ref() != Some(&workspace)
                    || this.workflows.generation != generation
                {
                    return;
                }
                this.workflows.acting = false;
                match result {
                    Ok(_) => {
                        if policy {
                            this.workflows.policy_dirty = false;
                        }
                        this.workflows.deleting = None;
                        this.refresh_workflows(cx);
                    }
                    Err(error) => {
                        this.workflows.action_error =
                            Some(WorkflowError::Action(error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
