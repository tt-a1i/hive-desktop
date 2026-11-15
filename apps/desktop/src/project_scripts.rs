pub(crate) mod model;
use crate::{
    app::{HiveApp, SettingsSection},
    language::tr,
    palette::Color,
    transport::Runtime,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use model::*;
use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Duration};

#[derive(Clone)]
pub(crate) struct Scope {
    workspace: String,
    session: Option<String>,
    runtime: Arc<Runtime>,
    connection: u64,
    open: u64,
    path: String,
}
impl Scope {
    fn identity(&self) -> Navigation<'_> {
        Navigation {
            workspace: &self.workspace,
            session: self.session.as_deref(),
            connection: self.connection,
            open: self.open,
        }
    }
    fn same(&self, other: &Self) -> bool {
        self.identity() == other.identity()
            && self.path == other.path
            && Arc::ptr_eq(&self.runtime, &other.runtime)
    }
    fn current(&self, app: &HiveApp) -> bool {
        app.connected
            && !app.closing
            && self.identity()
                == Navigation {
                    workspace: app.workspace.as_deref().unwrap_or(""),
                    session: app
                        .session
                        .as_ref()
                        .filter(|s| s.workspace_id == self.workspace)
                        .map(|s| s.id.as_str()),
                    connection: app.connection_generation,
                    open: app.open_request,
                }
            && app
                .workspaces
                .iter()
                .any(|p| p.id == self.workspace && p.path == self.path)
            && app
                .runtime
                .as_ref()
                .is_some_and(|r| Arc::ptr_eq(r, &self.runtime))
    }
}
impl HiveApp {
    pub(crate) fn render_project_scripts(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Page> {
        let workspace = self.workspace.clone();
        let scope = workspace.as_ref().and_then(|workspace| {
            if !self.connected || self.closing {
                return None;
            }
            let project = self.workspaces.iter().find(|p| &p.id == workspace)?;
            Some(Scope {
                workspace: workspace.clone(),
                session: self
                    .session
                    .as_ref()
                    .filter(|s| &s.workspace_id == workspace)
                    .map(|s| s.id.clone()),
                runtime: self.runtime.clone()?,
                connection: self.connection_generation,
                open: self.open_request,
                path: project.path.clone(),
            })
        });
        self.project_scripts
            .update(cx, |page, cx| page.sync(workspace, scope, window, cx));
        self.project_scripts.clone()
    }
}
struct Form {
    name: Entity<InputState>,
    command: Entity<TextareaState>,
    run_on_worktree_create: bool,
}
impl Form {
    fn new(script: Option<&Script>, window: &mut Window, cx: &mut Context<Page>) -> Self {
        let name = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value(script.map(|s| s.name.as_str()).unwrap_or(""), window, cx);
            input
        });
        let command = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx).auto_grow(3, 10);
            input.set_value(script.map(|s| s.command.as_str()).unwrap_or(""), window, cx);
            input
        });
        cx.subscribe(&name, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.notice = None;
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&command, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.notice = None;
                cx.notify();
            }
        })
        .detach();
        Self {
            name,
            command,
            run_on_worktree_create: script.is_some_and(|s| s.run_on_worktree_create),
        }
    }
}
#[derive(Clone)]
struct Pending {
    request: String,
    script: Script,
    session: Option<String>,
    cwd: String,
    recovery: Recovery,
    stamp: AttemptStamp,
    last_error: Option<String>,
}
impl Pending {
    fn retry_target(&self, target: &str) -> Result<(), String> {
        if self.cwd == target {
            Ok(())
        } else {
            Err(format!(
                "{}\n{}",
                tr("执行目录已变化，未提交重试启动。"),
                directory_error(&self.cwd, target)
            ))
        }
    }
    fn matches(&self, run: &Run) -> bool {
        run.request_id == self.request
            && run.script.id == self.script.id
            && run.script.version == self.script.version
            && run.script.name == self.script.name
            && run.script.command == self.script.command
            && run.session_id == self.session
    }
}
#[derive(Default)]
struct WorkspaceState {
    snapshot: Option<Snapshot>,
    forms: HashMap<String, Form>,
    selected: String,
    runs: Vec<Run>,
    pending: Option<Pending>,
    abandoned: Vec<Pending>,
    resolved_requests: Vec<String>,
    selected_run: Option<String>,
    output: Output,
}
impl WorkspaceState {
    fn release_pending(&mut self) -> bool {
        if self.abandoned.len() >= 20
            || !self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.recovery.releasable())
        {
            return false;
        }
        self.abandoned.push(self.pending.take().unwrap());
        self.selected.clear();
        true
    }
    fn reconcile_abandoned(&mut self, runs: &[Run]) {
        let mut resolved = Vec::new();
        self.abandoned.retain(|pending| {
            if runs
                .iter()
                .any(|run| pending.matches(run) && run.cwd == pending.cwd)
            {
                resolved.push(pending.request.clone());
                false
            } else {
                true
            }
        });
        if !resolved.is_empty() {
            self.resolved_requests = resolved;
        }
    }
}
pub(crate) struct Page {
    app: WeakEntity<HiveApp>,
    workspace: Option<String>,
    scope: Option<Scope>,
    epoch: u64,
    attempt: u64,
    selection_generation: u64,
    states: HashMap<String, WorkspaceState>,
    settings_busy: bool,
    runs_busy: bool,
    log_busy: bool,
    run_busy: bool,
    settings_valid: bool,
    error: Option<String>,
    notice: Option<&'static str>,
    use_session: bool,
    target: Option<String>,
    target_busy: bool,
    scroll: ScrollHandle,
    output_scroll: ScrollHandle,
    feedback_scroll: ScrollHandle,
    polling: bool,
    log_generation: u64,
}
impl Page {
    pub(crate) fn new(
        app: WeakEntity<HiveApp>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            app,
            workspace: None,
            scope: None,
            epoch: 0,
            attempt: 0,
            selection_generation: 0,
            states: HashMap::new(),
            settings_busy: false,
            runs_busy: false,
            log_busy: false,
            run_busy: false,
            settings_valid: false,
            error: None,
            notice: None,
            use_session: true,
            target: None,
            target_busy: false,
            scroll: ScrollHandle::new(),
            output_scroll: ScrollHandle::new(),
            feedback_scroll: ScrollHandle::new(),
            polling: false,
            log_generation: 0,
        }
    }
    fn sync(
        &mut self,
        workspace: Option<String>,
        scope: Option<Scope>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self.workspace != workspace
            || match (&self.scope, &scope) {
                (Some(a), Some(b)) => !a.same(b),
                (None, None) => false,
                _ => true,
            };
        self.workspace = workspace;
        self.scope = scope;
        if let Some(workspace) = &self.workspace {
            let state = self.states.entry(workspace.clone()).or_default();
            state
                .forms
                .entry(String::new())
                .or_insert_with(|| Form::new(None, window, cx));
        }
        if changed {
            self.error = None;
            self.notice = None;
            if let Some(pending) = self.state_mut().and_then(|state| state.pending.as_mut()) {
                pending.recovery.invalidate_refresh();
            }
            self.epoch += 1;
            self.settings_busy = false;
            self.runs_busy = false;
            self.log_busy = false;
            self.run_busy = false;
            self.target_busy = false;
            self.target = None;
            self.settings_valid = false;
            self.polling = false;
            self.log_generation += 1;
            let epoch = self.epoch;
            let pending = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = pending.update(cx, |this, cx| {
                    if this.epoch == epoch {
                        this.reload_settings(cx);
                        this.refresh_runs(false, cx);
                        this.resolve_target(cx);
                    }
                });
            });
        }
        if !self.polling && self.scope.is_some() {
            self.polling = true;
            let epoch = self.epoch;
            cx.spawn(async move |this, cx| {
                loop {
                    smol::Timer::after(Duration::from_secs(2)).await;
                    let keep = this
                        .update(cx, |this, cx| {
                            if this.epoch != epoch {
                                return false;
                            }
                            if !this.visible(cx) {
                                this.polling = false;
                                return false;
                            }
                            this.refresh_runs(false, cx);
                            true
                        })
                        .unwrap_or(false);
                    if !keep {
                        break;
                    }
                }
            })
            .detach();
        }
    }
    fn valid(&self, cx: &App) -> bool {
        self.scope.as_ref().is_some_and(|scope| {
            self.app
                .upgrade()
                .is_some_and(|app| scope.current(app.read(cx)))
        })
    }
    fn visible(&self, cx: &App) -> bool {
        self.valid(cx)
            && self.app.upgrade().is_some_and(|app| {
                let app = app.read(cx);
                app.settings_panel
                    && !app.roles.visible
                    && app.settings_section == SettingsSection::ProjectScripts
            })
    }
    fn accepts(&self, epoch: u64, cx: &App) -> bool {
        self.epoch == epoch && self.valid(cx)
    }
    fn state(&self) -> Option<&WorkspaceState> {
        self.workspace.as_ref().and_then(|ws| self.states.get(ws))
    }
    fn state_mut(&mut self) -> Option<&mut WorkspaceState> {
        self.workspace
            .as_ref()
            .and_then(|ws| self.states.get_mut(ws))
    }
    fn reload_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings_busy || !self.valid(cx) {
            return;
        }
        let scope = self.scope.clone().unwrap();
        let epoch = self.epoch;
        self.settings_busy = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this
                .read_with(cx, |this, cx| this.accepts(epoch, cx))
                .unwrap_or(false)
                == false
            {
                return;
            }
            let result = scope
                .runtime
                .request(
                    "script.settings.list",
                    json!({"workspaceId":scope.workspace}),
                )
                .await
                .and_then(decode::<Snapshot>)
                .and_then(Snapshot::validate);
            let _ = this.update(cx, |this, cx| {
                if !this.accepts(epoch, cx) {
                    return;
                }
                this.settings_busy = false;
                match result {
                    Ok(snapshot) => {
                        this.settings_valid = true;
                        this.state_mut().unwrap().snapshot = Some(snapshot);
                    }
                    Err(error) => {
                        this.settings_valid = false;
                        this.error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn resolve_target(&mut self, cx: &mut Context<Self>) {
        self.target = None;
        self.error = None;
        if !self.valid(cx) {
            return;
        }
        let scope = self.scope.clone().unwrap();
        let session = if self.use_session {
            let Some(session) = scope.session.clone() else {
                cx.notify();
                return;
            };
            Some(session)
        } else {
            None
        };
        let epoch = self.epoch;
        self.target_busy = true;
        cx.spawn(async move |this, cx| {
            if !this
                .read_with(cx, |this, cx| this.accepts(epoch, cx))
                .unwrap_or(false)
            {
                return;
            }
            let result = execution_target(&scope, session.as_deref()).await;
            let _ = this.update(cx, |this, cx| {
                if !this.accepts(epoch, cx) {
                    return;
                }
                this.target_busy = false;
                match result {
                    Ok(path) => this.target = Some(path),
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn save(&mut self, remove: bool, cx: &mut Context<Self>) {
        if self.settings_busy || !self.settings_valid || !self.visible(cx) {
            return;
        }
        let scope = self.scope.clone().unwrap();
        let state = self.state().unwrap();
        let Some(snapshot) = &state.snapshot else {
            return;
        };
        let key = state.selected.clone();
        let previous_ids = snapshot
            .scripts
            .iter()
            .map(|script| script.id.clone())
            .collect::<Vec<_>>();
        let Some(form) = state.forms.get(&key) else {
            return;
        };
        let name = form.name.read(cx).value().to_string();
        let command = form.command.read(cx).value().to_string();
        let run_on_worktree_create = form.run_on_worktree_create;
        if remove && key.is_empty()
            || !remove && (name.trim().is_empty() || command.trim().is_empty())
        {
            return;
        }
        let mut params =
            json!({"workspaceId":scope.workspace,"expectedRevision":snapshot.revision});
        if remove {
            params["scriptId"] = json!(key);
        } else {
            params["script"] =
                json!({"name":name,"command":command,"runOnWorktreeCreate":run_on_worktree_create});
            if !key.is_empty() {
                params["script"]["id"] = json!(key);
            }
        }
        let epoch = self.epoch;
        self.settings_busy = true;
        self.error = None;
        self.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            if !this
                .read_with(cx, |this, cx| this.accepts(epoch, cx))
                .unwrap_or(false)
            {
                return;
            }
            let result = scope
                .runtime
                .request(
                    if remove {
                        "script.settings.remove"
                    } else {
                        "script.settings.save"
                    },
                    params,
                )
                .await
                .and_then(decode::<Snapshot>)
                .and_then(Snapshot::validate);
            let _ = this.update(cx, |this, cx| {
                if !this.accepts(epoch, cx) {
                    return;
                }
                this.settings_busy = false;
                match result {
                    Ok(snapshot) => {
                        let selected = match snapshot.saved_selection(
                            &previous_ids,
                            &key,
                            &name,
                            &command,
                            run_on_worktree_create,
                            remove,
                        ) {
                            Ok(selected) => selected,
                            Err(error) => {
                                this.settings_valid = false;
                                this.error = Some(error);
                                cx.notify();
                                return;
                            }
                        };
                        let state = this.state_mut().unwrap();
                        state.snapshot = Some(snapshot);
                        state.selected = selected;
                        if remove || key.is_empty() {
                            state.forms.remove(&key);
                        }
                        this.notice = Some(if remove {
                            "项目脚本已删除"
                        } else {
                            "项目脚本已保存，尚未执行"
                        });
                    }
                    Err(error) => {
                        this.settings_valid = false;
                        this.error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn refresh_runs(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.runs_busy || !self.valid(cx) {
            return;
        }
        let scope = self.scope.clone().unwrap();
        // A read clicked before the rejection may inspect status, but cannot prove a later rejection safe to leave.
        let qualify_absence = manual && !self.run_busy;
        let epoch = self.epoch;
        self.runs_busy = true;
        cx.spawn(async move |this, cx| {
            if !this
                .read_with(cx, |this, cx| this.accepts(epoch, cx))
                .unwrap_or(false)
            {
                return;
            }
            let result = scope
                .runtime
                .request("script.runs", json!({"workspaceId":scope.workspace}))
                .await
                .and_then(decode::<Runs>)
                .and_then(|value| {
                    for run in &value.runs {
                        run.validate(&scope.workspace)?;
                    }
                    Ok(value.runs)
                });
            let _ = this.update(cx, |this, cx| {
                if !this.accepts(epoch, cx) {
                    return;
                }
                this.runs_busy = false;
                match result {
                    Ok(runs) => {
                        let selection = this.selection_generation;
                        let mut confirmed = None;
                        let mut directory_conflict = None;
                        {
                            let state = this.state_mut().unwrap();
                            if let Some(pending) = state.pending.as_mut() {
                                pending.recovery.refreshed(
                                    qualify_absence,
                                    runs.iter().any(|run| run.request_id == pending.request),
                                );
                            }
                            if let Some((pending, found)) =
                                state.pending.as_ref().and_then(|pending| {
                                    runs.iter()
                                        .find(|run| pending.matches(run))
                                        .map(|run| (pending.clone(), run))
                                })
                            {
                                if pending.stamp.may_select(selection)
                                    && state.selected_run.as_ref() != Some(&found.run_id)
                                {
                                    state.selected_run = Some(found.run_id.clone());
                                    state.output = Output::default();
                                }
                                if pending.cwd != found.cwd {
                                    directory_conflict =
                                        Some(directory_error(&pending.cwd, &found.cwd));
                                }
                                confirmed = Some((pending.stamp, pending.last_error));
                                state.pending = None;
                            }
                            if manual {
                                state.reconcile_abandoned(&runs);
                            }
                            state.runs = runs;
                        }
                        if let Some((stamp, last_error)) = confirmed {
                            if stamp.current(this.attempt) {
                                this.attempt += 1;
                                this.run_busy = false;
                            }
                            if let Some(error) = directory_conflict {
                                this.error = Some(error);
                            } else if last_error.is_some() && this.error == last_error {
                                this.error = None;
                            }
                            this.notice = Some("运行请求已由运行记录确认；关闭此页不会停止脚本");
                        }
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn run(&mut self, retry: bool, cx: &mut Context<Self>) {
        if self.run_busy || self.runs_busy || !self.visible(cx) {
            return;
        }
        let scope = self.scope.clone().unwrap();
        let state = self.state().unwrap();
        let mut pending = if retry {
            let Some(pending) = state.pending.clone() else {
                return;
            };
            pending
        } else {
            if self.settings_busy
                || !self.settings_valid
                || self.target.is_none()
                || self.target_busy
                || state.pending.is_some()
            {
                return;
            }
            let Some(script) = state
                .snapshot
                .as_ref()
                .and_then(|snapshot| {
                    snapshot
                        .scripts
                        .iter()
                        .find(|script| script.id == state.selected)
                })
                .cloned()
            else {
                return;
            };
            Pending {
                request: uuid::Uuid::new_v4().to_string(),
                script,
                session: if self.use_session {
                    scope.session.clone()
                } else {
                    None
                },
                cwd: self.target.clone().unwrap(),
                recovery: Recovery::default(),
                last_error: None,
                stamp: AttemptStamp {
                    attempt: 0,
                    selection: 0,
                },
            }
        };
        self.attempt += 1;
        pending.stamp = AttemptStamp {
            attempt: self.attempt,
            selection: self.selection_generation,
        };
        pending.recovery = Recovery::default();
        pending.last_error = None;
        self.state_mut().unwrap().pending = Some(pending.clone());
        let mut params = json!({"workspaceId":scope.workspace,"scriptId":pending.script.id,"expectedVersion":pending.script.version,"requestId":pending.request});
        if let Some(session) = &pending.session {
            params["sessionId"] = json!(session);
        }
        let epoch = self.epoch;
        self.run_busy = true;
        self.error = None;
        self.notice = None;
        cx.notify();
        cx.spawn(async move|this,cx|{
            if !this.read_with(cx,|this,cx|this.accepts(epoch,cx)&&pending.stamp.current(this.attempt)).unwrap_or(false){return;}
            let result=async {
                if retry {
                    let target=execution_target(&scope,pending.session.as_deref()).await.map_err(|error|format!("{}\n{}",tr("未提交重试启动；原请求仍需核对。"),error))?;
                    pending.retry_target(&target)?;
                }
                // The authoritative directory lookup may have yielded across navigation or a list confirmation.
                if !this.read_with(cx,|this,cx|this.accepts(epoch,cx)&&pending.stamp.current(this.attempt)).unwrap_or(false){return Ok(None);}
                let run=scope.runtime.request("script.run",params).await.and_then(decode::<Run>)?;
                run.validate(&scope.workspace)?;
                if !pending.matches(&run){return Err(format!("{}\nrequestId: {}\n{}: {}\n{}: {}",tr("运行来源与原请求不匹配，请核对记录"),run.request_id,tr("来源会话"),run.session_id.as_deref().unwrap_or("—"),tr("实际执行目录"),run.cwd));}
                Ok(Some(run))
            }.await;
            let _=this.update(cx,|this,cx|{
                if !this.accepts(epoch,cx)||!pending.stamp.current(this.attempt){return;}
                this.run_busy=false;
                match result {
                    Ok(Some(run))=>{
                        let directory_conflict=(run.cwd!=pending.cwd).then(||directory_error(&pending.cwd,&run.cwd));
                        let selection=this.selection_generation;
                        let state=this.state_mut().unwrap();state.pending=None;
                        if pending.stamp.may_select(selection)&&state.selected_run.as_ref()!=Some(&run.run_id){state.selected_run=Some(run.run_id.clone());state.output=Output::default();}
                        state.runs.retain(|old|old.run_id!=run.run_id);state.runs.insert(0,run);
                        if let Some(error)=directory_conflict{this.error=Some(error);}this.notice=Some("运行请求已确认；关闭此页不会停止脚本");
                    }
                    Ok(None)=>{},
                    Err(error)=>{
                        // A list response may already have confirmed this request. Never downgrade it.
                        if let Some(current)=this.state_mut().unwrap().pending.as_mut().filter(|current|current.request==pending.request&&current.stamp.current(pending.stamp.attempt)){
                            current.recovery.rejected(&error);
                            let message=format!("{}\n{}",tr("运行是否已启动尚未确认；请保留此请求编号并刷新核对，不要另建请求重跑。"),error);
                            current.last_error=Some(message.clone());this.error=Some(message);
                        }
                    }
                }cx.notify();
            });
        }).detach();
    }
    fn release_pending(&mut self, cx: &mut Context<Self>) {
        if !self.visible(cx)
            || self.run_busy
            || self.runs_busy
            || !self.state().is_some_and(|state| {
                state.abandoned.len() < 20
                    && state
                        .pending
                        .as_ref()
                        .is_some_and(|pending| pending.recovery.releasable())
            })
        {
            return;
        }
        let state = self.state_mut().unwrap();
        if !state.release_pending() {
            return;
        }
        self.error = None;
        self.notice = Some("已放弃本次核对，未取消原请求；请检查记录后重新选择脚本");
        cx.notify();
    }
    fn stop(&mut self, cx: &mut Context<Self>) {
        if self.run_busy || !self.visible(cx) {
            return;
        }
        let scope = self.scope.clone().unwrap();
        let Some(run_id) = self.state().and_then(|s| s.selected_run.clone()) else {
            return;
        };
        let epoch = self.epoch;
        self.run_busy = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            if !this
                .read_with(cx, |this, cx| this.accepts(epoch, cx))
                .unwrap_or(false)
            {
                return;
            }
            let result = scope
                .runtime
                .request(
                    "script.stop",
                    json!({"workspaceId":scope.workspace,"runId":run_id}),
                )
                .await
                .and_then(decode::<Run>)
                .and_then(|run| {
                    run.validate(&scope.workspace)?;
                    if run.run_id != run_id {
                        return Err("Script run identity mismatch".into());
                    }
                    Ok(run)
                });
            let _ = this.update(cx, |this, cx| {
                if !this.accepts(epoch, cx) {
                    return;
                }
                this.run_busy = false;
                match result {
                    Ok(run) => {
                        let state = this.state_mut().unwrap();
                        if let Some(current) =
                            state.runs.iter_mut().find(|r| r.run_id == run.run_id)
                        {
                            *current = run;
                        }
                        this.notice = Some("停止请求已处理，请核对运行状态与退出码");
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn load_output(&mut self, next_segment: bool, cx: &mut Context<Self>) {
        if self.log_busy || !self.valid(cx) {
            return;
        }
        let scope = self.scope.clone().unwrap();
        let state = self.state_mut().unwrap();
        let Some(run_id) = state.selected_run.clone() else {
            return;
        };
        let mut output = state.output.clone();
        if next_segment {
            output.next_segment();
        }
        let offset = output.next;
        let limit = 65536.min(UI_LOG_LIMIT - output.bytes.len());
        if limit == 0 {
            return;
        }
        let epoch = self.epoch;
        self.log_generation += 1;
        let log_generation = self.log_generation;
        self.log_busy = true;
        self.error = None;
        cx.spawn(async move|this,cx|{
            if !this.read_with(cx,|this,cx|this.accepts(epoch,cx)&&this.log_generation==log_generation).unwrap_or(false){return;}
            let result=scope.runtime.request("script.get",json!({"workspaceId":scope.workspace,"runId":run_id,"offset":offset,"limit":limit})).await.and_then(decode::<RunPage>).and_then(|page|{page.run.validate(&scope.workspace)?;if page.run.run_id!=run_id{return Err("Script output identity mismatch".into());}Ok(page)});
            let _=this.update(cx,|this,cx|{if !this.accepts(epoch,cx)||this.log_generation!=log_generation{return;}this.log_busy=false;match result{Ok(page)=>{let state=this.state_mut().unwrap();if state.selected_run.as_ref()!=Some(&run_id){return;}if let Err(error)=output.append(page.output,limit){this.error=Some(error);}else{state.output=output;if let Some(run)=state.runs.iter_mut().find(|r|r.run_id==run_id){*run=page.run;}}},Err(error)=>this.error=Some(error)}cx.notify();});
        }).detach();
        cx.notify();
    }
}
async fn execution_target(scope: &Scope, session: Option<&str>) -> Result<String, String> {
    let Some(session) = session else {
        let value = scope
            .runtime
            .request(
                "hive.request",
                json!({"method":"GET","path":"/api/workspaces"}),
            )
            .await?;
        let projects = decode::<Vec<crate::protocol::Workspace>>(value)?;
        return projects
            .into_iter()
            .find(|project| project.id == scope.workspace && !project.path.is_empty())
            .map(|project| project.path)
            .ok_or_else(|| "Project execution directory is unavailable".into());
    };
    let value = scope
        .runtime
        .request("chat.execution", json!({"sessionId":session}))
        .await?;
    if value["sessionId"] != session
        || value["workspaceId"] != scope.workspace
        || !matches!(value["kind"].as_str(), Some("worktree" | "project"))
        || !value["path"].as_str().is_some_and(|path| !path.is_empty())
    {
        return Err("Invalid execution directory response".into());
    }
    Ok(value["path"].as_str().unwrap().to_owned())
}
fn directory_error(expected: &str, actual: &str) -> String {
    format!(
        "{}\n{}: {}\n{}: {}",
        tr("执行目录与原目标不匹配；请核对实际运行记录。"),
        tr("原目标执行目录"),
        expected,
        tr("实际执行目录"),
        actual
    )
}
impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(state) = self.state_mut() {
            state
                .forms
                .entry(String::new())
                .or_insert_with(|| Form::new(None, window, cx));
            if let Some(snapshot) = &state.snapshot {
                for script in &snapshot.scripts {
                    state
                        .forms
                        .entry(script.id.clone())
                        .or_insert_with(|| Form::new(Some(script), window, cx));
                }
            }
        }
        let online = self.scope.is_some();
        let settings_enabled = online && self.settings_valid && !self.settings_busy;
        let scroll = self.scroll.clone();
        let content=v_flex().id("project-scripts-content").flex_shrink_0().min_w_0().gap_3().pr_1()
            .child(crate::text_reading::selectable_text("project-scripts-project",format!("{}: {}\n{}: {}",tr("项目"),self.workspace.as_deref().unwrap_or("—"),tr("项目目录"),self.scope.as_ref().map(|scope|scope.path.as_str()).unwrap_or("—"))))
            .child(div().text_sm().child(tr("保存脚本与执行分开。可手动运行已保存的完整命令，也可指定一个脚本用于新工作树初始化；切换会话或关闭设置不会停止进程，退出应用会取消并等待进程退出。")))
            .when(!online,|view|view.child(div().id("scripts-offline").role(Role::Status).text_sm().text_color(Color::Error.get(cx)).child(if self.workspace.is_none(){tr("请先选择项目，再配置或运行项目脚本。")}else{tr("后台未连接，不能读取、保存或运行；本地编辑仍保留。")})))
            .child(h_flex().flex_wrap().gap_2()
                .child(crate::focus_scroll::Reveal::new("reload-script-settings-reveal",&scroll,Button::new("reload-script-settings").outline().small().label(tr("刷新已保存脚本（保留编辑）")).disabled(!online||self.settings_busy).on_click(cx.listener(|this,_,_,cx|this.reload_settings(cx)))))
                .child(crate::focus_scroll::Reveal::new("refresh-script-runs-reveal",&scroll,Button::new("refresh-script-runs").outline().small().label(tr("刷新运行记录")).disabled(!online||self.runs_busy).on_click(cx.listener(|this,_,_,cx|this.refresh_runs(true,cx))))))
            ;
        let content = if let Some(state) = self.state() {
            let selected = state.selected.clone();
            let form = state.forms.get(&selected).unwrap();
            let name = form.name.clone();
            let command = form.command.clone();
            let saved = state
                .snapshot
                .as_ref()
                .and_then(|s| s.scripts.iter().find(|s| s.id == selected));
            let dirty = saved
                .map(|s| {
                    s.name != name.read(cx).value().as_str()
                        || s.command != command.read(cx).value().as_str()
                        || s.run_on_worktree_create != form.run_on_worktree_create
                })
                .unwrap_or(
                    !name.read(cx).value().is_empty()
                        || !command.read(cx).value().is_empty()
                        || form.run_on_worktree_create,
                );
            let can_save = settings_enabled
                && dirty
                && !name.read(cx).value().trim().is_empty()
                && !command.read(cx).value().trim().is_empty();
            let pending = state.pending.clone();
            let run = state
                .selected_run
                .as_ref()
                .and_then(|id| state.runs.iter().find(|r| &r.run_id == id));
            content.child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("已保存的项目脚本")))
                .child(crate::focus_scroll::Reveal::new("new-project-script-reveal",&scroll,Button::new("new-project-script").outline().small().label(tr("新增项目脚本")).disabled(self.settings_busy).on_click(cx.listener(|this,_,_,cx|{if this.settings_busy{return;}this.state_mut().unwrap().selected=String::new();this.notice=None;cx.notify();}))))
                .children(state.snapshot.as_ref().into_iter().flat_map(|s|s.scripts.iter()).enumerate().map(|(index,script)|{let id=script.id.clone();crate::focus_scroll::Reveal::new(("script-select-reveal",index),&scroll,Button::new(("select-project-script",index)).outline().small().h_auto().label(script.name.clone()).selected(selected==script.id).disabled(self.settings_busy).on_click(cx.listener(move|this,_,_,cx|{if this.settings_busy{return;}this.state_mut().unwrap().selected=id.clone();this.notice=None;cx.notify();})))}))
                .when(!selected.is_empty() && saved.is_none(),|view|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(tr("此脚本已被删除；本地编辑仍保留，请复制内容后新增脚本。"))))
                .child(div().text_sm().child(tr("脚本名称")))
                .child(crate::focus_scroll::Reveal::new("project-script-name-reveal",&scroll,Input::new(&name).aria_label(tr("脚本名称")).disabled(self.settings_busy)).track_focus(&name.read(cx).focus_handle(cx)))
                .child(div().text_sm().child(tr("完整命令")))
                .child(crate::focus_scroll::Reveal::new("project-script-command-reveal",&scroll,Textarea::new(&command).aria_label(tr("完整命令")).disabled(self.settings_busy)).track_focus(&command.read(cx).focus_handle(cx)))
                .child(crate::focus_scroll::Reveal::new("script-worktree-setup-reveal",&scroll,Button::new("script-worktree-setup").outline().small().label(if form.run_on_worktree_create { tr("新工作树创建时运行：已启用") } else { tr("新工作树创建时运行：未启用") }).selected(form.run_on_worktree_create).disabled(self.settings_busy || (!form.run_on_worktree_create && state.snapshot.as_ref().is_some_and(|s| s.scripts.iter().any(|s| s.id != selected && s.run_on_worktree_create)))).on_click(cx.listener(|this,_,_,cx|{let state=this.state_mut().unwrap();let key=state.selected.clone();if let Some(form)=state.forms.get_mut(&key){form.run_on_worktree_create=!form.run_on_worktree_create;}this.notice=None;cx.notify();}))))
                .child(div().text_xs().child(tr("只能指定一个初始化脚本。新工作树会等待此脚本完成后才能发送消息。")))
                .when(dirty,|view|view.child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("有未保存的编辑。运行按钮只使用下方已保存的命令快照。"))))
                .child(h_flex().flex_wrap().gap_2()
                    .child(crate::focus_scroll::Reveal::new("save-project-script-reveal",&scroll,Button::new("save-project-script").primary().label(tr("保存项目脚本")).disabled(!can_save||!selected.is_empty()&&saved.is_none()).on_click(cx.listener(|this,_,_,cx|this.save(false,cx)))))
                    .child(crate::focus_scroll::Reveal::new("remove-project-script-reveal",&scroll,Button::new("remove-project-script").outline().label(tr("删除已保存脚本")).disabled(!settings_enabled||saved.is_none()).on_click(cx.listener(|this,_,_,cx|this.save(true,cx))))))
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("运行已保存脚本")))
                .child(h_flex().flex_wrap().gap_2()
                    .child(crate::focus_scroll::Reveal::new("script-scope-project-reveal",&scroll,Button::new("script-scope-project").outline().small().label(tr("在项目目录运行")).selected(!self.use_session).disabled(!online||self.run_busy||self.target_busy).on_click(cx.listener(|this,_,_,cx|{if this.run_busy||this.target_busy{return;}this.use_session=false;this.resolve_target(cx);}))))
                    .child(crate::focus_scroll::Reveal::new("script-scope-session-reveal",&scroll,Button::new("script-scope-session").outline().small().label(tr("在当前会话目录运行")).selected(self.use_session).disabled(!online||self.scope.as_ref().is_none_or(|s|s.session.is_none())||self.run_busy||self.target_busy).on_click(cx.listener(|this,_,_,cx|{if this.run_busy||this.target_busy{return;}this.use_session=true;this.resolve_target(cx);})))))
                .child(crate::text_reading::selectable_text("script-target",format!("{}: {}\n{}: {}\n{}: {}",tr("项目"),self.workspace.as_deref().unwrap_or("—"),tr("来源会话"),if self.use_session{self.scope.as_ref().and_then(|s|s.session.as_deref()).unwrap_or("—")}else{"—"},tr("执行目录"),self.target.as_deref().unwrap_or("—"))))
                .when(self.use_session&&self.scope.as_ref().is_none_or(|s|s.session.is_none()),|view|view.child(div().text_sm().child(tr("没有当前会话；请选择在项目目录运行。"))))
                .when_some(saved,|view,saved|view.child(crate::text_reading::selectable_text("saved-script-command",format!("{}: {}\n{}",tr("将运行已保存命令"),saved.name,saved.command))))
                .child(crate::focus_scroll::Reveal::new("run-saved-project-script-reveal",&scroll,Button::new("run-saved-project-script").primary().label(tr("运行上述已保存命令")).disabled(!online||self.run_busy||self.runs_busy||self.settings_busy||!self.settings_valid||self.target.is_none()||self.target_busy||saved.is_none()||pending.is_some()).on_click(cx.listener(|this,_,_,cx|this.run(false,cx)))))
                .when_some(pending.as_ref(),|view,pending|view
                    .child(crate::text_reading::selectable_text("script-pending-request",format!("{}\nrequestId: {}\n{}: {}\n{}: {}\n{}\n{}",tr("运行是否已启动尚未确认；请保留此请求编号并刷新核对，不要另建请求重跑。"),pending.request,tr("来源会话"),pending.session.as_deref().unwrap_or("—"),tr("目标执行目录"),pending.cwd,pending.script.name,pending.script.command)))
                    .child(div().text_xs().child(tr("刷新运行记录可核对此编号。再次提交同一编号不会重复已登记的运行；若此前未登记，此操作会启动保存的脚本。")))
                    .child(crate::focus_scroll::Reveal::new("lookup-script-request-reveal",&scroll,Button::new("lookup-script-request").outline().label(tr("仅刷新核对此请求")).disabled(!online||self.runs_busy).on_click(cx.listener(|this,_,_,cx|this.refresh_runs(true,cx)))))
                    .child(crate::focus_scroll::Reveal::new("reconcile-script-request-reveal",&scroll,Button::new("reconcile-script-request").outline().label(tr("按原请求重试启动")).disabled(!online||self.run_busy||self.runs_busy).on_click(cx.listener(|this,_,_,cx|this.run(true,cx)))))
                    .when(pending.recovery.releasable(),|view|view
                        .child(div().text_sm().child(tr("这不会取消原请求；其他并发调用仍可能稍后启动。请检查运行记录后再决定是否另行运行。")))
                        .child(crate::focus_scroll::Reveal::new("release-script-reconciliation-reveal",&scroll,Button::new("release-script-reconciliation").outline().label(tr("放弃本次核对，重新选择脚本")).disabled(!online||self.run_busy||self.runs_busy||state.abandoned.len()>=20).on_click(cx.listener(|this,_,_,cx|this.release_pending(cx)))))))
                .when(state.abandoned.len()>=20,|view|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(tr("已保留 20 条未确认请求，不能继续放弃核对。请刷新并核实这些原请求；不会静默删除它们。"))))
                .children(state.abandoned.iter().enumerate().map(|(index,pending)|crate::text_reading::selectable_text(format!("unconfirmed-script-request-{index}"),format!("{}\nrequestId: {}\n{}: {}\n{}: {}\n{}\n{}\n{}",tr("上次未确认请求"),pending.request,tr("来源会话"),pending.session.as_deref().unwrap_or("—"),tr("目标执行目录"),pending.cwd,pending.script.name,pending.script.command,tr("这不会取消原请求；其他并发调用仍可能稍后启动。请检查运行记录后再决定是否另行运行。")))))
                .when(!state.resolved_requests.is_empty(),|view|view.child(crate::text_reading::selectable_text("resolved-script-requests",format!("{}\n{}",tr("以下原请求已在运行记录中确认，可按请求编号查看实际运行"),state.resolved_requests.join("\n")))))
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("项目运行记录")))
                .when(state.runs.is_empty(),|view|view.child(div().text_sm().child(tr("此项目没有已读取的运行记录。"))))
                .children(state.runs.iter().enumerate().map(|(index,run)|{let id=run.run_id.clone();crate::focus_scroll::Reveal::new(("script-run-reveal",index),&scroll,Button::new(("select-script-run",index)).outline().small().h_auto().label(format!("{} · {} · {}",run.script.name,run.status.label(),run.run_id)).selected(state.selected_run.as_ref()==Some(&run.run_id)).on_click(cx.listener(move|this,_,_,cx|{this.selection_generation+=1;this.log_generation+=1;this.log_busy=false;let state=this.state_mut().unwrap();state.selected_run=Some(id.clone());state.output=Output::default();cx.notify();})))}))
                .when_some(run,|view,run|view
                    .child(crate::text_reading::selectable_text("project-script-run-detail",format!("{}: {}\nrunId: {}\nrequestId: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}\n{}",tr("状态"),run.status.label(),run.run_id,run.request_id,tr("来源会话"),run.session_id.as_deref().unwrap_or("—"),tr("实际执行目录"),run.cwd,tr("开始时间（毫秒）"),run.started_at,tr("结束时间（毫秒）"),run.finished_at.map(|v|v.to_string()).unwrap_or_else(||"—".into()),tr("退出码"),run.exit_code.map(|v|v.to_string()).unwrap_or_else(||"—".into()),tr("退出信号"),run.signal.map(|v|v.to_string()).unwrap_or_else(||"—".into()),tr("原生运行编号"),run.native_run_id.as_deref().unwrap_or("—"),tr("已请求停止"),run.cancel_requested,tr("保留日志字节数"),run.log_bytes,tr("运行时错误代码"),run.error_code.as_deref().unwrap_or("—"),run.script.name,run.script.command)))
                    .child(crate::focus_scroll::Reveal::new("stop-project-script-reveal",&scroll,Button::new("stop-project-script").outline().label(tr("停止此后台脚本")).disabled(!online||self.run_busy||!run.status.active()).on_click(cx.listener(|this,_,_,cx|this.stop(cx)))))
                    .when(run.log_truncated||state.output.truncated,|view|view.child(div().id("script-log-truncated").role(Role::Status).text_sm().text_color(Color::Error.get(cx)).child(tr("后台日志已达到 16 MiB 上限，后续输出未保留。"))))
                    .child(div().text_xs().child(format!("{}: {}–{} · {}",tr("当前显示字节范围"),state.output.start,state.output.next,tr("每页最多 64 KiB，当前显示最多 1 MiB；按字节读取，不自动无限追加。"))))
                    .child(crate::text_reading::region("script-output".into(),tr("项目脚本输出").to_owned(),&self.output_scroll,px(320.),cx).child(crate::text_reading::selectable_text("script-output-text",state.output.text())))
                    .child(h_flex().flex_wrap().gap_2()
                        .child(crate::focus_scroll::Reveal::new("load-script-output-reveal",&scroll,Button::new("load-script-output").outline().small().label(if state.output.bytes.len()>=UI_LOG_LIMIT{tr("读取下一段（替换当前显示）")}else{tr("读取下一页输出")}).disabled(!online||self.log_busy).on_click(cx.listener(|this,_,_,cx|{let next=this.state().is_some_and(|s|s.output.bytes.len()>=UI_LOG_LIMIT);this.load_output(next,cx);}))))
                        .child(crate::focus_scroll::Reveal::new("copy-script-output-reveal",&scroll,Button::new("copy-script-output").outline().small().label(tr("复制当前输出")).on_click(cx.listener(|this,_,_,cx|{if let Some(state)=this.state(){cx.write_to_clipboard(ClipboardItem::new_string(state.output.text()));}}))))))
        } else {
            content
        };
        let feedback = self.error.clone().or_else(|| {
            if self.settings_busy || self.run_busy || self.log_busy || self.target_busy {
                Some(tr("正在处理项目脚本请求…").to_owned())
            } else {
                self.notice.map(|notice| tr(notice).to_owned())
            }
        });
        v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .gap_2()
            .when_some(feedback, |view, message| {
                view.child(
                    crate::text_reading::region(
                        "project-scripts-feedback".into(),
                        tr("项目脚本反馈").to_owned(),
                        &self.feedback_scroll,
                        px(112.),
                        cx,
                    )
                    .role(Role::Status)
                    .flex_shrink_0()
                    .text_sm()
                    .when(self.error.is_some(), |view| {
                        view.text_color(Color::Error.get(cx))
                    })
                    .child(crate::text_reading::selectable_text(
                        "project-scripts-feedback-text",
                        message,
                    )),
                )
            })
            .child(crate::focus_scroll::Viewport::new(
                "project-scripts-viewport",
                &scroll,
                div()
                    .id("project-scripts-scroll")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .vertical_scrollbar(&scroll)
                    .child(content),
            ))
    }
}

#[cfg(test)]
mod controller_tests {
    use super::{
        Pending, WorkspaceState,
        model::{AttemptStamp, Recovery, Run, decode},
    };
    use serde_json::json;
    fn run(number: u64) -> Run {
        let request = format!("00000000-0000-4000-8000-{number:012}");
        decode(json!({"version":1,"runId":request,"requestId":request,"workspaceId":"A","sessionId":"original-session","cwd":"/original/worktree","script":{"id":"00000000-0000-4000-8000-000000000101","version":"00000000-0000-4000-8000-000000000102","name":"build","command":"make"},"status":"running","startedAt":1,"exitCode":null,"cancelRequested":false,"logBytes":0,"logTruncated":false})).unwrap()
    }
    fn pending(run: &Run) -> Pending {
        Pending {
            request: run.request_id.clone(),
            script: run.script.clone(),
            session: run.session_id.clone(),
            cwd: run.cwd.clone(),
            recovery: Recovery::default(),
            last_error: None,
            stamp: AttemptStamp {
                attempt: 1,
                selection: 0,
            },
        }
    }
    #[test]
    fn original_request_identity_and_directory_are_required_before_retry_and_on_acknowledgement() {
        let mut actual = run(1);
        let pending = pending(&actual);
        assert!(pending.matches(&actual));
        assert!(pending.retry_target("/original/worktree").is_ok());
        assert!(pending.retry_target("/different/worktree").is_err());
        actual.session_id = Some("new-current-session".into());
        assert!(!pending.matches(&actual));
        actual.session_id = pending.session.clone();
        actual.script.version = uuid::Uuid::new_v4().to_string();
        assert!(!pending.matches(&actual));
        actual.script = pending.script.clone();
        actual.request_id = uuid::Uuid::new_v4().to_string();
        assert!(!pending.matches(&actual));
    }
    #[test]
    fn list_confirmation_supersedes_old_rpc_and_user_selection_supersedes_auto_selection() {
        let old = AttemptStamp {
            attempt: 1,
            selection: 0,
        };
        assert!(old.current(1));
        assert!(old.may_select(0));
        // Poll confirms the request; production increments attempt before accepting a later RPC.
        assert!(!old.current(2));
        assert!(!old.current(3));
        // Selecting a different run while a request is pending must not be undone by its response.
        assert!(!old.may_select(1));
    }
    #[test]
    fn two_abandoned_requests_remain_visible_and_capacity_never_discards_unresolved_ids() {
        let mut state = WorkspaceState::default();
        for number in 1..=20 {
            let mut p = pending(&run(number));
            p.recovery
                .rejected("[SCRIPT_RUN_INPUT_INVALID] Before dispatch");
            p.recovery.refreshed(true, false);
            state.pending = Some(p);
            assert!(state.release_pending());
        }
        assert_eq!(state.abandoned.len(), 20);
        assert_eq!(state.abandoned[0].request, run(1).request_id);
        assert_eq!(state.abandoned[1].request, run(2).request_id);
        let mut extra = pending(&run(21));
        extra
            .recovery
            .rejected("[SCRIPT_RUN_INPUT_INVALID] Before dispatch");
        extra.recovery.refreshed(true, false);
        state.pending = Some(extra);
        assert!(!state.release_pending());
        assert_eq!(state.abandoned.len(), 20);
        assert_eq!(state.pending.as_ref().unwrap().request, run(21).request_id);
        state.reconcile_abandoned(&[run(1)]);
        assert_eq!(state.abandoned.len(), 19);
        assert_eq!(state.resolved_requests, vec![run(1).request_id]);
        assert!(state.release_pending());
        assert!(
            state
                .abandoned
                .iter()
                .any(|p| p.request == run(2).request_id)
        );
        assert!(
            state
                .abandoned
                .iter()
                .any(|p| p.request == run(21).request_id)
        );
    }
}
