use super::*;
use serde::Deserialize;
#[derive(Deserialize)]
struct RunList {
    runs: Vec<TerminalRun>,
}

impl HiveApp {
    pub(super) fn load_terminals(&mut self, auto_attach: bool, cx: &mut Context<Self>) {
        if !self.connected || self.terminal.loading {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        self.terminal.generation += 1;
        let generation = self.terminal.generation;
        self.terminal.loading = true;
        self.terminal.error = None;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("terminal.list", json!({"workspaceId":workspace}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<RunList>(value)
                        .map_err(|_| "Shell 列表格式错误".to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.terminal.generation != generation
                    || this.workspace.as_ref() != Some(&workspace)
                    || !this.terminal.visible
                {
                    return;
                }
                this.terminal.loading = false;
                match result {
                    Ok(list) => {
                        this.terminal.runs = list.runs;
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
                                this.terminal.status =
                                    "此工作区没有运行中的 Shell；可启动一个".into();
                            }
                        }
                    }
                    Err(error) => this.terminal.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn start_terminal(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.terminal.loading || !self.workspace_writes_allowed() {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        self.terminal.generation += 1;
        let generation = self.terminal.generation;
        self.terminal.loading = true;
        self.terminal.error = None;
        self.terminal.status = "正在启动 Shell…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("terminal.start", json!({"workspaceId":workspace}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<TerminalRun>(value)
                        .map_err(|_| "Shell 启动响应格式错误".to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.terminal.generation != generation
                    || this.workspace.as_ref() != Some(&workspace)
                    || !this.terminal.visible
                {
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
        if !self.connected || !self.terminal.visible || !self.workspace_writes_allowed() {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        self.detach_terminal(cx);
        let attachment = uuid::Uuid::new_v4().to_string();
        self.terminal.attachment = Some(attachment.clone());
        self.bind_terminal_view(attachment.clone(), cx);
        self.terminal.run_id = Some(run.clone());
        self.terminal.sequence = 0;
        self.terminal.loading = true;
        self.terminal.error = None;
        self.terminal.status = "正在连接并恢复终端…".into();
        let (cols, rows) = (self.terminal.cols, self.terminal.rows);
        cx.notify();
        let reply = runtime.begin_request("terminal.attach", json!({"workspaceId":workspace,"runId":run,"attachmentId":attachment,"cols":cols,"rows":rows}));
        cx.spawn(async move |this, cx| {
            let result = reply.await;
            let current = this
                .update(cx, |this, cx| {
                    if !this.terminal_identity(&attachment, &workspace, &run) {
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
            // An attach may finish after hide/switch sent its first detach.
            if !current {
                let _ = runtime
                    .request("terminal.detach", json!({"attachmentId":attachment}))
                    .await;
            }
        })
        .detach();
    }
    pub(super) fn stop_terminal(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.terminal.loading || !self.workspace_writes_allowed() {
            return;
        }
        let (Some(workspace), Some(run), Some(runtime)) = (
            self.workspace.clone(),
            self.terminal.run_id.clone(),
            self.runtime.clone(),
        ) else {
            return;
        };
        self.terminal.loading = true;
        self.terminal.ready = false;
        self.set_terminal_input_enabled(false, cx);
        self.terminal.status = "正在停止 Shell…".into();
        cx.notify();
        let reply = runtime.begin_request(
            "terminal.stop",
            json!({"workspaceId":workspace,"runId":run}),
        );
        cx.spawn(async move |this, cx| {
            let result = reply.await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() != Some(&workspace)
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
