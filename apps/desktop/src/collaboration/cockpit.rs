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
    /// Subtle live team status pill shown in the main chat topbar ONLY when at
    /// least one worker is active or the inbox contains conversation activity.
    pub(crate) fn render_topbar_team_status_pill(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let mut working = 0usize;
        let mut waiting = 0usize;
        let mut errored = 0usize;
        for m in self.members.iter().filter(|m| !m.is_lead()) {
            match m.live_state() {
                "waiting_interaction" => waiting += 1,
                "error" => errored += 1,
                "working" => working += 1,
                _ => {}
            }
        }
        let inbox_count = self.inbox_session_count();
        if working == 0 && waiting == 0 && errored == 0 && inbox_count == 0 {
            return None;
        }
        let mut parts = Vec::new();
        if working > 0 {
            parts.push(if english() {
                format!("{working} active")
            } else {
                format!("{working} 执行中")
            });
        }
        if waiting > 0 {
            parts.push(if english() {
                format!("{waiting} waiting")
            } else {
                format!("{waiting} 等待确认")
            });
        }
        if errored > 0 {
            parts.push(if english() {
                format!("{errored} error")
            } else {
                format!("{errored} 异常")
            });
        }
        if inbox_count > 0 {
            parts.push(if english() {
                if inbox_count == 1 {
                    "1 conversation needs attention".to_owned()
                } else {
                    format!("{inbox_count} conversations need attention")
                }
            } else {
                format!("{inbox_count} 个会话待关注")
            });
        }
        let label = parts.join(" · ");
        let dot_color = if waiting > 0 || inbox_count > 0 {
            crate::chrome::attention_dot(cx)
        } else if errored > 0 {
            Color::Error.get(cx)
        } else {
            Color::Accent.get(cx)
        };
        Some(
            Button::new("topbar-live-team-pill")
                .custom(palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(24.))
                .px_2()
                .rounded_full()
                .tooltip(tr("协作收件箱"))
                .accessibility_label(format!("{}: {label}", tr("团队实时态势")))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(5.))
                        .px_1()
                        .rounded_full()
                        .child(div().size(px(6.)).rounded_full().bg(dot_color))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Color::Secondary.get(cx))
                                .child(label),
                        ),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.open_collaboration(cx);
                }))
                .into_any_element(),
        )
    }

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
            .when(unread_reports > 0, |row| {
                row.child(self.cockpit_stat_pill(
                    crate::chrome::attention_dot(cx),
                    if english() {
                        format!("{unread_reports} reports pending")
                    } else {
                        format!("{unread_reports} 份待处理汇报")
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
                    && unread_reports == 0
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
            .when(!self.collaboration.dispatches.is_empty(), |card| {
                card.child(self.render_dispatch_timeline(cx))
            })
            .into_any_element()
    }

    fn render_dispatch_timeline(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let member_name = |agent_id: &str| -> String {
            self.members
                .iter()
                .find(|m| m.id == agent_id)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| agent_id.rsplit(':').next().unwrap_or(agent_id).to_owned())
        };
        v_flex()
            .w_full()
            .gap_2()
            .pt_2p5()
            .border_t_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(tr("派单时间线")),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(Color::Muted.get(cx))
                            .child(if english() {
                                format!("{} recent dispatches", self.collaboration.dispatches.len())
                            } else {
                                format!("最近 {} 条派单", self.collaboration.dispatches.len())
                            }),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap_1p5()
                    .children(
                        self.collaboration
                            .dispatches
                            .iter()
                            .take(10)
                            .enumerate()
                            .map(|(i, d)| {
                                let from = d
                                    .from_agent_id
                                    .as_deref()
                                    .map(member_name)
                                    .unwrap_or_else(|| tr("主控").to_owned());
                                let to = member_name(&d.to_agent_id);
                                let dot_color = match d.state.as_str() {
                                    "reported" | "completed" | "done" => Color::Success.get(cx),
                                    "failed" | "error" | "cancelled" => Color::Error.get(cx),
                                    _ => Color::Accent.get(cx),
                                };
                                let task_preview: String = d.text.lines().next().unwrap_or("").chars().take(120).collect();
                                let report_preview = d.report_text.as_deref().map(|r| {
                                    r.lines().next().unwrap_or("").chars().take(120).collect::<String>()
                                });
                                h_flex()
                                    .id(("cockpit-timeline-item", i))
                                    .w_full()
                                    .min_w_0()
                                    .items_start()
                                    .justify_between()
                                    .gap_2()
                                    .px_2p5()
                                    .py_1p5()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .items_start()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .mt(px(5.))
                                                    .w(px(7.))
                                                    .h(px(7.))
                                                    .rounded_full()
                                                    .flex_shrink_0()
                                                    .bg(dot_color),
                                            )
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap_0p5()
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap_1p5()
                                                            .flex_wrap()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(Color::Text.get(cx))
                                                                    .child(format!("{from} → {to}")),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px_1p5()
                                                                    .py(px(0.5))
                                                                    .rounded_sm()
                                                                    .bg(Color::Card.get(cx))
                                                                    .text_size(px(10.))
                                                                    .text_color(dot_color)
                                                                    .child(d.state.clone()),
                                                            )
                                                            .when(!d.artifacts.is_empty(), |row| {
                                                                row.child(
                                                                    div()
                                                                        .text_size(px(10.5))
                                                                        .text_color(Color::Muted.get(cx))
                                                                        .child(if english() {
                                                                            format!("{} artifacts", d.artifacts.len())
                                                                        } else {
                                                                            format!("{} 个产物", d.artifacts.len())
                                                                        }),
                                                                )
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Color::Secondary.get(cx))
                                                            .truncate()
                                                            .child(task_preview),
                                                    )
                                                    .when_some(report_preview, |col, rep| {
                                                        col.child(
                                                            div()
                                                                .text_size(px(11.))
                                                                .text_color(Color::Muted.get(cx))
                                                                .truncate()
                                                                .child(format!("{}: {rep}", tr("汇报"))),
                                                        )
                                                    }),
                                            ),
                                    )
                                    .child(self.dispatch_session_button(("timeline-split-peek", i), d, true, cx))
                            }),
                    ),
            )
    }

    pub(crate) fn open_split_peek_session(&mut self, session_id: String, cx: &mut Context<Self>) {
        if !self.connected || self.closing {
            return;
        }
        let session_meta = self.sessions.iter().find(|s| s.id == session_id);
        let title = session_meta
            .map(|s| self.session_title(s).to_owned())
            .unwrap_or_else(|| tr("成员会话").to_owned());
        let backend_label = session_meta
            .map(|s| tr(s.backend.label()).to_owned())
            .unwrap_or_default();
        let branch = session_meta
            .and_then(|s| s.execution.as_ref())
            .filter(|e| e.kind == "worktree")
            .map(|e| e.branch.trim().to_owned())
            .filter(|b| !b.is_empty());

        self.git_drawer_open = false;
        self.close_collaboration();
        self.split_peek = Some(crate::app::SplitPeekState {
            session_id: session_id.clone(),
            title,
            backend_label,
            branch,
            loading: true,
            error: None,
            messages: vec![],
            scroll: ScrollHandle::new(),
        });
        cx.notify();
        self.refresh_split_peek(cx);
    }

    pub(crate) fn refresh_split_peek(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.closing {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let Some(session_id) = self.split_peek.as_ref().map(|p| p.session_id.clone()) else {
            return;
        };
        if let Some(peek) = self.split_peek.as_mut() {
            peek.loading = true;
            peek.error = None;
        }
        cx.notify();
        let generation = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let res = runtime
                .request(
                    "chat.open",
                    serde_json::json!({
                        "sessionId": session_id,
                        "includeLiveHistory": true,
                        "historyLimit": 30,
                        "deferHistoryImages": true,
                    }),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != generation {
                    return;
                }
                let Some(peek) = this.split_peek.as_mut() else {
                    return;
                };
                if peek.session_id != session_id {
                    return;
                }
                peek.loading = false;
                match res {
                    Ok(value) => {
                        let mut msgs = Vec::new();
                        if let Some(arr) = value.get("history").and_then(|h| h.as_array()) {
                            for item in arr.iter().rev().take(30).rev() {
                                let role = item
                                    .get("role")
                                    .and_then(|r| r.as_str())
                                    .unwrap_or("assistant")
                                    .to_owned();
                                let mut text = String::new();
                                if let Some(s) = item.get("content").and_then(|c| c.as_str()) {
                                    text.push_str(s);
                                } else if let Some(blocks) = item.get("content").and_then(|c| c.as_array()) {
                                    for b in blocks {
                                        if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                                            if !text.is_empty() {
                                                text.push('\n');
                                            }
                                            text.push_str(t);
                                        }
                                    }
                                }
                                let trimmed = text.trim();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let bounded: String = if trimmed.chars().count() > 800 {
                                    format!("{}…", trimmed.chars().take(800).collect::<String>())
                                } else {
                                    trimmed.to_owned()
                                };
                                msgs.push(crate::app::SplitPeekMessage { role, text: bounded });
                            }
                        }
                        peek.messages = msgs;
                        peek.error = None;
                    }
                    Err(err) => {
                        peek.error = Some(err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn close_split_peek(&mut self, cx: &mut Context<Self>) {
        self.split_peek = None;
        cx.notify();
    }

    pub(crate) fn render_split_peek_panel(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let peek = self.split_peek.as_ref()?;
        let sid_promote = peek.session_id.clone();
        Some(
            v_flex()
                .id("split-peek-right-drawer")
                .w(px(400.))
                .h_full()
                .flex_shrink_0()
                .border_l_1()
                .border_color(Color::Divider.get(cx))
                .bg(Color::Surface.get(cx))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .bg(Color::Sidebar.get(cx))
                        .border_b_1()
                        .border_color(Color::Divider.get(cx))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .px_1p5()
                                                .py(px(0.5))
                                                .rounded_sm()
                                                .bg(Color::AccentSoft.get(cx))
                                                .text_size(px(10.))
                                                .text_color(Color::Accent.get(cx))
                                                .child(tr("分屏对照")),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(Color::Text.get(cx))
                                                .truncate()
                                                .child(peek.title.clone()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .text_color(Color::Muted.get(cx))
                                        .truncate()
                                        .child(match &peek.branch {
                                            Some(b) => format!("{} · {}", peek.backend_label, b),
                                            None => peek.backend_label.clone(),
                                        }),
                                ),
                        )
                        .child(
                            h_flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new("split-peek-refresh")
                                        .custom(palette::quiet_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(24.))
                                        .w(px(24.))
                                        .rounded_md()
                                        .icon(Glyph::RefreshCw)
                                        .tooltip(tr("刷新分屏会话"))
                                        .accessibility_label(tr("刷新分屏会话"))
                                        .disabled(peek.loading)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.refresh_split_peek(cx);
                                        })),
                                )
                                .child(
                                    Button::new("split-peek-promote")
                                        .custom(palette::subtle_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(24.))
                                        .px_2()
                                        .rounded_md()
                                        .label(tr("切为主会话"))
                                        .accessibility_label(tr("切为主会话"))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.split_peek = None;
                                            this.open_member_session(sid_promote.clone(), window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("split-peek-close")
                                        .custom(palette::quiet_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(24.))
                                        .w(px(24.))
                                        .rounded_md()
                                        .icon(Glyph::X)
                                        .tooltip(tr("关闭分屏"))
                                        .accessibility_label(tr("关闭分屏"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.close_split_peek(cx);
                                        })),
                                ),
                        ),
                )
                .child(
                    v_flex()
                        .id("split-peek-messages")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&peek.scroll)
                        .p_3()
                        .gap_2()
                        .when(peek.loading, |col| {
                            col.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("正在读取会话快照…")),
                            )
                        })
                        .when_some(peek.error.as_ref(), |col, err| {
                            col.child(
                                div()
                                    .p_2()
                                    .rounded_md()
                                    .bg(Color::ErrorSurface.get(cx))
                                    .text_xs()
                                    .text_color(Color::Error.get(cx))
                                    .child(err.clone()),
                            )
                        })
                        .when(!peek.loading && peek.error.is_none() && peek.messages.is_empty(), |col| {
                            col.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("该会话暂无可显示的文本消息。")),
                            )
                        })
                        .children(peek.messages.iter().enumerate().map(|(i, msg)| {
                            let is_user = msg.role == "user";
                            let is_tool = msg.role == "toolResult";
                            let role_label = match msg.role.as_str() {
                                "user" => tr("用户"),
                                "assistant" => tr("助手"),
                                "toolResult" => tr("工具输出"),
                                _ => tr("系统"),
                            };
                            v_flex()
                                .id(("split-peek-msg", i))
                                .w_full()
                                .gap_1()
                                .p_2p5()
                                .rounded_lg()
                                .bg(if is_user {
                                    Color::AccentSoft.get(cx)
                                } else {
                                    Color::Card.get(cx)
                                })
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(if is_user {
                                            Color::Accent.get(cx)
                                        } else {
                                            Color::Muted.get(cx)
                                        })
                                        .child(role_label.to_owned()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .line_height(px(17.5))
                                        .text_color(if is_tool {
                                            Color::Secondary.get(cx)
                                        } else {
                                            Color::Text.get(cx)
                                        })
                                        .child(msg.text.clone()),
                                )
                        })),
                )
                .into_any_element(),
        )
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
        let highlight_report = pending_reports > 0;

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
                                        .child(tr("待处理的汇报")),
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
                let peek_sid = sid.clone();
                row.child(
                    h_flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new(("cockpit-split-peek", index))
                                .custom(palette::subtle_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(26.))
                                .px_2()
                                .rounded_md()
                                .label(tr("分屏对照"))
                                .accessibility_label(if english() {
                                    format!("Split peek member session: {member_name}")
                                } else {
                                    format!("分屏对照成员会话：{member_name}")
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_split_peek_session(peek_sid.clone(), cx);
                                })),
                        )
                        .child(
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
                        ),
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
