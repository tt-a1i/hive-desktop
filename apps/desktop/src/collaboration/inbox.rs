mod global;
mod schedule;
mod snoozes;
pub(super) use global::{GlobalInboxView, InboxBadge};

use super::read_dispatch;
use crate::{
    app::{ErrorNotice, HiveApp},
    focus_scroll::Reveal,
    language::{localize_owned_error, tr},
    palette::Color,
    protocol::{Member, Session},
    transport::Runtime,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenuItem},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
use snoozes::{Delay, Reminder, Snoozes, Wake, now_ms};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

#[derive(Default)]
pub(super) struct Inbox {
    workspace: Option<String>,
    generation: u64,
    connection: u64,
    loading: bool,
    action_center: Option<ActionCenter>,
    error: Option<String>,
    reminders: Reminders,
    show_snoozed: bool,
}

impl Inbox {
    pub(super) fn invalidate(&mut self) {
        self.generation += 1;
        self.loading = false;
    }
}

#[derive(Default)]
struct Reminders {
    saved: Option<Snoozes>,
    read_error: Option<io::Error>,
    write_error: Option<io::Error>,
}

const SNOOZE_CHANGED: &str = "收件箱事项已变更，请关闭后重新选择。";

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
}

impl Reminders {
    fn reconcile(
        &mut self,
        root: &std::path::Path,
        workspace: &str,
        snapshot: Option<&ActionCenter>,
        interactions: &BTreeMap<String, BTreeSet<String>>,
    ) {
        match Snoozes::load(root, workspace) {
            Ok(saved) => {
                self.read_error = None;
                match saved.reconcile(root, now_ms(), snapshot, interactions) {
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
    ) -> Result<(), &'static str> {
        if self.saved.is_none() {
            return Err("无法读取已暂缓事项");
        }
        let now = now_ms();
        let next = delay
            .map(|delay| delay.reminder(now, &edit.wake))
            .transpose()?;
        match Snoozes::edit(
            root,
            &edit.workspace,
            &edit.key,
            edit.previous.as_ref(),
            next,
            now,
        ) {
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
        self.saved
            .as_ref()?
            .active_reminder(entry.snooze_key.as_deref()?, now)
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
}

impl ActionCenter {
    fn latest_input(&self, dispatch: &str) -> Option<u64> {
        self.recent_dispatch_messages
            .iter()
            .filter(|message| {
                message.dispatch_id == dispatch && message.delivery_state != "cancelled"
            })
            .map(|message| message.sequence)
            .max()
    }

    fn has_new_input(&self, dispatch: &str, sequence: Option<u64>, observed_at: u64) -> bool {
        if let Some(sequence) = sequence {
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
        self.attention
            .iter()
            .filter_map(|attention| attention.entry(self, &mut name))
            .collect()
    }
}

async fn read_action_center(runtime: &Runtime, workspace: &str) -> Result<ActionCenter, String> {
    let encoded: String = url::form_urlencoded::byte_serialize(workspace.as_bytes()).collect();
    let value = runtime
        .request(
            "hive.request",
            json!({"method":"GET","path":format!("/api/ui/workspaces/{encoded}/action-center")}),
        )
        .await?;
    let snapshot: ActionCenter =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    if snapshot.workspace_id != workspace
        || snapshot
            .recent_dispatch_messages
            .iter()
            .any(|message| message.workspace_id != workspace)
    {
        return Err(tr("收件箱返回了其他工作区的数据。").into());
    }
    Ok(snapshot)
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
    submitted_at: Option<u64>,
}

#[derive(Clone)]
enum Destination {
    Session(Session),
    Dispatch(String),
    Member(String),
}

impl Destination {
    fn action(&self) -> &'static str {
        match self {
            Self::Session(_) => "打开会话",
            Self::Dispatch(_) => "派单详情",
            Self::Member(_) => "团队成员",
        }
    }

    fn key(&self) -> String {
        match self {
            Self::Session(session) => format!("session:{}", session.id),
            Self::Dispatch(id) => format!("dispatch:{id}"),
            Self::Member(id) => format!("member:{id}"),
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
    snooze_key: Option<String>,
    wake: Option<Wake>,
    priority: Priority,
    updated_at: u64,
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
            Some(id) if uuid::Uuid::try_parse(id).is_ok() => Destination::Dispatch(id.clone()),
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
        let subject = if let Some(message) = message {
            let preview: String = message
                .text
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(120)
                .collect();
            format!("{} · {preview}", name(&message.from_agent_id))
        } else {
            let member = self
                .worker_name
                .clone()
                .or_else(|| self.worker_id.as_deref().map(name))
                .unwrap_or_default();
            self.message_id.as_ref().map_or_else(
                || member.clone(),
                |id| {
                    let short_id: String = id.chars().take(8).collect();
                    format!("{member} · {short_id}")
                },
            )
        };
        let snooze_key = self.snooze_key();
        let wake = self
            .dispatch_id
            .as_deref()
            .map(|id| snapshot.dispatch_wake(id));
        Some(Entry {
            label,
            subject,
            target,
            snooze_key,
            wake,
            priority,
            updated_at: message
                .map(|message| message.created_at)
                .or(self.submitted_at)
                .unwrap_or(0),
        })
    }
}

impl HiveApp {
    pub(super) fn refresh_collaboration_inbox(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
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
            let result = read_action_center(&runtime, &workspace).await;
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
                    inbox
                        .reminders
                        .reconcile(root, &workspace, snapshot.as_ref(), &interactions);
                }
                if let Some(snapshot) = snapshot {
                    inbox.action_center = Some(snapshot);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_inbox_dispatch(&mut self, id: String, cx: &mut Context<Self>) {
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
            let result = read_dispatch(&runtime, &workspace, &id).await;
            let _ = this.update(cx, |this, cx| {
                if !target.current(this) || this.collaboration.detail_generation != generation {
                    return;
                }
                match result {
                    Ok(dispatch) if dispatch.id == id => this.show_dispatch(dispatch, cx),
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
        } else if self.unread.contains(&session.id) {
            ("未读会话", Priority::Unread)
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
        Some(Entry {
            label,
            subject,
            target: Destination::Session(session.clone()),
            snooze_key: exact_ids.map(|_| format!("session:{}", session.id)),
            wake: exact_ids.map(|ids| Wake::Session {
                interaction_ids: ids.clone(),
            }),
            priority,
            updated_at: session.updated_at.max(0) as u64,
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
                self.inbox_session_entry(session, &self.members, &interactions, false)
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
                self.inbox_session_entry(session, &self.members, &interactions, false)
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
        let mut rows: Vec<_> = self
            .scoped_entries(workspace)
            .into_iter()
            .map(|entry| (workspace, entry))
            .collect();
        sort_entries(&mut rows);
        let now = now_ms();
        let snoozed = |entry: &Entry| current && is_snoozed(entry, &inbox.reminders, now);
        let show_snoozed = current && inbox.show_snoozed;
        let has_snoozed = rows.iter().any(|(_, entry)| snoozed(entry));
        rows.retain(|(_, entry)| show_snoozed || !snoozed(entry));
        let status = if current && inbox.error.is_some() {
            Some("无法刷新协作收件箱")
        } else if rows.is_empty() {
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
            .children(rows.into_iter().map(|(workspace, entry)| {
                let reminder = current
                    .then(|| inbox.reminders.active_reminder(&entry, now))
                    .flatten();
                let workspace = workspace.to_owned();
                let snooze_workspace = workspace.clone();
                let target = entry.target.clone();
                let origin = self.history_link_target();
                let action = Button::new("open-inbox-item").on_click(cx.listener(
                    move |this, _, window, cx| {
                        if !this.connected
                            || this.closing
                            || this.workspace.as_deref() != Some(&workspace)
                            || !origin.as_ref().is_some_and(|origin| origin.current(this))
                        {
                            return;
                        }
                        match &target {
                            Destination::Dispatch(id) => this.open_inbox_dispatch(id.clone(), cx),
                            Destination::Session(session) => {
                                this.open_member_session(session.id.clone(), window, cx)
                            }
                            Destination::Member(_) => this.show_team(window, cx),
                        }
                    },
                ));
                inbox_row(
                    &snooze_workspace,
                    None,
                    entry,
                    reminder,
                    current && inbox.reminders.saved.is_some() && !self.closing,
                    action.disabled(!self.connected || self.closing),
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
                            || !this
                                .scoped_entries(workspace)
                                .iter()
                                .any(|entry| edit.matches(entry))
                        {
                            return Err(SNOOZE_CHANGED);
                        }
                        let root = this.data_dir.as_ref().ok_or("无法保存已暂缓事项")?;
                        let result = this.collaboration.inbox.reminders.set(root, edit, delay);
                        cx.notify();
                        result
                    },
                )
            }))
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
    reminders
        .saved
        .as_ref()
        .zip(entry.snooze_key.as_ref())
        .is_some_and(|(saved, key)| saved.active(key, now))
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

fn inbox_row<T: 'static>(
    workspace: &str,
    project: Option<String>,
    entry: Entry,
    reminder: Option<Reminder>,
    can_snooze: bool,
    open: Button,
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
                    )
                    .when_some(wake_label, |details, wake_label| {
                        details.child(
                            div()
                                .id("inbox-row-wake-time")
                                .role(Role::Label)
                                .aria_label(wake_label.clone())
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(wake_label),
                        )
                    }),
            )
            .child(
                h_flex()
                    .id("inbox-row-actions")
                    .gap_1()
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
                    .child(
                        open.ghost()
                            .compact()
                            .xsmall()
                            .label(action)
                            .accessibility_label(format!("{action} · {label} · {subject}")),
                    ),
            ),
    );
    row.when_some(interaction, |row, state| row.track_focus(&state.focus))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Attention, Destination, Entry, Priority, SnoozeEdit, Wake, sort_entries};
    use serde_json::json;

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
            snooze_key: Some("message:question".into()),
            wake: wake(7),
            priority: Priority::NeedsUser,
            updated_at: 0,
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
            snooze_key: None,
            wake: None,
            priority,
            updated_at,
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
