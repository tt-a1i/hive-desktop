use super::super::{Destination, Entry, Priority, snoozes::Snoozes};
use super::{GlobalInbox, now_ms};
use crate::app::HiveApp;
use gpui_kit::*;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
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
}

impl BadgeItem {
    fn workspace(&self) -> &str {
        match self {
            Self::Question { workspace, .. } | Self::UnreadSession { workspace, .. } => workspace,
        }
    }

    pub(super) fn from_entry(workspace: &str, entry: &Entry, app: &HiveApp) -> Option<Self> {
        match &entry.target {
            Destination::Dispatch(dispatch) if entry.priority == Priority::NeedsUser => {
                let message = entry.snooze_key.as_deref()?.strip_prefix("message:")?;
                Some(Self::Question {
                    workspace: workspace.into(),
                    dispatch: dispatch.clone(),
                    message: message.into(),
                })
            }
            Destination::Session(session)
                if entry.priority == Priority::Unread && app.unread.contains(&session.id) =>
            {
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

fn collect_questions(
    root: &Path,
    scope: &[String],
    questions: Vec<Question>,
    now: u64,
) -> HashSet<BadgeItem> {
    let scope: HashSet<_> = scope.iter().map(String::as_str).collect();
    let mut reminders = HashMap::new();
    questions
        .into_iter()
        .filter_map(|question| {
            if !scope.contains(question.workspace_id.as_str())
                || uuid::Uuid::try_parse(&question.dispatch_id).is_err()
                || question.message_id.is_empty()
            {
                return None;
            }
            let snoozes = reminders
                .entry(question.workspace_id.clone())
                .or_insert_with(|| Snoozes::load(root, &question.workspace_id).ok());
            let snoozes = snoozes.as_ref()?;
            (!snoozes.active(&format!("message:{}", question.message_id), now)).then_some(
                BadgeItem::Question {
                    workspace: question.workspace_id,
                    dispatch: question.dispatch_id,
                    message: question.message_id,
                },
            )
        })
        .collect()
}

#[derive(Default)]
pub(in crate::collaboration) struct InboxBadge {
    connection: u64,
    pub(super) dialog: Option<WeakEntity<GlobalInbox>>,
    pending: HashSet<BadgeItem>,
    seen: HashSet<BadgeItem>,
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

    pub(super) fn acknowledge(&mut self, items: impl IntoIterator<Item = BadgeItem>) -> bool {
        let before = self.seen.len();
        self.seen.extend(items);
        self.seen.len() != before
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
        let current = collect_questions(root.path(), &scope, pending(), 19);
        assert_eq!(current.len(), 1);
        assert!(current.contains(&BadgeItem::Question {
            workspace: workspace.into(),
            dispatch: dispatch.into(),
            message: "new".into()
        }));
        assert_eq!(
            collect_questions(root.path(), &scope, pending(), 20).len(),
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
        let active_project = |id: &str| {
            self.workspace_removing.as_deref() != Some(id)
                && self.workspaces.iter().any(|project| project.id == id)
        };
        badge
            .pending
            .iter()
            .any(|item| active_project(item.workspace()) && !badge.sees(item))
            || (!self.unread.is_empty()
                && self
                    .sessions
                    .iter()
                    .chain(self.project_sessions.values().flatten())
                    .any(|session| {
                        if !self.unread.contains(&session.id)
                            || !active_project(&session.workspace_id)
                        {
                            return false;
                        }
                        let interactions =
                            self.inbox_interactions(std::slice::from_ref(session), &self.members);
                        self.inbox_session_entry(session, &self.members, &interactions, false)
                            .and_then(|entry| {
                                BadgeItem::from_entry(&session.workspace_id, &entry, self)
                            })
                            .is_some_and(|item| !badge.sees(&item))
                    }))
    }

    pub(crate) fn start_global_inbox_badge(cx: &mut Context<Self>) {
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
                    badge.seen.retain(|item| {
                        active.iter().any(|id| id == item.workspace())
                            && match item {
                                BadgeItem::UnreadSession { session, .. } => {
                                    app.unread.contains(session)
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
                            let fresh = result.map(|questions| {
                                collect_questions(root, &scope, questions, now_ms())
                            });
                            app.collaboration.inbox_badge.replace(&scope, fresh);
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
