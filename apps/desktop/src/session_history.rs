use crate::{app::HiveApp, language::tr, palette::Color, transport::Runtime};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::{Value, json};
use std::sync::Arc;

struct HistoryDialog {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    workspace: String,
    session: String,
    connection: u64,
    open_request: u64,
    item: String,
    edit: bool,
    rewind: bool,
    capabilities: Option<Value>,
    checkpoints: Vec<Value>,
    selected: Option<String>,
    checkpoint_preview: Option<Value>,
    preview: Option<Value>,
    operation: Option<Value>,
    operations: Vec<Value>,
    request_id: Option<String>,
    capture_request_id: Option<String>,
    busy: bool,
    error: Option<String>,
}
impl HiveApp {
    pub(crate) fn open_history_fork(
        &mut self,
        item: String,
        edit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_history_dialog(item, edit, false, window, cx);
    }

    pub(crate) fn open_history_rewind(
        &mut self,
        item: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_history_dialog(item, false, true, window, cx);
    }

    fn open_history_dialog(
        &mut self,
        item: String,
        edit: bool,
        rewind: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing || window.has_active_dialog(cx) {
            return;
        }
        let (Some(runtime), Some(session)) = (self.runtime.clone(), self.session.as_ref()) else {
            return;
        };
        let workspace = session.workspace_id.clone();
        let session_id = session.id.clone();
        let connection = self.connection_generation;
        let open_request = self.open_request;
        let app = cx.weak_entity();
        let editor = cx.new(|cx: &mut Context<HistoryDialog>| {
            let editor = HistoryDialog {
                app,
                runtime,
                workspace,
                session: session_id,
                connection,
                open_request,
                item,
                edit,
                rewind,
                capabilities: None,
                checkpoints: vec![],
                selected: None,
                checkpoint_preview: None,
                preview: None,
                operation: None,
                operations: vec![],
                request_id: None,
                capture_request_id: None,
                busy: false,
                error: None,
            };
            let weak = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = weak.update(cx, |this, cx| this.load(cx));
            });
            editor
        });
        let cancel = editor.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let busy = editor.read(cx).busy;
            let cancel = cancel.clone();
            dialog
                .title(tr(if rewind {
                    "原地回退"
                } else {
                    "历史分叉"
                }))
                .w(px(650.))
                .overlay_closable(false)
                .keyboard(!busy)
                .close_button(!busy)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| {
                    let dialog = cancel.read(cx);
                    if dialog.busy {
                        return false;
                    }
                    if dialog.rewind && dialog.request_id.is_some() && dialog.current(cx) {
                        let app = dialog.app.clone();
                        let session = dialog.session.clone();
                        let _ = app.update(cx, |app, cx| app.show_session(session, cx));
                    }
                    true
                })
                .child(editor.clone())
        });
    }
}
impl HistoryDialog {
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|entity| {
            let app = entity.read(cx);
            app.connected
                && !app.closing
                && app.connection_generation == self.connection
                && app.open_request == self.open_request
                && app.workspace.as_deref() == Some(&self.workspace)
                && app.session.as_ref().is_some_and(|s| s.id == self.session)
                && app
                    .runtime
                    .as_ref()
                    .is_some_and(|r| Arc::ptr_eq(r, &self.runtime))
        })
    }
    fn params(&self) -> Value {
        json!({"workspaceId":self.workspace,"sessionId":self.session})
    }
    fn load(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.current(cx) {
            return;
        }
        self.busy = true;
        self.error = None;
        let runtime = self.runtime.clone();
        let params = self.params();
        cx.spawn(async move |this, cx| {
            let caps = runtime
                .request("history.capabilities", params.clone())
                .await;
            let list = runtime.request("checkpoint.list", params).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if this.current(cx) {
                    match caps {
                        Ok(v) => this.capabilities = Some(v),
                        Err(e) => this.error = Some(e),
                    }
                    match list {
                        Ok(v) => {
                            this.checkpoints =
                                v["checkpoints"].as_array().cloned().unwrap_or_default()
                        }
                        Err(e) => this.error = Some(e),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn call(&mut self, method: &'static str, params: Value, cx: &mut Context<Self>) {
        if self.busy || !self.current(cx) {
            return;
        }
        self.busy = true;
        self.error = None;
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if this.current(cx) {
                    match result {
                        Err(e) => this.error = Some(e),
                        Ok(v) => match method {
                            "checkpoint.preview" => {
                                this.checkpoint_preview = Some(v);
                                this.capture_request_id = None;
                            }
                            "checkpoint.capture" => {
                                this.checkpoint_preview = None;
                                this.load(cx);
                            }
                            "history.fork.preview" | "history.rewind.preview" => {
                                this.preview = Some(v);
                                this.request_id = None;
                            }
                            "history.fork.apply"
                            | "history.rewind.apply"
                            | "history.operations.get"
                            | "history.operations.recover" => this.operation = Some(v),
                            "history.operations.list" => {
                                this.operations =
                                    v["operations"].as_array().cloned().unwrap_or_default()
                            }
                            _ => {}
                        },
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn fork_preview(&mut self, cx: &mut Context<Self>) {
        let code = self
            .selected
            .as_ref()
            .map(|id| json!({"kind":"checkpoint","checkpointId":id}))
            .unwrap_or(json!({"kind":"current"}));
        self.preview = None;
        self.operation = None;
        self.request_id = None;
        let method = if self.rewind {
            "history.rewind.preview"
        } else {
            "history.fork.preview"
        };
        self.call(method, json!({"workspaceId":self.workspace,"sessionId":self.session,"boundary":{"nativeUserItemId":self.item},"code":code,"edit":self.edit}), cx);
    }
    fn open_ready(&mut self, value: Value, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !self.current(cx) || value["state"] != "ready" {
            return;
        }
        let Ok(session) =
            serde_json::from_value::<crate::protocol::Session>(value["session"].clone())
        else {
            self.error = Some(tr("操作结果缺少可打开的会话。").into());
            cx.notify();
            return;
        };
        if session.workspace_id != self.workspace {
            self.error = Some(tr("操作结果中的项目不匹配。").into());
            cx.notify();
            return;
        }
        self.busy = true;
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if !this.current(cx) {
                    cx.notify();
                    return;
                }
                let accepted = window
                    .update(cx, |_, window, cx| {
                        this.app.update(cx, |app, cx| {
                            app.sessions.retain(|s| s.id != session.id);
                            app.sessions.insert(0, session.clone());
                            app.show_session(session.id.clone(), cx);
                            window.close_dialog(cx);
                        })
                    })
                    .is_ok_and(|r| r.is_ok());
                if !accepted {
                    this.error = Some(tr("无法打开历史操作中的会话。").into());
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn apply(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self
            .preview
            .as_ref()
            .and_then(|v| v["previewToken"].as_str())
            .map(str::to_owned)
        else {
            return;
        };
        let request = self
            .request_id
            .get_or_insert_with(|| uuid::Uuid::new_v4().to_string())
            .clone();
        let mut params = json!({"workspaceId":self.workspace,"sessionId":self.session,"previewToken":token,"requestId":request});
        let method = if self.rewind {
            params["confirmed"] = json!(true);
            "history.rewind.apply"
        } else {
            "history.fork.apply"
        };
        self.call(method, params, cx);
    }
}
fn recoverable(value: &Value) -> bool {
    value["recoverable"] == true
        && matches!(
            value["state"].as_str(),
            Some("uncertain" | "nativeKnown" | "metadataReady")
        )
        && value["operationId"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
}
fn rewind_backup(operation: &Value) -> Option<Value> {
    if operation["state"] == "ready" && operation["backupSession"].is_object() {
        return Some(json!({"state": "ready", "session": operation["backupSession"]}));
    }
    None
}
fn strings(value: &Value) -> String {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("，")
}
impl Render for HistoryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current(cx);
        let disabled = !current || self.busy || self.operation.is_some();
        let selection_disabled = disabled || self.request_id.is_some();
        let allowed = self.capabilities.as_ref().is_some_and(|v| {
            v[if self.rewind {
                "rewind"
            } else if self.edit {
                "edit"
            } else {
                "fork"
            }] == true
        });
        let can_checkpoint = self.capabilities.as_ref().is_some_and(|v| {
            v[if self.rewind {
                "rewindCode"
            } else {
                "checkpoint"
            }] == true
        });
        let rewind = self.rewind;
        let checkpoints = if rewind && !can_checkpoint {
            vec![]
        } else {
            self.checkpoints.clone()
        };
        v_flex().id("history-dialog-content").gap_2().min_w_0().max_h(window.viewport_size().height * 0.75).overflow_y_scroll()
            .child(div().text_sm().child(tr(if rewind {"将原会话退回到所选消息之前。完整历史和当前代码会先备份到新的会话与工作树。"} else if self.edit {"在选中消息之前创建新分支；原消息成为未发送草稿。"} else {"保留选中回合，创建独立历史分支。"})))
            .child(div().text_sm().child(tr(if rewind {"回退仅处理对话和选定的代码。Hive 的任务、派发和汇报保留现状。"} else {"原会话和 Hive 协作状态保持不变。创建后不会自动发送或激活新会话。"})))
            .when(!rewind, |v| v.child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr("当前代码表示操作时的代码，并非该历史回合的代码。忽略文件保留在原目录，不会复制到新工作树。"))))
            .when(rewind && allowed && !can_checkpoint, |v| v.child(div().text_sm().child(tr("共享项目目录仅支持对话回退。"))))
            .when(!current, |v| v.child(div().id("history-context-changed").role(Role::Alert).child(tr("项目、会话或后台连接已变化，请关闭后重新打开。"))))
            .when_some(self.capabilities.as_ref().and_then(|v| v["reason"].as_str()).map(str::to_owned), |v, reason| v.child(div().text_sm().child(reason)))
            .child(h_flex().gap_2().flex_wrap()
                .child(Button::new("history-current-code").outline().small().selected(self.selected.is_none()).disabled(selection_disabled).label(tr(if rewind { "保留当前代码" } else { "历史对话 + 当前代码" }))
                    .on_click(cx.listener(|this, _, _, cx| {this.selected = None; this.preview = None; cx.notify();})))
                .children(checkpoints.into_iter().enumerate().filter_map(|(i, checkpoint)| {
                    let id = checkpoint["checkpointId"].as_str()?.to_owned(); let selected = self.selected.as_ref() == Some(&id); let label = format!("{} {}", tr("检查点"), id);
                    Some(Button::new(("history-checkpoint", i)).outline().small().selected(selected).disabled(selection_disabled).label(label)
                        .on_click(cx.listener(move |this, _, _, cx| {this.selected = Some(id.clone()); this.preview = None; cx.notify();})))
                })))
            .when(!rewind || can_checkpoint, |v| v.child(Button::new("history-checkpoint-preview").outline().small().disabled(selection_disabled || !can_checkpoint).label(tr("预览当前代码检查点"))
                .on_click(cx.listener(|this, _, _, cx| this.call("checkpoint.preview", this.params(), cx)))))
            .when_some(self.checkpoint_preview.clone(), |v, p| {
                let exclusions = strings(&p["exclusions"]); let version = p["version"].as_str().unwrap_or_default().to_owned();
                let can = p["canCapture"] == true && !version.is_empty();
                v.child(div().text_sm().child(format!("{}: {}", tr("排除的忽略文件"), exclusions)))
                    .child(div().text_sm().child(strings(&p["unsupported"])))
                    .child(Button::new("history-checkpoint-capture").outline().small().disabled(selection_disabled || !can).label(tr("保存检查点"))
                        .on_click(cx.listener(move |this, _, _, cx| {let mut params = this.params(); params["expectedVersion"] = json!(version); params["requestId"] = json!(this.capture_request_id.get_or_insert_with(|| uuid::Uuid::new_v4().to_string()).clone()); this.call("checkpoint.capture", params, cx);})))
            })
            .child(Button::new("history-fork-preview").outline().small().disabled(selection_disabled || !allowed).label(tr(if rewind { "预览回退" } else { "预览分叉" }))
                .on_click(cx.listener(|this, _, _, cx| this.fork_preview(cx))))
            .when_some(self.preview.clone(), |v, p| {
                let relation = p["codeRelation"]["label"].as_str().unwrap_or_default().to_owned();
                let exclusions = strings(&p["exclusions"]); let warnings = strings(&p["warnings"]);
                let draft = p["draft"]["text"].as_str().map(str::to_owned); let missing = strings(&p["draft"]["missingAttachments"]); let attachments = strings(&p["draft"]["attachmentIds"]);
                v.child(div().text_sm().child(relation)).child(div().text_sm().child(format!("{}: {}", tr("排除的忽略文件"), exclusions)))
                    .when_some(p["removedTurns"].as_u64(), |v, count| v.child(div().text_sm().child(format!("{} · {count}", tr("原会话将移除的回合数")))))
                    .when(!warnings.is_empty(), |v| v.child(div().text_sm().child(warnings)))
                    .when_some(draft, |v, draft| v.child(div().text_sm().child(format!("{}: {}", tr("未发送草稿"), draft))))
                    .when(!attachments.is_empty(), |v| v.child(div().text_sm().child(format!("{}: {}", tr("附件"), attachments))))
                    .when(!missing.is_empty(), |v| v.child(div().text_sm().text_color(Color::Error.get(cx)).child(format!("{}: {}", tr("缺失的附件"), missing))))
                    .child(Button::new("history-fork-apply").primary().small().disabled(disabled || p["previewToken"].as_str().is_none()).label(tr(if rewind { "确认原地回退" } else { "创建分叉" }))
                        .on_click(cx.listener(|this, _, _, cx| this.apply(cx))))
            })
            .when_some(self.operation.clone(), |v, op| {
                let state = op["state"].as_str().unwrap_or("unknown").to_owned();
                let operation_id = op["operationId"].as_str().unwrap_or_default().to_owned();
                let recovery_id = operation_id.clone(); let can_recover = recoverable(&op);
                let rewound = op["rewind"] == true;
                let backup = rewind_backup(&op);
                v.child(div().id("history-operation-status").role(Role::Status).text_sm().child(format!("{}: {} · {}", tr("操作状态"), state, operation_id)))
                    .when_some(op["error"].as_str().map(str::to_owned), |v, e| v.child(div().text_sm().child(e)))
                    .when_some(op["sessionUnavailable"].as_str().map(str::to_owned), |v, e| v.child(div().text_sm().child(e)))
                    .when(op["state"] == "ready" && op["session"].is_object(), |v| v.child(Button::new("history-open-ready").outline().small().disabled(!current || self.busy).label(tr(if rewound { "重新打开原会话" } else { "打开分叉会话" }))
                        .on_click(cx.listener(move |this, _, window, cx| this.open_ready(op.clone(), window, cx)))))
                    .when_some(backup, |v, backup| v.child(Button::new("history-open-backup").outline().small().disabled(!current || self.busy).label(tr("打开回退前的备份"))
                        .on_click(cx.listener(move |this, _, window, cx| this.open_ready(backup.clone(), window, cx)))))
                    .when(can_recover, |v| v.child(Button::new("history-operation-recover").outline().small().disabled(!current || self.busy).label(tr(if rewound { "核验并恢复回退" } else { "核验并恢复分叉" }))
                        .on_click(cx.listener(move |this, _, _, cx| this.call("history.operations.recover", json!({"workspaceId":this.workspace,"operationId":recovery_id}), cx)))))
                    .child(Button::new("history-operation-refresh").outline().small().disabled(!current || self.busy || operation_id.is_empty()).label(tr("刷新操作状态"))
                        .on_click(cx.listener(move |this, _, _, cx| this.call("history.operations.get", json!({"workspaceId":this.workspace,"operationId":operation_id}), cx))))
            })
            .child(Button::new("history-operations-list").outline().small().disabled(!current || self.busy).label(tr("查看历史操作"))
                .on_click(cx.listener(|this, _, _, cx| this.call("history.operations.list", json!({"workspaceId":this.workspace}), cx))))
            .children(self.operations.clone().into_iter().enumerate().filter_map(|(i, op)| {
                let id = op["operationId"].as_str()?.to_owned();
                let label = format!("{} · {}", id, op["state"].as_str().unwrap_or("unknown"));
                Some(Button::new(("history-operation", i)).outline().small().disabled(!current || self.busy).label(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.call("history.operations.get", json!({"workspaceId":this.workspace,"operationId":id}), cx))))
            }))
            .when_some(self.error.clone(), |v, e| v.child(div().id("history-error").role(Role::Alert).text_sm().text_color(Color::Error.get(cx)).child(e)))
    }
}

struct OperationsDialog {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    workspace: String,
    connection: u64,
    operations: Vec<Value>,
    busy: bool,
    error: Option<String>,
}
impl HiveApp {
    pub(crate) fn open_history_operations(
        &mut self,
        project: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing || window.has_active_dialog(cx) {
            return;
        }
        let Some(workspace) = project.or_else(|| self.workspace.clone()) else {
            return;
        };
        if self.workspace.as_ref() != Some(&workspace) {
            self.select_workspace(workspace.clone(), cx);
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        let app = cx.weak_entity();
        let editor = cx.new(|cx: &mut Context<OperationsDialog>| {
            let editor = OperationsDialog {
                app,
                runtime,
                workspace,
                connection,
                operations: vec![],
                busy: false,
                error: None,
            };
            let weak = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = weak.update(cx, |this, cx| this.load(cx));
            });
            editor
        });
        let cancel = editor.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let cancel = cancel.clone();
            let busy = editor.read(cx).busy;
            dialog
                .title(tr("历史操作"))
                .w(px(620.))
                .overlay_closable(false)
                .keyboard(!busy)
                .close_button(!busy)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !cancel.read(cx).busy)
                .child(editor.clone())
        });
    }
}
impl OperationsDialog {
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|e| {
            let app = e.read(cx);
            app.connected
                && !app.closing
                && app.connection_generation == self.connection
                && app.workspace.as_deref() == Some(&self.workspace)
                && app
                    .runtime
                    .as_ref()
                    .is_some_and(|r| Arc::ptr_eq(r, &self.runtime))
        })
    }
    fn load(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.current(cx) {
            return;
        }
        self.busy = true;
        self.error = None;
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("history.operations.list", json!({"workspaceId":workspace}))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if this.current(cx) {
                    match result {
                        Ok(v) => {
                            this.operations =
                                v["operations"].as_array().cloned().unwrap_or_default()
                        }
                        Err(e) => this.error = Some(e),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn recover(&mut self, operation_id: String, cx: &mut Context<Self>) {
        if self.busy
            || !self.current(cx)
            || !self
                .operations
                .iter()
                .any(|op| op["operationId"].as_str() == Some(&operation_id) && recoverable(op))
        {
            return;
        }
        self.busy = true;
        self.error = None;
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "history.operations.recover",
                    json!({"workspaceId":workspace,"operationId":operation_id}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if this.current(cx) {
                    match result {
                        Ok(value) if value["operationId"].as_str() == Some(&operation_id) => {
                            if let Some(op) = this
                                .operations
                                .iter_mut()
                                .find(|op| op["operationId"].as_str() == Some(&operation_id))
                            {
                                *op = value;
                            }
                        }
                        Ok(_) => this.error = Some(tr("操作结果的身份不匹配。").into()),
                        Err(e) => this.error = Some(e),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn open_ready(&mut self, value: Value, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !self.current(cx) || value["state"] != "ready" {
            return;
        }
        let Ok(session) =
            serde_json::from_value::<crate::protocol::Session>(value["session"].clone())
        else {
            self.error = Some(tr("操作结果缺少可打开的会话。").into());
            cx.notify();
            return;
        };
        if session.workspace_id != self.workspace {
            self.error = Some(tr("操作结果中的项目不匹配。").into());
            cx.notify();
            return;
        }
        self.busy = true;
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if !this.current(cx) {
                    cx.notify();
                    return;
                }
                let accepted = window
                    .update(cx, |_, window, cx| {
                        this.app.update(cx, |app, cx| {
                            app.sessions.retain(|s| s.id != session.id);
                            app.sessions.insert(0, session.clone());
                            app.show_session(session.id.clone(), cx);
                            window.close_dialog(cx);
                        })
                    })
                    .is_ok_and(|r| r.is_ok());
                if !accepted {
                    this.error = Some(tr("无法打开分叉会话。").into());
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
impl Render for OperationsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current(cx);
        v_flex()
            .id("workspace-history-operations")
            .gap_2()
            .max_h(px(480.))
            .overflow_y_scroll()
            .child(
                div()
                    .text_sm()
                    .child(tr("查看项目的分叉与回退记录。结果未确认时可以手动核验。")),
            )
            .when(!current, |v| {
                v.child(
                    div()
                        .id("operations-context-changed")
                        .role(Role::Alert)
                        .child(tr("项目或后台连接已变化，请关闭后重新打开。")),
                )
            })
            .child(
                Button::new("workspace-operations-refresh")
                    .outline()
                    .small()
                    .disabled(!current || self.busy)
                    .label(tr("刷新操作状态"))
                    .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
            )
            .when(!self.busy && self.operations.is_empty(), |v| {
                v.child(div().text_sm().child(tr("没有历史操作。")))
            })
            .children(
                self.operations
                    .clone()
                    .into_iter()
                    .enumerate()
                    .map(|(i, op)| {
                        let id = op["operationId"].as_str().unwrap_or_default();
                        let state = op["state"].as_str().unwrap_or("unknown");
                        let recovery_id = id.to_owned();
                        let can_recover = recoverable(&op);
                        let rewind = op["rewind"] == true;
                        let backup = rewind_backup(&op);
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child(format!(
                                "{} · {state} · {id}",
                                tr(if rewind {
                                    "原地回退"
                                } else {
                                    "历史分叉"
                                })
                            )))
                            .when_some(op["error"].as_str().map(str::to_owned), |v, e| {
                                v.child(div().text_sm().child(e))
                            })
                            .when(can_recover, |v| {
                                v.child(
                                    Button::new(("recover-history-operation", i))
                                        .outline()
                                        .small()
                                        .disabled(!current || self.busy)
                                        .label(tr(if rewind {
                                            "核验并恢复回退"
                                        } else {
                                            "核验并恢复分叉"
                                        }))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.recover(recovery_id.clone(), cx)
                                        })),
                                )
                            })
                            .when_some(
                                op["sessionUnavailable"].as_str().map(str::to_owned),
                                |v, e| v.child(div().text_sm().child(e)),
                            )
                            .when(op["state"] == "ready" && op["session"].is_object(), |v| {
                                v.child(
                                    Button::new(("open-history-child", i))
                                        .outline()
                                        .small()
                                        .disabled(!current || self.busy)
                                        .label(tr(if rewind {
                                            "重新打开原会话"
                                        } else {
                                            "打开分叉会话"
                                        }))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_ready(op.clone(), window, cx)
                                        })),
                                )
                            })
                            .when_some(backup, |v, backup| {
                                v.child(
                                    Button::new(("open-history-rewind-backup", i))
                                        .outline()
                                        .small()
                                        .disabled(!current || self.busy)
                                        .label(tr("打开回退前的备份"))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_ready(backup.clone(), window, cx)
                                        })),
                                )
                            })
                    }),
            )
            .when_some(self.error.clone(), |v, e| {
                v.child(
                    div()
                        .id("operations-error")
                        .role(Role::Alert)
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(e),
                )
            })
    }
}

