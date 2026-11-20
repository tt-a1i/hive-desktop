mod global;
mod placement;
mod schedule;
mod snoozes;
mod undo;
pub(super) use global::{GlobalInboxView, InboxBadge};

use super::{DispatchMessage, SelectedDispatchMessage, read_dispatch};
use crate::{
    app::{ErrorNotice, HiveApp},
    focus_scroll::Reveal,
    language::{localize_owned_error, tr},
    palette::Color,
    protocol::{Backend, Member, MemberDesktopActivity, Session},
    transport::Runtime,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        dialog::Confirm,
        menu::{DropdownMenu, PopupMenuItem},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use placement::{InboxSections, Row, RowId, Section, SectionDepth, SnoozedUntil};
use serde::Deserialize;
use serde_json::json;
use snoozes::{Delay, Reminder, Snoozes, Wake, now_ms, unread_session_key, workflow_run_key};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashSet},
    io,
    path::Path,
    rc::Rc,
};
use undo::{UNDO_CHANGED, UndoChange, UndoSlot, VerifiedUndo, expire_later};

#[derive(Default)]
pub(super) struct Inbox {
    workspace: Option<String>,
    generation: u64,
    connection: u64,
    loading: bool,
    action_center: Option<ActionCenter>,
    error: Option<String>,
    workflow_runs: Option<WorkflowRuns>,
    workflow_runs_error: bool,
    reminders: Reminders,
    show_snoozed: bool,
    section_depth: SectionDepth,
    return_position: Option<ProjectInboxPosition>,
    member_reveal: Option<InboxMemberReveal>,
    member_open_error: bool,
    undo: UndoSlot,
}

const OPEN_INBOX_MEMBER_FAILED: &str = "无法打开团队成员";

struct InboxMemberReveal {
    workspace: String,
    member: String,
    connection: u64,
    focus: FocusHandle,
    pending: bool,
}

fn exact_inbox_member<'a>(members: &'a [Member], id: &str) -> Option<&'a Member> {
    let mut matches = members.iter().filter(|member| member.id == id);
    let member = matches.next()?;
    matches.next().is_none().then_some(member)
}

async fn verify_inbox_member(
    runtime: &Runtime,
    workspace: &str,
    member: &str,
) -> Option<Vec<Member>> {
    let value = runtime
        .request("team.members", json!({"workspaceId":workspace}))
        .await
        .ok()?;
    let members: Vec<Member> = serde_json::from_value(value).ok()?;
    exact_inbox_member(&members, member)?;
    Some(members)
}

#[derive(Clone, PartialEq)]
struct ProjectInboxPosition {
    row: RowId,
    viewport_offset: Option<Pixels>,
    previous_offset: Pixels,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowRun {
    id: String,
    name: Option<String>,
    finished_at: u64,
    status: WorkflowRunStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum WorkflowRunStatus {
    Failed,
    Interrupted,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowRuns {
    workspace_id: String,
    runs: Vec<WorkflowRun>,
    limited: bool,
}

const RECENT_WORKFLOW_LIMIT: usize = 20;
const EXACT_WORKFLOW_BATCH: usize = 64;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExactWorkflowRuns {
    workspace_id: String,
    runs: Vec<WorkflowRun>,
}

fn valid_workflow_runs(runs: &[WorkflowRun], valid_id: impl Fn(&str) -> bool) -> bool {
    let mut seen = HashSet::new();
    runs.iter().all(|run| {
        run.finished_at > 0
            && run.id.len() == 36
            && uuid::Uuid::try_parse(&run.id).is_ok()
            && seen.insert(&run.id)
            && valid_id(&run.id)
    })
}

impl WorkflowRuns {
    fn verified(&self, workspace: &str) -> bool {
        self.workspace_id == workspace
            && self.runs.len() <= RECENT_WORKFLOW_LIMIT
            && valid_workflow_runs(&self.runs, |_| true)
    }

    fn entries(&self) -> impl Iterator<Item = Entry> + '_ {
        let mut suffixes = BTreeMap::new();
        for run in &self.runs {
            *suffixes.entry(short_uuid(&run.id)).or_insert(0) += 1;
        }
        self.runs.iter().map(move |run| {
            let short = short_uuid(&run.id);
            run.entry(if suffixes[short] > 1 { &run.id } else { short })
        })
    }
}

impl WorkflowRun {
    fn entry(&self, id_label: &str) -> Entry {
        let subject = self.name.as_ref().map_or_else(
            || self.id.clone(),
            |name| format!("{name} · {id_label}"),
        );
        Entry {
            label: match self.status {
                WorkflowRunStatus::Failed => "工作流运行失败",
                WorkflowRunStatus::Interrupted => "工作流运行中断",
            },
            subject,
            target: Destination::WorkflowRun(self.id.clone()),
            dispatch_backend: None,
            related_members: [None, None],
            snooze_key: Some(workflow_run_key(&self.id)),
            wake: Some(Wake::WorkflowRun),
            priority: Priority::NeedsUser,
            updated_at: self.finished_at,
            activity_at: self.finished_at,
        }
    }
}

async fn read_exact_workflow_runs(
    runtime: &Runtime,
    workspace: &str,
    ids: &[&String],
) -> Option<Vec<WorkflowRun>> {
    let mut runs = Vec::new();
    for batch in ids.chunks(EXACT_WORKFLOW_BATCH) {
        let value = runtime
            .request(
                "inbox.workflowRunsById",
                json!({"workspaceId": workspace, "runIds": batch}),
            )
            .await
            .ok()?;
        let snapshot: ExactWorkflowRuns = serde_json::from_value(value).ok()?;
        if snapshot.workspace_id != workspace
            || !valid_workflow_runs(&snapshot.runs, |id| {
                batch.iter().any(|candidate| *candidate == id)
            })
        {
            return None;
        }
        runs.extend(snapshot.runs);
    }
    Some(runs)
}

async fn read_workflow_runs(
    runtime: &Runtime,
    workspace: &str,
    root: Option<&Path>,
) -> Option<WorkflowRuns> {
    let value = runtime
        .request(
            "inbox.workflowFailures",
            json!({"workspaceId": workspace, "includeInterrupted": true}),
        )
        .await
        .ok()?;
    let mut snapshot: WorkflowRuns = serde_json::from_value(value).ok()?;
    if !snapshot.verified(workspace) {
        return None;
    }
    if let Some(saved) = root.and_then(|root| Snoozes::load(root, workspace).ok()) {
        let recent: HashSet<_> = snapshot.runs.iter().map(|run| &run.id).collect();
        let older_ids: Vec<_> = saved
            .tracked_workflow_runs()
            .filter(|id| !recent.contains(id))
            .collect();
        let older = read_exact_workflow_runs(runtime, workspace, &older_ids).await?;
        snapshot.runs.extend(older);
    }
    Some(snapshot)
}

enum ProjectInboxItem<'a> {
    Heading(Section, usize),
    Row(Row<'a>),
    More(Section, usize),
}

impl Inbox {
    pub(super) fn invalidate(&mut self) {
        self.generation += 1;
        self.loading = false;
        self.member_open_error = false;
        self.undo.clear();
    }
}

#[derive(Default)]
struct Reminders {
    saved: Option<Snoozes>,
    read_error: Option<io::Error>,
    write_error: Option<io::Error>,
}

const SNOOZE_CHANGED: &str = "收件箱事项已变更，请关闭后重新选择。";

#[derive(Clone)]
struct SnoozeEdit {
    workspace: String,
    key: String,
    wake: Option<Wake>,
    previous: Option<Reminder>,
}

impl SnoozeEdit {
    fn matches(&self, entry: &Entry) -> bool {
        entry.snooze_key.as_ref() == Some(&self.key) && same_wake_target(&entry.wake, &self.wake)
    }

    fn reply_is_current(&self, app: &HiveApp) -> bool {
        match (&self.wake, self.key.strip_prefix("unread-session:")) {
            (Some(Wake::UnreadReply { revision }), Some(session)) => {
                app.collaboration
                    .inbox_badge
                    .reply_revision(&self.workspace, session)
                    <= *revision
            }
            _ => true,
        }
    }
}

impl Reminders {
    fn apply_expiry(&mut self, outcome: Result<&Snoozes, &io::Error>) {
        match outcome {
            Ok(saved) => {
                self.saved = Some(saved.clone());
                self.read_error = None;
                self.write_error = None;
            }
            Err(error) => {
                self.write_error = Some(io::Error::new(error.kind(), error.to_string()));
            }
        }
    }

    fn reconcile(
        &mut self,
        root: &std::path::Path,
        workspace: &str,
        snapshot: Option<&ActionCenter>,
        interactions: &BTreeMap<String, BTreeSet<String>>,
        sessions: &[Session],
    ) {
        match Snoozes::load(root, workspace) {
            Ok(saved) => {
                self.read_error = None;
                let replies = sessions
                    .iter()
                    .filter(|session| session.workspace_id == workspace && !session.archived)
                    .map(|session| (session.id.clone(), session.inbox_reply_revision()))
                    .collect();
                match saved.reconcile(root, now_ms(), snapshot, interactions, &replies) {
                    Ok(saved) => {
                        self.saved = Some(saved);
                        self.write_error = None;
                    }
                    Err((saved, error)) => {
                        self.saved = Some(saved);
                        self.write_error = Some(error);
                    }
                }
            }
            Err(error) => {
                self.read_error = Some(error);
                self.saved = None;
            }
        }
    }

    fn set(
        &mut self,
        root: &std::path::Path,
        edit: &SnoozeEdit,
        delay: Option<Delay>,
    ) -> Result<Option<UndoChange>, &'static str> {
        let now = now_ms();
        let next = delay
            .map(|delay| delay.reminder(now, &edit.wake))
            .transpose()?;
        self.write(root, edit, edit.previous.as_ref(), next.clone(), now)?;
        Ok((edit.previous != next).then(|| UndoChange {
            edit: edit.clone(),
            after: next,
        }))
    }

    fn restore(&mut self, root: &std::path::Path, change: &UndoChange) -> Result<(), &'static str> {
        let now = now_ms();
        if !change.valid_at(now) {
            return Err("原暂缓时间已过，无法撤销该变更。");
        }
        self.write(
            root,
            &change.edit,
            change.after.as_ref(),
            change.edit.previous.clone(),
            now,
        )
        .map_err(|error| {
            if error == SNOOZE_CHANGED {
                UNDO_CHANGED
            } else {
                error
            }
        })
    }

