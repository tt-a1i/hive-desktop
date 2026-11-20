#[path = "badge.rs"]
mod badge;
#[path = "placement.rs"]
mod placement;

use self::badge::BadgeItem;
pub(in crate::collaboration) use self::badge::InboxBadge;
use self::placement::{InboxSections, RowId, RowPlacement};
use super::snoozes::now_ms;
use super::undo::{UNDO_CHANGED, UndoSlot, VerifiedUndo, expire_later};
use super::{
    ActionCenter, Destination, Entry, InboxRowInteraction, Reminders, SNOOZE_CHANGED, inbox_row,
    inbox_status, is_snoozed, read_action_center, toggle_button,
};
use crate::{
    app::HiveApp,
    focus_scroll::Reveal,
    language::{localize_owned_error, tr},
    palette::Color,
    protocol::{Member, Session, Workspace},
    transport::Runtime,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenuItem},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

#[derive(Clone, Default)]
pub(in crate::collaboration) struct GlobalInboxView {
    project_filter: Option<String>,
    show_waiting: bool,
    show_snoozed: bool,
    reading_position: Option<ReadingPosition>,
}

#[derive(Clone)]
struct ReadingPosition {
    row: RowId,
    viewport_offset: Pixels,
}

impl ReadingPosition {
    fn item_index(&self, row_ids: &[Option<RowId>]) -> Option<usize> {
        row_ids.iter().position(|id| id.as_ref() == Some(&self.row))
    }
}

struct NewItemsReview {
    previous_position: Option<ReadingPosition>,
}

struct GlobalInbox {
    app: WeakEntity<HiveApp>,
    connection: u64,
    projects: HashMap<String, ProjectInbox>,
    project_filter: Option<String>,
    show_waiting: bool,
    show_snoozed: bool,
    placement: RowPlacement,
    row_focus: HashMap<RowId, (FocusHandle, [Subscription; 2])>,
    row_ids: Vec<Option<RowId>>,
    restore_position: Option<ReadingPosition>,
    new_items: HashSet<BadgeItem>,
    reviewing_new: Option<NewItemsReview>,
    undo: UndoSlot,
    undo_review_item: Option<BadgeItem>,
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

pub(super) async fn read_list<T: DeserializeOwned>(
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
        let attention = self.has_new_global_inbox_items(cx);
        let title = tr("跨项目收件箱");
        let description = if attention {
            format!("{title} · {}", tr("有新事项需要关注"))
        } else {
            title.to_owned()
        };
        Button::new(id)
            .ghost()
            .compact()
            .small()
            .icon(Glyph::Inbox)
            .label(tr("收件箱"))
            .tooltip(description.clone())
            .accessibility_label(description)
            .child(
                div()
                    .flex_shrink_0()
                    .size(px(6.))
                    .rounded_full()
                    .bg(Color::Success.get(cx))
                    .opacity(if attention { 1. } else { 0. }),
            )
            .disabled(self.closing)
            .on_click(cx.listener(|this, _, window, cx| this.open_global_inbox(window, cx)))
    }

