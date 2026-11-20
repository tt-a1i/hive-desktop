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
    pub focus_navigation: bool,
    pub loading: bool,
    pub loaded: bool,
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
    inbox_origin: Option<InboxWorkflowOrigin>,
    pub detail: Option<Run>,
    pub dispatches: Vec<Dispatch>,
    pub logs: Vec<Log>,
    pub filter: &'static str,
    pub list_scroll: ScrollHandle,
    pub detail_scroll: ScrollHandle,
    pub deleting: Option<String>,
    pub delete_cancel_focus: Option<FocusHandle>,
}

enum InboxWorkflowOrigin {
    Project,
    Global { workspace: String, sidebar_open: bool },
}
pub enum WorkflowError { Refresh(String), Action(String), Unconfirmed }
impl WorkflowError {
    fn text(&self) -> String {
        match self {
            Self::Refresh(error) => format!("{}: {}", tr("工作流未能更新，保留上次结果"), crate::language::localize_owned_error(error)),
            Self::Action(error) => format!("{}: {}", tr("工作流操作失败，请刷新核对"), crate::language::localize_owned_error(error)),
            Self::Unconfirmed => tr("操作结果尚未确认，请刷新后核对；执行器草稿已保留。").to_owned(),
        }
    }
}
impl Default for Workflows {
    fn default() -> Self {
        Self {
            visible: false,
            focus_navigation: false,
            loading: false,
            loaded: false,
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
            inbox_origin: None,
            detail: None,
            dispatches: vec![],
            logs: vec![],
            filter: "all",
            list_scroll: ScrollHandle::new(),
            detail_scroll: ScrollHandle::new(),
            deleting: None,
            delete_cancel_focus: None,
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
}

type RunDetails = (Run, Vec<Dispatch>, Vec<Log>);

fn run_matches(run: &Run, workspace: &str, id: &str, inbox: bool) -> bool {
    run.id == id
        && run.workspace_id == workspace
        && (!inbox
            || matches!(run.status.as_str(), "failed" | "interrupted")
                && run.parent_run_id.is_none()
                && run.finished_at.is_some_and(|time| time > 0))
}

async fn read_run_details(
    runtime: &crate::transport::Runtime,
    workspace: &str,
    id: &str,
    inbox: bool,
) -> Result<RunDetails, String> {
    let path = format!("/api/workflows/runs/{id}");
    let RunDetail { run } = read(runtime, path.clone()).await?;
    if !run_matches(&run, workspace, id, inbox) {
        return Err(tr("无法打开工作流运行").into());
    }
    let (dispatches, logs) = smol::future::zip(
        read::<Dispatches>(runtime, format!("{path}/dispatches")),
        read::<Logs>(runtime, format!("{path}/logs")),
    )
    .await;
    Ok((run, dispatches?.dispatches, logs?.logs))
}

impl HiveApp {
    pub(crate) fn close_workflows(&mut self) {
        self.workflows.visible = false;
        let inbox = self.workflows.inbox_origin.take().is_some();
        self.invalidate_workflows();
        if inbox {
            self.clear_workflow_detail(None);
        }
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
        state.loaded = false;
        state.schedules.clear();
        state.inbox_origin = None;
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
        if self.workflows.inbox_origin.take().is_some() {
            self.invalidate_workflows();
            self.clear_workflow_detail(None);
        }
        self.activate_workflows(window, cx);
    }
    pub(crate) fn open_inbox_workflow(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_workflow_inbox_detail(id, InboxWorkflowOrigin::Project, window, cx);
    }

    pub(crate) fn open_global_inbox_workflow(
        &mut self,
        workspace: String,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let origin = InboxWorkflowOrigin::Global { workspace, sidebar_open: self.sidebar_open };
        self.open_workflow_inbox_detail(id, origin, window, cx);
    }

    fn open_workflow_inbox_detail(
        &mut self,
        id: String,
        origin: InboxWorkflowOrigin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_workflows();
        self.clear_workflow_detail(Some(id));
        self.workflows.inbox_origin = Some(origin);
        self.activate_workflows(window, cx);
    }
    fn activate_workflows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hide_terminal(cx);
        self.close_memory();
        self.close_roles();
        self.close_settings();
        self.close_collaboration();
        self.member_panel = false;
        self.workflows.visible = true;
        self.workflows.focus_navigation = true;
        if window.viewport_size().width < px(820.) {
            self.sidebar_open = false;
        }
        self.refresh_workflows(cx);
        cx.notify();
    }
    pub(crate) fn select_workflow(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.invalidate_workflows();
        self.clear_workflow_detail(id);
        self.refresh_workflows(cx);
        cx.notify();
    }
    fn clear_workflow_detail(&mut self, id: Option<String>) {
        let state = &mut self.workflows;
        state.selected = id;
        state.focus_navigation = true;
        state.detail_scroll = ScrollHandle::new();
        state.detail = None;
        state.dispatches.clear();
        state.logs.clear();
        state.error = None;
    }
    pub(crate) fn refresh_workflows(&mut self, cx: &mut Context<Self>) {
        if !self.workflows.visible
            || !self.connected
            || self.workflows.loading
            || self.workflows.acting
        {
            return;
        }
        let ambient_workspace = self.workspace.clone();
        let workspace = match &self.workflows.inbox_origin {
            Some(InboxWorkflowOrigin::Global { workspace, .. }) => Some(workspace.clone()),
            _ => ambient_workspace.clone(),
        };
        let (Some(workspace), Some(runtime)) = (workspace, self.runtime.clone()) else {
            return;
        };
        let state = &mut self.workflows;
        state.loading = true;
        state.generation += 1;
        let generation = state.generation;
        let selected = state.selected.clone();
        let inbox = state.inbox_origin.is_some();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result: Result<(Option<Snapshot>, Option<RunDetails>), String> = async {
                if inbox {
                    let id = selected.as_deref().ok_or(tr("无法打开工作流运行"))?;
                    return Ok((
                        None,
                        Some(read_run_details(&runtime, &workspace, id, true).await?),
                    ));
                }
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
                    Some(read_run_details(&runtime, &workspace, &id, false).await?)
                } else {
                    None
                };
                Ok((
                    Some(Snapshot {
                        runs: runs.runs,
                        schedules: schedules.schedules,
                        enabled: feature.enabled,
                        policy,
                    }),
                    detail,
                ))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.workspace != ambient_workspace
                    || this.workflows.generation != generation
                    || !this.workflows.visible
                {
                    return;
                }
                let state = &mut this.workflows;
                state.loading = false;
                match result {
                    Ok((next, detail)) => {
                        if let Some(next) = next {
                            state.loaded = true;
                            state.runs = next.runs;
                            state.schedules = next.schedules;
                            state.enabled = Some(next.enabled);
                            if !state.policy_dirty {
                                state.policy = Some(next.policy);
                            }
                        }
                        if let Some((run, dispatches, logs)) = detail {
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
                        if this.workflows.deleting.take().is_some() {
                            this.workflows.focus_navigation = true;
                        }
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

#[cfg(test)]
mod tests {
    use super::{Run, run_matches};
    use serde_json::json;

    #[test]
    fn inbox_workflow_detail_requires_an_exact_settled_root_failure_or_interruption() {
        let mut run: Run = serde_json::from_value(json!({
            "id": "selected", "workspace_id": "atlas", "name": "Daily audit",
            "script_path": "check.ts", "status": "failed", "started_at": 10,
            "finished_at": 20, "agent_count": 0, "args": null, "result": null,
        }))
        .unwrap();
        assert!(run_matches(&run, "atlas", "selected", true));
        run.status = "interrupted".into();
        assert!(run_matches(&run, "atlas", "selected", true));
        assert!(!run_matches(&run, "beacon", "selected", true));
        assert!(!run_matches(&run, "atlas", "different", true));
        run.parent_run_id = Some("outer".into());
        assert!(!run_matches(&run, "atlas", "selected", true));
        assert!(run_matches(&run, "atlas", "selected", false));
        run.parent_run_id = None;
        run.finished_at = None;
        assert!(!run_matches(&run, "atlas", "selected", true));
        run.finished_at = Some(20);
        for status in ["completed", "stopped", "running"] {
            run.status = status.into();
            assert!(!run_matches(&run, "atlas", "selected", true));
        }
    }
}
