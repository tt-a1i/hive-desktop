use crate::{
    app::{ErrorNotice, HiveApp},
    language::tr,
    palette::Color,
    protocol::{Session, Workspace},
    transport::Runtime,
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
use std::{sync::Arc, time::Duration};

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScratchProjectLink {
    source_workspace_id: String,
    source_session_id: String,
    target_workspace_id: String,
    path: String,
}

#[derive(Default)]
pub(crate) struct ScratchProjects {
    links: Vec<ScratchProjectLink>,
    refresh_revision: u64,
}

#[derive(Deserialize)]
struct LinksReply {
    links: Vec<ScratchProjectLink>,
}

#[derive(Deserialize)]
struct PromotionReply {
    workspace: Workspace,
    link: ScratchProjectLink,
}

impl HiveApp {
    pub(crate) fn scratch_project_target(&self, session: &Session) -> Option<String> {
        let link = self.scratch_projects.links.iter().find(|link| {
            link.source_workspace_id == session.workspace_id && link.source_session_id == session.id
        })?;
        self.workspaces
            .iter()
            .any(|workspace| workspace.id == link.target_workspace_id)
            .then(|| link.target_workspace_id.clone())
    }

    pub(crate) fn has_scratch_project_origin(&self, workspace: &str) -> bool {
        self.scratch_projects
            .links
            .iter()
            .any(|link| link.target_workspace_id == workspace)
    }

    pub(crate) fn refresh_scratch_projects(&mut self, cx: &mut Context<Self>) {
        let Some(runtime) = self.runtime.clone().filter(|_| self.connected) else {
            return;
        };
        self.scratch_projects.links.retain(|link| {
            self.workspaces
                .iter()
                .any(|workspace| workspace.id == link.target_workspace_id)
        });
        self.scratch_projects.refresh_revision += 1;
        let revision = self.scratch_projects.refresh_revision;
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime.request("workspace.scratch.links", json!({})).await;
            let _ = this.update(cx, |this, cx| {
                if connection != this.connection_generation
                    || revision != this.scratch_projects.refresh_revision
                {
                    return;
                }
                match result.and_then(|value| {
                    serde_json::from_value::<LinksReply>(value).map_err(|error| error.to_string())
                }) {
                    Ok(reply) => this.scratch_projects.links = reply.links,
                    Err(error) => eprintln!("[hive-desktop] Scratch project links: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn scratch_project_source_available(&self, session: &Session) -> bool {
        self.connected
            && !self.closing
            && session.scratch
            && self.workspace_removing.as_ref() != Some(&session.workspace_id)
            && !self.removed_sessions.contains(&session.id)
            && self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == session.workspace_id)
    }

    pub(crate) fn show_scratch_project(
        &mut self,
        session: Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.scratch_project_source_available(&session) {
            return;
        }
        if let Some(target) = self.scratch_project_target(&session) {
            if self.workspace_removing.as_ref() != Some(&target) {
                self.select_workspace(target, cx);
                if window.viewport_size().width < px(820.) {
                    self.sidebar_open = false;
                }
            }
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let default_name = if session.custom_title || session.title != "新会话" {
            self.session_title(&session)
        } else {
            tr("Scratch 项目")
        };
        let name = cx.new(|cx| InputState::new(window, cx).default_value(default_name.to_owned()));
        let focus = name.read(cx).focus_handle(cx);
        let app = cx.weak_entity();
        let connection = self.connection_generation;
        let form = cx.new(|cx: &mut Context<ScratchProjectForm>| {
            cx.subscribe_in(&name, window, |form, _, event, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter {
                    secondary: false,
                    shift: false,
                } => form.save(window, cx),
                _ => {}
            })
            .detach();
            ScratchProjectForm {
                app,
                session,
                runtime,
                connection,
                name,
                saving: false,
                error: None,
            }
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            let busy = form.read(cx).saving;
            let submit = form.downgrade();
            dialog
                .title(tr("将 Scratch 文件夹添加为项目"))
                .w(px(500.).min(window.viewport_size().width - px(32.)))
                .overlay_closable(false)
                .close_button(!busy)
                .keyboard(!busy)
                .on_ok(move |_, window, cx| {
                    let _ = submit.update(cx, |form, cx| form.save(window, cx));
                    false
                })
                .child(form.clone())
        });
        let dialog_focus = window.focused(cx);
        window.on_next_frame(move |window, cx| {
            if dialog_focus.is_some_and(|root| root.contains_focused(window, cx)) {
                focus.focus(window, cx);
            }
        });
    }

    pub(crate) fn open_scratch_project_origin(
        &mut self,
        workspace: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace.as_ref() != Some(&workspace)
            || self.closing
            || !self.workspace_writes_allowed()
        {
            return;
        }
        let Some(runtime) = self.runtime.clone().filter(|_| self.connected) else {
            return;
        };
        let Some(link) = self
            .scratch_projects
            .links
            .iter()
            .find(|link| link.target_workspace_id == workspace)
            .cloned()
        else {
            return;
        };
        let window = window.window_handle();
        let connection = self.connection_generation;
        let request = self.open_request;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.list",
                    json!({"workspaceId": link.source_workspace_id}),
                )
                .await
                .and_then(|value| {
                    serde_json::from_value::<Vec<Session>>(value).map_err(|error| error.to_string())
                });
            let _ = window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection
                        || this.open_request != request
                        || this.workspace.as_ref() != Some(&workspace)
                        || !this.scratch_projects.links.contains(&link)
                    {
                        return;
                    }
                    match result {
                        Ok(sessions) => {
                            if let Some(session) = sessions.into_iter().find(|session| {
                                session.workspace_id == link.source_workspace_id
                                    && session.id == link.source_session_id
                                    && session.scratch
                            }) {
                                if this.navigate_to_session(session, window, cx) {
                                    return;
                                }
                            }
                            this.error = Some("无法打开原 Scratch 会话".into());
                        }
                        Err(error) => {
                            this.error = Some(ErrorNotice::Detail("无法打开原 Scratch 会话", error))
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }
}

struct ScratchProjectForm {
    app: WeakEntity<HiveApp>,
    session: Session,
    runtime: Arc<Runtime>,
    connection: u64,
    name: Entity<InputState>,
    saving: bool,
    error: Option<String>,
}

impl ScratchProjectForm {
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_owned();
        if self.saving || name.is_empty() {
            return;
        }
        let session = self.session.clone();
        let app = self.app.clone();
        let connection = self.connection;
        if !app.upgrade().is_some_and(|app| {
            let app = app.read(cx);
            app.connection_generation == connection
                && app.scratch_project_source_available(&session)
        }) {
            self.error = Some(tr("Scratch 会话暂不可用，请重新打开后再试。").into());
            cx.notify();
            return;
        }
        self.saving = true;
        self.error = None;
        cx.notify();
        let window = window.window_handle();
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let result = smol::future::race(
                async {
                    runtime.request("workspace.scratch.promote", json!({
                        "workspaceId": session.workspace_id, "sessionId": session.id, "name": name,
                    })).await.and_then(|value| {
                        let reply: PromotionReply = serde_json::from_value(value).map_err(|error| error.to_string())?;
                        if reply.link.source_workspace_id != session.workspace_id
                            || reply.link.source_session_id != session.id
                            || reply.link.target_workspace_id != reply.workspace.id
                            || reply.link.target_workspace_id == session.workspace_id
                            || reply.link.path.is_empty()
                        {
                            return Err(tr("项目响应的会话身份不匹配").into());
                        }
                        Ok(reply)
                    })
                },
                async {
                    smol::Timer::after(Duration::from_secs(10)).await;
                    Err(tr("添加项目尚未确认，可再次提交核对结果。").into())
                },
            ).await;
            let _ = window.update(cx, |_, window, cx| {
                let closed = this.update(cx, |this, cx| {
                    this.saving = false;
                    match result {
                        Ok(reply) => {
                            app.update(cx, |app, cx| {
                                if app.connection_generation != connection || app.closing {
                                    return false;
                                }
                                let target = reply.workspace.id.clone();
                                if let Some(existing) = app.workspaces.iter_mut().find(|item| item.id == target) {
                                    *existing = reply.workspace;
                                } else {
                                    app.workspaces.push(reply.workspace);
                                }
                                app.scratch_projects.refresh_revision += 1;
                                app.scratch_projects.links.retain(|link| link.source_session_id != session.id);
                                app.scratch_projects.links.push(reply.link);
                                app.select_workspace(target, cx);
                                if window.viewport_size().width < px(820.) {
                                    app.sidebar_open = false;
                                }
                                true
                            }).unwrap_or(false)
                        }
                        Err(error) => {
                            this.error = Some(crate::language::localize_owned_error(&error));
                            cx.notify();
                            false
                        }
                    }
                }).unwrap_or(false);
                if closed {
                    window.close_dialog(cx);
                }
            });
        }).detach();
    }
}

impl Render for ScratchProjectForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(Color::Secondary.get(cx))
                    .child(tr(
                        "新项目使用这个会话的文件夹。原会话和附件保留，可从项目打开。",
                    )),
            )
            .when_some(self.session.execution_path.clone(), |form, path| {
                form.child(
                    div()
                        .id("scratch-project-path")
                        .role(Role::Label)
                        .aria_value(path.clone())
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .truncate()
                        .child(path),
                )
            })
            .child(
                v_flex()
                    .gap_1p5()
                    .child(div().text_sm().child(tr("项目名称")))
                    .child(
                        Input::new(&self.name)
                            .aria_label(tr("项目名称"))
                            .disabled(self.saving),
                    ),
            )
            .when_some(self.error.clone(), |form, error| {
                form.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-scratch-project")
                            .outline()
                            .label(tr("取消"))
                            .disabled(self.saving)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("add-scratch-project")
                            .primary()
                            .label(tr(if self.saving {
                                "正在添加项目…"
                            } else {
                                "添加为项目"
                            }))
                            .disabled(self.saving || self.name.read(cx).value().trim().is_empty())
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}
