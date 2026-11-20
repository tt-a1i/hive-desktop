use super::*;
use crate::language::localize_owned_error;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemovalPreview {
    workspace_id: String,
    session_id: String,
    version: String,
    path: String,
    branch: String,
    head_oid: String,
    verified_remote: Option<VerifiedRemote>,
}

#[derive(Deserialize)]
struct VerifiedRemote {
    remote: String,
    branch: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemovalResult {
    ok: bool,
    workspace_id: String,
    session_id: String,
}

struct WorktreeRemoval {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    connection: u64,
    session: Session,
    preview: Option<RemovalPreview>,
    busy: bool,
    removed: bool,
    error: Option<String>,
}

impl HiveApp {
    pub(crate) fn open_worktree_removal(
        &mut self,
        session: Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        if !self.connected
            || self.closing
            || window.has_active_dialog(cx)
            || self.workspace_removing.as_ref() == Some(&session.workspace_id)
            || session.execution.as_ref().is_some_and(|execution| execution.owner_session_id.is_some())
        {
            return;
        }
        let app = cx.weak_entity();
        let connection = self.connection_generation;
        let removal = cx.new(|_| WorktreeRemoval {
            app,
            runtime,
            connection,
            session,
            preview: None,
            busy: false,
            removed: false,
            error: None,
        });
        let check = removal.downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(tr("清理独立工作树"))
                .w(px(620.))
                .child(removal.clone())
        });
        window.on_next_frame(move |_, cx| {
            let _ = check.update(cx, |this, cx| this.check(cx));
        });
    }
}

impl WorktreeRemoval {
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|app| {
            let app = app.read(cx);
            app.connected
                && !app.closing
                && app.connection_generation == self.connection
                && app.workspace_removing.as_ref() != Some(&self.session.workspace_id)
        })
    }

    fn check(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.removed || !self.current(cx) {
            return;
        }
        self.busy = true;
        self.preview = None;
        self.error = None;
        let runtime = self.runtime.clone();
        let workspace = self.session.workspace_id.clone();
        let session = self.session.id.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.worktrees.removal.preview",
                    json!({"workspaceId": workspace, "sessionId": session}),
                )
                .await
                .and_then(|value| {
                    serde_json::from_value::<RemovalPreview>(value)
                        .map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if this.current(cx) {
                    match result {
                        Ok(preview)
                            if preview.workspace_id == workspace
                                && preview.session_id == session
                                && !preview.path.is_empty()
                                && !preview.version.is_empty()
                                && this.session.execution.as_ref().is_some_and(|execution| {
                                    execution.branch == preview.branch
                                }) =>
                        {
                            this.preview = Some(preview);
                        }
                        Ok(_) => this.error = Some(tr("工作树清理响应与原会话不匹配。").into()),
                        Err(error) => this.error = Some(localize_owned_error(&error)),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.current(cx) {
            return;
        }
        let Some(preview) = self.preview.take() else {
            return;
        };
        self.busy = true;
        self.error = None;
        let runtime = self.runtime.clone();
        let app = self.app.clone();
        let connection = self.connection;
        let workspace = self.session.workspace_id.clone();
        let session = self.session.id.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request("chat.worktrees.removal.apply",
                json!({"workspaceId":workspace,"sessionId":session,"expectedVersion":preview.version})).await
                .and_then(|value| serde_json::from_value::<RemovalResult>(value).map_err(|error| error.to_string()))
                .and_then(|result| {
                    if result.ok && result.session_id == session && result.workspace_id == workspace {
                        Ok(())
                    } else {
                        Err(tr("工作树清理响应与原会话不匹配。").into())
                    }
                });
            if result.is_ok() {
                let _ = app.update(cx, |app, cx| {
                    if app.connection_generation == connection && app.viewing_conversation()
                        && app.session.as_ref().is_some_and(|item| item.id == session && item.workspace_id == workspace)
                    {
                        app.show_session(session, cx);
                    }
                });
            }
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => this.removed = true,
                    Err(error) => this.error = Some(localize_owned_error(&error)),
                }
                cx.notify();
            });
        }).detach();
    }
}

impl Render for WorktreeRemoval {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = !self.busy && self.current(cx);
        let description =
            tr("移除这份本地检出。Git 分支和会话历史会保留，原会话将以只读方式打开。");
        let checking = tr("正在核对工作树和在途操作…");
        let removed = tr("工作树已清理。原会话历史和 Git 分支已保留。");
        v_flex()
            .gap_3()
            .min_w_0()
            .child(
                div()
                    .id("worktree-removal-description")
                    .role(Role::Label)
                    .aria_value(description)
                    .text_sm()
                    .child(description),
            )
            .when_some(self.preview.as_ref(), |view, preview| {
                let revision = format!("{} · {}", preview.branch, preview.head_oid);
                view.child(
                    v_flex()
                        .gap_1()
                        .min_w_0()
                        .text_sm()
                        .child(
                            div()
                                .id("worktree-removal-path")
                                .role(Role::Label)
                                .aria_value(preview.path.clone())
                                .child(preview.path.clone()),
                        )
                        .child(
                            div()
                                .id("worktree-removal-revision")
                                .role(Role::Label)
                                .aria_value(revision.clone())
                                .child(revision),
                        )
                        .when_some(preview.verified_remote.as_ref(), |detail, remote| {
                            let verified = format!(
                                "{} {}/{}",
                                tr("已核对远端分支"),
                                remote.remote,
                                remote.branch
                            );
                            detail.child(
                                div()
                                    .id("worktree-removal-remote")
                                    .role(Role::Label)
                                    .aria_value(verified.clone())
                                    .child(verified),
                            )
                        }),
                )
            })
            .when(self.busy, |view| {
                view.child(
                    div()
                        .id("worktree-removal-busy")
                        .role(Role::Status)
                        .aria_label(checking)
                        .child(checking),
                )
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .id("worktree-removal-error")
                        .role(Role::Alert)
                        .aria_label(error.clone())
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when(self.removed, |view| {
                view.child(
                    div()
                        .id("worktree-removal-done")
                        .role(Role::Status)
                        .aria_label(removed)
                        .child(removed),
                )
            })
            .when(!self.removed, |view| {
                view.child(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("review-worktree-removal")
                                .outline()
                                .label(tr("重新检查"))
                                .disabled(!enabled || self.preview.is_some())
                                .on_click(cx.listener(|this, _, _, cx| this.check(cx))),
                        )
                        .child(
                            Button::new("confirm-worktree-removal")
                                .danger()
                                .label(tr("确认清理工作树"))
                                .disabled(!enabled || self.preview.is_none())
                                .on_click(cx.listener(|this, _, _, cx| this.remove(cx))),
                        ),
                )
            })
    }
}
