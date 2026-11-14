use super::*;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunList {
    runs: Vec<TerminalRun>,
    workspace_id: Option<String>,
    session_id: Option<String>,
    source_unavailable: Option<String>,
}
fn checked_runs(list: RunList, target: &TerminalTarget) -> Result<RunList, String> {
    if list
        .workspace_id
        .as_ref()
        .is_some_and(|workspace| workspace != &target.workspace_id)
        || list.session_id != target.session_id
        || (target.session_id.is_some() && list.workspace_id.as_ref() != Some(&target.workspace_id))
        || list.runs.iter().any(|run| {
            run.session_id != target.session_id
                || run.run_id.is_empty()
                || !matches!(run.status.as_str(), "starting" | "running")
        })
    {
        return Err("后台未确认终端的工作区与会话身份，已拒绝连接。".into());
    }
    Ok(list)
}
impl TerminalTarget {
    fn params(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("terminal identity serializes")
    }
}
impl HiveApp {
    pub(super) fn load_terminals(&mut self, auto_attach: bool, cx: &mut Context<Self>) {
        self.sync_terminal_workspace(cx);
        if !self.connected || self.terminal.loading {
            return;
        }
        let Some(mut request) = self.terminal_request_target() else {
            return;
        };
        self.terminal.generation += 1;
        request.generation = self.terminal.generation;
        self.terminal.loading = true;
        self.terminal.error = None;
        self.terminal.scope_confirmed = false;
        cx.spawn(async move |this, cx| {
            let result = request
                .runtime
                .request("terminal.list", request.target.params())
                .await
                .and_then(|value| {
                    let list = serde_json::from_value::<RunList>(value)
                        .map_err(|_| "Shell 列表格式错误".to_string())?;
                    checked_runs(list, &request.target)
                });
            let _ = this.update(cx, |this, cx| {
                if !this.terminal_request_current(&request) {
                    return;
                }
                this.terminal.loading = false;
                match result {
                    Ok(list) => {
                        this.terminal.scope_confirmed = true;
                        this.terminal.runs = list.runs;
                        this.terminal.source_unavailable = list.source_unavailable;
                        if let Some(reason) = this.terminal.source_unavailable.clone() {
                            this.detach_terminal(cx);
                            this.terminal.error = Some(reason);
                            if !this
                                .terminal
                                .runs
                                .iter()
                                .any(|run| Some(&run.run_id) == this.terminal.run_id.as_ref())
                            {
                                this.terminal.run_id =
                                    this.terminal.runs.first().map(|run| run.run_id.clone());
                            }
                            this.terminal.status = "执行目录不可用；可停止已运行的 Shell".into();
                            cx.notify();
                            return;
                        }
                        if auto_attach && this.terminal.attachment.is_none() {
                            let next = this
                                .terminal
                                .run_id
                                .as_ref()
                                .and_then(|id| {
                                    this.terminal.runs.iter().find(|run| &run.run_id == id)
                                })
                                .or_else(|| this.terminal.runs.first())
                                .map(|run| run.run_id.clone());
                            if let Some(run) = next {
                                this.attach_terminal(run, cx);
                            } else {
                                this.terminal.run_id = None;
                                this.terminal.status = if request.target.session_id.is_some() {
                                    "此会话没有运行中的 Shell；可启动一个"
                                } else {
                                    "此工作区没有运行中的 Shell；可启动一个"
                                }
                                .into();
                            }
                        }
                    }
                    Err(error) => {
                        this.terminal.scope_confirmed = false;
                        this.terminal_fault(error, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn start_terminal(&mut self, cx: &mut Context<Self>) {
        self.sync_terminal_workspace(cx);
        if !self.connected || self.terminal.loading || !self.workspace_writes_allowed() {
            return;
        }
        let Some(mut request) = self.terminal_request_target() else {
            return;
        };
        if self.terminal.source_unavailable.is_some() {
            return;
        }
        if request.target.session_id.is_some() && !self.terminal.scope_confirmed {
            self.terminal.error = Some("请先刷新并确认当前会话终端身份，再启动 Shell。".into());
            cx.notify();
            return;
        }
        self.terminal.generation += 1;
        request.generation = self.terminal.generation;
        self.terminal.loading = true;
        self.terminal.error = None;
        self.terminal.status = "正在启动 Shell…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = request
                .runtime
                .request("terminal.start", request.target.params())
                .await
                .and_then(|value| {
                    let run = serde_json::from_value::<TerminalRun>(value)
                        .map_err(|_| "Shell 启动响应格式错误".to_string())?;
                    if run.session_id != request.target.session_id || run.run_id.is_empty() {
                        return Err("后台未确认终端的工作区与会话身份，已拒绝连接。".into());
                    }
                    Ok(run)
                });
            let _ = this.update(cx, |this, cx| {
                if !this.terminal_request_current(&request) {
                    return;
                }
                this.terminal.loading = false;
                match result {
                    Ok(run) => {
                        let id = run.run_id.clone();
                        this.terminal.runs.push(run);
                        this.attach_terminal(id, cx);
                    }
                    Err(error) => {
                        this.terminal.status = "Shell 启动失败".into();
                        this.terminal.error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn attach_terminal(&mut self, run: String, cx: &mut Context<Self>) {
        self.sync_terminal_workspace(cx);
        if !self.connected || !self.terminal.visible || !self.workspace_writes_allowed() {
            return;
        }
        if !self.terminal.scope_confirmed
            || !self
                .terminal
                .runs
                .iter()
                .any(|item| item.run_id == run && item.session_id == self.terminal.session_id)
        {
            return;
        }
        if self.terminal.source_unavailable.is_some() {
            // Selecting an owned run remains possible for Stop, without reconnecting its PTY.
            self.terminal.run_id = Some(run);
            self.terminal.status = "执行目录不可用；可停止已运行的 Shell".into();
            cx.notify();
            return;
        }
        self.detach_terminal(cx);
        let attachment = uuid::Uuid::new_v4().to_string();
        self.terminal.attachment = Some(attachment.clone());
        self.bind_terminal_view(attachment.clone(), cx);
        self.terminal.run_id = Some(run.clone());
        self.terminal.sequence = 0;
        self.terminal.loading = true;
        self.terminal.error = None;
        self.terminal.status = "正在连接并恢复终端…".into();
        let Some(request) = self.terminal_request_target() else {
            return;
        };
        let mut params = request.target.params();
        params["runId"] = json!(run);
        params["attachmentId"] = json!(attachment);
        params["cols"] = json!(self.terminal.cols);
        params["rows"] = json!(self.terminal.rows);
        cx.notify();
        let reply = request.runtime.begin_request("terminal.attach", params);
        cx.spawn(async move |this, cx| {
            let result = reply.await;
            let current = this
                .update(cx, |this, cx| {
                    if !this.terminal_request_current(&request)
                        || !this.terminal_identity(
                            &attachment,
                            &request.target.workspace_id,
                            request.target.session_id.as_deref(),
                            &run,
                        )
                    {
                        return false;
                    }
                    match result {
                        Ok(value) if value["ok"] == true && value["attachmentId"] == attachment => {
                        }
                        Ok(_) => this.terminal_fault("终端连接响应身份不匹配".into(), cx),
                        Err(error) => this.terminal_fault(error, cx),
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !current {
                let _ = request
                    .runtime
                    .request("terminal.detach", json!({"attachmentId":attachment}))
                    .await;
            }
        })
        .detach();
    }
    pub(super) fn stop_terminal(&mut self, cx: &mut Context<Self>) {
        self.sync_terminal_workspace(cx);
        if !self.connected
            || !self.terminal.scope_confirmed
            || self.terminal.loading
            || !self.workspace_writes_allowed()
        {
            return;
        }
        let (Some(mut request), Some(run)) =
            (self.terminal_request_target(), self.terminal.run_id.clone())
        else {
            return;
        };
        if !self
            .terminal
            .runs
            .iter()
            .any(|item| item.run_id == run && item.session_id == request.target.session_id)
        {
            return;
        }
        self.terminal.generation += 1;
        request.generation = self.terminal.generation;
        self.terminal.loading = true;
        self.terminal.ready = false;
        self.set_terminal_input_enabled(false, cx);
        self.terminal.status = "正在停止 Shell…".into();
        cx.notify();
        let mut params = request.target.params();
        params["runId"] = json!(run);
        let reply = request.runtime.begin_request("terminal.stop", params);
        cx.spawn(async move |this, cx| {
            let result = reply.await;
            let _ = this.update(cx, |this, cx| {
                if !this.terminal_request_current(&request)
                    || this.terminal.run_id.as_ref() != Some(&run)
                {
                    return;
                }
                this.terminal.loading = false;
                match result {
                    Ok(value) if value["ok"] == true => {
                        this.detach_terminal(cx);
                        this.terminal.run_id = None;
                        this.terminal.runs.retain(|record| record.run_id != run);
                        this.terminal.status = "Shell 已停止".into();
                    }
                    Ok(_) => {
                        this.terminal_fault("未能确认 Shell 已停止，请刷新后查看状态".into(), cx)
                    }
                    Err(error) => this.terminal_fault(format!("停止未确认：{error}"), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
#[cfg(test)]
mod tests {
    use super::{checked_runs, RunList, TerminalTarget};
    use serde_json::json;
    #[test]
    fn session_lists_require_explicit_identity_even_when_empty() {
        let target = TerminalTarget {
            workspace_id: "workspace".into(),
            session_id: Some("session".into()),
        };
        let decode = |value| serde_json::from_value::<RunList>(value).unwrap();
        assert!(checked_runs(decode(json!({"runs":[]})), &target).is_err());
        assert!(checked_runs(
            decode(json!({"workspaceId":"workspace","sessionId":"session","runs":[]})),
            &target
        )
        .is_ok());
        assert!(checked_runs(
            decode(json!({"workspaceId":"workspace","sessionId":"other","runs":[]})),
            &target
        )
        .is_err());
        assert!(checked_runs(decode(json!({"workspaceId":"workspace","sessionId":"session","runs":[{"runId":"r","name":"shell","status":"running"}]})),&target).is_err());
        let workspace = TerminalTarget {
            workspace_id: "workspace".into(),
            session_id: None,
        };
        assert!(checked_runs(decode(json!({"runs":[]})), &workspace).is_ok());
        assert!(target.params()["sessionId"] == "session");
        assert!(workspace.params().get("sessionId").is_none());
    }
}

#[cfg(test)]
mod source_tests {
    use super::{checked_runs, RunList, TerminalTarget};
    use serde_json::json;
    #[test]
    fn unavailable_source_retains_owned_runs_but_requires_exact_scope() {
        let target = TerminalTarget {
            workspace_id: "w".into(),
            session_id: Some("s".into()),
        };
        let value = json!({"workspaceId":"w","sessionId":"s","sourceUnavailable":"source directory is missing","runs":[{"runId":"owned","sessionId":"s","name":"shell","status":"running"}]});
        let list = checked_runs(
            serde_json::from_value::<RunList>(value.clone()).unwrap(),
            &target,
        )
        .unwrap();
        assert_eq!(
            list.source_unavailable.as_deref(),
            Some("source directory is missing")
        );
        assert_eq!(list.runs[0].run_id, "owned");
        let mut foreign = value;
        foreign["sessionId"] = json!("other");
        assert!(checked_runs(serde_json::from_value(foreign).unwrap(), &target).is_err());
        let restored = checked_runs(
            serde_json::from_value(json!({"workspaceId":"w","sessionId":"s","runs":[]})).unwrap(),
            &target,
        )
        .unwrap();
        assert!(restored.source_unavailable.is_none());
    }
}