    fn open_global_inbox(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing || window.has_active_dialog(cx) {
            return;
        }
        let parent = cx.entity();
        let connection = self.connection_generation;
        let saved = self.collaboration.inbox_view.clone();
        let new_items = self.unseen_global_inbox_items().collect();
        let inbox = cx.new(move |cx: &mut Context<GlobalInbox>| {
            cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            GlobalInbox {
                app: parent.downgrade(),
                connection,
                projects: HashMap::new(),
                project_filter: saved.project_filter,
                show_waiting: saved.show_waiting,
                show_snoozed: saved.show_snoozed,
                placement: RowPlacement::default(),
                row_focus: HashMap::new(),
                row_ids: Vec::new(),
                restore_position: saved.reading_position,
                new_items,
                reviewing_new: None,
                undo: UndoSlot::default(),
                undo_review_item: None,
                dismissed: false,
                scroll: ScrollHandle::new(),
            }
        });
        self.collaboration.inbox_badge.dialog = Some(inbox.downgrade());
        cx.notify();
        inbox.update(cx, |inbox, cx| inbox.poll(cx));
        let width = (window.viewport_size().width - px(36.)).min(px(740.));
        window.open_dialog(cx, move |dialog, _, _| {
            let dismiss = inbox.downgrade();
            dialog
                .title(tr("跨项目收件箱"))
                .w(width)
                .on_ok(|_, _, _| false)
                .on_close(move |_, _, cx| {
                    let _ = dismiss.update(cx, |inbox, cx| inbox.dismiss(cx));
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
    fn review_new_items(&mut self, cx: &mut Context<Self>) {
        if self.reviewing_new.is_some() || self.new_items.is_empty() {
            return;
        }
        let pending = self.restore_position.take();
        self.reviewing_new = Some(NewItemsReview {
            previous_position: self.reading_position().or(pending),
        });
        self.placement.clear();
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn return_to_inbox(&mut self, cx: &mut Context<Self>) {
        let Some(review) = self.reviewing_new.take() else {
            return;
        };
        self.restore_position = review.previous_position;
        self.placement.clear();
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn reading_position(&self) -> Option<ReadingPosition> {
        self.reading_position_without(None)
    }

    fn reading_position_without(&self, excluded: Option<&RowId>) -> Option<ReadingPosition> {
        let offset = self.scroll.offset().y;
        if offset >= px(0.) {
            return None;
        }
        let viewport = self.scroll.bounds();
        let last_visible = self.scroll.bottom_item();
        self.row_ids
            .iter()
            .enumerate()
            .skip(self.scroll.top_item())
            .take_while(|(index, _)| *index <= last_visible)
            .find_map(|(index, row)| {
                let row = row.as_ref()?;
                if Some(row) == excluded {
                    return None;
                }
                let bounds = self.scroll.bounds_for_item(index)?;
                Some(ReadingPosition {
                    row: row.clone(),
                    viewport_offset: bounds.top() + offset - viewport.top(),
                })
            })
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.dismissed {
            return;
        }
        self.dismissed = true;
        self.undo.clear();
        self.undo_review_item = None;
        let saved = GlobalInboxView {
            project_filter: self.project_filter.clone(),
            show_waiting: self.show_waiting,
            show_snoozed: self.show_snoozed,
            reading_position: if let Some(review) = &self.reviewing_new {
                review.previous_position.clone()
            } else {
                self.reading_position()
                    .or_else(|| self.restore_position.clone())
            },
        };
        if let Some(app) = self.app.upgrade() {
            app.update(cx, |app, cx| {
                app.collaboration.inbox_view = saved;
                app.collaboration.inbox_badge.dialog = None;
                cx.notify();
            });
        }
    }

    fn select_project(&mut self, project: Option<String>, cx: &mut Context<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let app = app.read(cx);
        if self.dismissed
            || self.project_filter == project
            || (project.is_some()
                && selected_project(
                    &app.workspaces,
                    app.workspace_removing.as_deref(),
                    project.as_deref(),
                )
                .is_none())
        {
            return;
        }
        self.project_filter = project;
        self.restore_position = None;
        self.placement.clear();
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

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
                        this.placement.clear();
                        this.new_items.clear();
                        this.undo.clear();
                        this.undo_review_item = None;
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

    fn undo_snooze(&mut self, cx: &mut Context<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let connection = self.connection;
        if !self.is_current(connection, cx) {
            return;
        }
        let Some(runtime) = app.read(cx).runtime.clone() else {
            return;
        };
        let Some((generation, change)) = self.undo.claim() else {
            return;
        };
        let review_item = self.undo_review_item.take();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let verified = VerifiedUndo::read(&runtime, &change).await;
            let _ = this.update(cx, |this, cx| {
                if !this.is_current(connection, cx) || !this.undo.is_checking(generation) {
                    return;
                }
                let Some(app) = this.app.upgrade() else {
                    return;
                };
                let state = app.read(cx);
                let workspace = change.edit.workspace.clone();
                let moved_row = RowId::from_key(&workspace, change.edit.key.clone());
                let reading = this.reading_position_without(Some(&moved_row));
                let result = (|| {
                    if state.workspace_removing.as_ref() == Some(&workspace)
                        || !state
                            .workspaces
                            .iter()
                            .any(|project| project.id == workspace)
                        || !verified?.matches(&change.edit, state)
                    {
                        return Err(UNDO_CHANGED);
                    }
                    let project = this.projects.get_mut(&workspace).ok_or(UNDO_CHANGED)?;
                    if !project
                        .entries(state)
                        .iter()
                        .any(|entry| change.edit.matches(entry))
                    {
                        return Err(UNDO_CHANGED);
                    }
                    let root = state.data_dir.as_ref().ok_or("无法保存已暂缓事项")?;
                    project.reminders.restore(root, &change)
                })();
                if result.is_ok() {
                    if let Some(item) = review_item {
                        this.new_items.insert(item);
                    }
                    if let Some(reading) = reading {
                        this.restore_position = Some(reading);
                    }
                    if state.workspace.as_ref() == Some(&workspace) && state.collaboration.visible {
                        app.update(cx, |app, cx| app.refresh_collaboration_inbox(cx));
                    }
                }
                let generation = this.undo.finish(result);
                expire_later(cx, generation, |inbox| &mut inbox.undo);
                cx.notify();
            });
        })
        .detach();
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
        if project.errors.is_empty() {
            let now = now_ms();
            let eligible: HashSet<_> = project
                .entries(app)
                .iter()
                .filter_map(|entry| project.badge_item(app, workspace, entry, now))
                .collect();
            update_new_items(
                &mut self.new_items,
                workspace,
                eligible,
                &app.collaboration.inbox_badge,
            );
        }
        cx.notify();
    }
}

impl ProjectInbox {
    fn badge_item(
        &self,
        app: &HiveApp,
        workspace: &str,
        entry: &Entry,
        now: u64,
    ) -> Option<BadgeItem> {
        if is_snoozed(entry, &self.reminders, now) {
            return None;
        }
        let item = BadgeItem::from_entry(workspace, entry, app)?;
        (!matches!(item, BadgeItem::Question { .. }) || self.reminders.saved.is_some())
            .then_some(item)
    }

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

fn selected_project<'a>(
    workspaces: &'a [Workspace],
    removing: Option<&str>,
    selected: Option<&str>,
) -> Option<&'a Workspace> {
    let id = selected?;
    if removing == Some(id) {
        return None;
    }
    workspaces.iter().find(|project| project.id == id)
}

fn section_title(id: &'static str, title: &str, count: usize, cx: &App) -> impl IntoElement {
    div()
        .id(id)
        .role(Role::Heading)
        .aria_level(2)
        .px_2()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(Color::Text.get(cx))
        .child(format!("{title} ({count})"))
}

fn row_is_visible(viewport: Bounds<Pixels>, row: Bounds<Pixels>, scroll_y: Pixels) -> bool {
    let center = row.top() + row.size.height / 2. + scroll_y;
    viewport.top() <= center && center < viewport.bottom()
}

fn update_new_items(
    items: &mut HashSet<BadgeItem>,
    workspace: &str,
    eligible: HashSet<BadgeItem>,
    badge: &InboxBadge,
) {
    // Seen rows stay in this dialog's review until they are answered, snoozed, or removed.
    items.retain(|item| item.workspace() != workspace || eligible.contains(item));
    items.extend(eligible.into_iter().filter(|item| !badge.sees(item)));
}

impl Render for GlobalInbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex().gap_2().min_w_0();
        let Some(parent) = self.app.upgrade() else {
            return body;
        };
        let app = parent.read(cx);
        let ready = self.is_current(self.connection, cx);
        let reviewing = self.reviewing_new.is_some();
        let removing = app.workspace_removing.as_deref();
        let selected = selected_project(&app.workspaces, removing, self.project_filter.as_deref());
        if ready && selected.is_none() && self.project_filter.take().is_some() {
            self.restore_position = None;
            if let Some(review) = self.reviewing_new.as_mut() {
                review.previous_position = None;
            } else {
                self.placement.clear();
                self.scroll.set_offset(point(px(0.), px(0.)));
            }
        }
        let selected = selected.filter(|_| !reviewing);
        let selected_title = selected.map(|project| display_project(project, &app.workspaces));
        let available: Vec<_> = app
            .workspaces
            .iter()
            .filter(|workspace| removing != Some(workspace.id.as_str()))
            .cloned()
            .collect();
        self.new_items.retain(|item| {
            available
                .iter()
                .any(|project| project.id == item.workspace())
        });
        let projects: Vec<_> = available
            .iter()
            .filter(|project| selected.is_none_or(|selected| project.id == selected.id))
            .collect();
        let now = now_ms();
        let rows: Vec<_> = if ready {
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
                        .filter(|entry| {
                            !reviewing
                                || project
                                    .badge_item(app, &workspace.id, entry, now)
                                    .is_some_and(|item| self.new_items.contains(&item))
                        })
                        .map(|entry| (workspace.id.as_str(), entry))
                })
                .collect()
        } else {
            Vec::new()
        };
        let fresh = InboxSections::from_rows(rows, |id, entry| {
            is_snoozed(entry, &self.projects[id].reminders, now)
        });
        let has_snoozed = !fresh.snoozed.is_empty();
        let loading = projects
            .iter()
            .any(|project| !self.projects.contains_key(&project.id));
        let status = if !ready {
            Some("后台连接后将显示跨项目收件箱。")
        } else if loading {
            Some("正在读取收件箱…")
        } else if fresh.attention.is_empty()
            && fresh.waiting.is_empty()
            && !(has_snoozed && self.show_snoozed)
            && projects.iter().all(|workspace| {
                self.projects
                    .get(&workspace.id)
                    .is_none_or(|project| project.errors.is_empty())
            })
        {
            Some(if reviewing {
                "暂无新事项需要关注。"
            } else if has_snoozed {
                "此范围内的协作事项均已暂缓。"
            } else if selected_title.is_some() {
                "此项目暂无需要关注的协作事项。"
            } else {
                "暂无需要关注的协作事项"
            })
        } else {
            None
        };
        let focused = window
            .last_input_was_keyboard()
            .then(|| {
                self.row_focus
                    .iter()
                    .find_map(|(id, (focus, _))| focus.contains_focused(window, cx).then_some(id))
            })
            .flatten();
        let sections = self
            .placement
            .arrange(fresh, focused, self.show_waiting, self.show_snoozed);
        self.row_focus.retain(|id, _| self.placement.shows(id));
        for id in self.placement.visible_rows() {
            self.row_focus.entry(id.clone()).or_insert_with(|| {
                let focus = cx.focus_handle();
                let enter = cx.on_focus_in(&focus, window, |_, _, cx| cx.notify());
                let exit = cx.on_focus_out(&focus, window, |_, _, _, cx| cx.notify());
                (focus, [enter, exit])
            });
        }
        let app = parent.read(cx);
        if reviewing {
            body = body.child(
                Button::new("global-inbox-back-from-new")
                    .ghost()
                    .compact()
                    .small()
                    .icon(Glyph::ArrowLeft)
                    .label(tr("返回收件箱"))
                    .on_click(cx.listener(|this, _, _, cx| this.return_to_inbox(cx))),
            );
        } else if available.len() > 1 || has_snoozed || !self.new_items.is_empty() {
            let mut controls = h_flex()
                .items_center()
                .justify_between()
                .flex_wrap()
                .gap_2();
            if available.len() > 1 {
                let all = tr("全部项目");
                let title = selected_title.unwrap_or_else(|| all.to_owned());
                let tooltip = format!("{} · {title}", tr("按项目筛选收件箱"));
                let choices: Vec<_> = std::iter::once((None, all.to_owned()))
                    .chain(available.iter().map(|project| {
                        (
                            Some(project.id.clone()),
                            display_project(project, &app.workspaces),
                        )
                    }))
                    .collect();
                let current = self.project_filter.clone();
                let inbox = cx.weak_entity();
                controls = controls.child(
                    Button::new("global-inbox-project-filter")
                        .outline()
                        .compact()
                        .xsmall()
                        .icon(Glyph::ListFilter)
                        .dropdown_caret(true)
                        .max_w(px(320.))
                        .label(title)
                        .tooltip(tooltip.clone())
                        .accessibility_label(tooltip)
                        .dropdown_menu(move |menu, window, _| {
                            let menu = menu
                                .scrollable(true)
                                .max_h((window.viewport_size().height - px(200.)).max(px(120.)))
                                .max_w((window.viewport_size().width - px(48.)).min(px(600.)));
                            choices.iter().fold(menu, |menu, (id, title)| {
                                let checked = current == *id;
                                let id = id.clone();
                                let inbox = inbox.clone();
                                menu.item(
                                    PopupMenuItem::new(title.clone()).checked(checked).on_click(
                                        move |_, _, cx| {
                                            let _ = inbox.update(cx, |this, cx| {
                                                this.select_project(id.clone(), cx);
                                            });
                                        },
                                    ),
                                )
                            })
                        }),
                );
            }
            if has_snoozed {
                controls = controls.child(toggle_button(self.show_snoozed).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.show_snoozed = !this.show_snoozed;
                        this.restore_position = None;
                        this.placement.clear();
                        cx.notify();
                    },
                )));
            }
            if !self.new_items.is_empty() {
                controls = controls.child(
                    Button::new("global-inbox-review-new")
                        .outline()
                        .compact()
                        .xsmall()
                        .label(tr("查看新事项"))
                        .on_click(cx.listener(|this, _, _, cx| this.review_new_items(cx))),
                );
            }
            body = body.child(controls);
        }
        body = body.when_some(status, |body, status| {
            body.child(inbox_status(tr(status), cx))
        });

        // The scroll handle indexes direct children. Keep rows and headings in
        // the same sequence used to compute the saved reading position.
        let mut items: Vec<(Option<RowId>, AnyElement)> = Vec::new();
        if ready {
            for workspace in &projects {
                let Some(project) = self.projects.get(&workspace.id) else {
                    continue;
                };
                let title = display_project(workspace, &app.workspaces);
                if let Some(error) = project.errors.first() {
                    items.push((
                        None,
                        inbox_status(
                            format!(
                                "{title} · {} · {}",
                                tr("无法刷新协作收件箱"),
                                localize_owned_error(error)
                            ),
                            cx,
                        ),
                    ));
                }
                if let Some((label, error)) = project.reminders.notice() {
                    items.push((
                        None,
                        inbox_status(
                            format!(
                                "{title} · {} · {}",
                                tr(label),
                                localize_owned_error(&error.to_string())
                            ),
                            cx,
                        ),
                    ));
                }
            }
        }

        let connection = self.connection;
        let mut candidates = HashMap::new();
        let awaiting_position = self.restore_position.is_some() && (!ready || loading);
        let mut render_row = |(id, entry): (&str, Entry)| {
            let project = projects.iter().find(|project| project.id == id)?;
            let row_id = RowId::for_entry(id, &entry);
            let layout_id = row_id.clone();
            let title = display_project(project, &app.workspaces);
            let data = &self.projects[id];
            let reminders = &data.reminders;
            let reminder = reminders.active_reminder(&entry, now);
            if !awaiting_position
                && let Some(item) = data.badge_item(app, id, &entry, now)
                && !app.collaboration.inbox_badge.sees(&item)
            {
                candidates.insert(row_id.clone(), item);
            }
            let target = entry.target.clone();
            let id = id.to_owned();
            let button_workspace = id.clone();
            let open_id = row_id.clone();
            let open =
                Button::new("open-inbox-item").on_click(cx.listener(move |this, _, window, cx| {
                    if !this.is_current(connection, cx) {
                        return;
                    }
                    let Some(app) = this.app.upgrade() else {
                        return;
                    };
                    if app.read(cx).workspace_removing.as_deref() == Some(&button_workspace)
                        || !this.placement.shows(&open_id)
                        || !this.projects.get(&button_workspace).is_some_and(|project| {
                            project.entries(app.read(cx)).iter().any(|entry| {
                                RowId::for_entry(&button_workspace, entry) == open_id
                                    && entry.target.key() == target.key()
                            })
                        })
                    {
                        return;
                    }
                    this.dismiss(cx);
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
            let (focus, _) = self.row_focus.get(&row_id)?;
            let hovered_id = row_id.clone();
            let interaction = InboxRowInteraction {
                focus: focus.clone(),
                hover: std::rc::Rc::new(cx.listener(move |this, over, _, cx| {
                    if this.placement.hover(&hovered_id, *over) {
                        cx.notify();
                    }
                })),
                snooze_menu: std::rc::Rc::new(cx.listener(move |this, open, _, cx| {
                    if this.placement.menu(&row_id, *open) {
                        cx.notify();
                    }
                })),
            };
            let row = inbox_row(
                &id,
                Some(title),
                entry,
                reminder,
                reminders.saved.is_some(),
                open,
                &self.scroll,
                cx,
                Some(interaction),
                move |this, edit, delay, cx| {
                    let workspace = edit.workspace.as_str();
                    if !this.is_current(connection, cx) {
                        return Err(SNOOZE_CHANGED);
                    }
                    let Some(app) = this.app.upgrade() else {
                        return Err(SNOOZE_CHANGED);
                    };
                    let Some(project) = this.projects.get_mut(workspace) else {
                        return Err(SNOOZE_CHANGED);
                    };
                    let state = app.read(cx);
                    if !state.workspaces.iter().any(|item| item.id == workspace)
                        || state.workspace_removing.as_deref() == Some(workspace)
                    {
                        return Err(SNOOZE_CHANGED);
                    }
                    let entries = project.entries(state);
                    let entry = entries
                        .iter()
                        .find(|entry| edit.matches(entry))
                        .ok_or(SNOOZE_CHANGED)?;
                    let review_item = this
                        .reviewing_new
                        .is_some()
                        .then(|| project.badge_item(state, workspace, entry, now_ms()))
                        .flatten()
                        .filter(|item| this.new_items.contains(item));
                    let root = state.data_dir.as_ref().ok_or("无法保存已暂缓事项")?;
                    let result = project.reminders.set(root, edit, delay);
                    if result.is_ok()
                        && state.workspace.as_deref() == Some(workspace)
                        && state.collaboration.visible
                    {
                        app.update(cx, |app, cx| app.refresh_collaboration_inbox(cx));
                    }
                    cx.notify();
                    if let Some(change) = result? {
                        let generation = this.undo.offer(change);
                        this.undo_review_item = review_item;
                        expire_later(cx, generation, |inbox| &mut inbox.undo);
                    }
                    Ok(())
                },
            );
            Some((Some(layout_id), row))
        };
        if !sections.attention.is_empty() {
            items.push((
                None,
                section_title(
                    "global-inbox-attention",
                    if reviewing {
                        tr("新事项")
                    } else {
                        tr("需要你关注")
                    },
                    sections.attention.len(),
                    cx,
                )
                .into_any_element(),
            ));
            items.extend(sections.attention.into_iter().filter_map(&mut render_row));
        }
        if !sections.waiting.is_empty() {
            let count = sections.waiting.len();
            let (icon, action) = if self.show_waiting {
                (Glyph::ChevronDown, tr("收起等待 Agent 的事项"))
            } else {
                (Glyph::ChevronRight, tr("展开等待 Agent 的事项"))
            };
            items.push((
                None,
                Reveal::new(
                    "global-inbox-waiting",
                    &self.scroll,
                    Button::new("toggle-waiting-inbox")
                        .ghost()
                        .compact()
                        .xsmall()
                        .icon(icon)
                        .label(format!("{} ({count})", tr("等待 Agent")))
                        .accessibility_label(format!("{action} ({count})"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_waiting = !this.show_waiting;
                            this.restore_position = None;
                            this.placement.clear();
                            cx.notify();
                        })),
                )
                .into_any_element(),
            ));
            if self.show_waiting {
                items.extend(sections.waiting.into_iter().filter_map(&mut render_row));
            }
        }
        if self.show_snoozed && has_snoozed {
            items.push((
                None,
                section_title(
                    "global-inbox-snoozed",
                    tr("已暂缓事项"),
                    sections.snoozed.len(),
                    cx,
                )
                .into_any_element(),
            ));
            items.extend(sections.snoozed.into_iter().filter_map(&mut render_row));
        }
        let (ids, elements): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        let candidates: Vec<_> = ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| Some((index, candidates.remove(id.as_ref()?)?)))
            .collect();
        self.row_ids = ids;
        let restore = if ready && !loading {
            self.restore_position.take().and_then(|position| {
                let index = position.item_index(&self.row_ids)?;
                self.scroll.scroll_to_top_of_item(index);
                Some((index, position.viewport_offset))
            })
        } else {
            None
        };
        let restore_or_acknowledge = restore.is_some() || !candidates.is_empty();
        let parent = parent.downgrade();
        let list = v_flex()
            .gap_2()
            .when(restore_or_acknowledge, |list| {
                let scroll = self.scroll.clone();
                list.on_children_prepainted(move |_, window, cx| {
                    if let Some((index, viewport_offset)) = restore
                        && let Some(bounds) = scroll.bounds_for_item(index)
                    {
                        let offset = (scroll.bounds().top() + viewport_offset - bounds.top())
                            .clamp(-scroll.max_offset().y, px(0.));
                        if offset != scroll.offset().y {
                            let scroll = scroll.clone();
                            window.on_next_frame(move |window, _| {
                                scroll.set_offset(point(px(0.), offset));
                                window.refresh();
                            });
                            return;
                        }
                    }
                    if !window.is_window_active() {
                        return;
                    }
                    let viewport = scroll.bounds();
                    let scroll_y = scroll.offset().y;
                    let visible: Vec<_> = candidates
                        .iter()
                        .filter_map(|(index, item)| {
                            row_is_visible(viewport, scroll.bounds_for_item(*index)?, scroll_y)
                                .then(|| item.clone())
                        })
                        .collect();
                    if !visible.is_empty() {
                        let parent = parent.clone();
                        window.defer(cx, move |_, cx| {
                            let _ = parent.update(cx, |app, cx| {
                                if app.connection_generation == connection
                                    && app.collaboration.inbox_badge.acknowledge(visible)
                                {
                                    cx.notify();
                                }
                            });
                        });
                    }
                })
            })
            .id("global-inbox-list")
            .track_scroll(&self.scroll)
            .overflow_y_scroll()
            .max_h((window.viewport_size().height - px(200.)).clamp(px(120.), px(600.)))
            .children(elements)
            .on_scroll(cx.listener(|this, _, _, _| this.restore_position = None));
        body.child(list)
            .when_some(self.undo.render(cx, Self::undo_snooze), |body, undo| {
                body.child(undo)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActionCenter, BadgeItem, Destination, Entry, InboxBadge, InboxSections, ReadingPosition,
        RowId, display_project, row_is_visible, selected_project, update_new_items,
    };
    use crate::collaboration::inbox::Priority;
    use crate::protocol::Workspace;
    use gpui_kit::{Bounds, point, px, size};
    use serde_json::json;
    use std::collections::HashSet;

    fn workspace(id: &str, path: &str) -> Workspace {
        Workspace {
            id: id.into(),
            name: "app".into(),
            path: path.into(),
        }
    }

    #[test]
    fn inbox_project_filter_distinguishes_projects_with_the_same_name() {
        let projects = [
            workspace("left", "/projects/left/app"),
            workspace("right", "/projects/right/app"),
        ];
        let selected = selected_project(&projects, None, Some("right")).unwrap();
        assert_eq!(selected.id, "right");
        assert_eq!(display_project(selected, &projects), "/projects/right/app");
        assert_eq!(
            display_project(&projects[0], &projects),
            "/projects/left/app"
        );
        assert!(selected_project(&projects, None, Some("app")).is_none());
    }

    #[test]
    fn inbox_project_filter_falls_back_when_the_selected_project_is_unavailable() {
        let projects = [workspace("left", "/projects/left/app")];
        assert!(selected_project(&projects, Some("left"), Some("left")).is_none());
        assert!(selected_project(&projects, None, Some("right")).is_none());
        assert!(selected_project(&projects, None, None).is_none());
    }

    #[test]
    fn review_rows_stay_visible_after_acknowledgement_and_drop_when_resolved() {
        let item = BadgeItem::Question {
            workspace: "atlas".into(),
            dispatch: "dispatch".into(),
            message: "first".into(),
        };
        let unseen = BadgeItem::Question {
            workspace: "beacon".into(),
            dispatch: "dispatch".into(),
            message: "first".into(),
        };
        let mut badge = InboxBadge::default();
        let mut items = HashSet::from([unseen.clone()]);
        update_new_items(&mut items, "atlas", HashSet::from([item.clone()]), &badge);
        assert!(badge.acknowledge([item.clone()]));
        update_new_items(&mut items, "atlas", HashSet::from([item.clone()]), &badge);
        assert_eq!(items, HashSet::from([item.clone(), unseen.clone()]));
        update_new_items(&mut items, "atlas", HashSet::new(), &badge);
        assert_eq!(items, HashSet::from([unseen]));
    }

    #[test]
    fn only_rows_with_their_center_in_the_scroll_view_count_as_shown() {
        let bounds = |top| Bounds::new(point(px(0.), px(top)), size(px(100.), px(40.)));
        let viewport = bounds(100.);
        assert!(row_is_visible(viewport, bounds(135.), px(-35.)));
        assert!(!row_is_visible(viewport, bounds(135.), px(0.)));
        assert!(!row_is_visible(viewport, bounds(90.), px(-11.)));
    }

    #[test]
    fn reading_position_follows_the_exact_row_after_insertions() {
        let entry = Entry {
            label: "",
            subject: "question".into(),
            target: Destination::Dispatch("dispatch".into()),
            snooze_key: Some("message:question".into()),
            wake: None,
            priority: Priority::NeedsUser,
            updated_at: 1,
        };
        let anchor = RowId::for_entry("beacon", &entry);
        let atlas = RowId::for_entry("atlas", &entry);
        let saved = ReadingPosition {
            row: anchor.clone(),
            viewport_offset: px(-8.),
        };
        assert_eq!(saved.item_index(&[None, Some(anchor.clone())]), Some(1));
        assert_eq!(
            saved.item_index(&[None, Some(atlas.clone()), None, Some(anchor)]),
            Some(3)
        );
        assert_eq!(saved.item_index(&[None, Some(atlas)]), None);
    }

    #[test]
    fn inbox_sections_keep_actions_and_failures_ahead_of_waiting_items() {
        let dispatch = "70035fa8-80a3-4215-b86c-9d940ae01610";
        let snapshot: ActionCenter = serde_json::from_value(json!({
            "workspace_id": "left",
            "generated_at": 200,
            "attention": [
                { "kind": "dispatch_waiting_report", "dispatch_id": dispatch, "submitted_at": 190 },
                { "kind": "message_delivery_pending", "dispatch_id": dispatch, "message_id": "queued", "submitted_at": 180 },
                { "kind": "question_waiting_answer", "dispatch_id": dispatch, "message_id": "question", "submitted_at": 5 },
                { "kind": "stopped_with_queue", "worker_id": "stopped", "submitted_at": 72 }
            ],
            "recent_dispatch_messages": []
        })).unwrap();
        let mut rows: Vec<_> = snapshot
            .entries(str::to_owned)
            .into_iter()
            .map(|entry| ("left", entry))
            .collect();
        rows.extend(
            [
                ("投递失败", Priority::NeedsUser, 1),
                ("未读会话", Priority::Unread, 220),
            ]
            .map(|(label, priority, updated_at)| {
                (
                    "right",
                    Entry {
                        label,
                        subject: label.into(),
                        target: Destination::Member(label.into()),
                        snooze_key: None,
                        wake: None,
                        priority,
                        updated_at,
                    },
                )
            }),
        );
        let sections = InboxSections::from_rows(rows, |project, entry| {
            project == "left" && entry.snooze_key.as_deref() == Some("message:queued")
        });
        let labels = |entries: Vec<(&str, Entry)>| {
            entries
                .into_iter()
                .map(|(_, entry)| entry.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(sections.attention),
            [
                "成员已停止，仍有待办",
                "等待答复的协作问题",
                "投递失败",
                "未读会话"
            ]
        );
        assert_eq!(labels(sections.waiting), ["等待汇报的派单"]);
        assert_eq!(labels(sections.snoozed), ["等待投递的协作消息"]);
    }
}
