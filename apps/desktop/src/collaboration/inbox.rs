use super::read_dispatch;
use crate::{
    app::{ErrorNotice, HiveApp},
    focus_scroll::Reveal,
    language::tr,
    palette::Color,
    protocol::Session,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct Inbox {
    workspace: Option<String>,
    generation: u64,
    connection: u64,
    loading: bool,
    loaded: bool,
    attention: Vec<Attention>,
    recent_messages: Vec<RecentMessage>,
    error: Option<String>,
    snoozed: HashSet<(String, String)>,
    show_snoozed: bool,
}

impl Inbox {
    pub(super) fn invalidate(&mut self) {
        self.generation += 1;
        self.loading = false;
    }
}

#[derive(Deserialize)]
struct ActionCenter {
    workspace_id: String,
    attention: Vec<Attention>,
    recent_dispatch_messages: Vec<RecentMessage>,
}

#[derive(Deserialize)]
struct RecentMessage {
    id: String,
    from_agent_id: String,
    text: String,
}

#[derive(Deserialize)]
struct Attention {
    kind: String,
    dispatch_id: Option<String>,
    message_id: Option<String>,
    worker_id: Option<String>,
    worker_name: Option<String>,
}

enum Destination {
    Session(String),
    Dispatch(String),
    Member(String),
}

struct Entry {
    label: &'static str,
    subject: String,
    target: Destination,
    snooze_key: Option<String>,
}

impl Attention {
    fn snooze_key(&self) -> Option<String> {
        match self.kind.as_str() {
            // A queued question may later become eligible for an answer. Keep
            // the same message snoozed through its delivery status change.
            "question_waiting_answer" | "message_delivery_pending" => {
                self.message_id.as_ref().map(|id| format!("message:{id}"))
            }
            "dispatch_waiting_report" => self.dispatch_id.as_ref().map(|id| format!("report:{id}")),
            _ => None,
        }
    }

    fn entry(&self, app: &HiveApp, recent_messages: &[RecentMessage]) -> Option<Entry> {
        let label = match self.kind.as_str() {
            "question_waiting_answer" => "等待答复的协作问题",
            "message_delivery_pending" => "等待投递的协作消息",
            "dispatch_waiting_report" => "等待汇报的派单",
            "stopped_with_queue" => "成员已停止，仍有待办",
            _ => return None,
        };
        let target = match &self.dispatch_id {
            Some(id) if uuid::Uuid::try_parse(id).is_ok() => Destination::Dispatch(id.clone()),
            None if self.kind == "stopped_with_queue" => {
                Destination::Member(self.worker_id.clone()?)
            }
            _ => return None,
        };
        let mut subject = self
            .worker_name
            .clone()
            .or_else(|| self.worker_id.as_ref().map(|id| app.agent_name(id)))
            .unwrap_or_default();
        if let Some(id) = &self.message_id {
            subject =
                if let Some(message) = recent_messages.iter().find(|message| &message.id == id) {
                    let preview: String = message
                        .text
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .chars()
                        .take(120)
                        .collect();
                    format!("{} · {preview}", app.agent_name(&message.from_agent_id))
                } else {
                    let short_id: String = id.chars().take(8).collect();
                    format!("{subject} · {short_id}")
                };
        }
        Some(Entry {
            label,
            subject,
            target,
            snooze_key: self.snooze_key(),
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
        if inbox.connection != connection {
            inbox.invalidate();
            inbox.connection = connection;
        }
        if inbox.workspace.as_ref() != Some(&workspace) {
            inbox.invalidate();
            inbox.workspace = Some(workspace.clone());
            inbox.attention.clear();
            inbox.recent_messages.clear();
            inbox.error = None;
            inbox.show_snoozed = false;
            inbox.loaded = false;
        }
        if inbox.loading {
            return;
        }
        inbox.loading = true;
        inbox.generation += 1;
        let generation = inbox.generation;
        cx.spawn(async move |this, cx| {
            let encoded: String = url::form_urlencoded::byte_serialize(workspace.as_bytes()).collect();
            let result = runtime
                .request("hive.request", json!({"method":"GET","path":format!("/api/ui/workspaces/{encoded}/action-center")}))
                .await
                .and_then(|value| serde_json::from_value::<ActionCenter>(value).map_err(|error| error.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() != Some(&workspace) || this.connection_generation != connection {
                    return;
                }
                let inbox = &mut this.collaboration.inbox;
                if !this.collaboration.visible || inbox.generation != generation {
                    return;
                }
                inbox.loading = false;
                match result {
                    Ok(snapshot) if snapshot.workspace_id == workspace => {
                        let active: HashSet<_> = snapshot.attention.iter().filter_map(Attention::snooze_key).collect();
                        inbox.snoozed.retain(|(id, key)| id != &workspace || active.contains(key));
                        inbox.attention = snapshot.attention;
                        inbox.recent_messages = snapshot.recent_dispatch_messages;
                        inbox.error = None;
                        inbox.loaded = true;
                    }
                    Ok(_) => inbox.error = Some(tr("收件箱返回了其他工作区的数据。").to_owned()),
                    Err(error) => inbox.error = Some(error),
                }
                cx.notify();
            });
        }).detach();
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

    fn inbox_sessions(&self) -> impl Iterator<Item = (&Session, &str, &'static str)> {
        self.sessions
            .iter()
            .filter(move |session| self.workspace.as_deref() == Some(session.workspace_id.as_str()))
            .filter_map(move |session| {
                let member = session.display_agent_id()?;
                let activity = self
                    .members
                    .iter()
                    .find(|entry| entry.id == member)
                    .and_then(|entry| entry.desktop_activity.as_ref())
                    .filter(|activity| activity.session_id.as_ref() == Some(&session.id));
                let label = if self.interaction_count(&session.id) > 0
                    || activity.is_some_and(|activity| activity.pending_interaction_count > 0)
                {
                    "等待你的操作"
                } else if activity
                    .is_some_and(|activity| activity.delivery_failure_message.is_some())
                {
                    "投递失败"
                } else if activity.is_some_and(|activity| activity.pending_report_count > 0) {
                    "待处理的汇报"
                } else if self.unread.contains(&session.id) {
                    "未读成员会话"
                } else {
                    return None;
                };
                Some((session, member, label))
            })
    }

    pub(super) fn inbox_session_count(&self) -> usize {
        self.inbox_sessions().count()
    }

    pub(super) fn render_collaboration_inbox(
        &self,
        scroll: &ScrollHandle,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let state = &self.collaboration.inbox;
        let current = self.workspace.is_some() && state.workspace == self.workspace;
        let mut entries: Vec<_> = self
            .inbox_sessions()
            .map(|(session, member, label)| Entry {
                label,
                subject: format!(
                    "{} · {}",
                    self.agent_name(member),
                    self.session_title(session)
                ),
                target: Destination::Session(session.id.clone()),
                snooze_key: None,
            })
            .collect();
        if current {
            entries.extend(
                state
                    .attention
                    .iter()
                    .filter_map(|attention| attention.entry(self, &state.recent_messages)),
            );
        }
        let is_snoozed = |entry: &Entry| {
            self.workspace
                .as_ref()
                .zip(entry.snooze_key.as_ref())
                .is_some_and(|(workspace, key)| {
                    state.snoozed.contains(&(workspace.clone(), key.clone()))
                })
        };
        let snoozed_count = entries.iter().filter(|entry| is_snoozed(entry)).count();
        entries.retain(|entry| state.show_snoozed || !is_snoozed(entry));
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
                    .when(snoozed_count > 0, |header| {
                        header.child(
                            Button::new("toggle-snoozed-inbox")
                                .ghost()
                                .compact()
                                .xsmall()
                                .label(if state.show_snoozed {
                                    tr("隐藏已暂缓事项")
                                } else {
                                    tr("显示已暂缓事项")
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.collaboration.inbox.show_snoozed =
                                        !this.collaboration.inbox.show_snoozed;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .when(entries.is_empty(), |panel| {
                let status = if current && state.error.is_some() {
                    "无法刷新协作收件箱"
                } else if self.workspace.is_some() && (!current || !state.loaded) {
                    "正在读取收件箱…"
                } else {
                    "暂无需要关注的协作事项"
                };
                panel.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr(status)),
                )
            })
            .when(
                !entries.is_empty() && current && state.error.is_some(),
                |panel| {
                    panel.child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("无法刷新协作收件箱")),
                    )
                },
            )
            .children(entries.into_iter().map(|entry| {
                let label = tr(entry.label);
                let target = self.history_link_target();
                let snoozed = is_snoozed(&entry);
                let row_id = entry
                    .snooze_key
                    .clone()
                    .unwrap_or_else(|| match &entry.target {
                        Destination::Dispatch(id) => format!("dispatch:{id}"),
                        Destination::Session(id) => format!("session:{id}"),
                        Destination::Member(id) => format!("member:{id}"),
                    });
                let snooze = self.workspace.clone().zip(entry.snooze_key);
                let destination = entry.target;
                let action = match destination {
                    Destination::Session(_) => tr("打开会话"),
                    Destination::Dispatch(_) => tr("派单详情"),
                    Destination::Member(_) => tr("团队成员"),
                };
                Reveal::new(
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
                                        .child(entry.subject),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .flex_wrap()
                                .justify_end()
                                .when_some(snooze, |buttons, (workspace, key)| {
                                    buttons.child(
                                        Button::new("snooze-inbox-item")
                                            .ghost()
                                            .compact()
                                            .xsmall()
                                            .label(if snoozed {
                                                tr("取消暂缓")
                                            } else {
                                                tr("暂缓")
                                            })
                                            .accessibility_label(format!(
                                                "{} · {label}",
                                                if snoozed {
                                                    tr("取消暂缓")
                                                } else {
                                                    tr("暂缓")
                                                }
                                            ))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if this.workspace.as_ref() != Some(&workspace) {
                                                    return;
                                                }
                                                let records = &mut this.collaboration.inbox.snoozed;
                                                let record = (workspace.clone(), key.clone());
                                                if !records.remove(&record) {
                                                    records.insert(record);
                                                }
                                                cx.notify();
                                            })),
                                    )
                                })
                                .child(
                                    Button::new("open-inbox-item")
                                        .ghost()
                                        .compact()
                                        .xsmall()
                                        .label(action)
                                        .accessibility_label(format!("{action} · {label}"))
                                        .disabled(!self.connected || self.closing)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if !target
                                                .as_ref()
                                                .is_some_and(|target| target.current(this))
                                            {
                                                return;
                                            }
                                            match &destination {
                                                Destination::Dispatch(id) => {
                                                    this.open_inbox_dispatch(id.clone(), cx)
                                                }
                                                Destination::Session(id) => {
                                                    this.open_member_session(id.clone(), window, cx)
                                                }
                                                Destination::Member(_) => {
                                                    this.show_team(window, cx)
                                                }
                                            }
                                        })),
                                ),
                        ),
                )
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::Attention;
    use serde_json::json;

    #[test]
    fn snooze_tracks_the_same_message_across_delivery_but_allows_fresh_input() {
        let mut row = json!({
            "kind":"message_delivery_pending",
            "dispatch_id":"eecc5d9c-4874-4286-8f73-ea6143246780",
            "message_id":"old-question",
        });
        let decode = |row| {
            serde_json::from_value::<Attention>(row)
                .unwrap()
                .snooze_key()
        };
        let snoozed = decode(row.clone()).unwrap();
        row["kind"] = json!("question_waiting_answer");
        assert_eq!(decode(row.clone()).as_deref(), Some(snoozed.as_str()));
        row["message_id"] = json!("fresh-question");
        assert_ne!(decode(row.clone()).as_deref(), Some(snoozed.as_str()));
        row["kind"] = json!("remote_error");
        assert!(decode(row).is_none());
    }
}
