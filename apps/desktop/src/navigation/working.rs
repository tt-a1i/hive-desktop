use super::*;
use crate::protocol::Session;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const SETTLED_AGE_MS: i64 = 7 * 86_400_000;

#[derive(Default)]
struct Activity {
    busy: bool,
    needs_response: bool,
    failed: bool,
    has_draft: bool,
}

#[derive(Default)]
pub(crate) struct SettledGuard {
    pub(crate) is_selected: bool,
    pub(crate) busy: bool,
    pub(crate) needs_response: bool,
    pub(crate) pending_count: usize,
    pub(crate) unread: bool,
    pub(crate) failed: bool,
    pub(crate) has_draft: bool,
    pub(crate) has_pending_report: bool,
}

fn can_fold(session: &Session, activity: Activity) -> bool {
    activity.busy
        && !activity.needs_response
        && !activity.failed
        && !activity.has_draft
        && !session.archived
        && !session.pinned
}

pub(crate) fn member_activity_blocks_settle(
    members: &[crate::protocol::Member],
    session_id: &str,
) -> bool {
    members.iter().any(|member| {
        member.desktop_activity.as_ref().is_some_and(|act| {
            act.session_id.as_deref() == Some(session_id)
                && (act.pending_report_count > 0
                    || act.pending_interaction_count > 0
                    || act.queued_input_count > 0
                    || act.delivery_failure_message.is_some()
                    || matches!(
                        act.runtime_state.as_str(),
                        "working" | "waiting_interaction" | "queued" | "error"
                    ))
        })
    })
}

pub(crate) fn can_settle(session: &Session, guard: &SettledGuard, now_ms: i64) -> bool {
    !guard.is_selected
        && !session.pinned
        && !session.archived
        && session.identity_state.as_deref() != Some("new")
        && !guard.busy
        && !guard.needs_response
        && guard.pending_count == 0
        && !guard.unread
        && !guard.failed
        && !guard.has_draft
        && !guard.has_pending_report
        && session.updated_at > 0
        && now_ms.saturating_sub(session.updated_at) >= SETTLED_AGE_MS
}

pub(crate) struct SidebarSessions<'a> {
    pub(crate) regular: Vec<&'a Session>,
    pub(crate) working: Vec<WorkingProject<'a>>,
    pub(crate) settled: Vec<&'a Session>,
}

impl SidebarSessions<'_> {
    pub(super) fn len(&self) -> usize {
        self.regular.len()
            + self
                .working
                .iter()
                .map(|project| project.sessions.len())
                .sum::<usize>()
            + self.settled.len()
    }
}

pub(crate) struct WorkingProject<'a> {
    pub(crate) id: &'a str,
    sessions: Vec<&'a Session>,
}

impl<'a> WorkingProject<'a> {
    fn visible<'s>(
        &'s self,
        expanded: bool,
        current: Option<&'s str>,
    ) -> impl Iterator<Item = &'a Session> + 's {
        self.sessions
            .iter()
            .copied()
            .filter(move |session| expanded || current == Some(session.id.as_str()))
    }
}

fn partition_with_now<'a>(
    sessions: impl IntoIterator<Item = &'a Session>,
    fold_working: bool,
    now_ms: i64,
    activity: impl Fn(&Session) -> Activity,
    guard: impl Fn(&Session) -> SettledGuard,
) -> SidebarSessions<'a> {
    let mut regular = Vec::new();
    let mut working: Vec<WorkingProject<'_>> = Vec::new();
    let mut settled = Vec::new();
    for session in sessions {
        if fold_working && can_fold(session, activity(session)) {
            if let Some(project) = working
                .iter_mut()
                .find(|group| group.id == session.workspace_id)
            {
                project.sessions.push(session);
            } else {
                working.push(WorkingProject {
                    id: &session.workspace_id,
                    sessions: vec![session],
                });
            }
        } else if now_ms > 0 && can_settle(session, &guard(session), now_ms) {
            settled.push(session);
        } else {
            regular.push(session);
        }
    }
    SidebarSessions {
        regular,
        working,
        settled,
    }
}

