use super::{ActionCenter, Reminder, SnoozeEdit, Wake, global::read_list, read_action_center};
use crate::{
    app::HiveApp,
    language::tr,
    palette::Color,
    protocol::{Member, Session},
    transport::Runtime,
};
use gpui_kit::{
    component::{button::Button, *},
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};
use uuid::Uuid;

const CHECK_FAILED: &str = "无法核对事项以撤销暂缓。";
pub(super) const UNDO_CHANGED: &str = "事项或暂缓状态已变化，无法撤销。";
pub(super) const UNDO_WINDOW: Duration = Duration::from_secs(8);

#[derive(Clone)]
pub(super) struct UndoChange {
    pub(super) edit: SnoozeEdit,
    pub(super) after: Option<Reminder>,
}

impl UndoChange {
    fn label(&self) -> &'static str {
        match (&self.edit.previous, &self.after) {
            (None, _) => "事项已暂缓",
            (_, None) => "已取消暂缓",
            _ => "已调整暂缓时间",
        }
    }

    pub(super) fn valid_at(&self, now: u64) -> bool {
        [&self.edit.previous, &self.after]
            .into_iter()
            .all(|reminder| {
                reminder
                    .as_ref()
                    .is_none_or(|reminder| reminder.expires_at.is_none_or(|until| until > now))
            })
    }
}

#[derive(Default)]
pub(super) struct UndoSlot {
    pending: Option<(Instant, UndoChange)>,
    checking: bool,
    message: Option<&'static str>,
    generation: Option<Uuid>,
}

impl UndoSlot {
    pub(super) fn offer(&mut self, change: UndoChange) -> Uuid {
        self.clear();
        self.pending = Some((Instant::now(), change));
        let generation = Uuid::new_v4();
        self.generation = Some(generation);
        generation
    }

    pub(super) fn expire(&mut self, generation: Uuid) -> bool {
        if self.generation == Some(generation)
            && !self.checking
            && (self.pending.is_some() || self.message.is_some())
        {
            self.clear();
            return true;
        }
        false
    }

    pub(super) fn claim(&mut self) -> Option<(Uuid, UndoChange)> {
        let generation = self.generation?;
        let (_, change) = self
            .pending
            .take()
            .filter(|(issued, _)| issued.elapsed() < UNDO_WINDOW)?;
        self.checking = true;
        Some((generation, change))
    }

    pub(super) fn is_checking(&self, generation: Uuid) -> bool {
        self.checking && self.generation == Some(generation)
    }

    pub(super) fn finish(&mut self, result: Result<(), &'static str>) -> Uuid {
        self.clear();
        self.message = Some(result.err().unwrap_or("已撤销暂缓变更"));
        let generation = Uuid::new_v4();
        self.generation = Some(generation);
        generation
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn render<T: 'static>(
        &self,
        cx: &Context<T>,
        undo: impl Fn(&mut T, &mut Context<T>) + 'static,
    ) -> Option<AnyElement> {
        let current = self
            .pending
            .as_ref()
            .filter(|(issued, _)| issued.elapsed() < UNDO_WINDOW);
        let label = if self.checking {
            "正在核对暂缓状态…"
        } else if let Some((_, change)) = current {
            change.label()
        } else {
            self.message?
        };
        let label = tr(label);
        Some(
            h_flex()
                .gap_2()
                .justify_between()
                .child(
                    div()
                        .id("snooze-undo-status")
                        .role(Role::Status)
                        .aria_label(label)
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(label),
                )
                .when(current.is_some(), |row| {
                    row.child(
                        Button::new("undo-inbox-snooze")
                            .outline()
                            .compact()
                            .xsmall()
                            .label(tr("撤销"))
                            .accessibility_label(tr("撤销收件箱变更"))
                            .on_click(cx.listener(move |this, _, _, cx| undo(this, cx))),
                    )
                })
                .into_any_element(),
        )
    }
}

