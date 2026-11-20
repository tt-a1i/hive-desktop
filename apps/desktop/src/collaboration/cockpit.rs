use crate::{
    app::HiveApp,
    language::{english, tr},
    palette::{self, Color},
    protocol::Member,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub(crate) fn live_state_dot(state: &str, cx: &App) -> Rgba {
    match state {
        "working" => Color::Accent.get(cx),
        "waiting_interaction" => crate::chrome::attention_dot(cx),
        "queued" => Color::Secondary.get(cx),
        "error" => Color::Error.get(cx),
        "stopped" => Color::Muted.get(cx),
        _ => Color::Success.get(cx),
    }
}

impl HiveApp {
    pub(crate) fn render_team_cockpit(&self, cx: &Context<Self>) -> AnyElement {
        if self.members.is_empty() {
            return div().into_any_element();
        }
        let mut working = 0usize;
        let mut waiting = 0usize;
        let mut queued = 0usize;
        let mut errored = 0usize;
        let mut unread_reports = 0usize;
        for m in &self.members {
            match m.live_state() {
                "working" => working += 1,
                "waiting_interaction" => waiting += 1,
                "queued" => queued += 1,
                "error" => errored += 1,
                _ => {}
            }
            if let Some(a) = &m.desktop_activity {
                unread_reports += a.pending_report_count as usize;
            }
        }
        let submitted_dispatches = self
            .collaboration
            .dispatches
            .iter()
            .filter(|d| d.state == "submitted")
            .count();
        let pending_reports_total = unread_reports.max(submitted_dispatches);

        // Sort Lead first, then active/waiting/errored workers, then idle/stopped.
        let mut ordered: Vec<&Member> = self.members.iter().collect();
        ordered.sort_by_key(|m| {
            let lead_rank = if m.is_lead() { 0 } else { 1 };
            let state_rank = match m.live_state() {
                "waiting_interaction" => 0,
                "error" => 1,
                "working" => 2,
                "queued" => 3,
                "idle" => 4,
                _ => 5,
            };
            (lead_rank, state_rank, m.name.as_str())
        });

        let summary_pills = h_flex()
            .flex_wrap()
            .items_center()
            .gap_1p5()
            .when(working > 0, |row| {
                row.child(self.cockpit_stat_pill(
                    Color::Accent.get(cx),
                    if english() {
                        format!("{working} active")
                    } else {
                        format!("{working} 执行中")
                    },
                    cx,
                ))
            })
            .when(waiting > 0, |row| {
                row.child(self.cockpit_stat_pill(
                    crate::chrome::attention_dot(cx),
                    if english() {
                        format!("{waiting} waiting")
                    } else {
                        format!("{waiting} 等待交互")
                    },
                    cx,
                ))
            })
            .when(pending_reports_total > 0, |row| {
                row.child(self.cockpit_stat_pill(
                    crate::chrome::attention_dot(cx),
                    if english() {
                        format!("{pending_reports_total} reports pending")
                    } else {
                        format!("{pending_reports_total} 份待阅汇报")
                    },
                    cx,
                ))
            })
            .when(queued > 0, |row| {
                row.child(self.cockpit_stat_pill(
                    Color::Secondary.get(cx),
                    if english() {
                        format!("{queued} queued")
                    } else {
                        format!("{queued} 队列待发")
                    },
                    cx,
                ))
            })
            .when(errored > 0, |row| {
                row.child(self.cockpit_stat_pill(
                    Color::Error.get(cx),
                    if english() {
                        format!("{errored} error")
                    } else {
                        format!("{errored} 异常停止")
                    },
                    cx,
                ))
            })
            .when(
                working == 0
                    && waiting == 0
                    && pending_reports_total == 0
                    && queued == 0
                    && errored == 0,
                |row| {
                    row.child(self.cockpit_stat_pill(
                        Color::Success.get(cx),
                        tr("全部待命").to_owned(),
                        cx,
                    ))
                },
            );

        v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .shadow(palette::card_shadow(cx))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("团队实时态势")),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(Color::Muted.get(cx))
                                    .child(if english() {
                                        format!("{} members", self.members.len())
                                    } else {
                                        format!("{} 位成员", self.members.len())
                                    }),
                            ),
                    )
                    .child(summary_pills),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap_1p5()
                    .children(
                        ordered
                            .into_iter()
                            .enumerate()
                            .map(|(idx, member)| self.render_cockpit_member_row(idx, member, cx)),
                    ),
            )
            .into_any_element()
    }

    fn cockpit_stat_pill(&self, dot: Rgba, label: String, cx: &Context<Self>) -> impl IntoElement + use<> {
        h_flex()
            .items_center()
            .gap(px(5.))
            .px_2()
            .py(px(2.))
            .rounded_md()
            .bg(Color::Inset.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(Color::Secondary.get(cx))
            .child(div().size(px(6.)).rounded_full().bg(dot))
            .child(label)
    }

    fn render_cockpit_member_row(
        &self,
        index: usize,
        member: &Member,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let state = member.live_state();
        let dot = live_state_dot(state, cx);
        let session_id = self.member_session_id(&member.id);
        let activity = member.desktop_activity.as_ref();
        let summary = member.activity_summary();
        let pending_reports = activity.map(|a| a.pending_report_count).unwrap_or(0);
        let worktree_branch = activity
            .and_then(|a| a.worktree_branch.as_deref())
            .filter(|b| !b.is_empty())
            .map(str::to_owned);
        let worktree_removed = activity.is_some_and(|a| a.worktree_removed);
        let has_submitted = self
            .collaboration
            .dispatches
            .iter()
            .any(|d| d.to_agent_id == member.id && d.state == "submitted");
        let highlight_report = pending_reports > 0 || has_submitted;

        let monogram = palette::project_monogram(&member.name);
        let is_lead = member.is_lead();
        h_flex()
            .id(("cockpit-member", index))
            .w_full()
            .min_w_0()
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .py_2p5()
            .rounded_lg()
            .bg(Color::Inset.get(cx))
            .border_1()
            .border_color(if highlight_report || state == "waiting_interaction" {
                crate::chrome::attention_dot(cx).opacity(0.55)
            } else if state == "error" {
                Color::Error.get(cx).opacity(0.55)
            } else {
                Color::Divider.get(cx)
            })
            .hover(|style| style.bg(Color::Card.get(cx)))
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(26.))
                    .rounded_md()
                    .bg(if is_lead {
                        Color::AccentSoft.get(cx)
                    } else {
                        Color::Card.get(cx)
                    })
                    .border_1()
                    .border_color(if is_lead {
                        Color::Accent.get(cx).opacity(0.28)
                    } else {
                        Color::Divider.get(cx)
                    })
                    .text_size(px(10.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if is_lead {
                        Color::Accent.get(cx)
                    } else {
                        Color::Secondary.get(cx)
                    })
                    .child(monogram),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .min_w_0()
                            .flex_wrap()
                            .items_center()
                            .gap_1p5()
                            .child(div().size(px(7.)).rounded_full().flex_shrink_0().bg(dot))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .truncate()
                                    .child(member.name.clone()),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py(px(1.))
                                    .rounded_sm()
                                    .bg(Color::Card.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .text_size(px(10.5))
                                    .text_color(if member.is_lead() {
                                        Color::Accent.get(cx)
                                    } else {
                                        Color::Muted.get(cx)
                                    })
                                    .child(member.role_label().to_owned()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(Color::Secondary.get(cx))
                                    .child(member.status_label().to_owned()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(Color::Muted.get(cx))
                                    .child(member.backend_label().to_owned()),
                            )
                            .when_some(worktree_branch, |row, branch| {
                                row.child(
                                    h_flex()
                                        .items_center()
                                        .gap_1()
                                        .px_1p5()
                                        .py(px(1.))
                                        .rounded_sm()
                                        .bg(Color::Card.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .text_size(px(10.5))
                                        .text_color(Color::Muted.get(cx))
                                        .child(if worktree_removed {
                                            format!("{branch} ({})", tr("已移除"))
                                        } else {
                                            format!("worktree · {branch}")
                                        }),
                                )
                            })
                            .when(highlight_report, |row| {
                                row.child(
                                    div()
                                        .px_1p5()
                                        .py(px(1.))
                                        .rounded_sm()
                                        .bg(crate::chrome::attention_dot(cx).opacity(0.14))
                                        .text_size(px(10.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(crate::chrome::attention_dot(cx))
                                        .child(tr("待阅汇报").to_owned()),
                                )
                            }),
                    )
                    .when_some(summary, |col, line| {
                        col.child(
                            div()
                                .w_full()
                                .min_w_0()
                                .truncate()
                                .text_size(px(11.5))
                                .text_color(if state == "error" {
                                    Color::Error.get(cx)
                                } else {
                                    Color::Muted.get(cx)
                                })
                                .child(line),
                        )
                    }),
            )
            .when_some(session_id, |row, sid| {
                let member_name = member.name.clone();
                row.child(
                    Button::new(("cockpit-open-session", index))
                        .custom(palette::quiet_icon_variant(cx))
                        .compact()
                        .xsmall()
                        .h(px(26.))
                        .px_2()
                        .rounded_md()
                        .icon(Glyph::ArrowRight)
                        .label(tr("进入会话"))
                        .accessibility_label(if english() {
                            format!("Open member session: {member_name}")
                        } else {
                            format!("打开成员会话：{member_name}")
                        })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_member_session(sid.clone(), window, cx);
                    })),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::protocol::{Member, MemberDesktopActivity};

    fn sample_member(status: &str, activity: Option<MemberDesktopActivity>) -> Member {
        Member {
            id: "ws:worker-1".into(),
            description: String::new(),
            name: "Reviewer".into(),
            role: "worker".into(),
            status: status.into(),
            pending_task_count: 1,
            configured_command: Some("hive:codex".into()),
            desktop_model: None,
            desktop_profile: None,
            desktop_launch_error: None,
            desktop_activity: activity,
        }
    }

    #[test]
    fn member_live_state_and_activity_summary_cover_tool_interaction_and_error() {
        let tool_member = sample_member(
            "working",
            Some(MemberDesktopActivity {
                runtime_state: "working".into(),
                activity_kind: Some("tool".into()),
                active_tool: Some("edit_file".into()),
                active_target: Some("src/app.rs".into()),
                ..Default::default()
            }),
        );
        assert_eq!(tool_member.live_state(), "working");
        assert_eq!(
            tool_member.activity_summary().as_deref(),
            Some("edit_file · src/app.rs")
        );
        assert!(!tool_member.is_lead());

        let waiting = sample_member(
            "working",
            Some(MemberDesktopActivity {
                runtime_state: "waiting_interaction".into(),
                activity_kind: Some("interaction".into()),
                pending_interaction_count: 2,
                ..Default::default()
            }),
        );
        assert_eq!(waiting.live_state(), "waiting_interaction");
        assert!(waiting.activity_summary().unwrap().contains('2'));

        let errored = sample_member(
            "stopped",
            Some(MemberDesktopActivity {
                runtime_state: "error".into(),
                activity_kind: Some("error".into()),
                delivery_failure_message: Some("connection reset".into()),
                ..Default::default()
            }),
        );
        assert_eq!(errored.live_state(), "error");
        assert_eq!(errored.activity_summary().as_deref(), Some("connection reset"));
    }
}
