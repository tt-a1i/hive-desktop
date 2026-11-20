#[path = "activity.rs"]
mod activity;
#[path = "badge.rs"]
mod badge;
#[path = "completed.rs"]
mod completed;
#[path = "intro.rs"]
mod intro;
#[path = "view_prefs.rs"]
mod view_prefs;

use self::activity::ActivityRange;
pub(in crate::collaboration) use self::badge::InboxBadge;
use self::intro::{IntroMode, IntroProgress};
use self::view_prefs::{MemberFilter, ViewPreferences};
use self::{
    badge::BadgeItem,
    completed::{Completed, Scope as CompletedScope},
};
use super::placement::{InboxSections, RowId, RowPlacement, Section, SectionDepth};
use super::snoozes::now_ms;
use super::undo::{UNDO_CHANGED, UndoSlot, VerifiedUndo, expire_later};
use super::{
    ActionCenter, ArchiveInboxSession, Destination, Entry, InboxRowInteraction, MarkInboxRead,
    OPEN_INBOX_MEMBER_FAILED, Reminders, SNOOZE_CHANGED, WorkflowRuns, activity_matches,
    inbox_open_button, inbox_row, inbox_status, is_snoozed, read_action_center,
    read_workflow_runs, session_inbox_activity, show_more_button, toggle_button,
    verify_inbox_member, waiting_toggle_button,
};
use crate::{
    app::HiveApp,
    focus_scroll::Reveal,
    language::{localize_owned_error, tr},
    palette::Color,
    protocol::{Backend, Member, Session, SessionHit, Workspace},
    transport::Runtime,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        menu::{DropdownMenu, PopupMenu, PopupMenuItem},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Clone, Default)]
pub(in crate::collaboration) struct GlobalInboxView {
    preferences: ViewPreferences,
    section_depth: SectionDepth,
    reading_position: Option<ReadingPosition>,
    intro_progress: IntroProgress,
    workspaces_loaded: Option<u64>,
    workflow_review: Option<WorkflowReviewResume>,
}

#[derive(Clone)]
struct ReadingPosition(Vec<ReadingAnchor>);

#[derive(Clone)]
struct ReadingAnchor {
    row: RowId,
    viewport_offset: Pixels,
}

impl ReadingPosition {
    fn first_visible(&self, row_ids: &[Option<RowId>]) -> Option<(usize, Pixels)> {
        self.0.iter().find_map(|anchor| {
            let index = row_ids
                .iter()
                .position(|id| id.as_ref() == Some(&anchor.row))?;
            Some((index, anchor.viewport_offset))
        })
    }

    fn without(mut self, excluded: Option<&RowId>) -> Option<Self> {
        self.0.retain(|anchor| Some(&anchor.row) != excluded);
        (!self.0.is_empty()).then_some(self)
    }
}

#[derive(Clone)]
struct NewItemsReview {
    previous_position: Option<ReadingPosition>,
    previous_depth: SectionDepth,
}

#[derive(Clone)]
struct WorkflowReviewResume {
    review: NewItemsReview,
    position: Option<ReadingPosition>,
    depth: SectionDepth,
    items: HashSet<BadgeItem>,
    connection: u64,
}

struct GlobalInboxRowFocus {
    group: FocusHandle,
    open: FocusHandle,
    _subscriptions: [Subscription; 2],
}

struct GlobalInbox {
    app: WeakEntity<HiveApp>,
    connection: u64,
    archive_revision: u64,
    member_open_generation: u64,
    member_open_error: bool,
    completed: Completed,
    projects: HashMap<String, ProjectInbox>,
    preferences: ViewPreferences,
    placement: RowPlacement,
    section_depth: SectionDepth,
    reveal_more: Option<RowId>,
    row_focus: HashMap<RowId, GlobalInboxRowFocus>,
    row_ids: Vec<Option<RowId>>,
    restore_position: Option<ReadingPosition>,
    new_items: HashSet<BadgeItem>,
    reviewing_new: Option<NewItemsReview>,
    undo: UndoSlot,
    undo_review_item: Option<BadgeItem>,
    archive_undo: Option<ArchiveUndoOffer>,
    archive_undo_focus: FocusHandle,
    intro: Option<IntroMode>,
    intro_progress: IntroProgress,
    intro_save_failed: bool,
    intro_focus: FocusHandle,
    help_focus: FocusHandle,
    filter_entry_focus: FocusHandle,
    clear_filters_focus: FocusHandle,
    dismissed: bool,
    scroll: ScrollHandle,
}

type ArchiveUndoAction = Rc<dyn Fn(&mut Window, &mut App)>;

type SelectInboxFilter<T> = fn(&mut GlobalInbox, Option<T>, &mut Context<GlobalInbox>);

#[derive(Clone)]
struct InboxFilter<T> {
    selected: Option<T>,
    choices: Vec<(Option<T>, String)>,
    title: String,
}

impl<T: Clone + PartialEq + 'static> InboxFilter<T> {
    fn new(
        selected: Option<T>,
        available: impl IntoIterator<Item = (T, String)>,
        all: &str,
        unavailable: &str,
    ) -> Self {
        let choices: Vec<_> = std::iter::once((None, all.to_owned()))
            .chain(
                available
                    .into_iter()
                    .map(|(choice, title)| (Some(choice), title)),
            )
            .collect();
        let title = choices
            .iter()
            .find(|(choice, _)| *choice == selected)
            .map_or_else(|| unavailable.to_owned(), |(_, title)| title.clone());
        Self {
            selected,
            choices,
            title,
        }
    }

    fn menu(
        &self,
        menu: PopupMenu,
        window: &Window,
        inbox: &WeakEntity<GlobalInbox>,
        select: SelectInboxFilter<T>,
        max_width: Pixels,
    ) -> PopupMenu {
        let menu = menu
            .scrollable(true)
            .max_h((window.viewport_size().height - px(200.)).max(px(120.)))
            .max_w((window.viewport_size().width - px(48.)).min(max_width));
        self.choices.iter().fold(menu, |menu, (choice, title)| {
            let checked = self.selected == *choice;
            let choice = choice.clone();
            let inbox = inbox.clone();
            menu.item(PopupMenuItem::new(title.clone()).checked(checked).on_click(
                move |_, _, cx| {
                    let _ = inbox.update(cx, |this, cx| select(this, choice.clone(), cx));
                },
            ))
        })
    }

    fn button(
        self,
        button: Button,
        name: &str,
        inbox: WeakEntity<GlobalInbox>,
        select: SelectInboxFilter<T>,
    ) -> AnyElement {
        let label = format!("{name} · {}", self.title);
        button
            .outline()
            .compact()
            .xsmall()
            .dropdown_caret(true)
            .label(self.title.clone())
            .tooltip(label.clone())
            .accessibility_label(label)
            .dropdown_menu(move |menu, window, _| self.menu(menu, window, &inbox, select, px(600.)))
            .into_any_element()
    }

    fn submenu(
        self,
        menu: PopupMenu,
        label: String,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
        inbox: WeakEntity<GlobalInbox>,
        select: SelectInboxFilter<T>,
    ) -> PopupMenu {
        menu.submenu(label, window, cx, move |menu, window, _| {
            self.menu(menu, window, &inbox, select, px(360.))
        })
    }
}

struct ArchiveUndoOffer {
    title: String,
    expires: Instant,
    apply: ArchiveUndoAction,
}

#[derive(Default)]
struct ProjectInbox {
    action_center: Option<ActionCenter>,
    workflow_runs: Option<WorkflowRuns>,
    workflow_runs_error: bool,
    sessions: Vec<Session>,
    members: Vec<Member>,
    reminders: Reminders,
    errors: Vec<String>,
}

struct ProjectRead {
    action_center: Result<ActionCenter, String>,
    sessions: Result<Vec<Session>, String>,
    members: Result<Vec<Member>, String>,
    workflow_runs: Option<WorkflowRuns>,
}

