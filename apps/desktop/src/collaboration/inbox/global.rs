use super::snoozes::{Reminder, now_ms};
use super::{
    ActionCenter, Destination, Entry, Reminders, inbox_row, inbox_status, is_snoozed,
    read_action_center, same_wake_target, sort_entries, toggle_button,
};
use crate::{
    app::HiveApp,
    language::{localize_owned_error, tr},
    protocol::{Member, Session, Workspace},
    transport::Runtime,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use std::{collections::HashMap, time::Duration};

struct GlobalInbox {
    app: WeakEntity<HiveApp>,
    connection: u64,
    projects: HashMap<String, ProjectInbox>,
    show_snoozed: bool,
    dismissed: bool,
    scroll: ScrollHandle,
}

#[derive(Default)]
struct ProjectInbox {
    action_center: Option<ActionCenter>,
    sessions: Vec<Session>,
    members: Vec<Member>,
    reminders: Reminders,
    errors: Vec<String>,
}

struct ProjectRead {
    action_center: Result<ActionCenter, String>,
    sessions: Result<Vec<Session>, String>,
    members: Result<Vec<Member>, String>,
}

async fn read_project(runtime: &Runtime, workspace: &str) -> ProjectRead {
    let (action_center, (sessions, members)) = smol::future::zip(
        read_action_center(runtime, workspace),
        smol::future::zip(
            read_list(runtime, "chat.list", workspace),
            read_list(runtime, "team.members", workspace),
        ),
    )
    .await;
    let sessions = sessions.and_then(|sessions: Vec<Session>| {
        if sessions
            .iter()
            .any(|session| session.workspace_id != workspace)
        {
            return Err(tr("收件箱返回了其他工作区的数据。").into());
        }
        Ok(sessions)
    });
    ProjectRead {
        action_center,
        sessions,
        members,
    }
}

async fn read_list<T: DeserializeOwned>(
    runtime: &Runtime,
    method: &'static str,
    workspace: &str,
) -> Result<Vec<T>, String> {
    let value = runtime
        .request(method, json!({"workspaceId": workspace}))
        .await?;
    Vec::deserialize(value).map_err(|error| error.to_string())
}

impl HiveApp {
    pub(crate) fn global_inbox_button(&self, id: &'static str, cx: &Context<Self>) -> Button {
        Button::new(id)
            .ghost()
            .compact()
            .small()
            .icon(Glyph::Inbox)
            .label(tr("收件箱"))
            .tooltip(tr("跨项目收件箱"))
            .accessibility_label(tr("跨项目收件箱"))
            .disabled(self.closing)
            .on_click(cx.listener(|this, _, window, cx| this.open_global_inbox(window, cx)))
    }

    fn open_global_inbox(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing || window.has_active_dialog(cx) {
            return;
        }
        let parent = cx.entity();
        let connection = self.connection_generation;
        let inbox = cx.new(|cx: &mut Context<GlobalInbox>| {
            cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            GlobalInbox {
                app: parent.downgrade(),
                connection,
                projects: HashMap::new(),
                show_snoozed: false,
                dismissed: false,
                scroll: ScrollHandle::new(),
            }
        });
        inbox.update(cx, |inbox, cx| inbox.poll(cx));
        let width = (window.viewport_size().width - px(36.)).min(px(740.));
        window.open_dialog(cx, move |dialog, _, _| {
            let dismiss = inbox.downgrade();
            dialog
                .title(tr("跨项目收件箱"))
                .w(width)
                .on_ok(|_, _, _| false)
                .on_close(move |_, _, cx| {
                    let _ = dismiss.update(cx, |inbox, _| inbox.dismissed = true);
                })
                .child(inbox.clone())
        });
    }

    fn open_global_destination(
        &mut self,
        workspace: &str,
        target: &Destination,
        connection: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self.closing
            || self.connection_generation != connection
            || self.workspace_removing.as_deref() == Some(workspace)
            || !self.workspaces.iter().any(|item| item.id == workspace)
        {
            return;
        }
        self.showing_welcome_guide = false;
        match target {
            Destination::Session(session) if session.workspace_id == workspace => {
                self.navigate_to_session(session.clone(), window, cx);
            }
            Destination::Session(_) => {}
            Destination::Member(member) => {
                if self.workspace.as_deref() != Some(workspace) {
                    self.select_workspace(workspace.to_owned(), cx);
                }
                self.show_team(window, cx);
                self.member_details_open
                    .insert(member.clone(), ScrollHandle::new());
            }
            Destination::Dispatch(id) => {
                // Open the actual workspace first. The dispatch detail then
                // resolves its original execution session through Hive's existing route.
                if self.workspace.as_deref() != Some(workspace) {
                    self.select_workspace(workspace.to_owned(), cx);
                }
                self.open_collaboration(cx);
                if window.viewport_size().width < px(820.) {
                    self.sidebar_open = false;
                }
                self.open_inbox_dispatch(id.clone(), cx);
            }
        }
    }
}

impl GlobalInbox {
    fn poll(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some((connection, runtime, workspaces))) = this.update(cx, |this, cx| {
                    let parent = this.app.upgrade()?;
                    let app = parent.read(cx);
                    if this.dismissed || app.closing {
                        return None;
                    }
                    if this.connection != app.connection_generation {
                        this.connection = app.connection_generation;
                        this.projects.clear();
                    }
                    this.projects
                        .retain(|id, _| app.workspaces.iter().any(|project| project.id == *id));
                    Some((
                        this.connection,
                        app.runtime
                            .clone()
                            .filter(|_| app.connected && !app.restarting),
                        app.workspaces
                            .iter()
                            .map(|project| project.id.clone())
                            .collect::<Vec<_>>(),
                    ))
                }) else {
                    break;
                };
                if let Some(runtime) = runtime {
                    // Keep a single sweep in flight and at most three requests
                    // running together. Update each workspace independently.
                    for workspace in workspaces {
                        if !this
                            .update(cx, |this, cx| this.is_current(connection, cx))
                            .unwrap_or(false)
                        {
                            break;
                        }
                        let result = read_project(&runtime, &workspace).await;
                        if this
                            .update(cx, |this, cx| {
                                this.apply(&workspace, connection, result, cx)
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                smol::Timer::after(Duration::from_secs(4)).await;
            }
        })
        .detach();
    }

    fn is_current(&self, connection: u64, cx: &App) -> bool {
        !self.dismissed
            && self.app.upgrade().is_some_and(|app| {
                let app = app.read(cx);
                self.connection == connection
                    && app.connection_generation == connection
                    && app.connected
                    && !app.closing
            })
    }

    fn apply(
        &mut self,
        workspace: &str,
        connection: u64,
        result: ProjectRead,
        cx: &mut Context<Self>,
    ) {
        if !self.is_current(connection, cx) {
            return;
        }
        let Some(parent) = self.app.upgrade() else {
            return;
        };
        let app = parent.read(cx);
        if !app.workspaces.iter().any(|project| project.id == workspace) {
            return;
        }
        let project = self.projects.entry(workspace.to_owned()).or_default();
        let mut errors = Vec::new();
        let snapshot = match result.action_center {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                errors.push(error);
                None
            }
        };
        match result.sessions {
            Ok(sessions) => project.sessions = sessions,
            Err(error) => errors.push(error),
        }
        match result.members {
            Ok(members) => project.members = members,
            Err(error) => errors.push(error),
        }
        if let Some(root) = &app.data_dir {
            let interactions = app.inbox_interactions(&project.sessions, &project.members);
            project
                .reminders
                .reconcile(root, workspace, snapshot.as_ref(), &interactions);
        }
        if let Some(snapshot) = snapshot {
            project.action_center = Some(snapshot);
        }
        project.errors = errors;
        cx.notify();
    }
}

