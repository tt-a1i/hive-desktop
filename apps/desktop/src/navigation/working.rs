use super::*;
use crate::protocol::Session;

#[derive(Default)]
struct Activity {
    busy: bool,
    needs_response: bool,
    failed: bool,
    has_draft: bool,
}

fn can_fold(session: &Session, activity: Activity) -> bool {
    activity.busy
        && !activity.needs_response
        && !activity.failed
        && !activity.has_draft
        && !session.archived
        && !session.pinned
}

pub(crate) struct SidebarSessions<'a> {
    pub(crate) regular: Vec<&'a Session>,
    pub(crate) working: Vec<WorkingProject<'a>>,
}

impl SidebarSessions<'_> {
    pub(super) fn len(&self) -> usize {
        self.regular.len()
            + self
                .working
                .iter()
                .map(|project| project.sessions.len())
                .sum::<usize>()
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

fn partition<'a>(
    sessions: impl IntoIterator<Item = &'a Session>,
    fold_working: bool,
    activity: impl Fn(&Session) -> Activity,
) -> SidebarSessions<'a> {
    if !fold_working {
        return SidebarSessions {
            regular: sessions.into_iter().collect(),
            working: Vec::new(),
        };
    }
    let mut regular = Vec::new();
    let mut working: Vec<WorkingProject<'_>> = Vec::new();
    for session in sessions {
        if !can_fold(session, activity(session)) {
            regular.push(session);
        } else if let Some(project) = working
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
    }
    SidebarSessions { regular, working }
}

impl HiveApp {
    pub(crate) fn sidebar_session_sections<'a>(
        &self,
        sessions: impl IntoIterator<Item = &'a Session>,
        fold_working: bool,
    ) -> SidebarSessions<'a> {
        partition(sessions, fold_working, |session| {
            let busy = self.busy.contains(&session.id);
            if !busy || session.pinned || session.archived {
                return Activity::default();
            }
            let key = format!("session:{}", session.id);
            Activity {
                busy,
                needs_response: self.interaction_count(&session.id) > 0,
                failed: self.sidebar_failed_turns.contains(&session.id)
                    || self
                        .failed_drafts
                        .get(&session.id)
                        .is_some_and(|entries| !entries.is_empty()),
                has_draft: self
                    .drafts
                    .get(&key)
                    .is_some_and(|text| !text.trim().is_empty())
                    || self
                        .attachments
                        .drafts
                        .get(&key)
                        .is_some_and(|items| !items.is_empty()),
            }
        })
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
    use super::{Activity, partition};
    use crate::protocol::Session;

    fn session(id: &str, workspace: &str, pinned: bool) -> Session {
        serde_json::from_value(serde_json::json!({
            "id": id, "workspaceId": workspace, "title": id, "pinned": pinned,
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
}