async fn read_project(
    runtime: &Runtime,
    workspace: &str,
    root: Option<&std::path::Path>,
) -> ProjectRead {
    let ((action_center, workflow_runs), (sessions, members)) = smol::future::zip(
        smol::future::zip(
            read_action_center(runtime, workspace, root),
            read_workflow_runs(runtime, workspace, root),
        ),
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
        workflow_runs,
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
    pub(crate) fn offer_inbox_archive_undo(
        &self,
        title: String,
        expires: Instant,
        apply: ArchiveUndoAction,
        cx: &mut Context<Self>,
    ) -> bool {
        self.collaboration
            .inbox_badge
            .dialog
            .as_ref()
            .is_some_and(|dialog| {
                dialog
                    .update(cx, |inbox, cx| {
                        if inbox.dismissed || inbox.connection != self.connection_generation {
                            return false;
                        }
                        inbox.archive_undo = Some(ArchiveUndoOffer {
                            title,
                            expires,
                            apply,
                        });
                        cx.spawn(async move |this, cx| {
                            smol::Timer::after(expires.saturating_duration_since(Instant::now()))
                                .await;
                            let _ = this.update(cx, |inbox, cx| {
                                if inbox
                                    .archive_undo
                                    .as_ref()
                                    .is_some_and(|offer| offer.expires == expires)
                                {
                                    inbox.archive_undo = None;
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                        cx.notify();
                        true
                    })
                    .unwrap_or(false)
            })
    }

    pub(crate) fn apply_inbox_archive_session(
        &mut self,
        session: &Session,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.collaboration.inbox_badge.dialog.clone() else {
            return;
        };
        let _ = dialog.update(cx, |inbox, cx| {
            if inbox.dismissed {
                return;
            }
            inbox.archive_revision = inbox.archive_revision.wrapping_add(1);
            if inbox
                .completed
                .scope
                .as_ref()
                .is_some_and(|scope| scope.contains(session))
            {
                let removed = (!session.archived)
                    .then(|| RowId::for_completed(&session.workspace_id, &session.id));
                if let Some(row) = &removed
                    && inbox
                        .completed
                        .has(&session.workspace_id, &session.id)
                        .is_some()
                {
                    inbox.reserve_completed_row(row);
                }
                inbox.restore_position = inbox
                    .restore_position
                    .take()
                    .and_then(|position| position.without(removed.as_ref()))
                    .or_else(|| inbox.reading_position_without(removed.as_ref()));
                inbox.completed.refresh_loaded(session);
                cx.notify();
            }
            let Some(current) = inbox
                .projects
                .get_mut(&session.workspace_id)
                .and_then(|project| {
                    project.sessions.iter_mut().find(|item| {
                        item.workspace_id == session.workspace_id && item.id == session.id
                    })
                })
            else {
                return;
            };
            current.archived = session.archived;
            current.updated_at = session.updated_at;
            let item = BadgeItem::UnreadSession {
                workspace: session.workspace_id.clone(),
                session: session.id.clone(),
            };
            if session.archived {
                inbox.new_items.remove(&item);
            } else if self.unread.contains(&session.id)
                && !self.collaboration.inbox_badge.sees(&item)
            {
                inbox.new_items.insert(item);
            }
            cx.notify();
        });
    }

    pub(crate) fn show_inbox_restore_error(
        &self,
        workspace: &str,
        id: &str,
        error: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = &self.collaboration.inbox_badge.dialog {
            let error = if error.contains("ARCHIVE_INBOX_STALE") {
                tr("收件箱中的会话已变化，请刷新后重试。").to_owned()
            } else {
                localize_owned_error(error)
            };
            let _ = dialog.update(cx, |inbox, cx| {
                if inbox.completed.has(workspace, id).is_some() {
                    inbox.completed.restore_error = Some(error);
                    cx.notify();
                }
            });
        }
    }

    pub(crate) fn mark_global_inbox_workspaces_loaded(&mut self) {
        self.collaboration.inbox_view.workspaces_loaded = Some(self.connection_generation);
    }

    fn global_inbox_workspace_list(&self) -> Option<&[Workspace]> {
        (self.connected
            && !self.restarting
            && self.collaboration.inbox_view.workspaces_loaded == Some(self.connection_generation))
        .then_some(self.workspaces.as_slice())
    }

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

    pub(crate) fn open_global_inbox(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing || window.has_active_dialog(cx) {
            return;
        }
        let parent = cx.entity();
        let connection = self.connection_generation;
        let mut saved = self.collaboration.inbox_view.clone();
        let root = self.data_dir.as_deref();
        let root_changed = saved.preferences.restore_root(root);
        let project_removed = saved
            .preferences
            .forget_unavailable(self.global_inbox_workspace_list());
        if !project_removed {
            saved.preferences.retry_save();
        }
        if root_changed || project_removed {
            saved.reading_position = None;
        }
        let workflow_review = self
            .collaboration
            .inbox_view
            .workflow_review
            .take()
            .filter(|resume| !root_changed && !project_removed && resume.connection == connection);
        let intro_save_failed = saved.intro_progress.save_pending(root).is_err();
        let intro = saved
            .intro_progress
            .should_show(root)
            .then_some(IntroMode::FirstUse);
        let mut new_items: HashSet<_> = self.unseen_global_inbox_items().collect();
        if let Some(resume) = &workflow_review {
            new_items.extend(
                resume
                    .items
                    .iter()
                    .filter(|item| {
                        self.workspaces
                            .iter()
                            .any(|workspace| workspace.id == item.workspace())
                    })
                    .cloned(),
            );
        }
        let inbox = cx.new(move |cx: &mut Context<GlobalInbox>| {
            cx.observe(&parent, |this, parent, cx| {
                let app = parent.read(cx);
                if this
                    .intro_progress
                    .save_pending(app.data_dir.as_deref())
                    .is_err()
                {
                    this.intro_save_failed = true;
                }
                if !this.dismissed {
                    let root_changed = this.preferences.restore_root(app.data_dir.as_deref());
                    if root_changed {
                        this.projects.clear();
                        this.new_items.clear();
                        this.reviewing_new = None;
                    }
                    let project_removed = this
                        .preferences
                        .forget_unavailable(app.global_inbox_workspace_list());
                    if root_changed || project_removed {
                        this.clear_filter_position();
                    }
                }
                this.sync_completed(cx);
                cx.notify();
            })
            .detach();
            GlobalInbox {
                app: parent.downgrade(),
                connection,
                archive_revision: 0,
                member_open_generation: 0,
                member_open_error: false,
                completed: Completed::default(),
                projects: HashMap::new(),
                preferences: saved.preferences,
                placement: RowPlacement::default(),
                section_depth: workflow_review
                    .as_ref()
                    .map_or(saved.section_depth, |resume| resume.depth),
                reveal_more: None,
                row_focus: HashMap::new(),
                row_ids: Vec::new(),
                restore_position: workflow_review
                    .as_ref()
                    .map_or(saved.reading_position, |resume| resume.position.clone()),
                new_items,
                reviewing_new: workflow_review.map(|resume| resume.review),
                undo: UndoSlot::default(),
                undo_review_item: None,
                archive_undo: None,
                archive_undo_focus: cx.focus_handle(),
                intro,
                intro_progress: saved.intro_progress,
                intro_save_failed,
                intro_focus: cx.focus_handle(),
                help_focus: cx.focus_handle(),
                filter_entry_focus: cx.focus_handle(),
                clear_filters_focus: cx.focus_handle(),
                dismissed: false,
                scroll: ScrollHandle::new(),
            }
        });
        self.collaboration.inbox_badge.dialog = Some(inbox.downgrade());
        cx.notify();
        inbox.update(cx, |inbox, cx| inbox.poll(cx));
        let intro_focus = intro.map(|_| inbox.read(cx).intro_focus.clone());
        window.open_dialog(cx, move |dialog, window, _| {
            let dismiss = inbox.downgrade();
            let cancel = inbox.downgrade();
            let confirm = inbox.downgrade();
            dialog
                .title(tr("跨项目收件箱"))
                .w((window.viewport_size().width - px(36.)).min(px(740.)))
                .on_ok(move |_, window, cx| {
                    let _ = confirm.update(cx, |inbox, cx| inbox.confirm(window, cx));
                    false
                })
                .on_cancel(move |_, window, cx| {
                    cancel
                        .update(cx, |inbox, cx| !inbox.finish_intro(window, cx))
                        .unwrap_or(true)
                })
                .on_close(move |_, _, cx| {
                    let _ = dismiss.update(cx, |inbox, cx| inbox.dismiss(cx));
                })
                .child(inbox.clone())
        });
        if let Some(focus) = intro_focus {
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
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
            Destination::Session(_) | Destination::Member(_) => {}
            Destination::WorkflowRun { id, running } => {
                self.open_global_inbox_workflow(
                    workspace.to_owned(),
                    id.clone(),
                    *running,
                    window,
                    cx,
                );
            }
            Destination::Dispatch(_) | Destination::DispatchMessage { .. } => {
                // Open the actual workspace first. The dispatch detail then
                // resolves its original execution session through Hive's existing route.
                if self.workspace.as_deref() != Some(workspace) {
                    self.select_workspace(workspace.to_owned(), cx);
                }
                self.open_collaboration(cx);
                if window.viewport_size().width < px(820.) {
                    self.sidebar_open = false;
                }
                self.open_inbox_dispatch(target, None, cx);
            }
        }
    }
}

impl GlobalInbox {
    fn open_inbox_member(
        &mut self,
        workspace: String,
        member: String,
        row: RowId,
        connection: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(parent) = self.app.upgrade() else {
            return;
        };
        let app = parent.read(cx);
        let (Some(origin), Some(runtime)) = (app.history_link_target(), app.runtime.clone()) else {
            return;
        };
        self.member_open_generation += 1;
        self.member_open_error = false;
        let generation = self.member_open_generation;
        cx.spawn_in(window, async move |this, cx| {
            let roster = verify_inbox_member(&runtime, &workspace, &member).await;
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |inbox, cx| {
                    let app = parent.read(cx);
                    if !inbox.is_current(connection, cx)
                        || !origin.current(app)
                        || inbox.member_open_generation != generation
                        || !inbox.row_focus.contains_key(&row)
                        || app.workspace_removing.as_deref() == Some(&workspace)
                        || !app.workspaces.iter().any(|listed| listed.id == workspace)
                    {
                        return;
                    }
                    if let Some(roster) = roster {
                        inbox.dismiss(cx);
                        window.close_dialog(cx);
                        parent.update(cx, |app, cx| {
                            app.show_verified_inbox_member(&workspace, &member, roster, window, cx);
                        });
                    } else {
                        inbox.member_open_error = true;
                        cx.notify();
                    }
                });
            });
        })
        .detach();
    }

    fn completed_scope(&self, cx: &App) -> Option<CompletedScope> {
        if !self.completed.expanded
            || self.dismissed
            || self.intro.is_some()
            || self.reviewing_new.is_some()
        {
            return None;
        }
        let binding = self.app.upgrade()?;
        let app = binding.read(cx);
        if app.closing || app.connection_generation != self.connection {
            return None;
        }
        let workspaces = app.global_inbox_workspace_list()?;
        let project = self.preferences.values.project_filter.as_ref();
        let member = self.preferences.values.member_filter.as_ref();
        if let (Some(project), Some(member)) = (project, member)
            && project != &member.workspace_id
        {
            return None;
        }
        let workspace = project
            .or(member.map(|member| &member.workspace_id))
            .cloned();
        let visible: std::collections::BTreeSet<_> = workspaces
            .iter()
            .filter(|project| {
                app.workspace_removing.as_ref() != Some(&project.id)
                    && workspace.as_ref().is_none_or(|id| id == &project.id)
            })
            .map(|project| project.id.clone())
            .collect();
        Some(CompletedScope {
            connection: self.connection,
            workspace,
            member: member.map(|member| member.member_id.clone()),
            backend: self.preferences.values.backend_filter,
            activity: self.preferences.values.activity_filter,
            workspaces: visible,
        })
    }

    fn sync_completed(&mut self, cx: &mut Context<Self>) {
        self.completed.change_scope(self.completed_scope(cx));
        if self
            .completed
            .scope
            .as_ref()
            .is_some_and(|scope| scope.workspaces.is_empty())
        {
            self.completed.refresh = false;
        }
        if self.completed.refresh {
            self.load_completed(0, cx);
        }
    }

    fn toggle_completed(&mut self, cx: &mut Context<Self>) {
        self.completed.expanded = !self.completed.expanded;
        self.sync_completed(cx);
        cx.notify();
    }

    fn load_completed(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.completed.task.is_some()
            || (self.completed.requested_offset() != Some(offset)
                && !(offset == 0 && self.completed.refresh))
        {
            return;
        }
        let Some(scope) = self.completed.scope.clone() else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let Some(runtime) = app.read(cx).runtime.clone() else {
            return;
        };
        let generation = self.completed.generation;
        let limit = self.completed.request_limit(offset);
        if offset == 0 {
            self.completed.activity_since = scope.activity.map(|range| range.cutoff(now_ms()));
        }
        let activity_since = self.completed.activity_since;
        let snapshot = if offset == 0 {
            None
        } else {
            self.completed.snapshot.clone()
        };
        self.completed.refresh = false;
        self.completed.task = Some(cx.spawn(async move |this, cx| {
            if offset == 0 {
                smol::Timer::after(Duration::from_millis(150)).await;
            }
            let result = completed::read(
                &runtime,
                &scope,
                offset,
                limit,
                snapshot.as_deref(),
                activity_since,
            )
            .await;
            let _ = this.update(cx, |inbox, cx| {
                if inbox.completed_scope(cx).as_ref() == Some(&scope)
                    && inbox.completed.generation == generation
                {
                    if let (Some(range), Ok(page)) = (scope.activity, &result) {
                        let current = range.cutoff(now_ms());
                        if inbox.completed.clock_moved_back(current)
                            || page.has_older_activity(current)
                        {
                            inbox.completed.refresh_activity();
                            inbox.sync_completed(cx);
                            return;
                        }
                    }
                    let reading = match &result {
                        Ok(page) if offset == 0 && !inbox.completed.rows.is_empty() => {
                            let removed = inbox.completed.missing_rows(page);
                            inbox.reading_position_matching(|row| !removed.contains(row))
                        }
                        _ => None,
                    };
                    if !inbox.completed.apply(generation, offset, result) {
                        return;
                    }
                    if reading.is_some() {
                        inbox.restore_position = reading;
                    }
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn completed_hit(&self, workspace: &str, id: &str, cx: &App) -> Option<SessionHit> {
        let scope = self.completed_scope(cx)?;
        let hit = self.completed.has(workspace, id)?;
        (self.completed.scope.as_ref() == Some(&scope)
            && scope.contains(&hit.session)
            && activity_matches(
                session_inbox_activity(&hit.session),
                scope.activity.map(|range| range.cutoff(now_ms())),
            ))
        .then(|| hit.clone())
    }

    fn restore_completed(
        &mut self,
        workspace: &str,
        id: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(hit) = self.completed_hit(workspace, id, cx) else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        self.completed.restore_error = None;
        app.update(cx, |app, cx| app.restore_inbox_session(hit.session, cx));
        cx.notify();
    }

    fn open_completed(
        &mut self,
        workspace: &str,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(hit) = self.completed_hit(workspace, id, cx) else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.update(cx, |app, cx| app.navigate_to_search_hit(hit, window, cx)) {
            self.dismiss(cx);
            window.close_dialog(cx);
        }
    }

    fn request_archive(
        &self,
        archive: &ArchiveInboxSession,
        row: &RowId,
        connection: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dialog = cx.weak_entity();
        let app = self.app.clone();
        let target = archive.clone();
        let row = row.clone();
        // Read the inbox after this row's entity update has returned.
        window.defer(cx, move |window, cx| {
            let _ = app.update(cx, |app, cx| {
                app.request_inbox_session_archive(
                    target.session.clone(),
                    target.revision,
                    true,
                    move |app, cx| {
                        let Some(dialog) = dialog.upgrade() else {
                            return false;
                        };
                        let dialog = dialog.read(cx);
                        let workspace = target.session.workspace_id.as_str();
                        !dialog.dismissed
                            && app.connection_generation == connection
                            && dialog.connection == connection
                            && dialog.placement.shows(&row)
                            && dialog.projects.get(workspace).is_some_and(|project| {
                                project.entries(app).iter().any(|entry| {
                                    RowId::for_entry(workspace, entry) == row
                                        && ArchiveInboxSession::for_entry(
                                            app,
                                            workspace,
                                            entry,
                                            &project.members,
                                            &project.reminders,
                                            is_snoozed(entry, &project.reminders, now_ms())
                                                || dialog.placement.held_reminder(&row).is_some(),
                                        )
                                        .is_some_and(|current| target.matches(&current))
                                })
                            })
                    },
                    window,
                    cx,
                );
            });
        });
    }

    fn review_new_items(&mut self, cx: &mut Context<Self>) {
        if self.reviewing_new.is_some() || self.new_items.is_empty() {
            return;
        }
        let pending = self.restore_position.take();
        self.reviewing_new = Some(NewItemsReview {
            previous_position: self.reading_position().or(pending),
            previous_depth: std::mem::take(&mut self.section_depth),
        });
        self.reveal_more = None;
        self.sync_completed(cx);
        self.placement.clear();
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn return_to_inbox(&mut self, cx: &mut Context<Self>) {
        let Some(review) = self.reviewing_new.take() else {
            return;
        };
        self.restore_position = review.previous_position;
        self.section_depth = review.previous_depth;
        self.reveal_more = None;
        self.sync_completed(cx);
        self.placement.clear();
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn reading_position(&self) -> Option<ReadingPosition> {
        self.reading_position_without(None)
    }

    fn replay_intro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.restore_position = self
            .reading_position()
            .or_else(|| self.restore_position.take());
        self.intro = Some(IntroMode::Replay);
        self.sync_completed(cx);
        let focus = self.intro_focus.clone();
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
        cx.notify();
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The dialog consumes Enter as Confirm before buttons receive the key.
        if self.intro.is_some() && self.intro_focus.is_focused(window) {
            self.finish_intro(window, cx);
        } else if self.intro.is_none() && self.help_focus.is_focused(window) {
            self.replay_intro(window, cx);
        } else if self.intro.is_none() && self.archive_undo_focus.is_focused(window) {
            self.undo_archive(window, cx);
        } else if self.clear_filters_focus.is_focused(window) {
            self.clear_filters(window, cx);
        }
    }

    fn undo_archive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(offer) = self.archive_undo.take() {
            (offer.apply)(window, cx);
            cx.notify();
        }
    }

    fn finish_intro(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(mode) = self.intro.take() else {
            return false;
        };
        self.sync_completed(cx);
        if matches!(mode, IntroMode::FirstUse) {
            let app = self.app.upgrade();
            let root = app
                .as_ref()
                .and_then(|app| app.read(cx).data_dir.as_deref());
            self.intro_save_failed = self.intro_progress.acknowledge(root).is_err();
        }
        let focus = self.help_focus.clone();
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
        cx.notify();
        true
    }

    fn help_button(&self, cx: &Context<Self>) -> Button {
        Button::new("global-inbox-help")
            .ghost()
            .compact()
            .xsmall()
            .icon(Glyph::Info)
            .label(tr("收件箱帮助"))
            .track_focus(&self.help_focus)
            .on_click(cx.listener(|this, _, window, cx| this.replay_intro(window, cx)))
    }

    fn reading_position_without(&self, excluded: Option<&RowId>) -> Option<ReadingPosition> {
        self.reading_position_matching(|row| Some(row) != excluded)
    }

    fn reading_position_matching(
        &self,
        mut matches: impl FnMut(&RowId) -> bool,
    ) -> Option<ReadingPosition> {
        let offset = self.scroll.offset().y;
        if offset >= px(0.) {
            return None;
        }
        let viewport = self.scroll.bounds();
        let last_visible = self.scroll.bottom_item();
        let anchors: Vec<_> = self
            .row_ids
            .iter()
            .enumerate()
            .skip(self.scroll.top_item())
            .take_while(|(index, _)| *index <= last_visible)
            .filter_map(|(index, row)| {
                let row = row.as_ref()?;
                if !matches(row) {
                    return None;
                }
                let bounds = self.scroll.bounds_for_item(index)?;
                let top = bounds.top() + offset;
                if top >= viewport.bottom() || bounds.bottom() + offset <= viewport.top() {
                    return None;
                }
                Some(ReadingAnchor {
                    row: row.clone(),
                    viewport_offset: top - viewport.top(),
                })
            })
            .collect();
        (!anchors.is_empty()).then_some(ReadingPosition(anchors))
    }

    fn reserve_completed_row(&mut self, row: &RowId) {
        let Some(index) = self.row_ids.iter().position(|id| id.as_ref() == Some(row)) else {
            return;
        };
        let Some(bounds) = self.scroll.bounds_for_item(index) else {
            return;
        };
        let row_extent = self
            .scroll
            .bounds_for_item(index + 1)
            .map_or(bounds.size.height, |next| {
                (next.top() - bounds.top()).max(bounds.size.height)
            });
        let footer_height = self
            .row_ids
            .len()
            .checked_sub(1)
            .filter(|last| *last != index && self.row_ids[*last].is_none())
            .and_then(|last| self.scroll.bounds_for_item(last))
            .map_or(px(0.), |footer| footer.size.height);
        self.completed
            .reserve_loading_tail(row_extent, footer_height);
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.dismissed {
            return;
        }
        self.dismissed = true;
        self.completed.change_scope(None);
        self.undo.clear();
        self.undo_review_item = None;
        self.archive_undo = None;
        if let Some(app) = self.app.upgrade() {
            app.update(cx, |app, cx| {
                app.collaboration.inbox_view = GlobalInboxView {
                    preferences: self.preferences.clone(),
                    section_depth: self
                        .reviewing_new
                        .as_ref()
                        .map_or(self.section_depth, |review| review.previous_depth),
                    reading_position: if let Some(review) = &self.reviewing_new {
                        review.previous_position.clone()
                    } else {
                        self.reading_position()
                            .or_else(|| self.restore_position.clone())
                    },
                    intro_progress: self.intro_progress.clone(),
                    workspaces_loaded: app.collaboration.inbox_view.workspaces_loaded,
                    workflow_review: None,
                };
                app.collaboration.inbox_badge.dialog = None;
                cx.notify();
            });
        }
    }

    fn dismiss_for_workflow(&mut self, cx: &mut Context<Self>) {
        let resume = self
            .reviewing_new
            .as_ref()
            .map(|review| WorkflowReviewResume {
                review: review.clone(),
                position: self
                    .reading_position()
                    .or_else(|| self.restore_position.clone()),
                depth: self.section_depth,
                items: self.new_items.clone(),
                connection: self.connection,
            });
        self.dismiss(cx);
        if let Some(parent) = self.app.upgrade() {
            parent.update(cx, |app, _| {
                app.collaboration.inbox_view.workflow_review = resume
            });
        }
    }

    fn select_project(&mut self, project: Option<String>, cx: &mut Context<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let app = app.read(cx);
        if !self.is_current(self.connection, cx)
            || self.reviewing_new.is_some()
            || app.global_inbox_workspace_list().is_none()
            || self.preferences.values.project_filter == project
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
        self.preferences.select_project(project);
        self.clear_filter_position();
        self.sync_completed(cx);
        cx.notify();
    }

    fn select_member(&mut self, member: Option<MemberFilter>, cx: &mut Context<Self>) {
        let Some(parent) = self.app.upgrade() else {
            return;
        };
        let app = parent.read(cx);
        if !self.is_current(self.connection, cx)
            || self.reviewing_new.is_some()
            || app.global_inbox_workspace_list().is_none()
            || self.preferences.values.member_filter == member
            || member.as_ref().is_some_and(|member| {
                self.preferences
                    .values
                    .project_filter
                    .as_ref()
                    .is_some_and(|project| project != &member.workspace_id)
                    || selected_project(
                        &app.workspaces,
                        app.workspace_removing.as_deref(),
                        Some(&member.workspace_id),
                    )
                    .is_none()
                    || self
                        .projects
                        .get(&member.workspace_id)
                        .is_none_or(|project| {
                            !project
                                .members
                                .iter()
                                .any(|item| item.id == member.member_id)
                        })
            })
        {
            return;
        }
        self.preferences.values.member_filter = member;
        self.preferences.save_changed();
        self.clear_filter_position();
        self.sync_completed(cx);
        cx.notify();
    }

    fn select_backend(&mut self, backend: Option<Backend>, cx: &mut Context<Self>) {
        if !self.is_current(self.connection, cx)
            || self.reviewing_new.is_some()
            || self.preferences.values.backend_filter == backend
        {
            return;
        }
        self.preferences.values.backend_filter = backend;
        self.preferences.save_changed();
        self.clear_filter_position();
        self.sync_completed(cx);
        cx.notify();
    }

    fn select_activity(&mut self, activity: Option<ActivityRange>, cx: &mut Context<Self>) {
        if !self.is_current(self.connection, cx)
            || self.reviewing_new.is_some()
            || self.preferences.values.activity_filter == activity
        {
            return;
        }
        self.preferences.values.activity_filter = activity;
        self.preferences.save_changed();
        self.clear_filter_position();
        self.sync_completed(cx);
        cx.notify();
    }

    fn set_hide_workflow_runs(&mut self, hide: bool, cx: &mut Context<Self>) {
        if !self.is_current(self.connection, cx)
            || self.reviewing_new.is_some()
            || self.preferences.values.hide_workflow_runs == hide
        {
            return;
        }
        self.preferences.values.hide_workflow_runs = hide;
        self.preferences.save_changed();
        self.clear_filter_position();
        cx.notify();
    }

    fn filter_controls(
        &self,
        app: &HiveApp,
        available: &[Workspace],
        compact: bool,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let current = &self.preferences.values;
        let projects = InboxFilter::new(
            current.project_filter.clone(),
            available.iter().map(|project| {
                (
                    project.id.clone(),
                    display_project(project, &app.workspaces),
                )
            }),
            tr("全部项目"),
            tr("全部项目"),
        );
        let members = InboxFilter::new(
            current.member_filter.clone(),
            member_choices(
                &app.workspaces,
                current.project_filter.as_deref(),
                app.workspace_removing.as_deref(),
                &self.projects,
            ),
            tr("全部成员"),
            tr("已选成员"),
        );
        let backends = InboxFilter::new(
            current.backend_filter,
            [
                Backend::Pi,
                Backend::PiRpc,
                Backend::Codex,
                Backend::Claude,
                Backend::Acp,
            ]
            .into_iter()
            .map(|backend| (backend, tr(backend.label()).to_owned())),
            tr("全部后端"),
            tr("全部后端"),
        );
        let activity = InboxFilter::new(
            current.activity_filter,
            ActivityRange::OPTIONS
                .into_iter()
                .flatten()
                .map(|range| (range, tr(ActivityRange::label(Some(range))).to_owned())),
            tr("全部时间"),
            tr("全部时间"),
        );

        let inbox = cx.weak_entity();
        let hide_workflow_runs = current.hide_workflow_runs;
        let hide_workflow_runs_label = tr("隐藏工作流运行");
        if compact {
            let count = self.preferences.filter_count();
            let title = tr("筛选");
            let button_title = if count == 0 {
                title.to_owned()
            } else {
                format!("{title} ({count})")
            };
            let labels = [
                format!("{}: {}", tr("项目"), projects.title),
                format!("{}: {}", tr("成员"), members.title),
                format!("{}: {}", tr("Agent 后端"), backends.title),
                format!("{}: {}", tr("最近活跃"), activity.title),
            ];
            let count_label = tr("{count} 项筛选生效").replace("{count}", &count.to_string());
            let mut summary = format!(
                "{} · {count_label} · {}",
                tr("收件箱筛选"),
                labels.join(" · ")
            );
            if hide_workflow_runs {
                summary.push_str(" · ");
                summary.push_str(hide_workflow_runs_label);
            }
            return vec![
                Button::new("global-inbox-compact-filters")
                    .outline()
                    .compact()
                    .xsmall()
                    .icon(Glyph::ListFilter)
                    .dropdown_caret(true)
                    .track_focus(&self.filter_entry_focus)
                    .label(button_title)
                    .tooltip(summary.clone())
                    .accessibility_label(summary)
                    .dropdown_menu(move |menu, window, cx| {
                        let menu =
                            menu.max_w((window.viewport_size().width - px(48.)).min(px(360.)));
                        let menu = projects.clone().submenu(
                            menu,
                            labels[0].clone(),
                            window,
                            cx,
                            inbox.clone(),
                            Self::select_project,
                        );
                        let menu = members.clone().submenu(
                            menu,
                            labels[1].clone(),
                            window,
                            cx,
                            inbox.clone(),
                            Self::select_member,
                        );
                        let menu = backends.clone().submenu(
                            menu,
                            labels[2].clone(),
                            window,
                            cx,
                            inbox.clone(),
                            Self::select_backend,
                        );
                        let menu = activity.clone().submenu(
                            menu,
                            labels[3].clone(),
                            window,
                            cx,
                            inbox.clone(),
                            Self::select_activity,
                        );
                        let toggle_inbox = inbox.clone();
                        let menu = menu.item(
                            PopupMenuItem::new(hide_workflow_runs_label)
                                .checked(hide_workflow_runs)
                                .on_click(move |_, _, cx| {
                                    let _ = toggle_inbox.update(cx, |this, cx| {
                                        this.set_hide_workflow_runs(!hide_workflow_runs, cx)
                                    });
                                }),
                        );
                        if count == 0 {
                            return menu;
                        }
                        let inbox = inbox.clone();
                        menu.separator()
                            .item(PopupMenuItem::new(tr("清除筛选")).on_click(
                                move |_, window, cx| {
                                    let _ =
                                        inbox.update(cx, |this, cx| this.clear_filters(window, cx));
                                },
                            ))
                    })
                    .into_any_element(),
            ];
        }

        let mut controls = vec![
            projects.button(
                Button::new("global-inbox-project-filter")
                    .track_focus(&self.filter_entry_focus)
                    .icon(Glyph::ListFilter)
                    .max_w(px(320.)),
                tr("按项目筛选收件箱"),
                inbox.clone(),
                Self::select_project,
            ),
        ];
        if members.choices.len() > 1 || members.selected.is_some() {
            controls.push(members.button(
                Button::new("global-inbox-member-filter").max_w(px(260.)),
                tr("按成员筛选收件箱"),
                inbox.clone(),
                Self::select_member,
            ));
        }
        controls.push(backends.button(
            Button::new("global-inbox-backend-filter"),
            tr("按后端筛选收件箱"),
            inbox.clone(),
            Self::select_backend,
        ));
        controls.push(activity.button(
            Button::new("global-inbox-activity-filter"),
            tr("按活跃时间筛选收件箱"),
            inbox,
            Self::select_activity,
        ));
        controls.push(
            Checkbox::new("global-inbox-hide-workflow-runs")
                .label(hide_workflow_runs_label)
                .checked(hide_workflow_runs)
                .on_change(cx.listener(|this, checked, _, cx| {
                    this.set_hide_workflow_runs(*checked, cx);
                }))
                .into_any_element(),
        );
        if self.preferences.has_filters() {
            controls.push(
                Button::new("global-inbox-clear-filters")
                    .ghost()
                    .compact()
                    .xsmall()
                    .label(tr("清除筛选"))
                    .track_focus(&self.clear_filters_focus)
                    .on_click(cx.listener(|this, _, window, cx| this.clear_filters(window, cx)))
                    .into_any_element(),
            );
        }
        controls
    }

    fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.intro.is_some()
            || self.reviewing_new.is_some()
            || !self.is_current(self.connection, cx)
            || !self.preferences.clear_filters()
        {
            return;
        }
        self.clear_filter_position();
        self.sync_completed(cx);
        let focus = self.filter_entry_focus.clone();
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
        cx.notify();
    }

    fn clear_filter_position(&mut self) {
        self.member_open_generation += 1;
        self.member_open_error = false;
        self.restore_position = None;
        self.reveal_more = None;
        if let Some(review) = self.reviewing_new.as_mut() {
            review.previous_position = None;
            review.previous_depth = SectionDepth::default();
        } else {
            self.section_depth = SectionDepth::default();
            self.placement.clear();
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
    }

    fn poll(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(Duration::from_secs(2)).await;
                let keep_running = this.update(cx, |this, cx| {
                    if this.dismissed {
                        return false;
                    }
                    if let Some(range) = this.preferences.values.activity_filter
                        && this.reviewing_new.is_none()
                        && this.intro.is_none()
                    {
                        if this
                            .completed
                            .activity_needs_refresh(range.cutoff(now_ms()))
                        {
                            this.completed.refresh_activity();
                            this.sync_completed(cx);
                        }
                        cx.notify();
                    }
                    true
                });
                if !matches!(keep_running, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some((connection, runtime, workspaces, root))) =
                    this.update(cx, |this, cx| {
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
                            this.archive_undo = None;
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
                            app.data_dir.clone(),
                        ))
                    })
                else {
                    break;
                };
                if let Some(runtime) = runtime {
                    // Keep a single sweep in flight and at most three requests
                    // running together. Update each workspace independently.
                    for workspace in workspaces {
                        let Ok(Some(archive_revision)) = this.update(cx, |this, cx| {
                            this.is_current(connection, cx)
                                .then_some(this.archive_revision)
                        }) else {
                            break;
                        };
                        let result = read_project(&runtime, &workspace, root.as_deref()).await;
                        if this
                            .update(cx, |this, cx| {
                                this.apply(
                                    &workspace,
                                    connection,
                                    archive_revision,
                                    root.as_deref(),
                                    result,
                                    cx,
                                )
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

    fn mark_read(
        &mut self,
        mark: &MarkInboxRead,
        row: &RowId,
        connection: u64,
        cx: &mut Context<Self>,
    ) {
        if !self.is_current(connection, cx) || !self.placement.shows(row) {
            return;
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let state = app.read(cx);
        if state.workspace_removing.as_deref() == Some(&mark.workspace)
            || !state
                .workspaces
                .iter()
                .any(|workspace| workspace.id == mark.workspace)
            || !self.projects.get(&mark.workspace).is_some_and(|project| {
                project
                    .entries(state)
                    .iter()
                    .any(|entry| mark.matches(entry))
            })
        {
            return;
        }
        if let Ok(saved) = app.update(cx, |app, cx| mark.clear(app, cx)) {
            if let Some(saved) = saved
                && let Some(project) = self.projects.get_mut(&mark.workspace)
            {
                project.reminders.saved = Some(saved);
            }
            self.new_items.remove(&BadgeItem::UnreadSession {
                workspace: mark.workspace.clone(),
                session: mark.session.clone(),
            });
            cx.notify();
        }
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
                        || !change.edit.reply_is_current(state)
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
                    let refresh_scoped =
                        state.workspace.as_ref() == Some(&workspace) && state.collaboration.visible;
                    app.update(cx, |app, cx| {
                        if let Some(saved) = this
                            .projects
                            .get(&workspace)
                            .and_then(|project| project.reminders.saved.as_ref())
                        {
                            app.collaboration
                                .inbox_badge
                                .remember_reminders(&workspace, saved);
                            app.sync_inbox_deadlines(cx);
                        }
                        if refresh_scoped {
                            app.refresh_collaboration_inbox(cx);
                        }
                    });
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
        archive_revision: u64,
        root: Option<&Path>,
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
        if app.data_dir.as_deref() != root
            || !app.workspaces.iter().any(|project| project.id == workspace)
        {
            return;
        }
        let project = self.projects.entry(workspace.to_owned()).or_default();
        let refreshed_workflows = result.workflow_runs.is_some();
        project.workflow_runs_error = !refreshed_workflows;
        if let Some(workflows) = result.workflow_runs {
            project.workflow_runs = Some(workflows);
        }
        let mut errors = Vec::new();
        let snapshot = match result.action_center {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                errors.push(error);
                None
            }
        };
        match result.sessions {
            Ok(sessions) if self.archive_revision == archive_revision => {
                project.sessions = sessions
            }
            Ok(_) => (), // A confirmed archive superseded this in-flight session list.
            Err(error) => errors.push(error),
        }
        let selected_member_removed = match result.members {
            Ok(members) => {
                let removed = self.preferences.forget_missing_member(workspace, &members);
                project.members = members;
                removed
            }
            Err(error) => {
                errors.push(error);
                false
            }
        };
        if let Some(root) = &app.data_dir {
            let interactions = app.inbox_interactions(&project.sessions, &project.members);
            project.reminders.reconcile(
                root,
                workspace,
                snapshot.as_ref(),
                &interactions,
                &project.sessions,
            );
        }
        let refreshed_questions = snapshot.is_some();
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
        if refreshed_workflows && let Some(snapshot) = &project.workflow_runs {
            update_workflow_new_items(
                &mut self.new_items,
                snapshot,
                &project.reminders,
                now_ms(),
            );
        }
        parent.update(cx, |app, cx| {
            if refreshed_questions && let Some(snapshot) = &project.action_center {
                app.collaboration.inbox_badge.remember_questions(snapshot);
            }
            if refreshed_workflows && let Some(snapshot) = &project.workflow_runs {
                app.collaboration
                    .inbox_badge
                    .remember_workflow_runs(workspace, snapshot.runs.iter().map(|run| &run.id));
            }
            if let Some(saved) = &project.reminders.saved {
                app.collaboration
                    .inbox_badge
                    .remember_reminders(workspace, saved);
                app.sync_inbox_deadlines(cx);
            }
        });
        if selected_member_removed {
            self.clear_filter_position();
            self.sync_completed(cx);
        }
        cx.notify();
    }

    fn apply_local_expiry(
        &mut self,
        workspace: &str,
        outcome: Result<&super::Snoozes, &std::io::Error>,
        app: &HiveApp,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.projects.get_mut(workspace) else {
            return;
        };
        let now = now_ms();
        if let Some(saved) = &project.reminders.saved {
            for (key, reminder) in saved.expired_reminders(now) {
                if !outcome.as_ref().is_ok_and(|latest| latest.active(key, now)) {
                    self.placement
                        .hold_expired(RowId::from_key(workspace, key.to_owned()), reminder);
                }
            }
        }
        project.reminders.apply_expiry(outcome);
        let eligible = project
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
        if let Some(snapshot) = &project.workflow_runs {
            update_workflow_new_items(
                &mut self.new_items,
                snapshot,
                &project.reminders,
                now,
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
        let unread = match &entry.target {
            Destination::Session(session) => {
                app.unread.contains(&session.id) || self.reminders.tracks_unread(session)
            }
            _ => false,
        };
        let item = BadgeItem::from_entry(workspace, entry, unread)?;
        (!matches!(item, BadgeItem::Question { .. } | BadgeItem::WorkflowRun { .. })
            || self.reminders.saved.is_some())
        .then_some(item)
    }

    fn entries(&self, app: &HiveApp) -> Vec<Entry> {
        let interactions = app.inbox_interactions(&self.sessions, &self.members);
        let mut rows: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|session| {
                app.inbox_session_entry(
                    session,
                    &self.members,
                    &interactions,
                    true,
                    self.reminders.tracks_unread(session),
                )
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
        if let Some(snapshot) = &self.workflow_runs {
            rows.extend(snapshot.entries());
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

fn member_choices(
    workspaces: &[Workspace],
    selected_project: Option<&str>,
    removing: Option<&str>,
    projects: &HashMap<String, ProjectInbox>,
) -> Vec<(MemberFilter, String)> {
    let mut choices = Vec::new();
    for workspace in workspaces.iter().filter(|workspace| {
        removing != Some(&workspace.id)
            && selected_project.is_none_or(|selected| selected == workspace.id)
    }) {
        let Some(project) = projects.get(&workspace.id) else {
            continue;
        };
        let title = display_project(workspace, workspaces);
        let mut names = HashMap::new();
        for member in &project.members {
            *names.entry(&member.name).or_insert(0) += 1;
        }
        choices.extend(project.members.iter().map(|member| {
            let label = if names[&member.name] > 1 {
                format!("{} · {title} · {}", member.name, member.id)
            } else {
                format!("{} · {title}", member.name)
            };
            (
                MemberFilter {
                    workspace_id: workspace.id.clone(),
                    member_id: member.id.clone(),
                },
                label,
            )
        }));
    }
    choices.sort_by(|(a, label_a), (b, label_b)| {
        (label_a, &a.workspace_id, &a.member_id).cmp(&(label_b, &b.workspace_id, &b.member_id))
    });
    choices
}

fn matches_member(workspace: &str, entry: &Entry, selected: Option<&MemberFilter>) -> bool {
    selected.is_none_or(|member| {
        member.workspace_id == workspace
            && entry
                .related_members
                .iter()
                .flatten()
                .any(|id| *id == member.member_id)
    })
}

fn matches_workflow_filter(entry: &Entry, hide_workflow_runs: bool, reviewing_new: bool) -> bool {
    reviewing_new || !hide_workflow_runs || !matches!(entry.target, Destination::WorkflowRun { .. })
}

pub(super) fn section_title(
    id: &'static str,
    title: &str,
    count: usize,
    cx: &App,
) -> impl IntoElement {
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
    items.retain(|item| {
        matches!(item, BadgeItem::WorkflowRun { .. })
            || item.workspace() != workspace
            || eligible.contains(item)
    });
    items.extend(
        eligible
            .into_iter()
            .filter(|item| !matches!(item, BadgeItem::WorkflowRun { .. }) && !badge.sees(item)),
    );
}

fn update_workflow_new_items(
    items: &mut HashSet<BadgeItem>,
    snapshot: &WorkflowRuns,
    reminders: &Reminders,
    now: u64,
) {
    let eligible: HashSet<_> = snapshot
        .runs
        .iter()
        .filter(|run| {
            reminders
                .saved
                .as_ref()
                .is_some_and(|saved| !saved.active(&super::workflow_run_key(&run.id), now))
        })
        .map(|run| BadgeItem::WorkflowRun {
            workspace: snapshot.workspace_id.clone(),
            run: run.id.clone(),
        })
        .collect();
    items.retain(|item| {
        !matches!(item, BadgeItem::WorkflowRun { workspace, .. }
        if workspace == &snapshot.workspace_id)
            || eligible.contains(item)
    });
    if let Some(saved) = &reminders.saved {
        items.extend(eligible.into_iter().filter(|item| {
            matches!(item, BadgeItem::WorkflowRun { run, .. } if !saved.has_seen_workflow_run(run))
        }));
    }
}

impl Render for GlobalInbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex().gap_2().min_w_0();
        if self.intro.is_some() {
            return body.child(intro::guide(
                &self.intro_focus,
                cx.listener(|this, _, window, cx| {
                    this.finish_intro(window, cx);
                }),
                cx,
            ));
        }
        let Some(parent) = self.app.upgrade() else {
            return body;
        };
        let app = parent.read(cx);
        let connected = self.is_current(self.connection, cx);
        let ready = connected && app.global_inbox_workspace_list().is_some();
        let reviewing = self.reviewing_new.is_some();
        let removing = app.workspace_removing.as_deref();
        let selected = self
            .preferences
            .values
            .project_filter
            .as_deref()
            .filter(|_| !reviewing);
        let selected_member = self
            .preferences
            .values
            .member_filter
            .as_ref()
            .filter(|_| !reviewing);
        let selected_backend = self
            .preferences
            .values
            .backend_filter
            .filter(|_| !reviewing);
        let selected_activity = self
            .preferences
            .values
            .activity_filter
            .filter(|_| !reviewing);
        let hide_workflow_runs = self.preferences.values.hide_workflow_runs;
        let selected_title = selected_project(&app.workspaces, None, selected)
            .map(|project| display_project(project, &app.workspaces));
        let available: Vec<_> = app
            .workspaces
            .iter()
            .filter(|workspace| removing != Some(workspace.id.as_str()))
            .cloned()
            .collect();
        if ready {
            self.new_items.retain(|item| {
                app.workspaces
                    .iter()
                    .any(|project| project.id == item.workspace())
            });
        }
        let projects: Vec<_> = available
            .iter()
            .filter(|project| {
                selected.is_none_or(|selected| project.id == selected)
                    && selected_member.is_none_or(|member| project.id == member.workspace_id)
            })
            .collect();
        let now = now_ms();
        let activity_since = selected_activity.map(|range| range.cutoff(now));
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
                            matches_member(&workspace.id, entry, selected_member)
                                && entry.matches_backend(selected_backend)
                                && activity_matches(entry.activity_at, activity_since)
                                && matches_workflow_filter(entry, hide_workflow_runs, reviewing)
                                && (!reviewing
                                    || project
                                        .badge_item(app, &workspace.id, entry, now)
                                        .is_some_and(|item| self.new_items.contains(&item)))
                        })
                        .map(|entry| (workspace.id.as_str(), entry))
                })
                .collect()
        } else {
            Vec::new()
        };
        let fresh = InboxSections::from_rows(rows, |id, entry| {
            self.projects[id].reminders.snoozed_until(entry, now)
        });
        let section_counts = Section::ALL.map(|section| fresh.rows(section).len());
        let has_snoozed = !fresh.snoozed.is_empty();
        let loading = projects
            .iter()
            .any(|project| !self.projects.contains_key(&project.id));
        let status = if !connected {
            Some("后台连接后将显示跨项目收件箱。")
        } else if !ready || loading {
            Some("正在读取收件箱…")
        } else if fresh.attention.is_empty()
            && fresh.waiting.is_empty()
            && !(has_snoozed && self.preferences.values.show_snoozed)
            && projects.iter().all(|workspace| {
                self.projects.get(&workspace.id).is_none_or(|project| {
                    project.errors.is_empty() && !project.workflow_runs_error
                })
            })
        {
            Some(if reviewing {
                "暂无新事项需要关注。"
            } else if has_snoozed {
                "此范围内的协作事项均已暂缓。"
            } else if selected_member.is_some() {
                "此成员暂无需要关注的协作事项。"
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
                    .find_map(|(id, focus)| focus.group.contains_focused(window, cx).then_some(id))
            })
            .flatten();
        let show_waiting = self.preferences.values.show_waiting;
        let show_snoozed = self.preferences.values.show_snoozed;
        let sections = self
            .placement
            .arrange(fresh, focused, show_waiting, show_snoozed);
        if let Some(position) = self.restore_position.as_ref().filter(|_| ready && !loading)
            && let Some((section, index)) = position
                .0
                .iter()
                .find_map(|anchor| sections.position(&anchor.row, show_waiting, show_snoozed))
        {
            self.section_depth.include_row(section, index);
        }
        let depth = self.section_depth;
        let has_snoozed = !sections.snoozed.is_empty();
        let rendered: HashSet<_> = [
            (Section::Attention, true),
            (Section::Waiting, show_waiting),
            (Section::Snoozed, show_snoozed),
        ]
        .into_iter()
        .filter(|(_, shown)| *shown)
        .flat_map(|(section, _)| sections.rows(section).iter().take(depth.limit(section)))
        .map(|(workspace, entry)| RowId::for_entry(workspace, entry))
        .collect();
        self.row_focus.retain(|id, _| rendered.contains(id));
        for id in rendered {
            self.row_focus.entry(id).or_insert_with(|| {
                let focus = cx.focus_handle();
                let enter = cx.on_focus_in(&focus, window, |_, _, cx| cx.notify());
                let exit = cx.on_focus_out(&focus, window, |_, _, _, cx| cx.notify());
                GlobalInboxRowFocus {
                    group: focus,
                    open: cx.focus_handle(),
                    _subscriptions: [enter, exit],
                }
            });
        }
        let app = parent.read(cx);
        if reviewing {
            body = body.child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        Button::new("global-inbox-back-from-new")
                            .ghost()
                            .compact()
                            .small()
                            .icon(Glyph::ArrowLeft)
                            .label(tr("返回收件箱"))
                            .on_click(cx.listener(|this, _, _, cx| this.return_to_inbox(cx))),
                    )
                    .child(self.help_button(cx)),
            );
        } else {
            let mut controls = h_flex()
                .items_center()
                .justify_between()
                .flex_wrap()
                .gap_2();
            if ready {
                controls = controls.children(self.filter_controls(
                    app,
                    &available,
                    window.viewport_size().width < px(820.),
                    cx,
                ));
            }
            if has_snoozed {
                controls = controls.child(
                    toggle_button(self.preferences.values.show_snoozed).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.preferences.values.show_snoozed =
                                !this.preferences.values.show_snoozed;
                            this.preferences.save_changed();
                            this.restore_position = None;
                            this.placement.clear();
                            cx.notify();
                        },
                    )),
                );
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
            body = body.child(controls.child(self.help_button(cx)));
        }
        if self.intro_save_failed {
            let message = tr("无法保存收件箱引导状态");
            body = body.child(
                div()
                    .id("inbox-guide-persistence-error")
                    .role(Role::Alert)
                    .aria_label(message)
                    .text_sm()
                    .text_color(Color::Error.get(cx))
                    .child(message),
            );
        }
        if let Some(error) = self.preferences.error {
            let message = tr(error);
            body = body.child(
                div()
                    .id("inbox-view-preferences-error")
                    .role(Role::Alert)
                    .aria_label(message)
                    .text_sm()
                    .text_color(Color::Error.get(cx))
                    .child(message),
            );
        }
        body = body.when_some(status, |body, status| {
            body.child(inbox_status(tr(status), cx))
        });
        if self.member_open_error {
            body = body.child(inbox_status(tr(OPEN_INBOX_MEMBER_FAILED), cx));
        }

        // The scroll handle indexes direct children. Keep rows and headings in
        // the same sequence used to compute the saved reading position.
        let mut items: Vec<(Option<RowId>, AnyElement)> = Vec::new();
        if ready {
            for workspace in &projects {
                let Some(project) = self.projects.get(&workspace.id) else {
                    continue;
                };
                let title = display_project(workspace, &app.workspaces);
                let workflow_status = if project.workflow_runs_error {
                    Some(tr("无法读取需关注的工作流运行"))
                } else if (reviewing || !hide_workflow_runs)
                    && project.workflow_runs.as_ref().is_some_and(|runs| runs.limited)
                {
                    Some(tr("收件箱保留最近 20 条失败或中断的工作流运行，以及暂缓过的运行"))
                } else {
                    None
                };
                if let Some(status) = workflow_status {
                    items.push((None, inbox_status(format!("{title} · {status}"), cx)));
                }
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
        let reveal_more = self.reveal_more.take();
        let mut candidates = HashMap::new();
        let awaiting_position = self.restore_position.is_some() && (!ready || loading);
        let mut render_row = |(id, entry): (&str, Entry)| {
            let project = projects.iter().find(|project| project.id == id)?;
            let row_id = RowId::for_entry(id, &entry);
            let layout_id = row_id.clone();
            let title = display_project(project, &app.workspaces);
            let data = &self.projects[id];
            let reminders = &data.reminders;
            let reminder = reminders
                .active_reminder(&entry, now)
                .or_else(|| self.placement.held_reminder(&row_id));
            if !awaiting_position
                && let Some(item) = data.badge_item(app, id, &entry, now)
                && (!matches!(item, BadgeItem::WorkflowRun { .. }) || reminder.is_none())
                && !app.collaboration.inbox_badge.sees(&item)
            {
                let revision = match &item {
                    BadgeItem::UnreadSession { session, .. } => {
                        app.background_sequences.get(session).copied()
                    }
                    BadgeItem::Question { .. } | BadgeItem::WorkflowRun { .. } => None,
                };
                candidates.insert(row_id.clone(), (item, revision));
            }
            let mark_read = MarkInboxRead::for_entry(app, id, &entry, reminders).map(|mark| {
                let row = row_id.clone();
                Button::new("mark-inbox-session-read").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.mark_read(&mark, &row, connection, cx);
                    },
                ))
            });
            let archive = ArchiveInboxSession::for_entry(
                app,
                id,
                &entry,
                &data.members,
                reminders,
                reminder.is_some(),
            )
            .map(|archive| {
                let row = row_id.clone();
                Button::new("archive-inbox-session").on_click(cx.listener(
                    move |this, _, window, cx| {
                        this.request_archive(&archive, &row, connection, window, cx)
                    },
                ))
            });
            let target = entry.target.clone();
            let id = id.to_owned();
            let button_workspace = id.clone();
            let open_id = row_id.clone();
            let open = inbox_open_button(
                "open-inbox-item",
                !connected,
                cx.listener(move |this, _, window, cx| {
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
                    if let Destination::Member(member) = &target {
                        this.open_inbox_member(
                            button_workspace.clone(),
                            member.clone(),
                            open_id.clone(),
                            connection,
                            window,
                            cx,
                        );
                        return;
                    }
                    if matches!(target, Destination::WorkflowRun { .. }) {
                        this.dismiss_for_workflow(cx);
                    } else {
                        this.dismiss(cx);
                    }
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
                }),
            );
            let focus = self.row_focus.get(&row_id)?;
            let open = open.track_focus(&focus.open);
            let hovered_id = row_id.clone();
            let interaction = InboxRowInteraction {
                focus: focus.group.clone(),
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
                mark_read,
                archive,
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
                        || !edit.reply_is_current(state)
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
                    let refresh_scoped = state.workspace.as_deref() == Some(workspace)
                        && state.collaboration.visible;
                    if result.is_ok() {
                        app.update(cx, |app, cx| {
                            if let Some(saved) = &project.reminders.saved {
                                app.collaboration
                                    .inbox_badge
                                    .remember_reminders(workspace, saved);
                                if delay.is_some() {
                                    app.collaboration.inbox_badge.forget_snoozed(edit);
                                }
                                app.sync_inbox_deadlines(cx);
                            }
                            if refresh_scoped {
                                app.refresh_collaboration_inbox(cx);
                            }
                        });
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
        let more = |section,
                    rows: &[super::placement::Row<'_>],
                    title: &'static str,
                    cx: &Context<Self>| {
            let (workspace, entry) = rows.get(depth.limit(section))?;
            let first_new_row = RowId::for_entry(workspace, entry);
            let more = show_more_button(
                section,
                title,
                rows.len() - depth.limit(section),
                cx.listener(move |this, _, window, cx| {
                    this.section_depth.show_more(section);
                    if window.last_input_was_keyboard() {
                        this.reveal_more = Some(first_new_row.clone());
                    }
                    cx.notify();
                }),
            );
            Some((
                None,
                Reveal::new(section.control_id(), &self.scroll, more).into_any_element(),
            ))
        };
        if !sections.attention.is_empty() || section_counts[Section::Attention as usize] > 0 {
            let section = Section::Attention;
            let title = if reviewing {
                "新事项"
            } else {
                section.title()
            };
            items.push((
                None,
                section_title(
                    "global-inbox-attention",
                    tr(title),
                    section_counts[section as usize],
                    cx,
                )
                .into_any_element(),
            ));
            let more = more(section, &sections.attention, title, cx);
            items.extend(
                sections
                    .attention
                    .into_iter()
                    .take(depth.limit(section))
                    .filter_map(&mut render_row),
            );
            items.extend(more);
        }
        if !sections.waiting.is_empty() || section_counts[Section::Waiting as usize] > 0 {
            let section = Section::Waiting;
            let count = section_counts[section as usize];
            items.push((
                None,
                Reveal::new(
                    "global-inbox-waiting",
                    &self.scroll,
                    waiting_toggle_button(self.preferences.values.show_waiting, count)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.preferences.values.show_waiting =
                                !this.preferences.values.show_waiting;
                            this.preferences.save_changed();
                            this.restore_position = None;
                            this.placement.clear();
                            cx.notify();
                        })),
                )
                .into_any_element(),
            ));
            if self.preferences.values.show_waiting {
                let more = more(section, &sections.waiting, section.title(), cx);
                items.extend(
                    sections
                        .waiting
                        .into_iter()
                        .take(depth.limit(section))
                        .filter_map(&mut render_row),
                );
                items.extend(more);
            }
        }
        if self.preferences.values.show_snoozed && has_snoozed {
            let section = Section::Snoozed;
            items.push((
                None,
                section_title(
                    "global-inbox-snoozed",
                    tr("已暂缓事项"),
                    section_counts[section as usize],
                    cx,
                )
                .into_any_element(),
            ));
            let more = more(section, &sections.snoozed, section.title(), cx);
            items.extend(
                sections
                    .snoozed
                    .into_iter()
                    .take(depth.limit(section))
                    .filter_map(&mut render_row),
            );
            items.extend(more);
        }
        if !reviewing {
            items.extend(self.completed.elements(app, &self.scroll, cx));
        }
        let (ids, elements): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        let candidates: Vec<_> = ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| Some((index, candidates.remove(id.as_ref()?)?)))
            .collect();
        self.row_ids = ids;
        if let Some(target) = reveal_more
            && let Some(index) = self
                .row_ids
                .iter()
                .position(|row| row.as_ref() == Some(&target))
            && let Some(focus) = self.row_focus.get(&target).map(|focus| focus.open.clone())
        {
            self.scroll.scroll_to_item(index);
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        let restore = if ready && !loading {
            self.restore_position.take().and_then(|position| {
                let (index, viewport_offset) = position.first_visible(&self.row_ids)?;
                self.scroll.scroll_to_top_of_item(index);
                Some((index, viewport_offset))
            })
        } else {
            None
        };
        let restore_or_acknowledge = restore.is_some() || !candidates.is_empty();
        let root = app.data_dir.clone();
        let dialog = cx.weak_entity();
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
                        let root = root.clone();
                        let dialog = dialog.clone();
                        window.defer(cx, move |window, cx| {
                            if !window.is_window_active() {
                                return;
                            }
                            let _ = parent.update(cx, |app, cx| {
                                let badge = &app.collaboration.inbox_badge;
                                if app.connection_generation != connection
                                    || app.data_dir != root
                                    || !app.connected
                                    || app.closing
                                    || badge.dialog.as_ref() != Some(&dialog)
                                {
                                    return;
                                }
                                let current: Vec<_> = visible
                                    .into_iter()
                                    .filter(|(item, _)| {
                                        let workspace = item.workspace();
                                        app.workspace_removing.as_deref() != Some(workspace)
                                            && app.workspaces.iter().any(|p| p.id == workspace)
                                    })
                                    .filter_map(|(item, revision)| {
                                        if let BadgeItem::UnreadSession { workspace, session } =
                                            &item
                                            && ((!app.unread.contains(session)
                                                && !app
                                                    .collaboration
                                                    .inbox_badge
                                                    .tracks_unread(workspace, session))
                                                || app.background_sequences.get(session).copied()
                                                    != revision)
                                        {
                                            return None;
                                        }
                                        Some(item)
                                    })
                                    .collect();
                                if app.collaboration.inbox_badge.acknowledge(
                                    root.as_deref(),
                                    now_ms(),
                                    current,
                                ) {
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
            .vertical_scrollbar(&self.scroll)
            .children(elements)
            .on_scroll(cx.listener(|this, _, _, _| this.restore_position = None));
        body.child(list)
            .when_some(self.archive_undo.as_ref(), |body, offer| {
                let status = format!("{} · {}", tr("会话已归档"), offer.title);
                body.child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .id("inbox-archive-undo-status")
                                .role(Role::Status)
                                .aria_label(status.clone())
                                .text_xs()
                                .child(status),
                        )
                        .child(
                            Button::new("inbox-undo-session-archive")
                                .outline()
                                .compact()
                                .small()
                                .label(tr("撤销归档"))
                                .track_focus(&self.archive_undo_focus)
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.undo_archive(window, cx)
                                    }),
                                ),
                        ),
                )
            })
            .when_some(self.undo.render(cx, Self::undo_snooze), |body, undo| {
                body.child(undo)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActionCenter, BadgeItem, Destination, Entry, InboxBadge, InboxFilter, InboxSections,
        MemberFilter, ProjectInbox, ReadingAnchor, ReadingPosition, RowId, WorkflowRuns,
        display_project, matches_member, matches_workflow_filter, member_choices, row_is_visible,
        selected_project, update_new_items, update_workflow_new_items,
    };
    use crate::collaboration::inbox::Priority;
    use crate::collaboration::inbox::placement::SnoozedUntil;
    use crate::protocol::{Backend, Member, Session, Workspace};
    use gpui_kit::{Bounds, point, px, size};
    use serde_json::json;
    use std::collections::{HashMap, HashSet};

    fn workspace(id: &str, path: &str) -> Workspace {
        Workspace {
            id: id.into(),
            name: "app".into(),
            path: path.into(),
        }
    }

    #[test]
    fn new_items_update_workflow_and_question_projections_independently() {
        let badge = InboxBadge::default();
        let root = tempfile::tempdir().unwrap();
        let workspace = "12345678-1234-1234-1234-123456789000";
        let reminders = super::Reminders {
            saved: Some(crate::collaboration::inbox::Snoozes::load(root.path(), workspace).unwrap()),
            ..super::Reminders::default()
        };
        let workflow = |workspace: &str, run: &str| BadgeItem::WorkflowRun {
            workspace: workspace.into(),
            run: run.into(),
        };
        let question = BadgeItem::Question {
            workspace: workspace.into(),
            dispatch: "dispatch".into(),
            message: "question".into(),
        };
        let mut current = HashSet::from([
            workflow(workspace, "stale"),
            workflow("beacon", "same"),
            question.clone(),
        ]);
        let atlas: WorkflowRuns = serde_json::from_value(json!({
            "workspaceId":workspace,"runs":[{"id":"same","name":"Daily audit","finishedAt":111,"status":"interrupted"}],"limited":false
        })).unwrap();
        update_workflow_new_items(&mut current, &atlas, &reminders, 0);
        assert_eq!(
            current,
            HashSet::from([
                workflow(workspace, "same"),
                workflow("beacon", "same"),
                question
            ])
        );
        update_new_items(&mut current, workspace, HashSet::new(), &badge);
        assert_eq!(
            current,
            HashSet::from([workflow(workspace, "same"), workflow("beacon", "same")])
        );
        let empty: WorkflowRuns = serde_json::from_value(json!({
            "workspaceId":workspace,"runs":[],"limited":false
        }))
        .unwrap();
        update_workflow_new_items(&mut current, &empty, &reminders, 0);
        assert_eq!(current, HashSet::from([workflow("beacon", "same")]));
    }

    #[test]
    fn running_workflow_becomes_attention_and_new_without_changing_the_row_identity() {
        use super::{Reminders, RowPlacement};
        use crate::collaboration::inbox::Snoozes;

        let root = tempfile::tempdir().unwrap();
        let workspace = "12345678-1234-1234-1234-123456789000";
        let run = "12345678-1234-1234-1234-123456789001";
        let mut items = HashSet::new();
        let reminders = Reminders {
            saved: Some(Snoozes::load(root.path(), workspace).unwrap()),
            ..Reminders::default()
        };
        let sections = |snapshot: &WorkflowRuns| {
            InboxSections::from_rows(
                snapshot.entries().map(|entry| (workspace, entry)).collect(),
                |_, entry| reminders.snoozed_until(entry, 20),
            )
        };
        let running: WorkflowRuns = serde_json::from_value(json!({
            "workspaceId": workspace, "limited": false, "runs": [],
            "runningRuns": [{"id": run, "name": "Daily audit", "startedAt": 10}],
        }))
        .unwrap();
        assert!(running.verified(workspace));
        let waiting = sections(&running);
        assert!(waiting.attention.is_empty());
        assert!(waiting.snoozed.is_empty());
        let entry = &waiting.waiting[0].1;
        assert_eq!(entry.label, "工作流运行中");
        assert!(entry.snooze_key.is_none());
        assert!(entry.wake.is_none());
        assert!(BadgeItem::from_entry(workspace, entry, true).is_none());
        let id = RowId::for_entry(workspace, entry);
        update_workflow_new_items(&mut items, &running, &reminders, 20);
        assert!(items.is_empty());

        let mut placement = RowPlacement::default();
        placement.arrange(waiting, None, true, false);
        let failed: WorkflowRuns = serde_json::from_value(json!({
            "workspaceId": workspace, "limited": false, "runningRuns": [],
            "runs": [{"id": run, "name": "Daily audit", "finishedAt": 20, "status": "failed"}],
        }))
        .unwrap();
        let attention = placement.arrange(sections(&failed), Some(&id), true, false);
        assert!(attention.waiting.is_empty());
        let entry = &attention.attention[0].1;
        assert_eq!(id, RowId::for_entry(workspace, entry));
        assert!(entry.snooze_key.is_some());
        update_workflow_new_items(&mut items, &failed, &reminders, 20);
        assert_eq!(
            items,
            HashSet::from([BadgeItem::WorkflowRun {
                workspace: workspace.into(),
                run: run.into(),
            }])
        );
    }

    #[test]
    fn hiding_workflow_runs_filters_each_section_and_keeps_workflow_questions() {
        let workspace = "atlas";
        let dispatch = "70035fa8-80a3-4215-b86c-9d940ae01610";
        let running = "70035fa8-80a3-4215-b86c-9d940ae01611";
        let failed = "70035fa8-80a3-4215-b86c-9d940ae01612";
        let interrupted = "70035fa8-80a3-4215-b86c-9d940ae01613";
        let snoozed_key = super::super::workflow_run_key(interrupted);
        let action_center: ActionCenter = serde_json::from_value(json!({
            "workspace_id": workspace,
            "generated_at": 200,
            "attention": [
                { "kind": "question_waiting_answer", "dispatch_id": dispatch, "message_id": "question",
                  "message_sender_id": "atlas:__workflow__" },
                { "kind": "dispatch_waiting_report", "dispatch_id": dispatch },
                { "kind": "message_delivery_pending", "dispatch_id": dispatch, "message_id": "queued" }
            ],
            "recent_dispatch_messages": []
        }))
        .unwrap();
        let workflows: WorkflowRuns = serde_json::from_value(json!({
            "workspaceId": workspace, "limited": false,
            "runningRuns": [{"id": running, "name": "Audit", "startedAt": 10}],
            "runs": [
                {"id": failed, "name": "Audit", "finishedAt": 20, "status": "failed"},
                {"id": interrupted, "name": "Audit", "finishedAt": 20, "status": "interrupted"}
            ],
        }))
        .unwrap();
        let sections = |hide, review| {
            InboxSections::from_rows(
                action_center
                    .entries(str::to_owned)
                    .into_iter()
                    .chain(workflows.entries())
                    .filter(|entry| matches_workflow_filter(entry, hide, review))
                    .map(|entry| (workspace, entry))
                    .collect(),
                |_, entry| {
                    entry
                        .snooze_key
                        .as_deref()
                        .is_some_and(|key| key == snoozed_key || key == "message:queued")
                        .then_some(SnoozedUntil::Indefinitely)
                },
            )
        };
        let visible = sections(false, false);
        assert_eq!(
            (visible.attention.len(), visible.waiting.len(), visible.snoozed.len()),
            (2, 2, 2)
        );

        let hidden = sections(true, false);
        assert_eq!(
            (hidden.attention.len(), hidden.waiting.len(), hidden.snoozed.len()),
            (1, 1, 1)
        );
        let (project, question) = &hidden.attention[0];
        assert!(matches!(
            BadgeItem::from_entry(project, question, false),
            Some(BadgeItem::Question { message, .. }) if message == "question"
        ));

        let review = sections(true, true);
        assert!(review.attention.iter().any(|(project, entry)| {
            matches!(
                BadgeItem::from_entry(project, entry, false),
                Some(BadgeItem::WorkflowRun { run, .. }) if run == failed
            )
        }));
    }

    #[test]
    fn reopening_new_items_suppresses_only_the_seen_run_and_keeps_both_attention_rows() {
        use crate::collaboration::inbox::Snoozes;

        let root = tempfile::tempdir().unwrap();
        let workspace = "12345678-1234-1234-1234-123456789000";
        let old = "11111111-0000-4000-8000-000012345678".to_owned();
        let new = "22222222-0000-4000-8000-000012345678".to_owned();
        Snoozes::acknowledge_workflow_runs(root.path(), workspace, std::slice::from_ref(&old), 150)
            .unwrap();
        let snapshot: WorkflowRuns = serde_json::from_value(json!({
            "workspaceId":workspace,
            "runs":[
                {"id":old,"name":"Daily audit","finishedAt":100,"status":"failed"},
                {"id":new,"name":"Daily audit","finishedAt":200,"status":"interrupted"},
            ],
            "limited":false,
        }))
        .unwrap();
        let reminders = super::Reminders {
            saved: Some(Snoozes::load(root.path(), workspace).unwrap()),
            ..super::Reminders::default()
        };
        let mut items = HashSet::new();
        update_workflow_new_items(&mut items, &snapshot, &reminders, 250);
        assert_eq!(
            items,
            HashSet::from([BadgeItem::WorkflowRun {
                workspace: workspace.into(),
                run: new.clone(),
            }])
        );
        assert_eq!(
            snapshot
                .entries()
                .filter_map(|entry| match entry.target {
                    super::Destination::WorkflowRun { id, .. } => Some(id),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [old, new],
        );
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
        let menu = InboxFilter::new(
            Some("right".to_owned()),
            projects
                .iter()
                .map(|project| (project.id.clone(), display_project(project, &projects))),
            "All projects",
            "Unavailable",
        );
        assert_eq!(menu.title, "/projects/right/app");
        assert_eq!(
            menu.choices[1],
            (Some("left".to_owned()), "/projects/left/app".to_owned())
        );
    }

    #[test]
    fn inbox_project_filter_falls_back_when_the_selected_project_is_unavailable() {
        let projects = [workspace("left", "/projects/left/app")];
        assert!(selected_project(&projects, Some("left"), Some("left")).is_none());
        assert!(selected_project(&projects, None, Some("right")).is_none());
        assert!(selected_project(&projects, None, None).is_none());
    }

    #[test]
    fn same_named_members_show_project_context_and_match_exact_messages() {
        let workspaces = [
            workspace("left", "/projects/left/app"),
            workspace("right", "/projects/right/app"),
        ];
        let make_project = || ProjectInbox {
            members: vec![
                serde_json::from_value::<Member>(json!({
                    "id": "reviewer", "name": "Reviewer", "role": "reviewer",
                    "status": "stopped", "pending_task_count": 0,
                }))
                .unwrap(),
            ],
            ..ProjectInbox::default()
        };
        let projects = HashMap::from([
            ("left".into(), make_project()),
            ("right".into(), make_project()),
        ]);
        let choices = member_choices(&workspaces, None, None, &projects);
        assert_eq!(choices[0].1, "Reviewer · /projects/left/app");
        assert_eq!(choices[1].1, "Reviewer · /projects/right/app");
        assert_eq!(
            member_choices(&workspaces, Some("right"), None, &projects),
            vec![choices[1].clone()]
        );
        let snapshot: ActionCenter = serde_json::from_value(json!({
            "workspace_id": "right", "generated_at": 10, "recent_dispatch_messages": [],
            "attention": [{
                "kind": "question_waiting_answer", "worker_id": "orchestrator",
                "message_sender_id": "reviewer", "message_id": "older-than-preview",
                "dispatch_id": "123e4567-e89b-12d3-a456-426614174000"
            }]
        }))
        .unwrap();
        let row = &snapshot.entries(str::to_owned)[0];
        assert!(matches_member("right", row, Some(&choices[1].0)));
        assert!(!matches_member("right", row, Some(&choices[0].0)));
        assert!(!matches_member(
            "right",
            row,
            Some(&MemberFilter {
                workspace_id: "right".into(),
                member_id: "different-member".into(),
            })
        ));
        assert!(
            matches!(row.target, Destination::DispatchMessage { ref dispatch, ref message }
                if dispatch == "123e4567-e89b-12d3-a456-426614174000" && message == "older-than-preview")
        );
    }

    #[test]
    fn inbox_backend_uses_exact_session_or_dispatch_and_keeps_unattributed_rows_in_all() {
        let snapshot: ActionCenter = serde_json::from_value(json!({
            "workspace_id": "right", "generated_at": 10, "recent_dispatch_messages": [],
            "attention": [
                {"kind": "question_waiting_answer", "worker_id": "reviewer", "message_id": "with-origin",
                    "dispatch_id": "123e4567-e89b-12d3-a456-426614174000", "dispatch_backend": "codex",
                    "message_created_at": 1200},
                {"kind": "dispatch_waiting_report", "worker_id": "reviewer",
                    "dispatch_id": "123e4567-e89b-12d3-a456-426614174001", "submitted_at": 300},
                {"kind": "stopped_with_queue", "worker_id": "reviewer"}
            ]
        })).unwrap();
        let rows = snapshot.entries(str::to_owned);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter().map(|row| row.activity_at).collect::<Vec<_>>(),
            [1200, 300, 0]
        );
        let member = MemberFilter {
            workspace_id: "right".into(),
            member_id: "reviewer".into(),
        };
        assert!(rows.iter().all(|row| matches_member("right", row, Some(&member)) && row.matches_backend(None)));
        assert!(rows[0].matches_backend(Some(Backend::Codex)));
        assert!(
            rows[1..]
                .iter()
                .all(|row| !row.matches_backend(Some(Backend::Codex)))
        );
        assert!(
            rows.iter()
                .all(|row| !row.matches_backend(Some(Backend::Pi)))
        );

        let mut session: Session = serde_json::from_value(json!({
            "id": "codex", "workspaceId": "right", "title": "Same title", "updatedAt": 100, "backend": "codex",
            "inboxReply": {"revision": 1, "receivedAt": 700}
        })).unwrap();
        assert_eq!(super::session_inbox_activity(&session), 700);
        let mut entry = Entry {
            label: "未读会话",
            subject: session.title.clone(),
            target: Destination::Session(session.clone()),
            dispatch_backend: None,
            related_members: [Some("reviewer".into()), None],
            snooze_key: None,
            wake: None,
            priority: Priority::Unread,
            updated_at: 0,
            activity_at: 0,
        };
        assert!(entry.matches_backend(Some(Backend::Codex)));
        session.id = "pi".into();
        session.backend = Backend::Pi;
        entry.target = Destination::Session(session);
        assert!(!entry.matches_backend(Some(Backend::Codex)));
        assert!(entry.matches_backend(Some(Backend::Pi)));
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
        assert!(badge.acknowledge(None, 0, [item.clone()]));
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
            dispatch_backend: None,
            related_members: [None, None],
            snooze_key: Some("message:question".into()),
            wake: None,
            priority: Priority::NeedsUser,
            updated_at: 1,
            activity_at: 1,
        };
        let anchor = RowId::for_entry("beacon", &entry);
        let atlas = RowId::for_entry("atlas", &entry);
        let saved = ReadingPosition(vec![ReadingAnchor {
            row: anchor.clone(),
            viewport_offset: px(-8.),
        }]);
        assert_eq!(
            saved.first_visible(&[None, Some(anchor.clone())]),
            Some((1, px(-8.)))
        );
        assert_eq!(
            saved.first_visible(&[None, Some(atlas.clone()), None, Some(anchor)]),
            Some((3, px(-8.)))
        );
        assert_eq!(saved.first_visible(&[None, Some(atlas)]), None);
    }

    #[test]
    fn reading_position_uses_each_surviving_neighbor_at_its_original_offset() {
        let opened = RowId::from_key("beacon", "session:opened".into());
        let second = RowId::from_key("beacon", "session:second".into());
        let third = RowId::from_key("beacon", "session:third".into());
        let same_id_in_another_project = RowId::from_key("atlas", "session:opened".into());
        let position = ReadingPosition(vec![
            ReadingAnchor {
                row: opened.clone(),
                viewport_offset: px(-5.),
            },
            ReadingAnchor {
                row: second.clone(),
                viewport_offset: px(29.),
            },
            ReadingAnchor {
                row: third.clone(),
                viewport_offset: px(65.),
            },
        ]);
        let rows_after_opening = [
            Some(same_id_in_another_project),
            Some(third.clone()),
            None,
            Some(second.clone()),
        ];
        assert_eq!(
            position.first_visible(&rows_after_opening),
            Some((3, px(29.)))
        );
        let rows_after_both_removed = [None, Some(third)];
        assert_eq!(
            position.first_visible(&rows_after_both_removed),
            Some((1, px(65.)))
        );
        assert_eq!(
            position
                .without(Some(&opened))
                .unwrap()
                .first_visible(&[Some(opened), Some(second)]),
            Some((1, px(29.)))
        );
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
                        dispatch_backend: None,
                        related_members: [None, None],
                        snooze_key: None,
                        wake: None,
                        priority,
                        updated_at,
                        activity_at: updated_at,
                    },
                )
            }),
        );
        let sections = InboxSections::from_rows(rows, |project, entry| {
            (project == "left" && entry.snooze_key.as_deref() == Some("message:queued"))
                .then_some(SnoozedUntil::Indefinitely)
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