    fn write(
        &mut self,
        root: &std::path::Path,
        edit: &SnoozeEdit,
        expected: Option<&Reminder>,
        next: Option<Reminder>,
        now: u64,
    ) -> Result<(), &'static str> {
        if self.saved.is_none() {
            return Err("无法读取已暂缓事项");
        }
        match Snoozes::edit(root, &edit.workspace, &edit.key, expected, next, now) {
            Ok((saved, applied)) => {
                self.saved = Some(saved);
                self.write_error = None;
                if applied { Ok(()) } else { Err(SNOOZE_CHANGED) }
            }
            Err(error) => {
                self.write_error = Some(error);
                Err("无法保存已暂缓事项")
            }
        }
    }

    fn active_reminder(&self, entry: &Entry, now: u64) -> Option<Reminder> {
        let reminder = self
            .saved
            .as_ref()?
            .active_reminder(entry.snooze_key.as_deref()?, now)?;
        if let (Some(current), Some(Wake::UnreadReply { revision })) =
            (entry.unread_reply_revision(), &reminder.wake)
            && current > *revision
        {
            return None;
        }
        Some(reminder)
    }

    fn tracks_unread(&self, session: &Session) -> bool {
        self.saved
            .as_ref()
            .is_some_and(|saved| saved.tracks_unread(&session.id))
    }

    fn snoozed_until(&self, entry: &Entry, now: u64) -> Option<SnoozedUntil> {
        let reminder = self.active_reminder(entry, now)?;
        Some(
            reminder
                .expires_at
                .map_or(SnoozedUntil::Indefinitely, SnoozedUntil::At),
        )
    }

    fn notice(&self) -> Option<(&'static str, &io::Error)> {
        self.read_error
            .as_ref()
            .map(|error| ("无法读取已暂缓事项", error))
            .or_else(|| {
                self.write_error
                    .as_ref()
                    .map(|error| ("无法保存已暂缓事项", error))
            })
    }
}

#[derive(Deserialize)]
struct ActionCenter {
    workspace_id: String,
    generated_at: u64,
    attention: Vec<Attention>,
    recent_dispatch_messages: Vec<RecentMessage>,
    inbox_actionable_cursor: Option<ActionableCursor>,
    #[serde(skip)]
    dispatch_inputs: BTreeMap<String, DispatchInput>,
}

const ACTIONABLE_PAGE_SIZE: usize = 100;
const INVALID_ACTIONABLE_PAGE: &str = "收件箱返回了无效的待办数据。";

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActionableCursor {
    through: u64,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    next_after: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActionablePage {
    workspace_id: String,
    questions_only: bool,
    #[serde(flatten)]
    cursor: ActionableCursor,
    rows: Vec<Attention>,
}

fn validate_actionable_rows<'a>(
    rows: impl IntoIterator<Item = &'a Attention>,
    questions_only: bool,
    cursor: ActionableCursor,
    seen: &mut HashSet<String>,
) -> Result<(), String> {
    let mut count = 0;
    for row in rows {
        count += 1;
        if count > ACTIONABLE_PAGE_SIZE
            || (!row.is_question() && (questions_only || !row.is_actionable_message()))
            || row
                .dispatch_id
                .as_ref()
                .is_none_or(|id| uuid::Uuid::try_parse(id).is_err())
            || row
                .message_id
                .as_ref()
                .is_none_or(|id| id.is_empty() || !seen.insert(id.clone()))
        {
            return Err(tr(INVALID_ACTIONABLE_PAGE).into());
        }
    }
    if cursor.next_after.is_some() && count != ACTIONABLE_PAGE_SIZE {
        return Err(tr(INVALID_ACTIONABLE_PAGE).into());
    }
    Ok(())
}

