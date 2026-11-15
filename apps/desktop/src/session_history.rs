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
                .title(tr("历史分叉"))
                .w(px(650.))
                .overlay_closable(false)
                .keyboard(!busy)
                .close_button(!busy)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !cancel.read(cx).busy)
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
                            "history.fork.preview" => {
                                this.preview = Some(v);
                                this.request_id = None;
                            }
                            "history.fork.apply" | "history.operations.get" => {
                                this.operation = Some(v)
                            }
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
        self.call("history.fork.preview", json!({"workspaceId":self.workspace,"sessionId":self.session,"boundary":{"nativeUserItemId":self.item},"code":code,"edit":self.edit}), cx);
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
        self.call("history.fork.apply", json!({"workspaceId":self.workspace,"sessionId":self.session,"previewToken":token,"requestId":request}), cx);
    }
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
        let allowed = self
            .capabilities
            .as_ref()
            .is_some_and(|v| v[if self.edit { "edit" } else { "fork" }] == true);
        let can_checkpoint = self
            .capabilities
            .as_ref()
            .is_some_and(|v| v["checkpoint"] == true);
        let checkpoints = self.checkpoints.clone();
        v_flex().id("history-dialog-content").gap_2().min_w_0().max_h(window.viewport_size().height * 0.75).overflow_y_scroll()
            .child(div().text_sm().child(tr(if self.edit {"在选中消息之前创建新分支；原消息成为未发送草稿。"} else {"保留选中回合，创建独立历史分支。"})))
            .child(div().text_sm().child(tr("原会话和 Hive 协作状态保持不变。创建后不会自动发送或激活新会话。")))
            .child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr("当前代码表示操作时的代码，并非该历史回合的代码。忽略文件保留在原目录，不会复制到新工作树。")))
            .when(!current, |v| v.child(div().id("history-context-changed").role(Role::Alert).child(tr("项目、会话或后台连接已变化，请关闭后重新打开。"))))
            .when_some(self.capabilities.as_ref().and_then(|v| v["reason"].as_str()).map(str::to_owned), |v, reason| v.child(div().text_sm().child(reason)))
            .child(h_flex().gap_2().flex_wrap()
                .child(Button::new("history-current-code").outline().small().selected(self.selected.is_none()).disabled(disabled).label(tr("历史对话 + 当前代码"))
                    .on_click(cx.listener(|this, _, _, cx| {this.selected = None; this.preview = None; cx.notify();})))
                .children(checkpoints.into_iter().enumerate().filter_map(|(i, checkpoint)| {
                    let id = checkpoint["checkpointId"].as_str()?.to_owned(); let selected = self.selected.as_ref() == Some(&id); let label = format!("{} {}", tr("检查点"), id);
                    Some(Button::new(("history-checkpoint", i)).outline().small().selected(selected).disabled(disabled).label(label)
                        .on_click(cx.listener(move |this, _, _, cx| {this.selected = Some(id.clone()); this.preview = None; cx.notify();})))
                })))
            .child(Button::new("history-checkpoint-preview").outline().small().disabled(disabled || !can_checkpoint).label(tr("预览当前代码检查点"))
                .on_click(cx.listener(|this, _, _, cx| this.call("checkpoint.preview", this.params(), cx))))
            .when_some(self.checkpoint_preview.clone(), |v, p| {
                let exclusions = strings(&p["exclusions"]); let version = p["version"].as_str().unwrap_or_default().to_owned();
                let can = p["canCapture"] == true && !version.is_empty();
                v.child(div().text_sm().child(format!("{}: {}", tr("排除的忽略文件"), exclusions)))
                    .child(div().text_sm().child(strings(&p["unsupported"])))
                    .child(Button::new("history-checkpoint-capture").outline().small().disabled(disabled || !can).label(tr("保存检查点"))
                        .on_click(cx.listener(move |this, _, _, cx| {let mut params = this.params(); params["expectedVersion"] = json!(version); params["requestId"] = json!(this.capture_request_id.get_or_insert_with(|| uuid::Uuid::new_v4().to_string()).clone()); this.call("checkpoint.capture", params, cx);})))
            })
            .child(Button::new("history-fork-preview").outline().small().disabled(disabled || !allowed).label(tr("预览分叉"))
                .on_click(cx.listener(|this, _, _, cx| this.fork_preview(cx))))
            .when_some(self.preview.clone(), |v, p| {
                let relation = p["codeRelation"]["label"].as_str().unwrap_or_default().to_owned();
                let exclusions = strings(&p["exclusions"]); let warnings = strings(&p["warnings"]);
                let draft = p["draft"]["text"].as_str().map(str::to_owned); let missing = strings(&p["draft"]["missingAttachments"]); let attachments = strings(&p["draft"]["attachmentIds"]);
                v.child(div().text_sm().child(relation)).child(div().text_sm().child(format!("{}: {}", tr("排除的忽略文件"), exclusions)))
                    .when(!warnings.is_empty(), |v| v.child(div().text_sm().child(warnings)))
                    .when_some(draft, |v, draft| v.child(div().text_sm().child(format!("{}: {}", tr("未发送草稿"), draft))))
                    .when(!attachments.is_empty(), |v| v.child(div().text_sm().child(format!("{}: {}", tr("附件"), attachments))))
                    .when(!missing.is_empty(), |v| v.child(div().text_sm().text_color(Color::Error.get(cx)).child(format!("{}: {}", tr("缺失的附件"), missing))))
                    .child(Button::new("history-fork-apply").primary().small().disabled(disabled || p["previewToken"].as_str().is_none()).label(tr("创建分叉"))
                        .on_click(cx.listener(|this, _, _, cx| this.apply(cx))))
            })
            .when_some(self.operation.clone(), |v, op| {
                let state = op["state"].as_str().unwrap_or("unknown").to_owned();
                let operation_id = op["operationId"].as_str().unwrap_or_default().to_owned();
                v.child(div().id("history-operation-status").role(Role::Status).text_sm().child(format!("{}: {} · {}", tr("操作状态"), state, operation_id)))
                    .when_some(op["error"].as_str().map(str::to_owned), |v, e| v.child(div().text_sm().child(e)))
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
