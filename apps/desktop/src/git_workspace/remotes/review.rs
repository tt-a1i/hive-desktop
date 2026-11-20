use super::{Fault, RemotePanel, RequestStamp, envelope};
use crate::{
    language::tr,
    palette::Color,
    protocol::{Backend, Session},
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewPreview {
    version: String,
    number: u64,
    url: String,
    title: String,
    state: String,
    base_branch: String,
    head_branch: String,
    base_oid: String,
    head_oid: String,
    current_branch: Option<String>,
    current_head: Option<String>,
    current_clean: bool,
}

pub(super) struct PrReviewPanel {
    reference: Entity<InputState>,
    preview: Option<(RequestStamp, ReviewPreview)>,
    opening: bool,
}

impl PrReviewPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<RemotePanel>) -> Self {
        let reference = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe(
            &reference,
            |this: &mut RemotePanel, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.review.preview = None;
                    this.invalidate(cx);
                }
            },
        )
        .detach();
        Self {
            reference,
            preview: None,
            opening: false,
        }
    }

    pub(super) fn opening(&self) -> bool {
        self.opening
    }
}

impl RemotePanel {
    fn preview_pr_review(&mut self, cx: &mut Context<Self>) {
        if !self.ready(cx) {
            return;
        }
        let Some(remote) = self.remote.clone() else {
            return;
        };
        let reference = self.review.reference.read(cx).value().trim().to_owned();
        if reference.is_empty() {
            return;
        }
        self.review.preview = None;
        self.error = None;
        self.busy = true;
        let stamp = RequestStamp::new(self.open_request, self.generation);
        let params =
            json!({"workspaceId": self.workspace, "remote": remote, "reference": reference});
        let runtime = self.runtime.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| this.before_request(stamp, cx))
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request("git.pr.review.preview", params)
                .await
                .map_err(Fault::transport)
                .and_then(|value| envelope::<ReviewPreview>(value, Some("preview")));
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if this.current(cx) && stamp.matches(this.open_request, this.generation) {
                    match result {
                        Ok(preview) => this.review.preview = Some((stamp, preview)),
                        Err(error) => this.error = Some(error),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_pr_review(
        &mut self,
        checkout: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((stamp, preview)) = self.review.preview.as_ref() else {
            return;
        };
        if !self.ready(cx)
            || !stamp.matches(self.open_request, self.generation)
            || (checkout == "current" && !preview.current_clean)
        {
            return;
        }
        let (Some(remote), Some(app)) = (self.remote.clone(), self.app.upgrade()) else {
            return;
        };
        let source = app.read(cx).session.as_ref();
        let backend = source.map_or(Backend::default(), |session| session.backend);
        let profile = source.and_then(|session| session.profile.clone());
        let mut params = json!({
            "workspaceId": self.workspace,
            "remote": remote,
            "reference": self.review.reference.read(cx).value().trim(),
            "expectedVersion": preview.version,
            "checkout": checkout,
            "backend": backend.wire(),
        });
        if let Some(profile) = &profile {
            params["profile"] = json!(profile);
        }
        let runtime = self.runtime.clone();
        let stamp = *stamp;
        let window = window.window_handle();
        self.busy = true;
        self.review.opening = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| this.before_request(stamp, cx))
                .ok()
                != Some(true)
            {
                let _ = this.update(cx, |this, _| this.review.opening = false);
                return;
            }
            let result = runtime
                .request("git.pr.review.open", params)
                .await
                .map_err(Fault::transport)
                .and_then(|value| envelope::<Session>(value, Some("session")));
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.review.opening = false;
                match result {
                    Ok(session)
                        if session.workspace_id == this.workspace
                            && session.backend.wire() == backend.wire()
                            && (session.execution.is_some() == (checkout == "worktree"))
                            && profile.as_ref().is_none_or(|expected| {
                                session.profile.as_ref() == Some(expected)
                            }) =>
                    {
                        let current =
                            this.current(cx) && stamp.matches(this.open_request, this.generation);
                        if !current {
                            let _ = this.app.update(cx, |app, cx| app.refresh_sessions(cx));
                        } else {
                            let _ = window.update(cx, |_, window, cx| {
                                let opened = this
                                    .app
                                    .update(cx, |app, cx| {
                                        let opened = app.navigate_to_session(session, window, cx);
                                        if opened {
                                            app.git_drawer_open = false;
                                            app.focus_project_input = true;
                                        }
                                        opened
                                    })
                                    .unwrap_or(false);
                                if opened && !this.in_drawer && window.has_active_dialog(cx) {
                                    window.close_dialog(cx);
                                }
                            });
                        }
                    }
                    Ok(_) => {
                        this.error =
                            Some(Fault::transport(tr("PR 评审会话与请求不匹配。").to_owned()))
                    }
                    Err(error) => this.error = Some(error),
                }
                this.review.preview = None;
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_pr_review(&self, cx: &Context<Self>) -> AnyElement {
        let ready = self.ready(cx);
        let preview = self.review.preview.as_ref().filter(|(stamp, _)| {
            self.current(cx) && stamp.matches(self.open_request, self.generation)
        });
        v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("评审现有 GitHub PR")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(tr("选择 PR 所在仓库的远端，再输入 GitHub PR 地址或编号。")),
            )
            .child(
                Input::new(&self.review.reference)
                    .aria_label(tr("PR 地址或编号"))
                    .disabled(self.busy),
            )
            .child(
                Button::new("preview-existing-pr")
                    .outline()
                    .small()
                    .label(tr("联网查找 PR"))
                    .disabled(
                        !ready
                            || self.remote.is_none()
                            || self.review.reference.read(cx).value().trim().is_empty(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.preview_pr_review(cx))),
            )
            .when_some(preview, |view, (_, pr)| {
                let state = match pr.state.as_str() {
                    "open" => tr("PR 开放中"),
                    "closed" => tr("PR 已关闭"),
                    "merged" => tr("PR 已合并"),
                    _ => "",
                };
                let current_label = if pr.current_head.as_ref() == Some(&pr.head_oid) {
                    tr("在项目当前工作区评审")
                } else {
                    tr("切换项目工作区并评审")
                };
                view.child(
                    crate::text_reading::selectable_text(
                        "review-pr-identity",
                        format!(
                            "PR #{} · {} · {state}\n{}\n{} → {}\n{} {}\n{} {}",
                            pr.number,
                            pr.title,
                            pr.url,
                            pr.head_branch,
                            pr.base_branch,
                            tr("Head 提交"),
                            pr.head_oid,
                            tr("Base 提交"),
                            pr.base_oid
                        ),
                    )
                    .text_xs(),
                )
                .child(
                    Button::new("review-pr-worktree")
                        .primary()
                        .small()
                        .label(tr("在独立工作树中评审"))
                        .disabled(!ready)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_pr_review("worktree", window, cx)
                        })),
                )
                .when_some(pr.current_branch.as_deref(), |view, branch| {
                    view.child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(format!("{} · {branch}", tr("项目工作区分支"))),
                    )
                })
                .child(
                    Button::new("review-pr-current")
                        .outline()
                        .small()
                        .label(current_label)
                        .disabled(!ready || !pr.current_clean)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_pr_review("current", window, cx)
                        })),
                )
                .when(!pr.current_clean, |view| {
                    view.child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("项目工作区必须干净且没有未完成的 Git 操作。")),
                    )
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("Hive 会获取 PR 提交并为新会话填好评审草稿。")),
                )
            })
            .into_any_element()
    }
}