async fn read_actionable_tail<'a>(
    runtime: &Runtime,
    workspace: &str,
    questions_only: bool,
    mut cursor: ActionableCursor,
    first: impl IntoIterator<Item = &'a Attention>,
) -> Result<Vec<Attention>, String> {
    let mut seen = HashSet::new();
    validate_actionable_rows(first, questions_only, cursor, &mut seen)?;
    let mut previous = 0;
    let mut rows = Vec::new();
    while let Some(after) = cursor.next_after {
        if after <= previous || after >= cursor.through {
            return Err(tr(INVALID_ACTIONABLE_PAGE).into());
        }
        let value = runtime.request("inbox.actionablePage", json!({
            "workspaceId":workspace, "after":after, "through":cursor.through, "questionsOnly":questions_only,
        })).await?;
        let page: ActionablePage =
            serde_json::from_value(value).map_err(|error| error.to_string())?;
        if page.workspace_id != workspace
            || page.questions_only != questions_only
            || page.cursor.through != cursor.through
        {
            return Err(tr(INVALID_ACTIONABLE_PAGE).into());
        }
        validate_actionable_rows(&page.rows, questions_only, page.cursor, &mut seen)?;
        previous = after;
        cursor = page.cursor;
        rows.extend(page.rows);
    }
    Ok(rows)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DispatchInput {
    dispatch_id: String,
    sequence: Option<u64>,
    created_at: Option<u64>,
}

impl DispatchInput {
    fn is_newer(&self, sequence: Option<u64>, observed_at: u64) -> bool {
        match sequence {
            Some(sequence) => self.sequence.is_some_and(|latest| latest > sequence),
            None => self.created_at.is_some_and(|latest| latest > observed_at),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DispatchInputs {
    workspace_id: String,
    inputs: Vec<DispatchInput>,
}

impl ActionCenter {
    fn latest_input(&self, dispatch: &str) -> Option<u64> {
        if let Some(input) = self.dispatch_inputs.get(dispatch) {
            return input.sequence;
        }
        self.recent_dispatch_messages
            .iter()
            .filter(|message| {
                message.dispatch_id == dispatch && message.delivery_state != "cancelled"
            })
            .map(|message| message.sequence)
            .max()
    }

    fn has_new_input(&self, dispatch: &str, sequence: Option<u64>, observed_at: u64) -> bool {
        if let Some(input) = self.dispatch_inputs.get(dispatch) {
            input.is_newer(sequence, observed_at)
        } else if let Some(sequence) = sequence {
            self.latest_input(dispatch)
                .is_some_and(|latest| latest > sequence)
        } else {
            // The message preview is bounded across the workspace. A dispatch
            // may have no recent message when the reminder is created.
            self.recent_dispatch_messages.iter().any(|message| {
                message.dispatch_id == dispatch
                    && message.delivery_state != "cancelled"
                    && message.created_at > observed_at
            })
        }
    }

    fn dispatch_wake(&self, dispatch: &str) -> Wake {
        let sequence = self.latest_input(dispatch);
        Wake::Dispatch {
            id: dispatch.to_owned(),
            sequence,
            observed_at: if sequence.is_none() {
                self.generated_at
            } else {
                0
            },
        }
    }

    fn legacy_wake(&self, key: &str) -> Option<Wake> {
        if let Some(dispatch) = key.strip_prefix("report:") {
            return uuid::Uuid::try_parse(dispatch)
                .ok()
                .map(|_| self.dispatch_wake(dispatch));
        }
        let id = key.strip_prefix("message:")?;
        let dispatch = self
            .recent_dispatch_messages
            .iter()
            .find(|message| message.id == id)
            .map(|message| &message.dispatch_id)
            .or_else(|| {
                self.attention.iter().find_map(|row| {
                    (row.message_id.as_deref() == Some(id))
                        .then_some(row.dispatch_id.as_ref())
                        .flatten()
                })
            })?;
        Some(self.dispatch_wake(dispatch))
    }

    fn entries(&self, mut name: impl FnMut(&str) -> String) -> Vec<Entry> {
        let mut rows: Vec<_> = self
            .attention
            .iter()
            .filter_map(|attention| Some((attention, attention.entry(self, &mut name)?)))
            .collect();
        let mut titles = BTreeMap::new();
        let mut references = BTreeMap::new();
        for (attention, _) in &rows {
            if let Some(id) = attention.waiting_report_id() {
                *titles.entry(attention.task_title()).or_insert(0) += 1;
                *references.entry(short_uuid(id)).or_insert(0) += 1;
            }
        }
        for (attention, entry) in &mut rows {
            if let Some(id) = attention.waiting_report_id()
                && (attention.task_title().is_none() || titles[&attention.task_title()] > 1)
            {
                let short = short_uuid(id);
                let reference = if references[short] == 1 { short } else { id };
                // Keep the exact reference visible when a narrow row clips the title.
                entry.subject = format!("{reference} · {}", entry.subject);
            }
        }
        rows.into_iter().map(|(_, entry)| entry).collect()
    }
}

async fn read_dispatch_inputs(
    runtime: &Runtime,
    workspace: &str,
    dispatches: &BTreeMap<String, bool>,
) -> BTreeMap<String, DispatchInput> {
    let ids: Vec<_> = dispatches.keys().collect();
    let mut inputs = BTreeMap::new();
    for chunk in ids.chunks(64) {
        let timestamps: Vec<_> = chunk.iter().copied().filter(|id| dispatches[*id]).collect();
        let Ok(value) = runtime
            .request(
                "inbox.dispatchInput",
                json!({"workspaceId":workspace,"dispatchIds":chunk,"timestampDispatchIds":timestamps}),
            )
            .await
        else {
            continue;
        };
        let Ok(response) = serde_json::from_value::<DispatchInputs>(value) else {
            continue;
        };
        if response.workspace_id != workspace {
            continue;
        }
        let mut verified = BTreeMap::new();
        for input in response.inputs {
            let id = input.dispatch_id.clone();
            if chunk.binary_search(&&id).is_err() || verified.insert(id, input).is_some() {
                verified.clear();
                break;
            }
        }
        if verified.len() == chunk.len() {
            inputs.append(&mut verified);
        }
    }
    inputs
}

async fn read_action_center(
    runtime: &Runtime,
    workspace: &str,
    root: Option<&Path>,
) -> Result<ActionCenter, String> {
    let encoded: String = url::form_urlencoded::byte_serialize(workspace.as_bytes()).collect();
    let value = runtime
        .request(
            "hive.request",
            json!({"method":"GET","path":format!("/api/ui/workspaces/{encoded}/action-center")}),
        )
        .await?;
    let mut snapshot: ActionCenter =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    if snapshot.workspace_id != workspace
        || snapshot
            .recent_dispatch_messages
            .iter()
            .any(|message| message.workspace_id != workspace)
    {
        return Err(tr("收件箱返回了其他工作区的数据。").into());
    }
    if let Some(cursor) = snapshot.inbox_actionable_cursor.take() {
        let tail = read_actionable_tail(
            runtime,
            workspace,
            false,
            cursor,
            snapshot
                .attention
                .iter()
                .filter(|row| row.is_actionable_message()),
        )
        .await?;
        snapshot.attention.extend(tail);
    }
    if let Some(root) = root
        && let Ok(snoozes) = Snoozes::load(root, workspace)
    {
        let ids = snoozes.watched_dispatches(now_ms(), Some(&snapshot));
        snapshot.dispatch_inputs = read_dispatch_inputs(runtime, workspace, &ids).await;
    }
    Ok(snapshot)
}

fn exact_message(
    value: serde_json::Value,
    workspace: &str,
    dispatch: &str,
    id: &str,
) -> Option<DispatchMessage> {
    if value.get("workspace_id")?.as_str()? != workspace {
        return None;
    }
    let message = serde_json::from_value::<DispatchMessage>(value).ok()?;
    (message.id == id && message.dispatch_id == dispatch && message.delivery_state != "cancelled")
        .then_some(message)
}

#[derive(Deserialize)]
struct RecentMessage {
    id: String,
    workspace_id: String,
    dispatch_id: String,
    sequence: u64,
    from_agent_id: String,
    text: String,
    created_at: u64,
    delivery_state: String,
}

#[derive(Deserialize)]
struct Attention {
    kind: String,
    dispatch_id: Option<String>,
    message_id: Option<String>,
    worker_id: Option<String>,
    worker_name: Option<String>,
    message_sender_id: Option<String>,
    message_created_at: Option<u64>,
    message_preview: Option<String>,
    dispatch_backend: Option<Backend>,
    submitted_at: Option<u64>,
    task_title: Option<String>,
}

fn short_uuid(id: &str) -> &str {
    // Callers have already verified that the ID is a UUID.
    &id[id.len() - 8..]
}

#[derive(Clone)]
enum Destination {
    Session(Session),
    Dispatch(String),
    DispatchMessage { dispatch: String, message: String },
    Member(String),
    WorkflowRun(String),
}

impl Destination {
    fn action(&self) -> &'static str {
        match self {
            Self::Session(_) => "打开会话",
            Self::Dispatch(_) | Self::DispatchMessage { .. } => "派单详情",
            Self::Member(_) => "团队成员",
            Self::WorkflowRun(_) => "工作流",
        }
    }

    fn key(&self) -> String {
        match self {
            Self::Session(session) => format!("session:{}", session.id),
            Self::Dispatch(id) => format!("dispatch:{id}"),
            Self::DispatchMessage { message, .. } => format!("message:{message}"),
            Self::Member(id) => format!("member:{id}"),
            Self::WorkflowRun(id) => workflow_run_key(id),
        }
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Priority {
    NeedsUser,
    Unread,
    Active,
}

struct Entry {
    label: &'static str,
    subject: String,
    target: Destination,
    dispatch_backend: Option<Backend>,
    related_members: [Option<String>; 2],
    snooze_key: Option<String>,
    wake: Option<Wake>,
    priority: Priority,
    updated_at: u64,
    activity_at: u64,
}

fn session_inbox_activity(session: &Session) -> u64 {
    (session.updated_at.max(0) as u64).max(
        session
            .inbox_reply
            .as_ref()
            .and_then(|reply| reply.received_at)
            .unwrap_or(0),
    )
}

fn activity_matches(activity: u64, cutoff: Option<u64>) -> bool {
    cutoff.is_none_or(|since| activity > 0 && activity >= since)
}

const UNREAD_CONVERSATION: &str = "未读会话";

#[derive(Clone)]
struct ArchiveInboxSession {
    session: Session,
    revision: u64,
}

impl ArchiveInboxSession {
    fn for_entry(
        app: &HiveApp,
        workspace: &str,
        entry: &Entry,
        members: &[Member],
        reminders: &Reminders,
        snoozed: bool,
    ) -> Option<Self> {
        let session = entry.plain_unread_session(workspace)?;
        let activity = session_activity(session, members);
        let archivable = app.connected
            && !app.closing
            && !snoozed
            && reminders.saved.is_some()
            && !session.archived
            && !app.removed_sessions.contains(&session.id)
            && !app.busy.contains(&session.id)
            && app.pending.get(&session.id).copied().unwrap_or(0) == 0
            && !app.session_pin_loading.contains(&session.id)
            && !app.session_archive_loading.contains(&session.id)
            && app.workspace_removing.as_deref() != Some(workspace)
            && app.workspaces.iter().any(|project| project.id == workspace)
            && activity.is_none_or(Self::settled_activity);
        if !archivable {
            return None;
        }
        Some(Self {
            session: session.clone(),
            revision: entry.unread_reply_revision()?,
        })
    }

    fn settled_activity(activity: &MemberDesktopActivity) -> bool {
        matches!(activity.runtime_state.as_str(), "" | "idle" | "stopped")
            && activity.queued_input_count == 0
            && activity.pending_report_count == 0
            && activity.pending_interaction_count == 0
            && activity.delivery_failure_message.is_none()
    }

    fn matches(&self, current: &Self) -> bool {
        current.session.id == self.session.id
            && current.session.workspace_id == self.session.workspace_id
            && current.revision == self.revision
    }
}

impl Entry {
    fn matches_backend(&self, selected: Option<Backend>) -> bool {
        selected.is_none_or(|backend| match &self.target {
            Destination::Session(session) => session.backend == backend,
            Destination::Dispatch(_) | Destination::DispatchMessage { .. } => {
                self.dispatch_backend == Some(backend)
            }
            Destination::Member(_) | Destination::WorkflowRun(_) => false,
        })
    }

    fn unread_reply_revision(&self) -> Option<u64> {
        match self.wake {
            Some(Wake::UnreadReply { revision }) => Some(revision),
            _ => None,
        }
    }

    fn plain_unread_session(&self, workspace: &str) -> Option<&Session> {
        match &self.target {
            Destination::Session(session)
                if self.label == UNREAD_CONVERSATION && session.workspace_id == workspace =>
            {
                Some(session)
            }
            _ => None,
        }
    }
}

struct MarkInboxRead {
    workspace: String,
    session: String,
    sequence: Option<u64>,
    reply_revision: u64,
    deferred: bool,
}

impl MarkInboxRead {
    fn for_entry(
        app: &HiveApp,
        workspace: &str,
        entry: &Entry,
        reminders: &Reminders,
    ) -> Option<Self> {
        let session = entry.plain_unread_session(workspace)?;
        Some(Self {
            workspace: workspace.to_owned(),
            session: session.id.clone(),
            sequence: app.background_sequences.get(&session.id).copied(),
            reply_revision: entry.unread_reply_revision()?,
            deferred: reminders.tracks_unread(session),
        })
    }

    fn matches(&self, entry: &Entry) -> bool {
        entry
            .plain_unread_session(&self.workspace)
            .is_some_and(|session| {
                session.id == self.session
                    && entry.unread_reply_revision() == Some(self.reply_revision)
            })
    }

    fn clear(&self, app: &mut HiveApp, cx: &mut Context<HiveApp>) -> Result<Option<Snoozes>, ()> {
        let sequence = app.background_sequences.get(&self.session).copied();
        if self.sequence != sequence
            || app
                .collaboration
                .inbox_badge
                .reply_revision(&self.workspace, &self.session)
                > self.reply_revision
        {
            return Err(());
        }
        if !self.deferred && !app.unread.contains(&self.session) {
            return Err(());
        }
        let saved = if self.deferred {
            app.clear_inbox_unread_reminder(&self.workspace, &self.session, cx)?
        } else {
            None
        };
        self.consume(&mut app.unread, sequence);
        app.collaboration
            .inbox_badge
            .forget_unread(&self.workspace, &self.session);
        cx.notify();
        Ok(saved)
    }

    fn consume(&self, unread: &mut HashSet<String>, sequence: Option<u64>) -> bool {
        self.sequence == sequence && unread.remove(&self.session)
    }
}

fn sort_entries(rows: &mut [(&str, Entry)]) {
    rows.sort_by_cached_key(|(workspace, entry)| {
        (
            entry.priority,
            std::cmp::Reverse(entry.updated_at),
            (*workspace).to_owned(),
            entry
                .snooze_key
                .clone()
                .unwrap_or_else(|| entry.target.key()),
        )
    });
}

impl Attention {
    fn is_question(&self) -> bool {
        self.kind == "question_waiting_answer"
    }

    fn is_actionable_message(&self) -> bool {
        self.is_question() || self.kind == "message_delivery_pending"
    }

    fn waiting_report_id(&self) -> Option<&str> {
        (self.kind == "dispatch_waiting_report")
            .then_some(self.dispatch_id.as_deref())
            .flatten()
    }

    fn task_title(&self) -> Option<&str> {
        self.task_title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
    }

    fn snooze_key(&self) -> Option<String> {
        match self.kind.as_str() {
            // Preserve identity when the same queued input becomes answerable.
            "question_waiting_answer" | "message_delivery_pending" => {
                self.message_id.as_ref().map(|id| format!("message:{id}"))
            }
            "dispatch_waiting_report" => self.dispatch_id.as_ref().map(|id| format!("report:{id}")),
            _ => None,
        }
    }

    fn entry(
        &self,
        snapshot: &ActionCenter,
        name: &mut impl FnMut(&str) -> String,
    ) -> Option<Entry> {
        let (label, priority) = match self.kind.as_str() {
            "question_waiting_answer" => ("等待答复的协作问题", Priority::NeedsUser),
            "message_delivery_pending" => ("等待投递的协作消息", Priority::Active),
            "dispatch_waiting_report" => ("等待汇报的派单", Priority::Active),
            "stopped_with_queue" => ("成员已停止，仍有待办", Priority::NeedsUser),
            _ => return None,
        };
        let target = match &self.dispatch_id {
            Some(id) if uuid::Uuid::try_parse(id).is_ok() => match self.message_id.as_ref() {
                Some(message)
                    if matches!(
                        self.kind.as_str(),
                        "question_waiting_answer" | "message_delivery_pending"
                    ) =>
                {
                    Destination::DispatchMessage {
                        dispatch: id.clone(),
                        message: message.clone(),
                    }
                }
                _ => Destination::Dispatch(id.clone()),
            },
            None if self.kind == "stopped_with_queue" => {
                Destination::Member(self.worker_id.clone()?)
            }
            _ => return None,
        };
        let message = self.message_id.as_ref().and_then(|id| {
            snapshot.recent_dispatch_messages.iter().find(|message| {
                &message.id == id && Some(&message.dispatch_id) == self.dispatch_id.as_ref()
            })
        });
        let sender = self
            .message_sender_id
            .as_deref()
            .or_else(|| message.map(|message| message.from_agent_id.as_str()));
        let subject = if let Destination::DispatchMessage {
            message: message_id,
            ..
        } = &target
        {
            let snippet = self
                .message_preview
                .as_deref()
                .or_else(|| message.and_then(|message| message.text.trim_start().lines().next()))
                .map(str::trim)
                .filter(|line| !line.is_empty());
            let text = snippet
                .unwrap_or(message_id)
                .chars()
                .take(if snippet.is_some() { 120 } else { 8 })
                .collect::<String>();
            if let Some(sender) = sender {
                format!("{} · {text}", name(sender))
            } else {
                text
            }
        } else {
            let member = self
                .worker_name
                .clone()
                .or_else(|| self.worker_id.as_deref().map(name))
                .unwrap_or_default();
            if self.waiting_report_id().is_some() {
                let title = self.task_title().unwrap_or_else(|| tr("未命名派单"));
                if member.is_empty() {
                    title.to_owned()
                } else {
                    format!("{title} · {member}")
                }
            } else {
                member
            }
        };
        let snooze_key = self.snooze_key();
        let wake = self
            .dispatch_id
            .as_deref()
            .map(|id| snapshot.dispatch_wake(id));
        let activity_at = self
            .message_created_at
            .filter(|time| *time > 0)
            .or_else(|| {
                message
                    .map(|message| message.created_at)
                    .filter(|time| *time > 0)
            })
            .or(self.submitted_at)
            .unwrap_or(0);
        Some(Entry {
            label,
            subject,
            target,
            dispatch_backend: self.dispatch_backend,
            related_members: [self.worker_id.clone(), sender.map(str::to_owned)],
            snooze_key,
            wake,
            priority,
            updated_at: activity_at,
            activity_at,
        })
    }
}

impl HiveApp {
    fn show_verified_inbox_member(
        &mut self,
        workspace: &str,
        member: &str,
        members: Vec<Member>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(exact) = exact_inbox_member(&members, member) else {
            return;
        };
        let search = self.member_search.read(cx).value().trim().to_lowercase();
        if !exact.matches_team_status(self.member_status_filter) {
            self.member_status_filter = "all";
        }
        if !exact.matches_team_search(&search) {
            self.member_search
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        if self.workspace.as_deref() != Some(workspace) {
            self.select_workspace(workspace.to_owned(), cx);
        }
        self.members = members;
        self.member_details_open
            .entry(member.to_owned())
            .or_default();
        self.member_focus_search = false;
        self.collaboration.inbox.member_reveal = Some(InboxMemberReveal {
            workspace: workspace.to_owned(),
            member: member.to_owned(),
            connection: self.connection_generation,
            focus: cx.focus_handle(),
            pending: true,
        });
        self.show_team_view(window, cx, false);
    }

    pub(crate) fn clear_inbox_member_reveal(&mut self) {
        self.collaboration.inbox.member_reveal = None;
    }

    pub(crate) fn inbox_member_focus(&self, member: &str) -> Option<&FocusHandle> {
        let reveal = self.collaboration.inbox.member_reveal.as_ref()?;
        (self.member_panel
            && self.connection_generation == reveal.connection
            && self.workspace.as_deref() == Some(&reveal.workspace)
            && reveal.member == member)
            .then_some(&reveal.focus)
    }

    pub(crate) fn reveal_inbox_member(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(reveal) = &mut self.collaboration.inbox.member_reveal else {
            return;
        };
        if !reveal.pending
            || !self.member_panel
            || self.workspace.as_deref() != Some(&reveal.workspace)
            || self.connection_generation != reveal.connection
            || exact_inbox_member(&self.members, &reveal.member).is_none()
        {
            return;
        }
        reveal.pending = false;
        let member = reveal.member.clone();
        let focus = reveal.focus.clone();
        let app = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            if app
                .upgrade()
                .is_some_and(|app| app.read(cx).inbox_member_focus(&member) == Some(&focus))
            {
                focus.focus(window, cx);
            }
        });
    }

    fn open_project_inbox_member(
        &mut self,
        workspace: String,
        member: String,
        position: ProjectInboxPosition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(origin), Some(runtime)) = (self.history_link_target(), self.runtime.clone())
        else {
            return;
        };
        self.collaboration.detail_generation += 1;
        let generation = self.collaboration.detail_generation;
        self.collaboration.inbox.member_open_error = false;
        cx.spawn_in(window, async move |this, cx| {
            let roster = verify_inbox_member(&runtime, &workspace, &member).await;
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |app, cx| {
                    if !origin.current(app)
                        || app.collaboration.detail_generation != generation
                        || app.workspace.as_deref() != Some(&workspace)
                        || app.workspace_removing.as_deref() == Some(&workspace)
                        || !app.collaboration.visible
                    {
                        return;
                    }
                    if let Some(roster) = roster {
                        app.show_verified_inbox_member(&workspace, &member, roster, window, cx);
                        app.collaboration.inbox.return_position = Some(position);
                    } else {
                        app.collaboration.inbox.member_open_error = true;
                        cx.notify();
                    }
                });
            });
        })
        .detach();
    }

    pub(crate) fn note_live_inbox_reply(
        &mut self,
        workspace: &str,
        session: &str,
        revision: u64,
        received_at: Option<u64>,
    ) {
        self.collaboration
            .inbox_badge
            .note_reply(workspace, session, revision, received_at);
    }

    fn clear_inbox_unread_reminder(
        &mut self,
        workspace: &str,
        session: &str,
        cx: &mut Context<Self>,
    ) -> Result<Option<Snoozes>, ()> {
        let Some(root) = &self.data_dir else {
            return Ok(None);
        };
        let saved = Snoozes::clear_unread(root, workspace, session).map_err(|error| {
            self.error = Some(ErrorNotice::Detail("无法保存已暂缓事项", error.to_string()));
            cx.notify();
        })?;
        if self.collaboration.inbox.workspace.as_deref() == Some(workspace) {
            self.collaboration.inbox.reminders.saved = Some(saved.clone());
        }
        self.collaboration
            .inbox_badge
            .remember_reminders(workspace, &saved);
        Ok(Some(saved))
    }

    fn current_inbox_reply_revision(&self, session: &Session) -> u64 {
        session.inbox_reply_revision().max(
            self.collaboration
                .inbox_badge
                .reply_revision(&session.workspace_id, &session.id),
        )
    }

    pub(crate) fn read_inbox_chat(&mut self, session: &Session, cx: &mut Context<Self>) {
        if self.current_inbox_reply_revision(session) == session.inbox_reply_revision()
            && self
                .clear_inbox_unread_reminder(&session.workspace_id, &session.id, cx)
                .is_ok()
        {
            self.collaboration
                .inbox_badge
                .forget_unread(&session.workspace_id, &session.id);
        }
    }

    pub(super) fn refresh_collaboration_inbox(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let root = self.data_dir.clone();
        let inbox = &mut self.collaboration.inbox;
        let connection = self.connection_generation;
        if inbox.workspace.as_ref() != Some(&workspace) || inbox.connection != connection {
            let generation = inbox.generation + 1;
            *inbox = Inbox {
                workspace: Some(workspace.clone()),
                connection,
                generation,
                ..Inbox::default()
            };
        }
        if inbox.loading {
            return;
        }
        inbox.loading = true;
        inbox.generation += 1;
        let generation = inbox.generation;
        cx.spawn(async move |this, cx| {
            let (result, workflow_runs) = smol::future::zip(
                read_action_center(&runtime, &workspace, root.as_deref()),
                read_workflow_runs(&runtime, &workspace, root.as_deref()),
            ).await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() != Some(&workspace)
                    || this.connection_generation != connection
                    || !this.collaboration.visible
                    || this.collaboration.inbox.generation != generation
                {
                    return;
                }
                let interactions = this.inbox_interactions(&this.sessions, &this.members);
                let inbox = &mut this.collaboration.inbox;
                inbox.loading = false;
                inbox.workflow_runs_error = workflow_runs.is_none();
                if let Some(workflow_runs) = workflow_runs {
                    inbox.workflow_runs = Some(workflow_runs);
                }
                let snapshot = match result {
                    Ok(snapshot) => {
                        inbox.error = None;
                        Some(snapshot)
                    }
                    Err(error) => {
                        inbox.error = Some(error);
                        None
                    }
                };
                if let Some(root) = &this.data_dir {
                    inbox.reminders.reconcile(
                        root,
                        &workspace,
                        snapshot.as_ref(),
                        &interactions,
                        &this.sessions,
                    );
                    if let Some(saved) = &inbox.reminders.saved {
                        this.collaboration
                            .inbox_badge
                            .remember_reminders(&workspace, saved);
                    }
                }
                if !inbox.workflow_runs_error && let Some(snapshot) = &inbox.workflow_runs {
                    this.collaboration
                        .inbox_badge
                        .remember_workflow_runs(&workspace, snapshot.runs.iter().map(|run| &run.id));
                }
                if let Some(snapshot) = snapshot {
                    this.collaboration.inbox_badge.remember_questions(&snapshot);
                    inbox.action_center = Some(snapshot);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_inbox_dispatch(
        &mut self,
        destination: &Destination,
        return_position: Option<ProjectInboxPosition>,
        cx: &mut Context<Self>,
    ) {
        let (id, message) = match destination {
            Destination::Dispatch(id) => (id.clone(), None),
            Destination::DispatchMessage { dispatch, message } => {
                (dispatch.clone(), Some(message.clone()))
            }
            _ => return,
        };
        let Some(target) = self.history_link_target() else {
            return;
        };
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        self.collaboration.detail_generation += 1;
        let generation = self.collaboration.detail_generation;
        cx.spawn(async move |this, cx| {
            let (result, selected_message) =
                smol::future::zip(read_dispatch(&runtime, &workspace, &id), async {
                    let message = message.as_ref()?;
                    let result = runtime
                        .request(
                            "inbox.dispatchMessage",
                            json!({
                                "workspaceId":workspace, "dispatchId":id, "messageId":message,
                            }),
                        )
                        .await;
                    Some(
                        result
                            .ok()
                            .and_then(|value| exact_message(value, &workspace, &id, message))
                            .map_or(
                                SelectedDispatchMessage::Unavailable,
                                SelectedDispatchMessage::Found,
                            ),
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !target.current(this) || this.collaboration.detail_generation != generation {
                    return;
                }
                match result {
                    Ok(dispatch) if dispatch.id == id => {
                        this.show_dispatch(dispatch, cx);
                        this.collaboration.selected_message = selected_message;
                        if return_position.is_some() {
                            this.collaboration.inbox.return_position = return_position;
                        }
                    }
                    Ok(_) => this.error = Some("收到的派单与所选派单不一致。".into()),
                    Err(error) => this.error = Some(ErrorNotice::Detail("无法打开派单", error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn inbox_interactions(
        &self,
        sessions: &[Session],
        members: &[Member],
    ) -> BTreeMap<String, BTreeSet<String>> {
        sessions
            .iter()
            .filter_map(|session| {
                let mut ids = self.inbox_interaction_ids(&session.id);
                if let Some(activity) = session_activity(session, members) {
                    ids.extend(activity.pending_interaction_ids.iter().cloned());
                }
                (!ids.is_empty()).then(|| (session.id.clone(), ids))
            })
            .collect()
    }

    fn inbox_session_entry(
        &self,
        session: &Session,
        members: &[Member],
        interactions: &BTreeMap<String, BTreeSet<String>>,
        include_active: bool,
        deferred_unread: bool,
    ) -> Option<Entry> {
        if session.archived || self.removed_sessions.contains(&session.id) {
            return None;
        }
        let activity = session_activity(session, members);
        let exact_ids = interactions.get(&session.id);
        let (label, priority) = if exact_ids.is_some()
            || self.interaction_count(&session.id) > 0
            || activity.is_some_and(|activity| activity.pending_interaction_count > 0)
        {
            ("等待你的操作", Priority::NeedsUser)
        } else if activity.is_some_and(|activity| activity.delivery_failure_message.is_some()) {
            ("投递失败", Priority::NeedsUser)
        } else if activity.is_some_and(|activity| activity.pending_report_count > 0) {
            ("待处理的汇报", Priority::Unread)
        } else if self.unread.contains(&session.id) || deferred_unread {
            (UNREAD_CONVERSATION, Priority::Unread)
        } else if include_active
            && (self.busy.contains(&session.id)
                || activity.is_some_and(|activity| activity.runtime_state == "working"))
        {
            ("处理中", Priority::Active)
        } else {
            return None;
        };
        let mut subject = if session.custom_title {
            session.title.clone()
        } else {
            session
                .display_agent_id()
                .and_then(|agent| members.iter().find(|member| member.id == agent))
                .map(|member| member.name.clone())
                .unwrap_or_else(|| session.title.clone())
        };
        if let Some(execution) = &session.execution {
            if execution.kind == "worktree" && !execution.branch.is_empty() {
                subject.push_str(&format!(" · {}", execution.branch));
            }
        }
        let (snooze_key, wake) = if let Some(ids) = exact_ids {
            (
                Some(format!("session:{}", session.id)),
                Some(Wake::Session {
                    interaction_ids: ids.clone(),
                }),
            )
        } else if label == UNREAD_CONVERSATION {
            (
                Some(unread_session_key(&session.id)),
                Some(Wake::UnreadReply {
                    revision: self.current_inbox_reply_revision(session),
                }),
            )
        } else {
            (None, None)
        };
        Some(Entry {
            label,
            subject,
            target: Destination::Session(session.clone()),
            dispatch_backend: None,
            related_members: [session.display_agent_id().map(str::to_owned), None],
            snooze_key,
            wake,
            priority,
            updated_at: session.updated_at.max(0) as u64,
            activity_at: session_inbox_activity(session).max(
                self.collaboration
                    .inbox_badge
                    .reply_time(&session.workspace_id, &session.id)
                    .unwrap_or(0),
            ),
        })
    }

    fn scoped_entries(&self, workspace: &str) -> Vec<Entry> {
        if self.workspace.as_deref() != Some(workspace) {
            return Vec::new();
        }
        let interactions = self.inbox_interactions(&self.sessions, &self.members);
        let mut entries: Vec<_> = self
            .sessions
            .iter()
            .filter(|session| {
                session.workspace_id == workspace && session.display_agent_id().is_some()
            })
            .filter_map(|session| {
                self.inbox_session_entry(
                    session,
                    &self.members,
                    &interactions,
                    false,
                    self.collaboration.inbox.workspace.as_deref() == Some(workspace)
                        && self.collaboration.inbox.reminders.tracks_unread(session),
                )
            })
            .collect();
        if let Some(snapshot) = self
            .collaboration
            .inbox
            .action_center
            .as_ref()
            .filter(|snapshot| snapshot.workspace_id == workspace)
        {
            entries.extend(snapshot.entries(|id| self.agent_name(id)));
        }
        if let Some(failures) = self.collaboration.inbox.workflow_runs.as_ref()
            && failures.workspace_id == workspace
        {
            entries.extend(failures.entries());
        }
        entries
    }

    pub(super) fn inbox_session_count(&self) -> usize {
        let Some(workspace) = &self.workspace else {
            return 0;
        };
        let interactions = self.inbox_interactions(&self.sessions, &self.members);
        self.sessions
            .iter()
            .filter(|session| {
                session.workspace_id == *workspace && session.display_agent_id().is_some()
            })
            .filter_map(|session| {
                self.inbox_session_entry(
                    session,
                    &self.members,
                    &interactions,
                    false,
                    self.collaboration.inbox.workspace.as_ref() == Some(workspace)
                        && self.collaboration.inbox.reminders.tracks_unread(session),
                )
            })
            .filter(|entry| {
                self.collaboration.inbox.workspace.as_ref() != Some(workspace)
                    || !is_snoozed(entry, &self.collaboration.inbox.reminders, now_ms())
            })
            .count()
    }

    pub(super) fn render_collaboration_inbox(
        &self,
        scroll: &ScrollHandle,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let inbox = &self.collaboration.inbox;
        let workspace = self.workspace.as_deref().unwrap_or("");
        let current = inbox.workspace.as_deref() == Some(workspace);
        let rows: Vec<_> = self
            .scoped_entries(workspace)
            .into_iter()
            .map(|entry| (workspace, entry))
            .collect();
        let now = now_ms();
        let sections = InboxSections::from_rows(rows, |_, entry| {
            current
                .then(|| inbox.reminders.snoozed_until(entry, now))
                .flatten()
        });
        let show_snoozed = current && inbox.show_snoozed;
        let has_snoozed = !sections.snoozed.is_empty();
        let has_items = !sections.attention.is_empty()
            || !sections.waiting.is_empty()
            || (show_snoozed && has_snoozed);
        let mut depth = if current {
            inbox.section_depth
        } else {
            SectionDepth::default()
        };
        let restore = inbox
            .return_position
            .clone()
            .filter(|_| current && !inbox.loading);
        if let Some(position) = &restore
            && let Some((section, index)) = sections.position(&position.row, true, show_snoozed)
        {
            depth.include_row(section, index);
        }
        let mut rows = Vec::new();
        for (section, items) in [
            (Section::Attention, sections.attention),
            (Section::Waiting, sections.waiting),
            (
                Section::Snoozed,
                if show_snoozed {
                    sections.snoozed
                } else {
                    Vec::new()
                },
            ),
        ] {
            if items.is_empty() {
                continue;
            }
            let count = items.len();
            rows.push(ProjectInboxItem::Heading(section, count));
            rows.extend(
                items
                    .into_iter()
                    .take(depth.limit(section))
                    .map(ProjectInboxItem::Row),
            );
            if count > depth.limit(section) {
                rows.push(ProjectInboxItem::More(
                    section,
                    count - depth.limit(section),
                ));
            }
        }
        let restore_index = restore.as_ref().and_then(|position| {
            rows.iter().position(|row| {
                matches!(row, ProjectInboxItem::Row((workspace, entry))
                if RowId::for_entry(workspace, entry) == position.row)
            })
        });
        let bounds = Rc::new(RefCell::new(Vec::<Bounds<Pixels>>::new()));
        let observed = bounds.clone();
        let viewport = scroll.clone();
        let scope = workspace.to_owned();
        let connection = self.connection_generation;
        let app = cx.weak_entity();
        let row_list =
            v_flex()
                .gap_2()
                .min_w_0()
                .on_children_prepainted(move |bounds, window, cx| {
                    if let Some(position) = &restore {
                        let exact = position
                            .viewport_offset
                            .zip(restore_index.and_then(|index| bounds.get(index)));
                        let offset = exact
                            .map_or(position.previous_offset, |(desired, actual)| {
                                viewport.offset().y + viewport.bounds().top() + desired
                                    - actual.top()
                            })
                            .clamp(-viewport.max_offset().y, px(0.));
                        let viewport = viewport.clone();
                        let scope = scope.clone();
                        let app = app.clone();
                        let expected = position.clone();
                        window.defer(cx, move |window, cx| {
                            let _ = app.update(cx, |app, cx| {
                                if app.connection_generation == connection
                                    && app.workspace.as_deref() == Some(&scope)
                                    && app.collaboration.visible
                                    && app.collaboration.selected.is_none()
                                    && app.collaboration.inbox.return_position.as_ref()
                                        == Some(&expected)
                                {
                                    app.collaboration.inbox.return_position = None;
                                    app.collaboration.inbox.section_depth = depth;
                                    viewport.set_offset(point(px(0.), offset));
                                    window.refresh();
                                    cx.notify();
                                }
                            });
                        });
                    }
                    *observed.borrow_mut() = bounds;
                });
        let status = if current && inbox.error.is_some() {
            Some("无法刷新协作收件箱")
        } else if !has_items {
            Some(
                if self.workspace.is_some() && (!current || inbox.action_center.is_none()) {
                    "正在读取收件箱…"
                } else {
                    "暂无需要关注的协作事项"
                },
            )
        } else {
            None
        };
        v_flex()
            .id("collaboration-inbox")
            .w_full()
            .min_w_0()
            .gap_2()
            .p_3()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(tr("协作收件箱")),
                    )
                    .when(has_snoozed, |header| {
                        header.child(toggle_button(show_snoozed).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.collaboration.inbox.show_snoozed =
                                    !this.collaboration.inbox.show_snoozed;
                                cx.notify();
                            },
                        )))
                    }),
            )
            .when_some(status, |panel, status| {
                panel.child(inbox_status(tr(status), cx))
            })
            .when(current && inbox.member_open_error, |panel| {
                panel.child(inbox_status(tr(OPEN_INBOX_MEMBER_FAILED), cx))
            })
            .when(current && inbox.workflow_runs_error, |panel| {
                panel.child(inbox_status(tr("无法读取需关注的工作流运行"), cx))
            })
            .when(
                current && inbox.workflow_runs.as_ref().is_some_and(|runs| runs.limited),
                |panel| {
                    panel.child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .items_center()
                            .child(inbox_status(tr("收件箱保留最近 20 条失败或中断的工作流运行，以及暂缓过的运行"), cx))
                            .child(
                                inbox_open_button(
                                    "inbox-all-workflows",
                                    !self.connected || self.closing,
                                    cx.listener(|this, _, window, cx| {
                                        this.select_workflow(None, cx);
                                        this.show_workflows(window, cx);
                                    }),
                                )
                                .label(tr("工作流")),
                            ),
                    )
                },
            )
            .when_some(
                current.then(|| inbox.reminders.notice()).flatten(),
                |panel, (label, error)| {
                    panel.child(inbox_status(
                        format!(
                            "{} · {}",
                            tr(label),
                            localize_owned_error(&error.to_string())
                        ),
                        cx,
                    ))
                },
            )
            .child(
                row_list.children(rows.into_iter().enumerate().map(|(index, row)| {
                    let (workspace, entry) = match row {
                        ProjectInboxItem::Row(row) => row,
                        ProjectInboxItem::Heading(section, count) => {
                            return global::section_title(
                                section.heading_id(),
                                tr(section.title()),
                                count,
                                cx,
                            )
                            .into_any_element();
                        }
                        ProjectInboxItem::More(section, remaining) => {
                            let scope = workspace.to_owned();
                            let more = show_more_button(
                                section,
                                section.title(),
                                remaining,
                                cx.listener(move |this, _, _, cx| {
                                    if this.workspace.as_deref() == Some(&scope)
                                        && this.collaboration.inbox.workspace.as_deref()
                                            == Some(&scope)
                                    {
                                        this.collaboration.inbox.section_depth.show_more(section);
                                        cx.notify();
                                    }
                                }),
                            );
                            return Reveal::new(section.control_id(), scroll, more)
                                .into_any_element();
                        }
                    };
                    let reminder = current
                        .then(|| inbox.reminders.active_reminder(&entry, now))
                        .flatten();
                    let workspace = workspace.to_owned();
                    let snooze_workspace = workspace.clone();
                    let connection = self.connection_generation;
                    let archive = ArchiveInboxSession::for_entry(
                        self,
                        &workspace,
                        &entry,
                        &self.members,
                        &inbox.reminders,
                        reminder.is_some(),
                    )
                    .map(|archive| {
                        Button::new("archive-inbox-session").on_click(cx.listener(
                            move |this, _, window, cx| {
                                let target = archive.clone();
                                this.request_inbox_session_archive(
                                    archive.session.clone(),
                                    archive.revision,
                                    false,
                                    move |app, _| {
                                        let workspace = target.session.workspace_id.as_str();
                                        let inbox = &app.collaboration.inbox;
                                        app.connection_generation == connection
                                            && app.collaboration.visible
                                            && app.workspace.as_deref() == Some(workspace)
                                            && inbox.workspace.as_deref() == Some(workspace)
                                            && app.scoped_entries(workspace).iter().any(|entry| {
                                                ArchiveInboxSession::for_entry(
                                                    app,
                                                    workspace,
                                                    entry,
                                                    &app.members,
                                                    &inbox.reminders,
                                                    is_snoozed(entry, &inbox.reminders, now_ms()),
                                                )
                                                .is_some_and(|current| target.matches(&current))
                                            })
                                    },
                                    window,
                                    cx,
                                );
                            },
                        ))
                    });
                    let mark_read =
                        MarkInboxRead::for_entry(self, &workspace, &entry, &inbox.reminders).map(
                            |mark| {
                                Button::new("mark-inbox-session-read")
                                    .disabled(!self.connected || self.closing)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.connected
                                            && !this.closing
                                            && this.connection_generation == connection
                                            && this.workspace.as_deref()
                                                == Some(mark.workspace.as_str())
                                            && this.workspace_removing.as_deref()
                                                != Some(mark.workspace.as_str())
                                            && this
                                                .scoped_entries(&mark.workspace)
                                                .iter()
                                                .any(|entry| mark.matches(entry))
                                        {
                                            let _ = mark.clear(this, cx);
                                        }
                                    }))
                            },
                        );
                    let target = entry.target.clone();
                    let origin = self.history_link_target();
                    let row_id = RowId::for_entry(&workspace, &entry);
                    let bounds = bounds.clone();
                    let viewport = scroll.clone();
                    let action = inbox_open_button(
                        "open-inbox-item",
                        !self.connected || self.closing,
                        cx.listener(move |this, _, window, cx| {
                            if !this.connected
                                || this.closing
                                || this.workspace.as_deref() != Some(&workspace)
                                || !origin.as_ref().is_some_and(|origin| origin.current(this))
                            {
                                return;
                            }
                            let position = ProjectInboxPosition {
                                row: row_id.clone(),
                                viewport_offset: bounds
                                    .borrow()
                                    .get(index)
                                    .map(|bounds| bounds.top() - viewport.bounds().top()),
                                previous_offset: viewport.offset().y,
                            };
                            match &target {
                                Destination::Dispatch(_) | Destination::DispatchMessage { .. } => {
                                    this.open_inbox_dispatch(&target, Some(position), cx);
                                }
                                Destination::Session(session) => {
                                    this.open_member_session(session.id.clone(), window, cx);
                                    if !this.collaboration.visible {
                                        this.collaboration.inbox.return_position = Some(position);
                                    }
                                }
                                Destination::Member(member) => this.open_project_inbox_member(
                                    workspace.clone(),
                                    member.clone(),
                                    position,
                                    window,
                                    cx,
                                ),
                                Destination::WorkflowRun(id) => {
                                    let inbox = &this.collaboration.inbox;
                                    if this.workspace_removing.as_deref() == Some(&workspace)
                                        || !inbox.workflow_runs.as_ref().is_some_and(|runs| {
                                            runs.workspace_id == workspace
                                                && runs.runs.iter().any(|run| run.id == *id)
                                        })
                                    {
                                        return;
                                    }
                                    this.collaboration.inbox.return_position = Some(position);
                                    this.open_inbox_workflow(id.clone(), window, cx);
                                }
                            }
                        }),
                    );
                    inbox_row(
                        &snooze_workspace,
                        None,
                        entry,
                        reminder,
                        current && inbox.reminders.saved.is_some() && !self.closing,
                        action,
                        mark_read,
                        archive,
                        scroll,
                        cx,
                        None,
                        |this, edit, delay, cx| {
                            let workspace = edit.workspace.as_str();
                            if !this.connected
                                || this.closing
                                || this.workspace.as_deref() != Some(workspace)
                                || this.workspace_removing.as_deref() == Some(workspace)
                                || this.collaboration.inbox.workspace.as_deref() != Some(workspace)
                                || this.collaboration.inbox.connection != this.connection_generation
                                || !edit.reply_is_current(this)
                                || !this
                                    .scoped_entries(workspace)
                                    .iter()
                                    .any(|entry| edit.matches(entry))
                            {
                                return Err(SNOOZE_CHANGED);
                            }
                            let root = this.data_dir.as_ref().ok_or("无法保存已暂缓事项")?;
                            let result = this.collaboration.inbox.reminders.set(root, edit, delay);
                            if result.is_ok()
                                && let Some(saved) = &this.collaboration.inbox.reminders.saved
                            {
                                this.collaboration
                                    .inbox_badge
                                    .remember_reminders(workspace, saved);
                                if delay.is_some() {
                                    this.collaboration.inbox_badge.forget_snoozed(edit);
                                }
                            }
                            cx.notify();
                            if let Some(change) = result? {
                                let generation = this.collaboration.inbox.undo.offer(change);
                                expire_later(cx, generation, |app| {
                                    &mut app.collaboration.inbox.undo
                                });
                            }
                            Ok(())
                        },
                    )
                })),
            )
            .when_some(
                current
                    .then(|| inbox.undo.render(cx, Self::undo_workspace_snooze))
                    .flatten(),
                |panel, undo| panel.child(undo),
            )
    }

    fn undo_workspace_snooze(&mut self, cx: &mut Context<Self>) {
        let Some(runtime) = self
            .runtime
            .clone()
            .filter(|_| self.connected && !self.closing)
        else {
            return;
        };
        let Some((generation, change)) = self.collaboration.inbox.undo.claim() else {
            return;
        };
        let connection = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let verified = VerifiedUndo::read(&runtime, &change).await;
            let _ = this.update(cx, |this, cx| {
                let workspace = change.edit.workspace.as_str();
                if !this.connected
                    || this.closing
                    || !this.collaboration.visible
                    || this.connection_generation != connection
                    || this.workspace.as_deref() != Some(workspace)
                    || this.workspace_removing.as_deref() == Some(workspace)
                    || !this.collaboration.inbox.undo.is_checking(generation)
                {
                    return;
                }
                let result = (|| {
                    if !change.edit.reply_is_current(this)
                        || !verified?.matches(&change.edit, this)
                        || !this
                            .scoped_entries(workspace)
                            .iter()
                            .any(|entry| change.edit.matches(entry))
                    {
                        return Err(UNDO_CHANGED);
                    }
                    let root = this.data_dir.as_ref().ok_or("无法保存已暂缓事项")?;
                    this.collaboration.inbox.reminders.restore(root, &change)
                })();
                if result.is_ok()
                    && let Some(saved) = &this.collaboration.inbox.reminders.saved
                {
                    this.collaboration
                        .inbox_badge
                        .remember_reminders(workspace, saved);
                }
                let generation = this.collaboration.inbox.undo.finish(result);
                expire_later(cx, generation, |app| &mut app.collaboration.inbox.undo);
                this.refresh_collaboration_inbox(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

fn session_activity<'a>(
    session: &Session,
    members: &'a [Member],
) -> Option<&'a crate::protocol::MemberDesktopActivity> {
    members
        .iter()
        .filter_map(|member| member.desktop_activity.as_ref())
        .find(|activity| activity.session_id.as_ref() == Some(&session.id))
}

fn same_wake_target(current: &Option<Wake>, selected: &Option<Wake>) -> bool {
    match (current, selected) {
        (
            Some(Wake::Dispatch {
                id: a,
                sequence: first,
                ..
            }),
            Some(Wake::Dispatch {
                id: b,
                sequence: last,
                ..
            }),
        ) => a == b && first == last,
        _ => current == selected,
    }
}

fn is_snoozed(entry: &Entry, reminders: &Reminders, now: u64) -> bool {
    reminders.active_reminder(entry, now).is_some()
}

fn toggle_button(show_snoozed: bool) -> Button {
    Button::new("toggle-snoozed-inbox")
        .ghost()
        .compact()
        .xsmall()
        .label(if show_snoozed {
            tr("隐藏已暂缓事项")
        } else {
            tr("显示已暂缓事项")
        })
}

fn inbox_status(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .id("inbox-status")
        .role(Role::Status)
        .text_xs()
        .text_color(Color::Muted.get(cx))
        .child(text.into())
        .into_any_element()
}

type InboxRowObserver = std::rc::Rc<dyn Fn(&bool, &mut Window, &mut App)>;

struct InboxRowInteraction {
    focus: FocusHandle,
    hover: InboxRowObserver,
    snooze_menu: InboxRowObserver,
}

fn inbox_open_button(
    id: &'static str,
    disabled: bool,
    open: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    let open = std::rc::Rc::new(open);
    let confirm = open.clone();
    Button::new(id)
        .ghost()
        .compact()
        .xsmall()
        .min_w_0()
        .h_auto()
        .px_1()
        .py_0()
        .cursor_pointer()
        .disabled(disabled)
        .on_click(move |event, window, cx| open(event, window, cx))
        // The cross-project dialog binds Return to Confirm. Consume it at the
        // focused primary button, so it invokes the same guarded action once.
        .on_action(move |action: &Confirm, window, cx| {
            if !disabled && !action.secondary {
                cx.stop_propagation();
                confirm(&ClickEvent::default(), window, cx);
            }
        })
}

fn show_more_button(
    section: Section,
    title: &'static str,
    remaining: usize,
    show: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    inbox_open_button(section.control_id(), false, show)
        .px_2()
        .py_1()
        .label(format!("{} · {} ({remaining})", tr("显示更多"), tr(title)))
}

fn inbox_row<T: 'static>(
    workspace: &str,
    project: Option<String>,
    entry: Entry,
    reminder: Option<Reminder>,
    can_snooze: bool,
    open: Button,
    mark_read: Option<Button>,
    archive: Option<Button>,
    scroll: &ScrollHandle,
    cx: &Context<T>,
    interaction: Option<InboxRowInteraction>,
    on_snooze: impl Fn(&mut T, &SnoozeEdit, Option<Delay>, &mut Context<T>) -> Result<(), &'static str>
    + 'static,
) -> AnyElement {
    let label = tr(entry.label);
    let action = tr(entry.target.action());
    let row_key = entry
        .snooze_key
        .clone()
        .unwrap_or_else(|| entry.target.key());
    let row_id = format!("{workspace}:{row_key}");
    let wake_label = reminder.as_ref().map(|reminder| match reminder.expires_at {
        Some(epoch) => format!(
            "{} · {}",
            tr("唤回时间"),
            schedule::Deadline::from_epoch_local(epoch).map_or_else(
                || tr("本地时间不可用").to_owned(),
                |deadline| deadline.label()
            )
        ),
        None => tr("无限期").to_owned(),
    });
    let snooze_label = if reminder.is_some() {
        tr("调整暂缓时间")
    } else {
        tr("暂缓")
    };
    let subject = project.map_or_else(
        || entry.subject.clone(),
        |project| format!("{project} · {}", entry.subject),
    );
    let snooze_button = Button::new("snooze-inbox-item")
        .ghost()
        .compact()
        .xsmall()
        .label(snooze_label)
        .disabled(!can_snooze)
        .accessibility_label(format!("{snooze_label} · {label} · {subject}"));
    let row = Reveal::new(
        SharedString::from(row_id),
        scroll,
        h_flex()
            .min_w_0()
            .gap_2()
            .items_center()
            .justify_between()
            .px_2()
            .py_1p5()
            .rounded_md()
            .bg(Color::Inset.get(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        open.w_full()
                            .accessibility_label(format!("{action} · {label} · {subject}"))
                            .child(
                                v_flex()
                                    .w_full()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .truncate()
                                            .child(label),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .truncate()
                                            .child(subject.clone()),
                                    ),
                            ),
                    )
                    .when_some(wake_label, |details, wake_label| {
                        details.child(
                            div()
                                .id("inbox-row-wake-time")
                                .role(Role::Label)
                                .aria_label(wake_label.clone())
                                .pl_1()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .truncate()
                                .child(wake_label),
                        )
                    }),
            )
            .child(
                h_flex()
                    .id("inbox-row-actions")
                    .gap_1()
                    .max_w(relative(0.55))
                    .flex_wrap()
                    .justify_end()
                    .when_some(interaction.as_ref(), |actions, interaction| {
                        let hover = interaction.hover.clone();
                        actions.on_hover(move |over, window, cx| {
                            hover(over, window, cx);
                        })
                    })
                    .when_some(entry.snooze_key, |buttons, key| {
                        let initial_deadline =
                            reminder.as_ref().and_then(|reminder| reminder.expires_at);
                        let editing = reminder.is_some();
                        let edit = SnoozeEdit {
                            workspace: workspace.to_owned(),
                            key,
                            wake: entry.wake,
                            previous: reminder,
                        };
                        let entity = cx.weak_entity();
                        let apply = std::rc::Rc::new(move |delay, cx: &mut App| {
                            entity
                                .update(cx, |this, cx| on_snooze(this, &edit, delay, cx))
                                .unwrap_or(Err(SNOOZE_CHANGED))
                        });
                        let buttons = buttons.when(editing, |buttons| {
                            let apply = apply.clone();
                            let unsnooze = tr("取消暂缓");
                            buttons.child(
                                Button::new("unsnooze-inbox-item")
                                    .ghost()
                                    .compact()
                                    .xsmall()
                                    .label(unsnooze)
                                    .disabled(!can_snooze)
                                    .accessibility_label(format!(
                                        "{unsnooze} · {label} · {subject}"
                                    ))
                                    .on_click(move |_, _, cx| {
                                        let _ = apply(None, cx);
                                    }),
                            )
                        });
                        let menu = snooze_button.dropdown_menu(move |menu, _, _| {
                            let apply = apply.clone();
                            let save =
                                std::rc::Rc::new(move |delay, cx: &mut App| apply(Some(delay), cx));
                            let menu = schedule::presets().into_iter().fold(
                                menu,
                                |menu, (title, deadline)| {
                                    let save = save.clone();
                                    let label = format!(
                                        "{} · {}",
                                        tr(title),
                                        deadline.map_or_else(
                                            || tr("本地时间不可用").to_owned(),
                                            |at| at.label()
                                        )
                                    );
                                    menu.item(
                                        PopupMenuItem::new(label)
                                            .disabled(deadline.is_none())
                                            .on_click(move |_, _, cx| {
                                                if let Some(at) = deadline {
                                                    let _ = save(at.delay(), cx);
                                                }
                                            }),
                                    )
                                },
                            );
                            let indefinite = save.clone();
                            menu.item(PopupMenuItem::new(tr("无限期")).on_click(move |_, _, cx| {
                                let _ = indefinite(Delay::Indefinite, cx);
                            }))
                            .separator()
                            .item(
                                PopupMenuItem::new(tr("自定义日期和时间…")).on_click(
                                    move |_, window, cx| {
                                        schedule::open_custom_snooze(
                                            save.clone(),
                                            initial_deadline,
                                            window,
                                            cx,
                                        );
                                    },
                                ),
                            )
                        });
                        buttons.child(menu.when_some(interaction.as_ref(), |menu, interaction| {
                            let observe_menu = interaction.snooze_menu.clone();
                            menu.on_open_change(move |open, window, cx| {
                                observe_menu(open, window, cx);
                            })
                        }))
                    })
                    .when_some(mark_read, |buttons, mark_read| {
                        let mark = tr("标为已读");
                        buttons.child(
                            mark_read
                                .ghost()
                                .compact()
                                .xsmall()
                                .label(mark)
                                .accessibility_label(format!("{mark} · {label} · {subject}")),
                        )
                    })
                    .when_some(archive, |buttons, archive| {
                        let archive_label = tr("归档会话");
                        buttons.child(
                            archive
                                .ghost()
                                .compact()
                                .xsmall()
                                .label(archive_label)
                                .accessibility_label(format!(
                                    "{archive_label} · {label} · {subject}"
                                )),
                        )
                    }),
            ),
    );
    row.when_some(interaction, |row, state| row.track_focus(&state.focus))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{
        ActionCenter, ActionableCursor, ActionablePage, ArchiveInboxSession, Attention, Delay,
        Destination, Entry, MarkInboxRead, Priority, Reminders, SnoozeEdit, Snoozes,
        UNREAD_CONVERSATION, Wake, WorkflowRuns, exact_inbox_member, exact_message, now_ms,
        sort_entries, validate_actionable_rows,
    };
    use crate::protocol::{Member, MemberDesktopActivity};
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn project_inbox_distinguishes_exact_same_name_workflow_runs() {
        let earlier = "12345678-1234-1234-1234-123456789001";
        let later = "12345678-1234-1234-1234-123456789002";
        let mut snapshot: WorkflowRuns = serde_json::from_value(json!({
            "workspaceId": "atlas", "limited": false, "runs": [
                {"id": earlier, "name": "Daily audit", "finishedAt": 10, "status": "failed"},
                {"id": later, "name": "Daily audit", "finishedAt": 20, "status": "interrupted"},
            ],
        }))
        .unwrap();
        assert!(snapshot.verified("atlas"));
        assert!(!snapshot.verified("beacon"));
        let mut rows: Vec<_> = snapshot.entries().map(|entry| ("atlas", entry)).collect();
        sort_entries(&mut rows);
        assert_eq!(
            rows.iter().map(|(_, row)| row.target.key()).collect::<Vec<_>>(),
            [format!("workflow-run:{later}"), format!("workflow-run:{earlier}")],
        );
        assert_ne!(rows[0].1.subject, rows[1].1.subject);
        assert_eq!(rows[0].1.label, "工作流运行中断");
        assert_eq!(rows[1].1.label, "工作流运行失败");
        assert!(rows.iter().all(|(_, row)| {
            row.priority == Priority::NeedsUser
                && row.snooze_key.as_ref() == Some(&row.target.key())
                && row.wake == Some(Wake::WorkflowRun)
        }));
        snapshot.runs[1].id = format!("87654321{}", &earlier[8..]);
        let collision_labels: Vec<_> = snapshot.entries().map(|entry| entry.subject).collect();
        assert!(collision_labels[0].ends_with(earlier));
        assert!(collision_labels[1].ends_with(&snapshot.runs[1].id));
        snapshot.runs[1].id = earlier.to_owned();
        assert!(!snapshot.verified("atlas"));
    }

    #[test]
    fn selecting_an_inbox_member_uses_unique_exact_id_and_the_same_team_filter_predicates() {
        let make = |id: &str, description: &str, status: &str| {
            serde_json::from_value::<Member>(json!({
                "id":id, "name":"Same reviewer", "description":description,
                "role":"reviewer", "status":status, "pending_task_count":3,
            }))
            .unwrap()
        };
        let first = make("first", "Review migration", "working");
        let second = make("second", "Verify billing", "stopped");
        let roster = [first, second];
        let target = exact_inbox_member(&roster, "second").unwrap();
        assert_eq!(target.description, "Verify billing");
        assert!(target.matches_team_search("reviewer"));
        assert!(!target.matches_team_search("migration"));
        assert!(target.matches_team_status("all"));
        assert!(!target.matches_team_status("working"));
        assert!(target.matches_team_status("stopped"));
        assert!(exact_inbox_member(&roster, "unknown").is_none());
        assert!(
            exact_inbox_member(
                &[
                    make("second", "Verify billing", "stopped"),
                    make("second", "Other", "idle")
                ],
                "second",
            )
            .is_none()
        );
    }

    #[test]
    fn actionable_pages_reject_missing_boundaries_incomplete_pages_and_duplicates() {
        let row = |id: &str, kind: &str| {
            serde_json::from_value::<Attention>(json!({
                "kind":kind, "message_id":id, "dispatch_id":"11111111-0000-4000-8000-0000aaaabbbb",
            }))
            .unwrap()
        };
        let no_continuation = ActionableCursor {
            through: 120,
            next_after: None,
        };
        let partial = ActionableCursor {
            next_after: Some(100),
            ..no_continuation
        };
        let first = row("first", "question_waiting_answer");
        assert!(validate_actionable_rows([&first], true, partial, &mut HashSet::new()).is_err());
        assert!(
            serde_json::from_value::<ActionablePage>(json!({
                "workspaceId":"project", "questionsOnly":true, "through":120, "rows":[],
            }))
            .is_err()
        );
        let mut seen = HashSet::new();
        assert!(validate_actionable_rows([&first], true, no_continuation, &mut seen).is_ok());
        assert!(validate_actionable_rows([&first], true, no_continuation, &mut seen).is_err());
        let delivery = row("delivery", "message_delivery_pending");
        assert!(validate_actionable_rows([&delivery], true, no_continuation, &mut seen).is_err());
        assert!(validate_actionable_rows([&delivery], false, no_continuation, &mut seen).is_ok());
    }

    #[test]
    fn old_question_rows_keep_the_exact_sender_preview_and_newest_first_order() {
        let dispatch = "11111111-0000-4000-8000-0000aaaabbbb";
        let row = |id: &str, time: u64, preview: Option<&str>| {
            json!({
                "kind":"question_waiting_answer", "dispatch_id":dispatch, "message_id":id,
                "worker_id":"orchestrator", "worker_name":"Wrong sender",
                "message_sender_id":"reviewer", "message_created_at":time, "message_preview":preview,
            })
        };
        let snapshot: ActionCenter = serde_json::from_value(json!({
            "workspace_id":"workspace", "generated_at":1000, "recent_dispatch_messages":[],
            "attention":[
                row("a-older", 110, Some("Refresh the diagram?")),
                row("z-newer", 210, Some("Approve the report?")),
                row("b-empty-preview", 100, None)
            ],
        }))
        .unwrap();
        let mut rows: Vec<_> = snapshot
            .entries(str::to_owned)
            .into_iter()
            .map(|entry| ("workspace", entry))
            .collect();
        sort_entries(&mut rows);
        for ((_, entry), (message, subject, created_at)) in rows.iter().zip([
            ("z-newer", "reviewer · Approve the report?", 210),
            ("a-older", "reviewer · Refresh the diagram?", 110),
            ("b-empty-preview", "reviewer · b-empty-", 100),
        ]) {
            assert_eq!(entry.subject, subject);
            assert_eq!(entry.updated_at, created_at);
            assert_eq!(entry.activity_at, created_at);
            assert_eq!(entry.snooze_key, Some(format!("message:{message}")));
            assert!(
                matches!(&entry.target, Destination::DispatchMessage { dispatch: d, message: m }
                if d == dispatch && m == message)
            );
        }
    }

    #[test]
    fn two_questions_on_one_dispatch_keep_distinct_exact_open_targets() {
        let dispatch = "11111111-0000-4000-8000-0000aaaabbbb";
        let attention: Vec<_> = ["question-a", "question-b"]
            .into_iter()
            .map(|id| {
                json!({
                    "kind": "question_waiting_answer", "dispatch_id": dispatch, "message_id": id,
                    "worker_id": "worker",
                })
            })
            .collect();
        let snapshot: ActionCenter = serde_json::from_value(json!({
            "workspace_id": "workspace", "generated_at": 1000, "attention": attention,
            "recent_dispatch_messages": [],
        }))
        .unwrap();
        let destinations = snapshot.entries(str::to_owned);
        for (entry, selected) in destinations.iter().zip(["question-a", "question-b"]) {
            assert!(
                matches!(&entry.target, Destination::DispatchMessage {dispatch: d, message: m}
                if d == dispatch && m == selected)
            );
            let response = json!({
                "id": selected, "dispatch_id": dispatch, "workspace_id":"workspace",
                "from_agent_id":"worker", "recipient_agent_id":"workspace:orchestrator",
                "kind":"question", "text": format!("Exact text for {selected}"),
                "delivery_state":"recorded",
            });
            let message = exact_message(response.clone(), "workspace", dispatch, selected).unwrap();
            assert_eq!(message.text, format!("Exact text for {selected}"));
            assert!(
                exact_message(response.clone(), "other-workspace", dispatch, selected).is_none()
            );
            assert!(
                exact_message(response.clone(), "workspace", "sibling-dispatch", selected)
                    .is_none()
            );
            assert!(exact_message(response, "workspace", dispatch, "other-question").is_none());
        }
    }

    #[test]
    fn waiting_report_titles_distinguish_same_member_at_same_time_and_keep_exact_targets() {
        let reports = [
            (
                "11111111-0000-4000-8000-0000aaaabbbb",
                Some("Review billing"),
            ),
            (
                "22222222-0000-4000-8000-0000ccccdddd",
                Some("Audit migration"),
            ),
            (
                "33333333-0000-4000-8000-0000eeeeffff",
                Some("Audit migration"),
            ),
            ("44444444-0000-4000-8000-000012345678", Some(" ")),
            ("55555555-0000-4000-8000-000012345678", None),
        ];
        let attention: Vec<_> = reports
            .iter()
            .map(|(id, title)| {
                json!({
                    "kind": "dispatch_waiting_report", "dispatch_id": id, "task_title": title,
                    "worker_id": "reviewer", "worker_name": "Same reviewer", "submitted_at": 100,
                })
            })
            .collect();
        let snapshot: ActionCenter = serde_json::from_value(json!({
            "workspace_id": "workspace", "generated_at": 1000, "attention": attention,
            "recent_dispatch_messages":[],
        }))
        .unwrap();
        let rows = snapshot.entries(str::to_owned);
        let untitled = super::tr("未命名派单");
        assert_eq!(
            rows.iter()
                .map(|entry| entry.subject.clone())
                .collect::<Vec<_>>(),
            [
                "Review billing · Same reviewer".to_owned(),
                "ccccdddd · Audit migration · Same reviewer".to_owned(),
                "eeeeffff · Audit migration · Same reviewer".to_owned(),
                format!("{} · {untitled} · Same reviewer", reports[3].0),
                format!("{} · {untitled} · Same reviewer", reports[4].0),
            ]
        );
        for (entry, (dispatch, _)) in rows.iter().zip(reports) {
            assert!(matches!(&entry.target, Destination::Dispatch(id) if id == dispatch));
            assert_eq!(entry.snooze_key, Some(format!("report:{dispatch}")));
            assert!(matches!(&entry.wake, Some(Wake::Dispatch { id, .. }) if id == dispatch));
        }
    }

    #[test]
    fn archive_allows_stopped_member_history_without_pending_work() {
        let mut activity = MemberDesktopActivity {
            runtime_state: "stopped".into(),
            ..Default::default()
        };
        assert!(ArchiveInboxSession::settled_activity(&activity));
        activity.queued_input_count = 1;
        assert!(!ArchiveInboxSession::settled_activity(&activity));
        activity.queued_input_count = 0;
        activity.pending_report_count = 1;
        assert!(!ArchiveInboxSession::settled_activity(&activity));
        activity.pending_report_count = 0;
        activity.runtime_state = "working".into();
        assert!(!ArchiveInboxSession::settled_activity(&activity));
    }

    #[test]
    fn inbox_archive_confirmation_is_bound_to_workspace_session_and_displayed_reply() {
        let at = |workspace: &str, id: &str, revision| ArchiveInboxSession {
            session: serde_json::from_value(json!({
                "id": id, "workspaceId": workspace, "title": "Same conversation",
            }))
            .unwrap(),
            revision,
        };
        let clicked = at("beacon", "id", 4);
        assert!(clicked.matches(&at("beacon", "id", 4)));
        assert!(!clicked.matches(&at("beacon", "id", 5)));
        assert!(!clicked.matches(&at("atlas", "id", 4)));
        assert!(!clicked.matches(&at("beacon", "other", 4)));
    }

    #[test]
    fn inbox_mark_read_rejects_stale_or_actionable_rows_and_clears_only_the_exact_unread() {
        let mark = MarkInboxRead {
            workspace: "beacon".into(),
            session: "beacon-chat".into(),
            sequence: Some(7),
            reply_revision: 0,
            deferred: false,
        };
        let session = |workspace: &str, id: &str| {
            serde_json::from_value(json!({
                "id": id,
                "workspaceId": workspace,
                "title": "Same title",
            }))
            .unwrap()
        };
        let mut entry = Entry {
            label: UNREAD_CONVERSATION,
            subject: "Same title".into(),
            target: Destination::Session(session("beacon", "beacon-chat")),
            dispatch_backend: None,
            related_members: [None, None],
            snooze_key: None,
            wake: Some(Wake::UnreadReply { revision: 0 }),
            priority: Priority::Unread,
            updated_at: 0,
            activity_at: 0,
        };
        assert!(mark.matches(&entry));
        entry.wake = Some(Wake::UnreadReply { revision: 1 });
        assert!(!mark.matches(&entry));
        entry.wake = Some(Wake::UnreadReply { revision: 0 });
        for label in ["等待你的操作", "投递失败", "待处理的汇报"] {
            entry.label = label;
            assert!(!mark.matches(&entry));
        }
        entry.label = UNREAD_CONVERSATION;
        entry.target = Destination::Session(session("atlas", "beacon-chat"));
        assert!(!mark.matches(&entry));
        entry.target = Destination::Session(session("beacon", "different-chat"));
        assert!(!mark.matches(&entry));

        let mut unread = HashSet::from(["beacon-chat".into(), "different-chat".into()]);
        assert!(!mark.consume(&mut unread, Some(8)));
        assert_eq!(unread.len(), 2);
        assert!(mark.consume(&mut unread, Some(7)));
        assert_eq!(unread, HashSet::from(["different-chat".into()]));
        unread.insert("beacon-chat".into());
        assert!(!mark.consume(&mut unread, Some(8)));
        assert!(unread.contains("beacon-chat"));
    }

    #[test]
    fn undo_restores_the_exact_reminder_in_all_three_directions() {
        let root = tempfile::tempdir().unwrap();
        let project = uuid::Uuid::new_v4().to_string();
        let key = "message:question";
        let now = now_ms();
        let wake = Some(Wake::Dispatch {
            id: "dispatch".into(),
            sequence: Some(7),
            observed_at: 0,
        });
        let mut reminders = Reminders {
            saved: Some(Snoozes::load(root.path(), &project).unwrap()),
            ..Reminders::default()
        };
        let edit = |previous| SnoozeEdit {
            workspace: project.clone(),
            key: key.into(),
            wake: wake.clone(),
            previous,
        };
        let read = || {
            Snoozes::load(root.path(), &project)
                .unwrap()
                .active_reminder(key, now)
        };
        let original = Delay::Until(now + 500_001);
        let change = reminders
            .set(root.path(), &edit(None), Some(original))
            .unwrap()
            .unwrap();
        let saved = original.reminder(now, &wake).unwrap();
        assert_eq!(read(), Some(saved.clone()));
        reminders.restore(root.path(), &change).unwrap();
        assert_eq!(read(), None);

        reminders
            .set(root.path(), &edit(None), Some(original))
            .unwrap();
        let change = reminders
            .set(root.path(), &edit(Some(saved.clone())), None)
            .unwrap()
            .unwrap();
        reminders.restore(root.path(), &change).unwrap();
        assert_eq!(read(), Some(saved.clone()));

        let change = reminders
            .set(
                root.path(),
                &edit(Some(saved.clone())),
                Some(Delay::Until(now + 900_000)),
            )
            .unwrap()
            .unwrap();
        reminders.restore(root.path(), &change).unwrap();
        assert_eq!(read(), Some(saved));
        assert!(reminders.restore(root.path(), &change).is_err());
    }

    #[test]
    fn reschedule_rejects_new_wake_signals_on_the_same_row() {
        let wake = |sequence| {
            Some(Wake::Dispatch {
                id: "dispatch".into(),
                sequence: Some(sequence),
                observed_at: 0,
            })
        };
        let mut entry = Entry {
            label: "",
            subject: "".into(),
            target: Destination::Dispatch("dispatch".into()),
            dispatch_backend: None,
            related_members: [None, None],
            snooze_key: Some("message:question".into()),
            wake: wake(7),
            priority: Priority::NeedsUser,
            updated_at: 0,
            activity_at: 0,
        };
        let edit = SnoozeEdit {
            workspace: "project".into(),
            key: "message:question".into(),
            wake: wake(7),
            previous: None,
        };
        assert!(edit.matches(&entry));
        entry.wake = wake(8);
        assert!(!edit.matches(&entry));
    }

    #[test]
    fn user_actions_precede_unread_and_active_across_projects() {
        let row = |priority, updated_at, id: &str| Entry {
            label: "",
            subject: id.into(),
            target: Destination::Dispatch(id.into()),
            dispatch_backend: None,
            related_members: [None, None],
            snooze_key: None,
            wake: None,
            priority,
            updated_at,
            activity_at: updated_at,
        };
        let mut rows = [
            ("A", row(Priority::Active, 100, "working")),
            ("B", row(Priority::Unread, 90, "unread")),
            ("C", row(Priority::NeedsUser, 1, "older action")),
            ("A", row(Priority::NeedsUser, 2, "newer action")),
        ];
        sort_entries(&mut rows);
        assert_eq!(
            rows.map(|(_, entry)| entry.subject),
            ["newer action", "older action", "unread", "working"],
        );
    }

    #[test]
    fn an_undelivered_question_keeps_its_exact_snooze_key_after_delivery() {
        let parse = |kind, id| {
            serde_json::from_value::<Attention>(json!({"kind": kind, "message_id": id}))
                .unwrap()
                .snooze_key()
        };
        assert_eq!(
            parse("message_delivery_pending", "original"),
            parse("question_waiting_answer", "original")
        );
        assert_ne!(
            parse("question_waiting_answer", "original"),
            parse("question_waiting_answer", "new")
        );
        assert!(parse("remote_error", "original").is_none());
    }
}
