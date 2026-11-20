#[path = "deadline.rs"]
mod deadline;

use self::deadline::DeadlineWake;
use super::super::{
    ActionCenter, Destination, Entry, Priority, SnoozeEdit, Wake, snoozes::Snoozes,
};
use super::{GlobalInbox, now_ms};
use crate::app::HiveApp;
use gpui_kit::*;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
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
}

impl BadgeItem {
    pub(super) fn workspace(&self) -> &str {
        match self {
            Self::Question { workspace, .. } | Self::UnreadSession { workspace, .. } => workspace,
        }
    }

    pub(super) fn from_entry(workspace: &str, entry: &Entry, unread: bool) -> Option<Self> {
        match &entry.target {
            Destination::Dispatch(dispatch) if entry.priority == Priority::NeedsUser => {
                let message = entry.snooze_key.as_deref()?.strip_prefix("message:")?;
                Some(Self::Question {
                    workspace: workspace.into(),
                    dispatch: dispatch.clone(),
                    message: message.into(),
                })
            }
            Destination::Session(session) if entry.priority == Priority::Unread && unread => {
                Some(Self::UnreadSession {
                    workspace: workspace.into(),
                    session: session.id.clone(),
                })
            }
            _ => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Question {
    workspace_id: String,
    dispatch_id: String,
    message_id: String,
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

#[derive(Default)]
pub(in crate::collaboration) struct InboxBadge {
    connection: u64,
    pub(super) dialog: Option<WeakEntity<GlobalInbox>>,
    pending: HashSet<BadgeItem>,
    seen: HashSet<BadgeItem>,
    reminders: HashMap<String, Snoozes>,
    wake: DeadlineWake,
    live_replies: HashMap<String, HashMap<String, u64>>,
}

impl InboxBadge {
    fn is_open(&self, cx: &App) -> bool {
        self.dialog
            .as_ref()
            .and_then(WeakEntity::upgrade)
            .is_some_and(|dialog| !dialog.read(cx).dismissed)
    }

    fn replace(&mut self, scope: &[String], fresh: Option<HashSet<BadgeItem>>) {
        self.pending
            .retain(|item| !scope.iter().any(|id| id == item.workspace()));
        if let Some(fresh) = fresh {
            self.pending.extend(fresh);
        }
    }

    pub(super) fn sees(&self, item: &BadgeItem) -> bool {
        self.seen.contains(item)
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

    pub(super) fn acknowledge(&mut self, items: impl IntoIterator<Item = BadgeItem>) -> bool {
        let before = self.seen.len();
        self.seen.extend(items);
        self.seen.len() != before
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
            .filter_map(|item| {
                (item.kind == "question_waiting_answer").then_some(())?;
                Some(Question {
                    workspace_id: snapshot.workspace_id.clone(),
                    dispatch_id: item.dispatch_id.clone()?,
                    message_id: item.message_id.clone()?,
                })
            })
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

    fn question_visible(&self, item: &BadgeItem, now: u64) -> bool {
        let BadgeItem::Question {
            workspace, message, ..
        } = item
        else {
            return false;
        };
        self.reminders
            .get(workspace)
            .is_some_and(|saved| !saved.active(&format!("message:{message}"), now))
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
    ) {
        let latest = self
            .live_replies
            .entry(workspace.to_owned())
            .or_default()
            .entry(session.to_owned())
            .or_default();
        *latest = (*latest).max(revision);
    }

    pub(in crate::collaboration::inbox) fn reply_revision(
        &self,
        workspace: &str,
        session: &str,
    ) -> u64 {
        self.live_replies
            .get(workspace)
            .and_then(|sessions| sessions.get(session))
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::{BadgeItem, InboxBadge, Question, collect_questions};
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
    fn viewing_one_project_does_not_acknowledge_another_or_a_later_question() {
        let scope = ["atlas".into(), "beacon".into()];
        let old = item("beacon", "old");
        let other = item("atlas", "old");
        let new = item("beacon", "new");
        let mut badge = InboxBadge::default();
        badge.replace(&scope, Some(HashSet::from([old.clone(), other.clone()])));
        assert!(badge.acknowledge([old.clone()]));
        assert!(badge.sees(&old));
        assert!(!badge.sees(&other));
        badge.replace(
            &scope,
            Some(HashSet::from([old.clone(), other.clone(), new.clone()])),
        );
        assert!(badge.sees(&old));
        assert!(!badge.sees(&new));
        badge.replace(&["beacon".into()], None);
        assert_eq!(badge.pending, HashSet::from([other]));
        assert!(badge.sees(&old));
        badge.replace(&["beacon".into()], Some(HashSet::new()));
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
        assert!(badge.acknowledge([atlas.clone(), beacon.clone(), question.clone()]));
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
            .filter(|item| badge.question_visible(item, 19))
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
                .filter(|item| badge.question_visible(item, 20))
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
                    && badge.question_visible(item, now_ms())
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
                    Some((runtime, app.connection_generation, scope))
                }) else {
                    break;
                };
                if let Some((runtime, connection, scope)) = plan {
                    let result = runtime
                        .request("inbox.actionableQuestions", json!({"workspaceIds": scope}))
                        .await
                        .ok()
                        .and_then(|value| Vec::<Question>::deserialize(value).ok());
                    if this
                        .update(cx, |app, cx| {
                            if !app.connected
                                || app.restarting
                                || app.closing
                                || app.connection_generation != connection
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
                            let fresh =
                                result.map(|questions| collect_questions(&scope, questions));
                            for workspace in &scope {
                                if let Ok(saved) = Snoozes::load(root, workspace) {
                                    app.collaboration
                                        .inbox_badge
                                        .remember_reminders(workspace, &saved);
                                }
                            }
                            app.collaboration.inbox_badge.replace(&scope, fresh);
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