pub(super) fn expire_later<T: 'static>(
    cx: &mut Context<T>,
    generation: Uuid,
    slot: impl Fn(&mut T) -> &mut UndoSlot + 'static,
) {
    cx.spawn(async move |this, cx| {
        smol::Timer::after(UNDO_WINDOW).await;
        let _ = this.update(cx, |this, cx| {
            if slot(this).expire(generation) {
                cx.notify();
            }
        });
    })
    .detach();
}

pub(super) enum VerifiedUndo {
    Dispatch,
    Session {
        sessions: Vec<Session>,
        members: Vec<Member>,
    },
}

impl VerifiedUndo {
    pub(super) async fn read(runtime: &Runtime, change: &UndoChange) -> Result<Self, &'static str> {
        smol::future::race(Self::read_current(runtime, change), async {
            smol::Timer::after(Duration::from_secs(5)).await;
            Err(CHECK_FAILED)
        })
        .await
    }

    async fn read_current(runtime: &Runtime, change: &UndoChange) -> Result<Self, &'static str> {
        let edit = &change.edit;
        match edit.wake.as_ref() {
            Some(Wake::Dispatch {
                id,
                sequence,
                observed_at,
            }) => {
                let (snapshot, history) = smol::future::zip(
                    read_action_center(runtime, &edit.workspace, None),
                    has_new_dispatch_input(runtime, &edit.workspace, id, *sequence, *observed_at),
                )
                .await;
                let snapshot = snapshot.map_err(|_| CHECK_FAILED)?;
                if history? || !contains_dispatch_row(edit, id, &snapshot) {
                    return Err(UNDO_CHANGED);
                }
                Ok(Self::Dispatch)
            }
            Some(Wake::Session { .. } | Wake::UnreadReply { .. }) => {
                let (sessions, members) = smol::future::zip(
                    read_list(runtime, "chat.list", &edit.workspace),
                    read_list(runtime, "team.members", &edit.workspace),
                )
                .await;
                Ok(Self::Session {
                    sessions: sessions.map_err(|_| CHECK_FAILED)?,
                    members: members.map_err(|_| CHECK_FAILED)?,
                })
            }
            None => Err(UNDO_CHANGED),
        }
    }

    pub(super) fn matches(&self, edit: &SnoozeEdit, app: &HiveApp) -> bool {
        match self {
            Self::Dispatch => true,
            Self::Session { sessions, members } => {
                let deferred_unread = matches!(edit.wake, Some(Wake::UnreadReply { .. }));
                let prefix = if deferred_unread {
                    "unread-session:"
                } else {
                    "session:"
                };
                let Some(id) = edit.key.strip_prefix(prefix) else {
                    return false;
                };
                let Some(session) = sessions
                    .iter()
                    .find(|session| session.id == id && session.workspace_id == edit.workspace)
                else {
                    return false;
                };
                let interactions = app.inbox_interactions(std::slice::from_ref(session), members);
                app.inbox_session_entry(session, members, &interactions, false, deferred_unread)
                    .is_some_and(|entry| edit.matches(&entry))
            }
        }
    }
}

fn contains_dispatch_row(edit: &SnoozeEdit, dispatch: &str, snapshot: &ActionCenter) -> bool {
    snapshot.attention.iter().any(|item| {
        item.dispatch_id.as_deref() == Some(dispatch)
            && item.snooze_key().as_ref() == Some(&edit.key)
    })
}

#[derive(Deserialize)]
struct DispatchPage {
    messages: Vec<super::RecentMessage>,
    next_after_seq: Option<u64>,
}

impl DispatchPage {
    fn has_new_input(
        &self,
        workspace: &str,
        dispatch: &str,
        cursor: u64,
        sequence: Option<u64>,
        observed_at: u64,
    ) -> Result<bool, &'static str> {
        if self.messages.iter().any(|message| {
            message.workspace_id != workspace
                || message.dispatch_id != dispatch
                || message.sequence <= cursor
        }) {
            return Err(CHECK_FAILED);
        }
        Ok(self.messages.iter().any(|message| {
            message.delivery_state != "cancelled"
                && (sequence.is_some() || message.created_at > observed_at)
        }))
    }
}

