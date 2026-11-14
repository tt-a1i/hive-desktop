use crate::{app::HiveApp, language::tr, palette::Color, protocol::Session, transport::Runtime};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputState},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingWorktree {
    session_id: String,
    branch: String,
    path: String,
    state: String,
}
#[derive(Default, Deserialize)]
struct PendingPage {
    pending: Vec<PendingWorktree>,
    unreadable: usize,
}
fn request_params(
    workspace: &str,
    backend: &str,
    branch: &str,
    recovering: Option<&str>,
) -> (&'static str, Value) {
    if let Some(id) = recovering {
        (
            "chat.worktrees.recover",
            json!({"workspaceId":workspace,"sessionId":id,"backend":backend}),
        )
    } else {
        let mut params = json!({"workspaceId":workspace,"backend":backend,"isolation":"worktree"});
        if !branch.trim().is_empty() {
            params["branch"] = json!(branch.trim());
        }
        ("chat.createRecord", params)
    }
}
fn response_session(
    value: Value,
    workspace: &str,
    expected: Option<&str>,
) -> Result<Session, String> {
    let session: Session = serde_json::from_value(value).map_err(|error| error.to_string())?;
    if session.workspace_id != workspace
        || expected.is_some_and(|id| session.id != id)
        || !session.execution.as_ref().is_some_and(|execution| {
            execution.kind == "worktree"
                && !execution.path.is_empty()
                && !execution.branch.is_empty()
        })
    {
        return Err(tr("工作树会话响应与原项目或会话身份不匹配，未切换当前会话。").to_owned());
    }
    Ok(session)
}
struct WorktreeDialog {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    workspace: String,
    connection: u64,
    branch: Entity<InputState>,
    backend: &'static str,
    recovering: bool,
    page: PendingPage,
    generation: u64,
    busy: bool,
    loading: bool,
    error: Option<String>,
    notice: Option<&'static str>,
    scroll: ScrollHandle,
}
impl HiveApp {
    pub(crate) fn open_session_execution(
        &mut self,
        project: Option<String>,
        recovering: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing || window.has_active_dialog(cx) {
            return;
        }
        let Some(workspace) = project.or_else(|| self.workspace.clone()) else {
            return;
        };
        if self.workspace_removing.as_ref() == Some(&workspace)
            || !self.workspaces.iter().any(|item| item.id == workspace)
        {
            return;
        }
        if self.workspace.as_ref() != Some(&workspace) {
            self.select_workspace(workspace.clone(), cx);
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        let backend = match self.session.as_ref().map(|session| session.backend) {
            Some(crate::protocol::Backend::Codex) => "codex",
            Some(crate::protocol::Backend::PiRpc) => "pi-rpc",
            Some(crate::protocol::Backend::Claude) => "claude",
            _ => "pi",
        };
        let app = cx.weak_entity();
        let editor = cx.new(|cx: &mut Context<WorktreeDialog>| {
            let branch =
                cx.new(|cx| InputState::new(window, cx).placeholder(tr("留空自动分配分支")));
            if let Some(parent) = app.upgrade() {
                cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            }
            let editor = WorktreeDialog {
                app,
                runtime,
                workspace,
                connection,
                branch,
                backend,
                recovering,
                page: PendingPage::default(),
                generation: 0,
                busy: false,
                loading: false,
                error: None,
                notice: None,
                scroll: ScrollHandle::new(),
            };
            if recovering {
                // HiveApp is still leased by the action constructing this
                // child. Read its identity only after that update finishes.
                let pending = cx.weak_entity();
                cx.defer(move |cx| {
                    let _ = pending.update(cx, |editor, cx| editor.load_pending(cx));
                });
            }
            editor
        });
        let cancel = editor.clone();
        let focus = editor.read(cx).branch.read(cx).focus_handle(cx);
        window.open_dialog(cx, move |dialog, _, cx| {
            let cancel = cancel.clone();
            let busy = editor.read(cx).busy;
            dialog
                .title(tr("独立工作树会话"))
                .w(px(620.))
                .overlay_closable(false)
                .keyboard(!busy)
                .close_button(!busy)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !cancel.read(cx).busy)
                .child(editor.clone())
        });
        if !recovering {
            let dialog_focus = window.focused(cx);
            window.on_next_frame(move |window, cx| {
                if dialog_focus.is_some_and(|root| root.contains_focused(window, cx)) {
                    focus.focus(window, cx);
                }
            });
        }
    }
}
impl WorktreeDialog {
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|entity| {
            let app = entity.read(cx);
            app.connected
                && !app.closing
                && app.connection_generation == self.connection
                && app.workspace.as_ref() == Some(&self.workspace)
                && app.workspace_removing.as_ref() != Some(&self.workspace)
                && app
                    .runtime
                    .as_ref()
                    .is_some_and(|runtime| Arc::ptr_eq(runtime, &self.runtime))
        })
    }
    fn load_pending(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.loading || !self.current(cx) {
            return;
        }
        self.recovering = true;
        self.loading = true;
        self.error = None;
        self.notice = None;
        self.generation += 1;
        let generation = self.generation;
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| {
                    if this.generation == generation && this.current(cx) {
                        return true;
                    }
                    if this.generation == generation {
                        this.loading = false;
                        cx.notify();
                    }
                    false
                })
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request("chat.worktrees.pending", json!({"workspaceId":workspace}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<PendingPage>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading = false;
                if !this.current(cx) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(page) => this.page = page,
                    Err(error) => this.error = Some(crate::language::localize_owned_error(&error)),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn submit(&mut self, pending: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.loading || !self.current(cx) {
            return;
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let open_request = app.read(cx).open_request;
        let (method, params) = request_params(
            &self.workspace,
            self.backend,
            &self.branch.read(cx).value(),
            pending.as_deref(),
        );
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        let window = window.window_handle();
        self.busy = true;
        self.error = None;
        self.notice = None;
        self.generation += 1;
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| {
                    if this.generation == generation
                        && this.current(cx)
                        && this
                            .app
                            .upgrade()
                            .is_some_and(|app| app.read(cx).open_request == open_request)
                    {
                        return true;
                    }
                    if this.generation == generation {
                        this.busy = false;
                        cx.notify();
                    }
                    false
                })
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request(method, params)
                .await
                .and_then(|value| response_session(value, &workspace, pending.as_deref()));
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                if !this.current(cx)
                    || this
                        .app
                        .upgrade()
                        .is_none_or(|app| app.read(cx).open_request != open_request)
                {
                    this.notice =
                        Some("操作已结束，当前视图已变化；请在原项目的会话或待恢复列表查看结果。");
                    cx.notify();
                    return;
                }
                match result {
                    Ok(session) => {
                        let accepted = this
                            .app
                            .update(cx, |app, cx| app.accept_created_session(session, cx))
                            .unwrap_or(false);
                        if accepted {
                            let _ = window.update(cx, |_, window, cx| window.close_dialog(cx));
                        }
                    }
                    Err(error) => this.error = Some(crate::language::localize_owned_error(&error)),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
impl Render for WorktreeDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current(cx);
        let disabled = !current || self.busy || self.loading;
        let backend = self.backend;
        let recovering = self.recovering;
        v_flex()
            .gap_3()
            .min_w_0()
            .child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr(
                "从当前 HEAD 创建独立代码目录；团队工作区保持不变。已有原生会话不会迁移。",
            )))
            .when(!current, |view| {
                view.child(
                    div()
                        .id("worktree-target-changed")
                        .role(Role::Status)
                        .text_sm()
                        .child(tr("项目或后台连接已变化，请关闭后在目标项目重新打开。")),
                )
            })
            .child(
                h_flex().gap_2().flex_wrap().children(
                    [
                        ("pi", "内置 Pi"),
                        ("codex", "Codex"),
                        ("pi-rpc", "外部 Pi"),
                        ("claude", "Claude Code"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (value, label))| {
                        Button::new(("worktree-backend", index))
                            .outline()
                            .small()
                            .label(tr(label))
                            .selected(backend == value)
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.backend = value;
                                cx.notify();
                            }))
                    }),
                ),
            )
            .when(!recovering, |view| {
                view.child(div().text_sm().child(tr("分支名（可选）")))
                    .child(Input::new(&self.branch).disabled(disabled))
                    .child(
                        Button::new("create-isolated-session")
                            .primary()
                            .label(tr(if self.busy {
                                "正在创建工作树…"
                            } else {
                                "新建独立工作树会话"
                            }))
                            .disabled(disabled)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.submit(None, window, cx)),
                            ),
                    )
            })
            .when(recovering, |view| {
                view.child(
                    div()
                        .text_sm()
                        .child(tr("仅恢复已保存的会话身份与目录，不创建其他目录。")),
                )
                .when(self.loading, |view| {
                    view.child(
                        div()
                            .id("pending-worktrees-loading")
                            .role(Role::Status)
                            .child(tr("正在读取待恢复工作树…")),
                    )
                })
                .when(!self.loading && self.page.pending.is_empty(), |view| {
                    view.child(div().text_sm().child(tr("没有待恢复的工作树。")))
                })
                .when(self.page.unreadable > 0, |view| {
                    view.child(
                        div()
                            .text_sm()
                            .text_color(Color::Error.get(cx))
                            .child(format!(
                                "{} ({})",
                                tr(
                                    "部分工作树记录暂时无法读取，可重新读取或在设置中复制诊断信息。"
                                ),
                                self.page.unreadable
                            )),
                    )
                })
                .child(
                    v_flex()
                        .id("pending-worktrees")
                        .gap_2()
                        .min_w_0()
                        .max_h(px(300.).min(window.viewport_size().height * 0.4))
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .vertical_scrollbar(&self.scroll)
                        .children(self.page.pending.clone().into_iter().enumerate().map(
                            |(index, item)| {
                                let id = item.session_id;
                                v_flex()
                                    .gap_1()
                                    .p_2()
                                    .min_w_0()
                                    .child(div().text_sm().child(item.branch))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(item.path),
                                    )
                                    .child(div().text_xs().child(tr(if item.state == "creating" {
                                        "创建中断，等待核验恢复"
                                    } else {
                                        "等待领取"
                                    })))
                                    .child(
                                        Button::new(("recover-worktree", index))
                                            .outline()
                                            .small()
                                            .label(tr("恢复此工作树会话"))
                                            .disabled(disabled)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.submit(Some(id.clone()), window, cx)
                                            })),
                                    )
                            },
                        )),
                )
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .id("worktree-error")
                        .role(Role::Alert)
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when_some(self.notice, |view, notice| {
                view.child(
                    div()
                        .id("worktree-notice")
                        .role(Role::Status)
                        .text_sm()
                        .child(tr(notice)),
                )
            })
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("show-pending-worktrees")
                            .outline()
                            .small()
                            .label(tr(if recovering {
                                "重新读取待恢复工作树"
                            } else {
                                "查看待恢复工作树…"
                            }))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| this.load_pending(cx))),
                    )
                    .when(recovering, |view| {
                        view.child(
                            Button::new("worktree-create-mode")
                                .outline()
                                .small()
                                .label(tr("新建独立工作树会话"))
                                .disabled(disabled)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.recovering = false;
                                    this.error = None;
                                    this.notice = None;
                                    cx.notify();
                                })),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{request_params, response_session};
    use crate::protocol::Session;
    use serde_json::json;
    #[test]
    fn creation_and_recovery_never_supply_an_execution_directory() {
        let (method, params) = request_params("workspace", "codex", "  feature/example  ", None);
        assert_eq!(method, "chat.createRecord");
        assert_eq!(
            params,
            json!({"workspaceId":"workspace","backend":"codex","isolation":"worktree","branch":"feature/example"})
        );
        assert!(
            request_params("workspace", "pi", " ", None)
                .1
                .get("branch")
                .is_none()
        );
        let (method, params) = request_params("workspace", "claude", "ignored", Some("original"));
        assert_eq!(method, "chat.worktrees.recover");
        assert_eq!(
            params,
            json!({"workspaceId":"workspace","sessionId":"original","backend":"claude"})
        );
    }
    #[test]
    fn session_execution_is_optional_and_private_ownership_is_not_a_ui_field() {
        let legacy = json!({"id":"s","title":"old","workspaceId":"w","agentId":null});
        let parsed: Session = serde_json::from_value(legacy).unwrap();
        assert!(parsed.execution.is_none());
        assert!(parsed.execution_path.is_none());
        let value = json!({"id":"s","title":"new","workspaceId":"w","agentId":null,"backend":"codex","execution":{"kind":"worktree","path":"/owned","branch":"hive/s","ownerToken":"private"},"executionPath":"/owned"});
        let parsed = response_session(value.clone(), "w", Some("s")).unwrap();
        assert_eq!(parsed.execution_path.as_deref(), Some("/owned"));
        assert_eq!(
            serde_json::to_value(parsed.execution.unwrap()).unwrap(),
            json!({"kind":"worktree","path":"/owned","branch":"hive/s"})
        );
        assert!(response_session(value.clone(), "other", Some("s")).is_err());
        assert!(response_session(value, "w", Some("replacement")).is_err());
    }
}