impl HiveApp {
    pub(crate) fn restore_fork_draft(
        &mut self,
        session: String,
        workspace: String,
        operation: String,
        draft: Value,
        cx: &mut Context<Self>,
    ) {
        let key = format!("session:{session}");
        if self
            .draft_store
            .as_ref()
            .is_some_and(|s| s.fork_imported(&key, &operation))
        {
            return;
        }
        let Some(text) = draft["text"].as_str().map(str::to_owned) else {
            return;
        };
        let Some(ids) = draft["attachmentIds"].as_array().and_then(|v| {
            v.iter()
                .map(|id| id.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        }) else {
            return;
        };
        if draft["missingAttachments"]
            .as_array()
            .is_none_or(|v| !v.is_empty())
        {
            self.error = Some(crate::app::ErrorNotice::Conversation(Box::new(
                tr("分叉草稿有缺失附件，未导入；请检查原会话。").into(),
            )));
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        let request = self.open_request;
        cx.spawn(async move |this, cx| {
            let result = if ids.is_empty() {
                Ok(vec![])
            } else {
                runtime
                    .request(
                        "chat.attachments.get",
                        json!({"workspaceId":workspace,"ids":ids}),
                    )
                    .await
                    .and_then(|v| {
                        let items = v.as_array().ok_or("Invalid attachment response")?;
                        let refs = items
                            .iter()
                            .map(|v| {
                                serde_json::from_value::<crate::protocol::AttachmentRef>(
                                    v["attachment"].clone(),
                                )
                                .map_err(|e| e.to_string())
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        if refs.len() != ids.len()
                            || refs
                                .iter()
                                .zip(&ids)
                                .any(|(r, id)| &r.id != id || r.workspace_id != workspace)
                        {
                            return Err("Attachment identity mismatch".into());
                        }
                        Ok(refs)
                    })
            };
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.open_request != request
                    || this.workspace.as_deref() != Some(&workspace)
                    || this.session.as_ref().is_none_or(|s| s.id != session)
                {
                    return;
                }
                let refs = match result {
                    Ok(v) => v,
                    Err(e) => {
                        this.error = Some(crate::app::ErrorNotice::Conversation(Box::new(
                            format!("{}: {e}", tr("无法恢复分叉草稿附件")).into(),
                        )));
                        cx.notify();
                        return;
                    }
                };
                if this.drafts.get(&key).is_some_and(|v| !v.is_empty())
                    || !this.attachments.refs(Some(&key)).is_empty()
                    || (this.draft_owner.as_ref() == Some(&key)
                        && !this.input.read(cx).value().is_empty())
                {
                    let marked = this.flush_drafts(cx).and_then(|_| {
                        this.draft_store
                            .as_mut()
                            .ok_or_else(|| std::io::Error::other("Draft store unavailable"))?
                            .mark_existing_fork(&key, &operation)
                    });
                    if let Err(e) = marked {
                        this.error = Some(crate::app::ErrorNotice::Conversation(Box::new(
                            format!("{}: {e}", tr("无法保存分叉草稿")).into(),
                        )));
                        cx.notify();
                    }
                    return;
                }
                let result = this
                    .draft_store
                    .as_mut()
                    .ok_or_else(|| "Draft store unavailable".to_owned())
                    .and_then(|store| {
                        store
                            .import_fork(&key, &operation, text.clone(), refs.clone())
                            .map_err(|e| e.to_string())
                    });
                match result {
                    Ok(()) => {
                        this.drafts.insert(key.clone(), text);
                        if !refs.is_empty() {
                            this.attachments.drafts.insert(key.clone(), refs);
                        }
                        this.draft_owner = None;
                    }
                    Err(e) => {
                        this.error = Some(crate::app::ErrorNotice::Conversation(Box::new(
                            format!("{}: {e}", tr("无法保存分叉草稿")).into(),
                        )))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::recoverable;
    use serde_json::json;
    #[test]
    fn recovery_requires_server_authorization_and_a_recoverable_state() {
        for state in ["uncertain", "nativeKnown", "metadataReady"] {
            assert!(recoverable(
                &json!({"operationId":"op", "state":state, "recoverable":true})
            ));
            assert!(!recoverable(
                &json!({"operationId":"op", "state":state, "recoverable":false})
            ));
        }
        for state in ["ready", "failed", "nativeRequested", "prepared"] {
            assert!(!recoverable(
                &json!({"operationId":"op", "state":state, "recoverable":true})
            ));
        }
        assert!(!recoverable(
            &json!({"state":"uncertain", "recoverable":true})
        ));
    }
}
