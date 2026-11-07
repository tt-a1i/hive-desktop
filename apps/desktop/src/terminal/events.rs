use super::*;
use serde_json::Value;

impl HiveApp {
    pub fn terminal_wire_event(&mut self, event: TerminalWireEvent, cx: &mut Context<Self>) {
        let (attachment, workspace, run) = (event.attachment_id, event.workspace_id, event.run_id);
        if !self.terminal_identity(&attachment, &workspace, &run) {
            return;
        }
        match event.frame {
            TerminalFrame::Restore {
                sequence,
                bytes,
                cols,
                rows,
            } => {
                if sequence != 1 || self.terminal.sequence != 0 {
                    self.terminal_fault("终端恢复顺序错误，请重新连接".into(), cx);
                    return;
                }
                let consumed = self.terminal.view.update(cx, |view, cx| {
                    view.reset(cx);
                    view.resize(cols, rows, cx);
                    view.feed(&bytes, cx)
                });
                if consumed != bytes.len() {
                    self.terminal_fault("终端快照未完整消费，未确认恢复".into(), cx);
                    return;
                }
                self.terminal.sequence = sequence;
                self.terminal_rpc(
                    "terminal.restoreComplete",
                    json!({"attachmentId":attachment,"sequence":sequence}),
                    true,
                    cx,
                );
            }
            TerminalFrame::Output { sequence, bytes } => {
                if sequence != self.terminal.sequence + 1 || self.terminal.sequence == 0 {
                    self.terminal_fault("终端输出顺序错误，请重新连接".into(), cx);
                    return;
                }
                let consumed = self
                    .terminal
                    .view
                    .update(cx, |view, cx| view.feed(&bytes, cx));
                if consumed != bytes.len() {
                    self.terminal_fault("终端输出未完整消费，未发送确认".into(), cx);
                    return;
                }
                self.terminal.sequence = sequence;
                self.terminal_rpc(
                    "terminal.ack",
                    json!({"attachmentId":attachment,"sequence":sequence,"bytes":consumed}),
                    false,
                    cx,
                );
            }
            TerminalFrame::Exit { code } => {
                self.detach_terminal(cx);
                self.terminal.runs.retain(|record| record.run_id != run);
                self.terminal.run_id = None;
                self.terminal.status = match code {
                    Some(code) => format!("Shell 已退出（{code}）"),
                    None => "Shell 已退出".into(),
                };
            }
            TerminalFrame::Error { message } => self.terminal_fault(message, cx),
            TerminalFrame::Closed { reason } => {
                self.detach_terminal(cx);
                self.terminal.status =
                    format!("终端视图已断开：{reason}；请重新连接确认 Shell 状态");
            }
        }
        cx.notify();
    }
    pub(super) fn terminal_component_event(
        &mut self,
        event: &TerminalEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TerminalEvent::Input { bytes, user_input } => {
                let Some(attachment) = self.terminal.attachment.clone() else {
                    return;
                };
                if !self.connected
                    || !self.terminal.visible
                    || self.terminal.workspace != self.workspace
                    || (*user_input && !self.can_type_in_terminal())
                {
                    return;
                }
                self.terminal_rpc(
                    "terminal.input",
                    json!({"attachmentId":attachment,"bytes":bytes,"userInput":user_input}),
                    false,
                    cx,
                );
            }
            TerminalEvent::Resize { cols, rows } => {
                self.terminal.cols = *cols;
                self.terminal.rows = *rows;
                if let Some(attachment) = self
                    .terminal
                    .attachment
                    .clone()
                    .filter(|_| self.terminal.ready)
                {
                    self.terminal_rpc(
                        "terminal.resize",
                        json!({"attachmentId":attachment,"cols":cols,"rows":rows}),
                        false,
                        cx,
                    );
                }
            }
            TerminalEvent::Title(title) => {
                self.terminal.title = title.clone();
                cx.notify();
            }
            TerminalEvent::Bell => {}
        }
    }
    fn terminal_rpc(
        &mut self,
        method: &'static str,
        params: Value,
        completes_restore: bool,
        cx: &mut Context<Self>,
    ) {
        let (Some(attachment), Some(workspace), Some(run), Some(runtime)) = (
            self.terminal.attachment.clone(),
            self.terminal.workspace.clone(),
            self.terminal.run_id.clone(),
            self.runtime.clone(),
        ) else {
            return;
        };
        let reply = runtime.begin_request(method, params);
        cx.spawn(async move |this, cx| {
            let result = reply.await;
            let _ = this.update(cx, |this, cx| {
                if !this.terminal_identity(&attachment, &workspace, &run) {
                    return;
                }
                match result {
                    Ok(value)
                        if value["ok"] == true
                            && (!completes_restore || value["ready"] == true) =>
                    {
                        if completes_restore {
                            this.terminal.ready = true;
                            this.terminal.loading = false;
                            this.terminal.status = "Shell 已连接".into();
                            this.set_terminal_input_enabled(this.can_type_in_terminal(), cx);
                            this.focus_ready_terminal(cx);
                            // Layout can change while the restore handshake is in flight.
                            // Publish the newest dimensions after ready, in the same writer FIFO.
                            this.terminal_rpc("terminal.resize", json!({"attachmentId":attachment,"cols":this.terminal.cols,"rows":this.terminal.rows}), false, cx);
                        }
                    }
                    Ok(_) => this.terminal_fault("终端操作未获确认，请重新连接".into(), cx),
                    Err(error) => this.terminal_fault(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