async fn has_new_dispatch_input(
    runtime: &Runtime,
    workspace: &str,
    dispatch: &str,
    sequence: Option<u64>,
    observed_at: u64,
) -> Result<bool, &'static str> {
    let workspace_part: String =
        url::form_urlencoded::byte_serialize(workspace.as_bytes()).collect();
    let dispatch_part: String = url::form_urlencoded::byte_serialize(dispatch.as_bytes()).collect();
    let mut cursor = sequence.unwrap_or(0);
    // The exact dispatch route is independent of the workspace's recent-preview cap.
    // If enough pages are inconclusive, keep the current state.
    for _ in 0..4 {
        let path = format!(
            "/api/ui/workspaces/{workspace_part}/dispatches/{dispatch_part}/messages?after_seq={cursor}"
        );
        let value = runtime
            .request("hive.request", json!({"method":"GET", "path":path}))
            .await
            .map_err(|_| CHECK_FAILED)?;
        let page: DispatchPage = serde_json::from_value(value).map_err(|_| CHECK_FAILED)?;
        if page.has_new_input(workspace, dispatch, cursor, sequence, observed_at)? {
            return Ok(true);
        }
        let Some(next) = page.next_after_seq else {
            return Ok(false);
        };
        if next <= cursor {
            return Err(CHECK_FAILED);
        }
        cursor = next;
    }
    Err(CHECK_FAILED)
}

#[cfg(test)]
mod tests {
    use super::{DispatchPage, SnoozeEdit, UNDO_WINDOW, UndoChange, UndoSlot};
    use serde_json::json;
    use std::time::Instant;

    #[test]
    fn an_undo_can_be_claimed_once_and_old_claims_do_not_cross_scope_resets() {
        let change = || UndoChange {
            edit: SnoozeEdit {
                workspace: "workspace".into(),
                key: "message:a".into(),
                wake: None,
                previous: None,
            },
            after: None,
        };
        let mut slot = UndoSlot::default();
        let first = slot.offer(change());
        assert_eq!(slot.claim().unwrap().0, first);
        assert!(slot.claim().is_none());
        slot.clear();
        let next = slot.offer(change());
        assert!(!slot.expire(first));
        slot.pending.as_mut().unwrap().0 = Instant::now() - UNDO_WINDOW;
        assert!(slot.claim().is_none());
        assert!(!slot.is_checking(next));
    }

    #[test]
    fn fresh_exact_dispatch_input_blocks_undo_even_without_a_saved_reminder() {
        let page = |workspace: &str, delivery: &str| {
            serde_json::from_value::<DispatchPage>(json!({
                "messages": [{"workspace_id": workspace, "dispatch_id": "dispatch", "id": "fresh", "sequence": 8,
                "from_agent_id": "agent", "text": "new ask", "created_at": 40, "delivery_state": delivery}],
                "next_after_seq": null
            })).unwrap()
        };
        assert!(
            page("beacon", "delivered")
                .has_new_input("beacon", "dispatch", 7, Some(7), 0)
                .unwrap()
        );
        assert!(
            !page("beacon", "cancelled")
                .has_new_input("beacon", "dispatch", 7, Some(7), 0)
                .unwrap()
        );
        assert!(
            !page("beacon", "delivered")
                .has_new_input("beacon", "dispatch", 0, None, 40)
                .unwrap()
        );
        assert!(
            page("beacon", "delivered")
                .has_new_input("beacon", "dispatch", 0, None, 39)
                .unwrap()
        );
        assert!(
            page("atlas", "delivered")
                .has_new_input("beacon", "dispatch", 7, Some(7), 0)
                .is_err()
        );
    }
}
