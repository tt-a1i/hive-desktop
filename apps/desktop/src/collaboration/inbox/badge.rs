#[path = "deadline.rs"]
mod deadline;

use self::deadline::DeadlineWake;
use super::super::{
    ActionCenter, ActionablePage, Attention, Destination, Entry, Priority, SnoozeEdit, Wake,
    read_actionable_tail, read_dispatch_inputs, read_exact_workflow_runs,
    snoozes::{Snoozes, workflow_run_key},
};
use super::{GlobalInbox, now_ms};
use crate::{app::HiveApp, transport::Runtime};
use gpui_kit::*;
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    time::Duration,
};

const BATCH_SIZE: usize = 32;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum BadgeItem {
    Question {
        workspace: String,
        dispatch: String,
        message: String,
    },
    UnreadSession {
        workspace: String,
        session: String,
    },
    WorkflowRun {
        workspace: String,
        run: String,
    },
}

impl BadgeItem {
    pub(super) fn workspace(&self) -> &str {
        match self {
            Self::Question { workspace, .. }
            | Self::UnreadSession { workspace, .. }
            | Self::WorkflowRun { workspace, .. } => workspace,
        }
    }

    pub(super) fn from_entry(workspace: &str, entry: &Entry, unread: bool) -> Option<Self> {
        match &entry.target {
            Destination::DispatchMessage { dispatch, message }
                if entry.priority == Priority::NeedsUser =>
            {
                Some(Self::Question {
                    workspace: workspace.into(),
                    dispatch: dispatch.clone(),
                    message: message.clone(),
                })
            }
            Destination::Session(session) if entry.priority == Priority::Unread && unread => {
                Some(Self::UnreadSession {
                    workspace: workspace.into(),
                    session: session.id.clone(),
                })
            }
            Destination::WorkflowRun(run) if entry.priority == Priority::NeedsUser => {
                Some(Self::WorkflowRun {
                    workspace: workspace.into(),
                    run: run.clone(),
                })
            }
            _ => None,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowRunBadge {
    workspace_id: String,
    run_ids: Vec<String>,
}

fn checked_workflow_badges(
    scope: &[String],
    response: Vec<WorkflowRunBadge>,
) -> BTreeMap<String, Vec<String>> {
    let mut checked = BTreeMap::new();
    for snapshot in response {
        if !scope.contains(&snapshot.workspace_id) {
            continue;
        }
        let unique: HashSet<_> = snapshot.run_ids.iter().collect();
        let valid = unique.len() == snapshot.run_ids.len()
            && unique.len() <= 20
            && unique
                .into_iter()
                .all(|id| uuid::Uuid::try_parse(id).is_ok());
        checked
            .entry(snapshot.workspace_id)
            .and_modify(|duplicate| *duplicate = None)
            .or_insert_with(|| valid.then_some(snapshot.run_ids));
    }
    checked
        .into_iter()
        .filter_map(|(workspace, runs)| Some((workspace, runs?)))
        .collect()
}

async fn read_badge_workflows(
    runtime: &Runtime,
    scope: &[String],
) -> BTreeMap<String, Vec<String>> {
    let result = runtime
        .request(
            "inbox.workflowFailureBadge",
            json!({"workspaceIds": scope, "includeInterrupted": true}),
        )
        .await;
    let Some(response) = result
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
    else {
        return BTreeMap::new();
    };
    checked_workflow_badges(scope, response)
}

struct Question {
    workspace_id: String,
    dispatch_id: String,
    message_id: String,
}

impl Question {
    fn from_attention(workspace: &str, item: &Attention) -> Option<Self> {
        item.is_question().then_some(())?;
        Some(Self {
            workspace_id: workspace.to_owned(),
            dispatch_id: item.dispatch_id.clone()?,
            message_id: item.message_id.clone()?,
        })
    }
}

fn collect_questions(scope: &[String], questions: Vec<Question>) -> HashSet<BadgeItem> {
    let scope: HashSet<_> = scope.iter().map(String::as_str).collect();
    questions
        .into_iter()
        .filter_map(|question| {
            if !scope.contains(question.workspace_id.as_str())
                || uuid::Uuid::try_parse(&question.dispatch_id).is_err()
                || question.message_id.is_empty()
            {
                return None;
            }
            Some(BadgeItem::Question {
                workspace: question.workspace_id,
                dispatch: question.dispatch_id,
                message: question.message_id,
            })
        })
        .collect()
}

async fn read_badge_questions(
    runtime: &Runtime,
    scope: &[String],
) -> BTreeMap<String, HashSet<BadgeItem>> {
    let Ok(value) = runtime
        .request(
            "inbox.actionableQuestionPages",
            json!({"workspaceIds":scope}),
        )
        .await
    else {
        return BTreeMap::new();
    };
    let Ok(first_pages) = serde_json::from_value::<Vec<ActionablePage>>(value) else {
        return BTreeMap::new();
    };
    let mut exact = BTreeMap::new();
    for page in first_pages {
        if scope.contains(&page.workspace_id) {
            exact
                .entry(page.workspace_id.clone())
                .and_modify(|duplicate| *duplicate = None)
                .or_insert(Some(page));
        }
    }
    let mut complete = BTreeMap::new();
    for (workspace, page) in exact {
        let Some(mut page) = page.filter(|page| page.questions_only) else {
            continue;
        };
        if let Ok(tail) =
            read_actionable_tail(runtime, &workspace, true, page.cursor, &page.rows).await
        {
            page.rows.extend(tail);
            let questions = page
                .rows
                .iter()
                .filter_map(|item| Question::from_attention(&workspace, item))
                .collect();
            let known = collect_questions(std::slice::from_ref(&workspace), questions);
            complete.insert(workspace, known);
        }
    }
    complete
}

#[derive(Default)]
pub(in crate::collaboration) struct InboxBadge {
    connection: u64,
    pub(super) dialog: Option<WeakEntity<GlobalInbox>>,
    pending: HashSet<BadgeItem>,
    seen: HashSet<BadgeItem>,
    reminders: HashMap<String, Snoozes>,
    wake: DeadlineWake,
    live_replies: HashMap<String, HashMap<String, (u64, Option<u64>)>>,
}

impl InboxBadge {
    fn is_open(&self, cx: &App) -> bool {
        self.dialog
            .as_ref()
            .and_then(WeakEntity::upgrade)
            .is_some_and(|dialog| !dialog.read(cx).dismissed)
    }

    fn replace(&mut self, scope: &[String], fresh: Option<HashSet<BadgeItem>>) {
        if let Some(fresh) = fresh {
            self.pending.retain(|item| {
                !matches!(item, BadgeItem::Question { workspace, .. }
                if scope.contains(workspace))
            });
            self.pending.extend(fresh);
        }
    }

    pub(in crate::collaboration::inbox) fn remember_workflow_runs<'a>(
        &mut self,
        workspace: &str,
        runs: impl IntoIterator<Item = &'a String>,
    ) {
        self.pending.retain(|item| !matches!(item, BadgeItem::WorkflowRun { workspace: id, .. } if id == workspace));
        self.pending
            .extend(runs.into_iter().map(|id| BadgeItem::WorkflowRun {
                workspace: workspace.to_owned(),
                run: id.clone(),
            }));
    }

    pub(super) fn sees(&self, item: &BadgeItem) -> bool {
        match item {
            BadgeItem::WorkflowRun { workspace, run } => self
                .reminders
                .get(workspace)
                .is_some_and(|saved| saved.has_seen_workflow_run(run)),
            _ => self.seen.contains(item),
        }
    }

    pub(in crate::collaboration::inbox) fn forget_unread(
        &mut self,
        workspace: &str,
        session: &str,
    ) {
        let item = BadgeItem::UnreadSession {
            workspace: workspace.to_owned(),
            session: session.to_owned(),
        };
        self.seen.remove(&item);
    }

    pub(super) fn acknowledge(
        &mut self,
        root: Option<&Path>,
        now: u64,
        items: impl IntoIterator<Item = BadgeItem>,
    ) -> bool {
        let before = self.seen.len();
        let mut workflow_runs = BTreeMap::<String, Vec<String>>::new();
        for item in items {
            if self.sees(&item) {
                continue;
            }
            match item {
                BadgeItem::WorkflowRun { workspace, run } => {
                    workflow_runs.entry(workspace).or_default().push(run);
                }
                _ => {
                    self.seen.insert(item);
                }
            }
        }
        let mut changed = self.seen.len() != before;
        if let Some(root) = root {
            for (workspace, runs) in workflow_runs {
                match Snoozes::acknowledge_workflow_runs(root, &workspace, &runs, now) {
                    Ok(saved) => {
                        changed |= runs.iter().any(|id| saved.has_seen_workflow_run(id));
                        self.remember_reminders(&workspace, &saved);
                    }
                    Err(error) => eprintln!("Could not acknowledge workflow inbox items: {error}"),
                }
            }
        }
        changed
    }

    pub(in crate::collaboration::inbox) fn remember_reminders(
        &mut self,
        workspace: &str,
        saved: &Snoozes,
    ) {
        self.reminders.insert(workspace.to_owned(), saved.clone());
    }

    pub(in crate::collaboration::inbox) fn remember_questions(&mut self, snapshot: &ActionCenter) {
        let scope = [snapshot.workspace_id.clone()];
        let known = snapshot
            .attention
            .iter()
            .filter_map(|item| Question::from_attention(&snapshot.workspace_id, item))
            .collect();
        self.replace(&scope, Some(collect_questions(&scope, known)));
    }

    pub(in crate::collaboration::inbox) fn forget_snoozed(&mut self, edit: &SnoozeEdit) {
        if let Some(session) = edit.key.strip_prefix("unread-session:") {
            self.forget_unread(&edit.workspace, session);
        } else if let Some(message) = edit.key.strip_prefix("message:")
            && let Some(Wake::Dispatch { id, .. }) = &edit.wake
        {
            self.seen.remove(&BadgeItem::Question {
                workspace: edit.workspace.clone(),
                dispatch: id.clone(),
                message: message.to_owned(),
            });
        }
    }

    fn pending_visible(&self, item: &BadgeItem, now: u64) -> bool {
        let hidden_key = match item {
            BadgeItem::Question { message, .. } => format!("message:{message}"),
            BadgeItem::WorkflowRun { run, .. } => workflow_run_key(run),
            BadgeItem::UnreadSession { .. } => return false,
        };
        self.reminders
            .get(item.workspace())
            .is_some_and(|saved| !saved.active(&hidden_key, now))
    }

    pub(super) fn tracks_unread(&self, workspace: &str, session: &str) -> bool {
        self.reminders
            .get(workspace)
            .is_some_and(|saved| saved.tracks_unread(session))
    }

    pub(in crate::collaboration::inbox) fn note_reply(
        &mut self,
        workspace: &str,
        session: &str,
        revision: u64,
        received_at: Option<u64>,
    ) {
        let latest = self
            .live_replies
            .entry(workspace.to_owned())
            .or_default()
            .entry(session.to_owned())
            .or_default();
        if revision > latest.0 {
            *latest = (revision, received_at);
        }
    }

    pub(in crate::collaboration::inbox) fn reply_revision(
        &self,
        workspace: &str,
        session: &str,
    ) -> u64 {
        self.live_replies
            .get(workspace)
            .and_then(|sessions| sessions.get(session))
            .map(|reply| reply.0)
            .unwrap_or(0)
    }

    pub(in crate::collaboration::inbox) fn reply_time(
        &self,
        workspace: &str,
        session: &str,
    ) -> Option<u64> {
        self.live_replies.get(workspace)?.get(session)?.1
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BadgeItem, InboxBadge, Question, SnoozeEdit, Wake, WorkflowRunBadge,
        checked_workflow_badges, collect_questions, workflow_run_key,
    };
    use crate::collaboration::inbox::snoozes::{Reminder, Snoozes};
    use std::collections::HashSet;

    fn item(workspace: &str, message: &str) -> BadgeItem {
        BadgeItem::Question {
            workspace: workspace.into(),
            dispatch: "dispatch".into(),
            message: message.into(),
        }
    }

    #[test]
    fn snoozing_a_workflow_hides_only_its_badge_until_the_exact_deadline() {
        let root = tempfile::tempdir().unwrap();
        let workspace = "a3b2cb99-3959-4adc-a770-b70f9d2a4cb9";
        let foreign = "c7761168-b30c-4020-80bb-23c1c357e111";
        let original = "2b2cb2a9-3959-4adc-a770-b70f9d2a4cb9";
        let later = "3b2cb2a9-3959-4adc-a770-b70f9d2a4cb9";
        let workflow = |workspace: &str, run: &str| BadgeItem::WorkflowRun {
            workspace: workspace.into(),
            run: run.into(),
        };
        let original_item = workflow(workspace, original);
        let later_item = workflow(workspace, later);
        let foreign_item = workflow(foreign, original);
        let edit = SnoozeEdit {
            workspace: workspace.to_owned(),
            key: workflow_run_key(original),
            wake: Some(Wake::WorkflowRun),
            previous: None,
        };
        let mut badge = InboxBadge::default();
        assert!(badge.acknowledge(
            Some(root.path()),
            9,
            [original_item.clone(), foreign_item.clone()],
        ));
        assert!(badge.sees(&original_item));
        assert!(!badge.sees(&later_item));
        let mut reopened = InboxBadge::default();
        reopened.remember_reminders(workspace, &Snoozes::load(root.path(), workspace).unwrap());
        assert!(reopened.sees(&original_item));
        assert!(!reopened.sees(&later_item));
        assert!(!reopened.sees(&foreign_item));
        let other_root = tempfile::tempdir().unwrap();
        reopened.remember_reminders(workspace, &Snoozes::load(other_root.path(), workspace).unwrap());
        assert!(!reopened.sees(&original_item));

        let (saved, applied) = Snoozes::edit(
            root.path(),
            workspace,
            &edit.key,
            None,
            Some(Reminder {
                expires_at: Some(20),
                wake: edit.wake.clone(),
            }),
            10,
        )
        .unwrap();
        assert!(applied);
        badge.remember_reminders(workspace, &saved);
        badge.remember_reminders(foreign, &Snoozes::load(root.path(), foreign).unwrap());
        assert!(!badge.sees(&original_item));
        assert!(badge.sees(&foreign_item));
        assert!(!badge.pending_visible(&original_item, 19));
        assert!(badge.pending_visible(&later_item, 19));
        assert!(badge.pending_visible(&foreign_item, 19));
        assert!(!badge.acknowledge(Some(root.path()), 19, [original_item.clone()]));
        let after_deadline = Snoozes::expire(root.path(), workspace, 20).unwrap();
        let mut badge = InboxBadge::default();
        badge.remember_reminders(workspace, &after_deadline);
        assert!(badge.pending_visible(&original_item, 20));
        assert!(!badge.sees(&original_item));
        assert!(after_deadline.tracked_workflow_runs().any(|id| id == original));
        assert!(badge.acknowledge(Some(root.path()), 20, [original_item.clone()]));
        assert!(badge.sees(&original_item));
    }

    #[test]
    fn workflow_sweeps_keep_omitted_projects_and_do_not_replace_questions() {
        let run = "11111111-0000-4000-8000-000012345678".to_owned();
        let other = "22222222-0000-4000-8000-000012345678".to_owned();
        let scope = ["atlas".into(), "beacon".into()];
        let workflow = |workspace: &str, run: &str| BadgeItem::WorkflowRun {
            workspace: workspace.into(),
            run: run.into(),
        };
        let mut badge = InboxBadge::default();
        let question = item("atlas", "question");
        badge.replace(&scope, Some(HashSet::from([question.clone()])));
        badge.remember_workflow_runs("atlas", [&run]);
        badge.remember_workflow_runs("beacon", [&other]);
        let snapshot = |workspace: &str, run_ids| WorkflowRunBadge {
            workspace_id: workspace.into(),
            run_ids,
        };
        let checked = checked_workflow_badges(
            &scope,
            vec![
                snapshot("atlas", vec![]),
                snapshot("beacon", vec![other.clone(), other.clone()]),
                snapshot("outside", vec![run.clone()]),
            ],
        );
        assert_eq!(checked.len(), 1);
        for (workspace, runs) in checked {
            badge.remember_workflow_runs(&workspace, &runs);
        }
        assert_eq!(
            badge.pending,
            HashSet::from([question, workflow("beacon", &other)])
        );
        badge.replace(&scope, Some(HashSet::new()));
        assert_eq!(badge.pending, HashSet::from([workflow("beacon", &other)]));
        assert!(
            checked_workflow_badges(
                &scope,
                vec![snapshot("beacon", vec![other]), snapshot("beacon", vec![]),]
            )
            .is_empty()
        );
    }

    #[test]
    fn viewing_one_project_does_not_acknowledge_another_or_a_later_question() {
        let scope = ["atlas".into(), "beacon".into()];
        let old = item("beacon", "old");
        let other = item("atlas", "old");
        let new = item("beacon", "new");
        let mut badge = InboxBadge::default();
        badge.replace(&scope, Some(HashSet::from([old.clone(), other.clone()])));
        assert!(badge.acknowledge(None, 0, [old.clone()]));
        assert!(badge.sees(&old));
        assert!(!badge.sees(&other));
        badge.replace(
            &scope,
            Some(HashSet::from([old.clone(), other.clone(), new.clone()])),
        );
        assert!(badge.sees(&old));
        assert!(!badge.sees(&new));
        badge.replace(&["beacon".into()], None);
        assert_eq!(
            badge.pending,
            HashSet::from([old.clone(), other.clone(), new])
        );
        assert!(badge.sees(&old));
        badge.replace(&["beacon".into()], Some(HashSet::new()));
        assert_eq!(badge.pending, HashSet::from([other]));
        badge.replace(&["beacon".into()], Some(HashSet::from([old.clone()])));
        assert!(badge.sees(&old));
    }

    #[test]
    fn marking_a_session_read_resets_only_its_badge_for_the_next_unread_event() {
        let session = |workspace: &str| BadgeItem::UnreadSession {
            workspace: workspace.into(),
            session: "example".into(),
        };
        let beacon = session("beacon");
        let atlas = session("atlas");
        let question = item("beacon", "question");
        let mut badge = InboxBadge::default();
        assert!(badge.acknowledge(None, 0, [atlas.clone(), beacon.clone(), question.clone()]));
        badge.forget_unread("beacon", "example");
        assert!(!badge.sees(&beacon));
        assert!(badge.sees(&atlas));
        assert!(badge.sees(&question));
    }

    #[test]
    fn compact_questions_respect_active_snoozes_and_exact_workspaces() {
        let root = tempfile::tempdir().unwrap();
        let workspace = "70035fa8-80a3-4215-b86c-9d940ae01610";
        let dispatch = "70035fa8-80a3-4215-b86c-9d940ae01611";
        Snoozes::set(
            root.path(),
            workspace,
            "message:old",
            Some(Reminder {
                expires_at: Some(20),
                wake: None,
            }),
        )
        .unwrap();
        let make = |workspace: &str, message: &str| Question {
            workspace_id: workspace.into(),
            dispatch_id: dispatch.into(),
            message_id: message.into(),
        };
        let scope = [workspace.to_owned()];
        let pending = || {
            vec![
                make(workspace, "old"),
                make(workspace, "new"),
                make("outside", "new"),
            ]
        };
        let mut badge = InboxBadge::default();
        badge.remember_reminders(workspace, &Snoozes::load(root.path(), workspace).unwrap());
        let known = collect_questions(&scope, pending());
        let current: HashSet<_> = known
            .iter()
            .filter(|item| badge.pending_visible(item, 19))
            .cloned()
            .collect();
        assert_eq!(current.len(), 1);
        assert!(current.contains(&BadgeItem::Question {
            workspace: workspace.into(),
            dispatch: dispatch.into(),
            message: "new".into()
        }));
        assert_eq!(
            known
                .iter()
                .filter(|item| badge.pending_visible(item, 20))
                .count(),
            2
        );
    }
}

impl HiveApp {
    pub(super) fn has_new_global_inbox_items(&self, cx: &App) -> bool {
        let badge = &self.collaboration.inbox_badge;
        if !self.connected
            || self.restarting
            || self.closing
            || badge.is_open(cx)
            || badge.connection != self.connection_generation
        {
            return false;
        }
        self.unseen_global_inbox_items().next().is_some()
    }

    pub(super) fn unseen_global_inbox_items(&self) -> impl Iterator<Item = BadgeItem> + '_ {
        let badge = &self.collaboration.inbox_badge;
        let active_project = move |id: &str| {
            self.workspace_removing.as_deref() != Some(id)
                && self.workspaces.iter().any(|project| project.id == id)
        };
        badge
            .pending
            .iter()
            .filter(move |item| {
                badge.connection == self.connection_generation
                    && badge.pending_visible(item, now_ms())
            })
            .cloned()
            .chain(
                self.sessions
                    .iter()
                    .chain(self.project_sessions.values().flatten())
                    .filter_map(move |session| {
                        let saved = badge.reminders.get(&session.workspace_id);
                        let deferred = saved.is_some_and(|saved| saved.tracks_unread(&session.id));
                        if (!self.unread.contains(&session.id) && !deferred)
                            || !active_project(&session.workspace_id)
                        {
                            return None;
                        }
                        let interactions =
                            self.inbox_interactions(std::slice::from_ref(session), &self.members);
                        let entry = self.inbox_session_entry(
                            session,
                            &self.members,
                            &interactions,
                            false,
                            deferred,
                        )?;
                        if entry.plain_unread_session(&session.workspace_id).is_some()
                            && saved.is_some_and(|saved| {
                                saved.hides_unread(
                                    &session.id,
                                    session.inbox_reply_revision().max(
                                        badge.reply_revision(&session.workspace_id, &session.id),
                                    ),
                                    now_ms(),
                                )
                            })
                        {
                            return None;
                        }
                        BadgeItem::from_entry(&session.workspace_id, &entry, true)
                    }),
            )
            .filter(move |item| active_project(item.workspace()) && !badge.sees(item))
    }

    pub(crate) fn start_global_inbox_badge(cx: &mut Context<Self>) {
        cx.observe_self(Self::sync_inbox_deadlines).detach();
        cx.spawn(async move |this, cx| {
            let mut start = 0;
            loop {
                let Ok(plan) = this.update(cx, |app, cx| {
                    let before = app.has_new_global_inbox_items(cx);
                    let badge = &mut app.collaboration.inbox_badge;
                    if badge.connection != app.connection_generation {
                        badge.connection = app.connection_generation;
                        badge.pending.clear();
                        start = 0;
                    }
                    let active: Vec<_> = app
                        .workspaces
                        .iter()
                        .filter(|workspace| app.workspace_removing.as_ref() != Some(&workspace.id))
                        .map(|workspace| workspace.id.clone())
                        .collect();
                    badge
                        .pending
                        .retain(|item| active.iter().any(|id| id == item.workspace()));
                    badge.reminders.retain(|id, _| active.contains(id));
                    badge.live_replies.retain(|id, _| active.contains(id));
                    let reminders = &badge.reminders;
                    badge.seen.retain(|item| {
                        active.iter().any(|id| id == item.workspace())
                            && match item {
                                BadgeItem::UnreadSession { session, workspace } => {
                                    app.unread.contains(session)
                                        || reminders
                                            .get(workspace)
                                            .is_some_and(|saved| saved.tracks_unread(session))
                                }
                                _ => true,
                            }
                    });
                    if before != app.has_new_global_inbox_items(cx) {
                        cx.notify();
                    }
                    if !app.connected
                        || app.restarting
                        || app.closing
                        || app.data_dir.is_none()
                        || app.collaboration.inbox_badge.is_open(cx)
                        || active.is_empty()
                    {
                        return None;
                    }
                    let runtime = app.runtime.clone()?;
                    if start >= active.len() {
                        start = 0;
                    }
                    let scope: Vec<_> = active.into_iter().skip(start).take(BATCH_SIZE).collect();
                    start += scope.len();
                    Some((
                        runtime,
                        app.connection_generation,
                        scope,
                        app.data_dir.clone()?,
                    ))
                }) else {
                    break;
                };
                if let Some((runtime, connection, scope, root)) = plan {
                    let (mut fresh, mut workflows) = smol::future::zip(
                        read_badge_questions(&runtime, &scope),
                        read_badge_workflows(&runtime, &scope),
                    )
                    .await;
                    let mut dispatch_inputs = BTreeMap::new();
                    for workspace in &scope {
                        if let Ok(saved) = Snoozes::load(&root, workspace) {
                            let watched = saved.watched_dispatches(now_ms(), None);
                            let inputs = read_dispatch_inputs(&runtime, workspace, &watched).await;
                            if !inputs.is_empty() {
                                dispatch_inputs.insert(workspace.clone(), inputs);
                            }
                            if let Some(runs) = workflows.get_mut(workspace) {
                                let older_ids: Vec<_> = saved
                                    .tracked_workflow_runs()
                                    .filter(|id| !runs.contains(id))
                                    .collect();
                                match read_exact_workflow_runs(&runtime, workspace, &older_ids).await {
                                    Some(older) => runs.extend(older.into_iter().map(|run| run.id)),
                                    None => {
                                        workflows.remove(workspace);
                                    }
                                }
                            }
                        }
                    }
                    if this
                        .update(cx, |app, cx| {
                            if !app.connected
                                || app.restarting
                                || app.closing
                                || app.connection_generation != connection
                                || app.data_dir.as_deref() != Some(root.as_path())
                                || app.collaboration.inbox_badge.is_open(cx)
                            {
                                return;
                            }
                            let Some(root) = &app.data_dir else {
                                return;
                            };
                            let scope: Vec<_> = scope
                                .into_iter()
                                .filter(|id| {
                                    app.workspace_removing.as_ref() != Some(id)
                                        && app
                                            .workspaces
                                            .iter()
                                            .any(|workspace| workspace.id == *id)
                                })
                                .collect();
                            let before = app.has_new_global_inbox_items(cx);
                            for workspace in &scope {
                                app.collaboration.inbox_badge.replace(
                                    std::slice::from_ref(workspace),
                                    fresh.remove(workspace),
                                );
                                if let Some(runs) = workflows.remove(workspace) {
                                    app.collaboration
                                        .inbox_badge
                                        .remember_workflow_runs(workspace, &runs);
                                }
                                if let Ok(saved) = Snoozes::load(root, workspace) {
                                    let saved = if let Some(inputs) = dispatch_inputs.get(workspace)
                                    {
                                        saved
                                            .reconcile_dispatch_inputs(root, inputs)
                                            .unwrap_or_else(|(saved, _)| saved)
                                    } else {
                                        saved
                                    };
                                    app.collaboration
                                        .inbox_badge
                                        .remember_reminders(workspace, &saved);
                                }
                            }
                            app.sync_inbox_deadlines(cx);
                            if before != app.has_new_global_inbox_items(cx) {
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                smol::Timer::after(Duration::from_secs(5)).await;
            }
        })
        .detach();
    }
}