impl ProjectInbox {
    fn entries(&self, app: &HiveApp) -> Vec<Entry> {
        let interactions = app.inbox_interactions(&self.sessions, &self.members);
        let mut rows: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|session| {
                app.inbox_session_entry(session, &self.members, &interactions, true)
            })
            .collect();
        if let Some(snapshot) = &self.action_center {
            rows.extend(snapshot.entries(|id| {
                self.members
                    .iter()
                    .find(|member| member.id == id)
                    .map(|member| member.name.clone())
                    .unwrap_or_else(|| {
                        if id.ends_with(":orchestrator") {
                            tr("主控").into()
                        } else if id.ends_with(":__workflow__") {
                            tr("工作流").into()
                        } else {
                            id.to_owned()
                        }
                    })
            }));
        }
        rows
    }
}

fn display_project(workspace: &Workspace, workspaces: &[Workspace]) -> String {
    if workspaces
        .iter()
        .filter(|item| item.name == workspace.name)
        .count()
        > 1
    {
        workspace.path.clone()
    } else {
        workspace.name.clone()
    }
}

impl Render for GlobalInbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex().gap_2().min_w_0();
        let Some(parent) = self.app.upgrade() else {
            return body;
        };
        let app = parent.read(cx);
        let ready = self.is_current(self.connection, cx);
        let projects: Vec<_> = app
            .workspaces
            .iter()
            .filter(|workspace| app.workspace_removing.as_ref() != Some(&workspace.id))
            .collect();
        let now = now_ms();
        let mut rows: Vec<_> = if ready {
            projects
                .iter()
                .filter_map(|workspace| {
                    self.projects
                        .get(&workspace.id)
                        .map(|project| (workspace, project))
                })
                .flat_map(|(workspace, project)| {
                    project
                        .entries(app)
                        .into_iter()
                        .map(|entry| (workspace.id.as_str(), entry))
                })
                .collect()
        } else {
            Vec::new()
        };
        sort_entries(&mut rows);
        let has_snoozed = rows
            .iter()
            .any(|(id, entry)| is_snoozed(entry, &self.projects[*id].reminders, now));
        rows.retain(|(id, entry)| {
            self.show_snoozed || !is_snoozed(entry, &self.projects[*id].reminders, now)
        });
        let status = if !ready {
            Some("后台连接后将显示跨项目收件箱。")
        } else if projects
            .iter()
            .any(|project| !self.projects.contains_key(&project.id))
        {
            Some("正在读取收件箱…")
        } else if rows.is_empty()
            && self
                .projects
                .values()
                .all(|project| project.errors.is_empty())
        {
            Some("暂无需要关注的协作事项")
        } else {
            None
        };
        body = body
            .when(has_snoozed, |body| {
                body.child(
                    h_flex()
                        .justify_end()
                        .child(toggle_button(self.show_snoozed).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.show_snoozed = !this.show_snoozed;
                                cx.notify();
                            },
                        ))),
                )
            })
            .when_some(status, |body, status| {
                body.child(inbox_status(tr(status), cx))
            });

        let mut list = v_flex()
            .id("global-inbox-list")
            .gap_2()
            .track_scroll(&self.scroll)
            .overflow_y_scroll()
            .max_h((window.viewport_size().height - px(200.)).clamp(px(120.), px(600.)));
        if ready {
            for workspace in &projects {
                let Some(project) = self.projects.get(&workspace.id) else {
                    continue;
                };
                let title = display_project(workspace, &app.workspaces);
                if let Some(error) = project.errors.first() {
                    list = list.child(inbox_status(
                        format!(
                            "{title} · {} · {}",
                            tr("无法刷新协作收件箱"),
                            localize_owned_error(error)
                        ),
                        cx,
                    ));
                }
                if let Some((label, error)) = project.reminders.notice() {
                    list = list.child(inbox_status(
                        format!(
                            "{title} · {} · {}",
                            tr(label),
                            localize_owned_error(&error.to_string())
                        ),
                        cx,
                    ));
                }
            }
        }

        let connection = self.connection;
        for (id, entry) in rows {
            let Some(project) = projects.iter().find(|project| project.id == id) else {
                continue;
            };
            let title = display_project(project, &app.workspaces);
            let reminders = &self.projects[id].reminders;
            let snoozed = is_snoozed(&entry, reminders, now);
            let target = entry.target.clone();
            let id = id.to_owned();
            let button_workspace = id.clone();
            let open =
                Button::new("open-inbox-item").on_click(cx.listener(move |this, _, window, cx| {
                    if !this.is_current(connection, cx) {
                        return;
                    }
                    let Some(app) = this.app.upgrade() else {
                        return;
                    };
                    this.dismissed = true;
                    window.close_dialog(cx);
                    app.update(cx, |app, cx| {
                        app.open_global_destination(
                            &button_workspace,
                            &target,
                            connection,
                            window,
                            cx,
                        )
                    });
                }));
            list = list.child(inbox_row(
                &id,
                Some(title),
                entry,
                snoozed,
                reminders.saved.is_some(),
                open,
                &self.scroll,
                cx,
                move |this, workspace, key, delay, wake, cx| {
                    if !this.is_current(connection, cx) {
                        return;
                    }
                    let Some(app) = this.app.upgrade() else {
                        return;
                    };
                    let Some(project) = this.projects.get_mut(workspace) else {
                        return;
                    };
                    let state = app.read(cx);
                    if !state.workspaces.iter().any(|item| item.id == workspace)
                        || state.workspace_removing.as_deref() == Some(workspace)
                        || !project.entries(state).iter().any(|entry| {
                            entry.snooze_key.as_deref() == Some(key)
                                && (delay.is_none() || same_wake_target(&entry.wake, wake))
                        })
                    {
                        return;
                    }
                    if let Some(root) = &state.data_dir {
                        project.reminders.set(
                            root,
                            workspace,
                            key,
                            delay.map(|delay| Reminder {
                                expires_at: delay.expires_at(now_ms()),
                                wake: wake.clone(),
                            }),
                        );
                        if state.workspace.as_deref() == Some(workspace)
                            && state.collaboration.visible
                        {
                            app.update(cx, |app, cx| app.refresh_collaboration_inbox(cx));
                        }
                        cx.notify();
                    }
                },
            ));
        }
        body.child(list)
    }
}