#[cfg(test)]
fn partition<'a>(
    sessions: impl IntoIterator<Item = &'a Session>,
    fold_working: bool,
    activity: impl Fn(&Session) -> Activity,
) -> SidebarSessions<'a> {
    partition_with_now(sessions, fold_working, 0, activity, |_| SettledGuard::default())
}

impl HiveApp {
    pub(crate) fn sidebar_session_sections<'a>(
        &self,
        sessions: impl IntoIterator<Item = &'a Session>,
        fold_working: bool,
    ) -> SidebarSessions<'a> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map_or(0, |d| d.as_millis() as i64);
        let selected_id = self.session.as_ref().map(|session| session.id.as_str());
        let session_has_activity = |session: &Session| {
            let key = format!("session:{}", session.id);
            let busy = self.busy.contains(&session.id);
            let needs_response = self.interaction_count(&session.id) > 0;
            let pending_count = self.pending.get(&session.id).copied().unwrap_or(0);
            let unread = self.unread.contains(&session.id);
            let failed = self.sidebar_failed_turns.contains(&session.id)
                || self
                    .failed_drafts
                    .get(&session.id)
                    .is_some_and(|entries| !entries.is_empty());
            let has_draft = self
                .drafts
                .get(&key)
                .is_some_and(|text| !text.trim().is_empty())
                || self
                    .attachments
                    .drafts
                    .get(&key)
                    .is_some_and(|items| !items.is_empty());
            (
                busy,
                needs_response,
                pending_count,
                unread,
                failed,
                has_draft,
            )
        };
        partition_with_now(
            sessions,
            fold_working,
            now_ms,
            |session| {
                let busy = self.busy.contains(&session.id);
                if !busy || session.pinned || session.archived {
                    return Activity::default();
                }
                let (_, needs_response, _, _, failed, has_draft) = session_has_activity(session);
                Activity {
                    busy,
                    needs_response,
                    failed,
                    has_draft,
                }
            },
            |session| {
                let (busy, needs_response, pending_count, unread, failed, has_draft) =
                    session_has_activity(session);
                let member_active = member_activity_blocks_settle(&self.members, &session.id);
                let has_pending_report = (self
                    .session
                    .as_ref()
                    .is_some_and(|s| s.id == session.id)
                    && !self.pending_reports.is_empty())
                    || member_active;
                SettledGuard {
                    is_selected: selected_id == Some(session.id.as_str()),
                    busy,
                    needs_response,
                    pending_count,
                    unread,
                    failed,
                    has_draft,
                    has_pending_report,
                }
            },
        )
    }

    pub(crate) fn visible_working_sessions<'a, 's>(
        &'s self,
        project: &'s WorkingProject<'a>,
    ) -> impl Iterator<Item = &'a Session> + 's {
        project.visible(
            self.sidebar.working_project_expanded(project.id),
            self.session.as_ref().map(|session| session.id.as_str()),
        )
    }

    pub(super) fn render_working_project(
        &self,
        project: &WorkingProject<'_>,
        show_project: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = project.id.to_owned();
        let expanded = self.sidebar.working_project_expanded(project.id);
        let workspace = self
            .workspaces
            .iter()
            .find(|workspace| workspace.id == project.id);
        let title = if show_project {
            workspace.map_or_else(
                || tr("正在工作").to_owned(),
                |workspace| format!("{} · {}", tr("正在工作"), workspace.name),
            )
        } else {
            tr("正在工作").to_owned()
        };
        let count = project.sessions.len().to_string();
        let action = tr(if expanded {
            "收起正在工作的会话"
        } else {
            "展开正在工作的会话"
        });
        v_flex()
            .min_w_0()
            .gap_0p5()
            .pt_1()
            .child(
                Button::new(SharedString::from(format!("toggle-working-{id}")))
                    .custom(palette::quiet_icon_variant(cx))
                    .compact()
                    .xsmall()
                    .h(px(24.))
                    .px_2()
                    .w_full()
                    .rounded_md()
                    .accessibility_label(format!("{action} · {title} · {count}"))
                    .when_some(workspace, |button, workspace| {
                        button.tooltip(workspace.path.clone())
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(title),
                            )
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(count),
                                    )
                                    .child(
                                        Icon::new(if expanded {
                                            Glyph::ChevronDown
                                        } else {
                                            Glyph::ChevronRight
                                        })
                                        .size(px(12.))
                                        .text_color(Color::Muted.get(cx)),
                                    ),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Err(error) = this.sidebar.toggle_working_project(&id) {
                            this.error = Some(crate::app::ErrorNotice::Detail(
                                "无法保存正在工作的会话区段",
                                error.to_string(),
                            ));
                        }
                        cx.notify();
                    })),
            )
            .children(
                self.visible_working_sessions(project)
                    .map(|session| self.render_session_link(session, cx)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Activity, SETTLED_AGE_MS, SettledGuard, can_settle, partition, partition_with_now};
    use crate::protocol::Session;

    fn session(id: &str, workspace: &str, pinned: bool) -> Session {
        serde_json::from_value(serde_json::json!({
            "id": id, "workspaceId": workspace, "title": id, "pinned": pinned,
        }))
        .unwrap()
    }

    fn dated_session(
        id: &str,
        workspace: &str,
        pinned: bool,
        archived: bool,
        identity_state: Option<&str>,
        updated_at: i64,
    ) -> Session {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "workspaceId": workspace,
            "title": id,
            "pinned": pinned,
            "archived": archived,
            "identityState": identity_state,
            "updatedAt": updated_at,
        }))
        .unwrap()
    }

    #[test]
    fn working_groups_keep_pins_waiting_failed_and_drafts_visible() {
        let sessions = [
            session("work-a", "alpha", false),
            session("pinned", "alpha", true),
            session("awaiting", "alpha", false),
            session("work-b", "beta", false),
            session("failed", "beta", false),
            session("paused", "beta", false),
            session("draft", "beta", false),
            session("work-a-next", "alpha", false),
        ];
        let sections = partition(&sessions, true, |item| Activity {
            busy: item.id != "paused",
            needs_response: item.id == "awaiting",
            failed: item.id == "failed",
            has_draft: item.id == "draft",
        });
        assert_eq!(
            sections
                .regular
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["pinned", "awaiting", "failed", "paused", "draft"]
        );
        assert_eq!(
            sections
                .working
                .iter()
                .map(|project| project.id)
                .collect::<Vec<_>>(),
            ["alpha", "beta"]
        );
        let alpha = &sections.working[0];
        assert_eq!(alpha.sessions.len(), 2);
        assert_eq!(alpha.visible(false, None).count(), 0);
        assert_eq!(
            alpha.visible(false, Some("work-a-next")).next().unwrap().id,
            "work-a-next"
        );
        assert_eq!(
            alpha
                .visible(true, None)
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["work-a", "work-a-next"]
        );
        assert_eq!(
            sections.working[1]
                .visible(false, Some("work-a-next"))
                .count(),
            0
        );

        let settled = partition(&sessions, true, |_| Activity::default());
        assert!(settled.working.is_empty());
        assert_eq!(settled.regular.len(), sessions.len());

        let unfolded = partition(&sessions, false, |_| {
            panic!("disabled grouping skips activity")
        });
        assert!(unfolded.working.is_empty());
        assert!(
            unfolded
                .regular
                .iter()
                .map(|item| &item.id)
                .eq(sessions.iter().map(|item| &item.id))
        );
    }

    #[test]
    fn settled_partition_enforces_collaboration_and_activity_guards() {
        let now_ms = 2_000_000_000_000i64;
        let old_ts = now_ms - SETTLED_AGE_MS - 1_000;
        let recent_ts = now_ms - SETTLED_AGE_MS + 60_000;

        let quiet_old = dated_session("quiet-old", "alpha", false, false, Some("known"), old_ts);
        let recent = dated_session("recent", "alpha", false, false, Some("known"), recent_ts);
        let pinned_old = dated_session("pinned-old", "alpha", true, false, Some("known"), old_ts);
        let archived_old =
            dated_session("archived-old", "alpha", false, true, Some("known"), old_ts);
        let new_draft = dated_session("new-draft", "alpha", false, false, Some("new"), old_ts);
        let selected_old =
            dated_session("selected-old", "alpha", false, false, Some("known"), old_ts);
        let pending_report =
            dated_session("pending-report", "alpha", false, false, Some("known"), old_ts);
        let unread_old = dated_session("unread-old", "alpha", false, false, Some("known"), old_ts);
        let draft_old = dated_session("draft-old", "alpha", false, false, Some("known"), old_ts);
        let queued_old = dated_session("queued-old", "alpha", false, false, Some("known"), old_ts);
        let failed_old = dated_session("failed-old", "alpha", false, false, Some("known"), old_ts);
        let zero_ts = dated_session("zero-ts", "alpha", false, false, Some("known"), 0);

        assert!(can_settle(&quiet_old, &SettledGuard::default(), now_ms));
        assert!(!can_settle(&recent, &SettledGuard::default(), now_ms));
        assert!(!can_settle(&pinned_old, &SettledGuard::default(), now_ms));
        assert!(!can_settle(&archived_old, &SettledGuard::default(), now_ms));
        assert!(!can_settle(&new_draft, &SettledGuard::default(), now_ms));
        assert!(!can_settle(&zero_ts, &SettledGuard::default(), now_ms));

        let sessions = [
            quiet_old,
            recent,
            pinned_old,
            archived_old,
            new_draft,
            selected_old,
            pending_report,
            unread_old,
            draft_old,
            queued_old,
            failed_old,
            zero_ts,
        ];
        let sections = partition_with_now(
            &sessions,
            true,
            now_ms,
            |_| Activity::default(),
            |item| SettledGuard {
                is_selected: item.id == "selected-old",
                busy: false,
                needs_response: false,
                pending_count: usize::from(item.id == "queued-old"),
                unread: item.id == "unread-old",
                failed: item.id == "failed-old",
                has_draft: item.id == "draft-old",
                has_pending_report: item.id == "pending-report",
            },
        );
        assert_eq!(
            sections
                .settled
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["quiet-old"]
        );
        assert_eq!(sections.regular.len(), sessions.len() - 1);
        assert_eq!(sections.len(), sessions.len());
    }

    #[test]
    fn member_delivery_failure_message_blocks_settle() {
        let now_ms = 2_000_000_000_000i64;
        let old_ts = now_ms - SETTLED_AGE_MS - 1_000;
        let target = dated_session("old-failed-delivery", "alpha", false, false, Some("known"), old_ts);
        let other = dated_session("old-quiet", "alpha", false, false, Some("known"), old_ts);
        let member_failed: crate::protocol::Member = serde_json::from_value(serde_json::json!({
            "id": "worker-1",
            "name": "Worker 1",
            "role": "worker",
            "status": "idle",
            "pending_task_count": 0,
            "desktop_model": null,
            "desktop_activity": {
                "sessionId": "old-failed-delivery",
                "runtimeState": "idle",
                "pendingInteractionCount": 0,
                "queuedInputCount": 0,
                "pendingReportCount": 0,
                "deliveryFailureMessage": "connection reset while delivering report"
            }
        }))
        .unwrap();
        let member_idle: crate::protocol::Member = serde_json::from_value(serde_json::json!({
            "id": "worker-2",
            "name": "Worker 2",
            "role": "worker",
            "status": "idle",
            "pending_task_count": 0,
            "desktop_model": null,
            "desktop_activity": {
                "sessionId": "old-quiet",
                "runtimeState": "idle",
                "pendingInteractionCount": 0,
                "queuedInputCount": 0,
                "pendingReportCount": 0,
                "deliveryFailureMessage": null
            }
        }))
        .unwrap();
        let members = [member_failed, member_idle];
        assert!(super::member_activity_blocks_settle(&members, &target.id));
        assert!(!super::member_activity_blocks_settle(&members, &other.id));

        let sections = partition_with_now(
            [&target, &other],
            true,
            now_ms,
            |_| Activity::default(),
            |item| SettledGuard {
                has_pending_report: super::member_activity_blocks_settle(&members, &item.id),
                ..SettledGuard::default()
            },
        );
        assert_eq!(
            sections.regular.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["old-failed-delivery"]
        );
        assert_eq!(
            sections.settled.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["old-quiet"]
        );
    }
}
