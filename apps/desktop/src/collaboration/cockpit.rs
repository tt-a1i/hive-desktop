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
                                                                    .child(
                                                                        super::dispatch_state(&d.state)
                                                                            .to_owned(),
                                                                    ),
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

    fn build_split_peek_meta(&self, session: &crate::protocol::Session) -> (
        String,
        String,
        Option<String>,
        &'static str,
        Option<String>,
        bool,
        bool,
        Option<String>,
        bool,
        bool,
        bool,
    ) {
        let title = self.session_title(session).to_owned();
        let backend_label = tr(session.backend.label()).to_owned();
        let branch = session
            .execution
            .as_ref()
            .filter(|e| e.kind == "worktree")
            .map(|e| e.branch.trim().to_owned())
            .filter(|b| !b.is_empty());
        let role_key = session.display_role_key();
        let agent_id = session.display_agent_id();
        let member_name = agent_id.and_then(|id| {
            self.members
                .iter()
                .find(|m| m.id == id)
                .map(|m| m.name.clone())
        });
        let identity_unknown = session.identity_state.as_deref() == Some("unknown");
        let member_missing = agent_id.is_some_and(|id| {
            !id.ends_with(":orchestrator")
                && !id.ends_with(":__workflow__")
                && !self.members.iter().any(|m| m.id == id)
        });
        let execution_path = session
            .execution
            .as_ref()
            .map(|e| e.path.clone())
            .or_else(|| session.execution_path.clone())
            .filter(|p| !p.trim().is_empty());
        let shared_worktree = session
            .execution
            .as_ref()
            .is_some_and(|e| e.kind == "worktree" && e.owner_session_id.is_some());
        (
            title,
            backend_label,
            branch,
            role_key,
            member_name,
            identity_unknown,
            member_missing,
            execution_path,
            shared_worktree,
            session.worktree_released,
            session.archived,
        )
    }

    pub(crate) fn open_split_peek_session(&mut self, session_id: String, cx: &mut Context<Self>) {
        if !self.connected || self.closing {
            return;
        }
        let session_meta = self.sessions.iter().find(|s| s.id == session_id).cloned();
        let (
            title,
            backend_label,
            branch,
            role_key,
            member_name,
            identity_unknown,
            member_missing,
            execution_path,
            shared_worktree,
            worktree_released,
            archived,
        ) = if let Some(s) = session_meta.as_ref() {
            self.build_split_peek_meta(s)
        } else {
            (
                tr("成员会话").to_owned(),
                String::new(),
                None,
                "会话",
                None,
                false,
                false,
                None,
                false,
                false,
                false,
            )
        };
        let remote_busy = self.busy.contains(&session_id);
        let pending_count = self.pending.get(&session_id).copied().unwrap_or(0);

        self.git_drawer_open = false;
        self.close_collaboration();
        self.split_peek = Some(crate::app::SplitPeekState {
            session_id: session_id.clone(),
            title,
            backend_label,
            branch,
            role_key,
            member_name,
            identity_unknown,
            member_missing,
            execution_path,
            shared_worktree,
            worktree_released,
            archived,
            remote_busy,
            pending_count,
            interaction_count: 0,
            session_snapshot: session_meta,
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
                let parsed_session = res
                    .as_ref()
                    .ok()
                    .and_then(|value| {
                        serde_json::from_value::<crate::protocol::Session>(value.clone()).ok()
                    });
                let meta_tuple = parsed_session
                    .as_ref()
                    .map(|s| this.build_split_peek_meta(s));
                let Some(peek) = this.split_peek.as_mut() else {
                    return;
                };
                if peek.session_id != session_id {
                    return;
                }
                peek.loading = false;
                match res {
                    Ok(value) => {
                        if let Some((
                            title,
                            backend_label,
                            branch,
                            role_key,
                            member_name,
                            identity_unknown,
                            member_missing,
                            execution_path,
                            shared_worktree,
                            worktree_released,
                            archived,
                        )) = meta_tuple
                        {
                            peek.title = title;
                            peek.backend_label = backend_label;
                            peek.branch = branch;
                            peek.role_key = role_key;
                            peek.member_name = member_name;
                            peek.identity_unknown = identity_unknown;
                            peek.member_missing = member_missing;
                            peek.execution_path = execution_path;
                            peek.shared_worktree = shared_worktree;
                            peek.worktree_released = worktree_released;
                            peek.archived = archived;
                        }
                        if let Some(session) = parsed_session {
                            peek.session_snapshot = Some(session);
                        }
                        peek.remote_busy = value
                            .get("busy")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        peek.pending_count = value
                            .get("pending")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        peek.interaction_count = value
                            .get("interactions")
                            .and_then(|v| v.as_array())
                            .map_or(0, Vec::len);

                        let extract_text = |item: &serde_json::Value| -> String {
                            let mut text = String::new();
                            if let Some(s) = item.get("content").and_then(|c| c.as_str()) {
                                text.push_str(s);
                            } else if let Some(blocks) =
                                item.get("content").and_then(|c| c.as_array())
                            {
                                for b in blocks {
                                    if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                                        if !text.is_empty() {
                                            text.push('\n');
                                        }
                                        text.push_str(t);
                                    }
                                }
                            }
                            text
                        };

                        let mut msgs = Vec::new();
                        if let Some(arr) = value.get("history").and_then(|h| h.as_array()) {
                            for item in arr.iter().rev().take(30).rev() {
                                let role = item
                                    .get("role")
                                    .and_then(|r| r.as_str())
                                    .unwrap_or("assistant")
                                    .to_owned();
                                let text = extract_text(item);
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
                        if let Some(active) = value
                            .get("activeMessage")
                            .and_then(|am| am.get("message").or(Some(am)))
                        {
                            let text = extract_text(active);
                            let trimmed = text.trim();
                            if !trimmed.is_empty() {
                                let bounded: String = if trimmed.chars().count() > 800 {
                                    format!("{}…", trimmed.chars().take(800).collect::<String>())
                                } else {
                                    trimmed.to_owned()
                                };
                                msgs.push(crate::app::SplitPeekMessage {
                                    role: active
                                        .get("role")
                                        .and_then(|r| r.as_str())
                                        .unwrap_or("assistant")
                                        .to_owned(),
                                    text: bounded,
                                });
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

    pub(crate) fn split_peek_origin_label(peek: &crate::app::SplitPeekState) -> String {
        if peek.identity_unknown {
            tr("只读历史 · 归属未知").to_owned()
        } else if let Some(name) = &peek.member_name {
            if name == &peek.title {
                tr(peek.role_key).to_owned()
            } else {
                format!("{} · {name}", tr(peek.role_key))
            }
        } else if peek.member_missing {
            format!("{} · {}", tr(peek.role_key), tr("已移除成员"))
        } else {
            tr(peek.role_key).to_owned()
        }
    }

    pub(crate) fn render_split_peek_panel(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let peek = self.split_peek.as_ref()?;
        let sid_promote = peek.session_id.clone();
        let is_current_main = self.session.as_ref().is_some_and(|s| s.id == sid_promote);
        let can_resolve_session = self.sessions.iter().any(|s| s.id == sid_promote)
            || peek.session_snapshot.as_ref().is_some_and(|s| {
                self.workspace.as_deref() == Some(s.workspace_id.as_str())
                    && !self.removed_sessions.contains(&s.id)
            });
        let can_promote_write =
            self.connected && !self.closing && self.workspace_writes_allowed();
        let can_promote = is_current_main || (can_promote_write && can_resolve_session);
        let promote_tooltip = if is_current_main {
            tr("已为主会话，点击关闭分屏")
        } else if !can_promote_write {
            tr("当前只读或未连接，无法切换主会话")
        } else if !can_resolve_session {
            tr("会话快照尚未就绪或不属于当前工作区")
        } else {
            tr("切为主会话")
        };
        let is_busy = peek.remote_busy || self.busy.contains(&peek.session_id);
        let status_dot = if peek.interaction_count > 0 {
            crate::chrome::attention_dot(cx)
        } else if is_busy {
            Color::Accent.get(cx)
        } else if peek.error.is_some() {
            Color::Error.get(cx)
        } else if peek.archived {
            Color::Muted.get(cx)
        } else {
            Color::Success.get(cx)
        };
        let origin_label = Self::split_peek_origin_label(peek);
        let state_label = if peek.interaction_count > 0 {
            Some((tr("等待确认"), crate::chrome::attention_dot(cx)))
        } else if is_busy {
            Some((tr("运行中"), Color::Accent.get(cx)))
        } else if peek.archived {
            Some((tr("已归档"), Color::Muted.get(cx)))
        } else {
            None
        };
        let exec_label = if peek.worktree_released {
            Some(tr("worktree 已解除").to_owned())
        } else if let Some(branch) = &peek.branch {
            if peek.shared_worktree {
                Some(format!("{} · {branch}", tr("共享 worktree")))
            } else {
                Some(format!("{} · {branch}", tr("独立 worktree")))
            }
        } else {
            peek.execution_path.as_deref().map(|path| {
                let base = std::path::Path::new(path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(path);
                format!("{} · {base}", tr("目录"))
            })
        };
        let exec_tooltip = peek
            .execution_path
            .as_deref()
            .map(palette::display_path);
        let snapshot_for_promote = peek.session_snapshot.clone();

        Some(
            v_flex()
                .id("split-peek-right-drawer")
                .track_focus(self.workspace_focus.region(crate::workspace_focus::Area::SplitPeek))
                .tab_stop(false)
                .role(accesskit::Role::Region)
                .aria_label(tr("分屏对照"))
                .focus(|style| style.border_color(Color::Accent.get(cx)))
                .w(px(400.))
                .h_full()
                .flex_shrink_0()
                .border_l_1()
                .border_color(Color::Divider.get(cx))
                .bg(Color::Surface.get(cx))
                .child(
                    v_flex()
                        .w_full()
                        .gap_1()
                        .px_3()
                        .py_2()
                        .bg(Color::Sidebar.get(cx))
                        .border_b_1()
                        .border_color(Color::Divider.get(cx))
                        .child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .items_center()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .size(px(6.))
                                                .flex_shrink_0()
                                                .rounded_full()
                                                .bg(status_dot),
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
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1()
                                        .when(cfg!(test), |row| {
                                            row.on_children_prepainted(|bounds, _, _| {
                                                #[cfg(test)]
                                                if let Some(promote) = bounds.get(1) {
                                                    crate::app::record_test_bounds(
                                                        "split-peek-promote",
                                                        *promote,
                                                    );
                                                }
                                                let _ = bounds;
                                            })
                                        })
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
                                                .custom(palette::history_nav_variant(cx))
                                                .compact()
                                                .xsmall()
                                                .h(px(24.))
                                                .px_2()
                                                .rounded_md()
                                                .label(if is_current_main {
                                                    tr("当前主会话")
                                                } else {
                                                    tr("切为主会话")
                                                })
                                                .tooltip(promote_tooltip)
                                                .accessibility_label(tr("切为主会话"))
                                                .disabled(!can_promote)
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    if is_current_main {
                                                        this.split_peek = None;
                                                        let focus = this.input.read(cx).focus_handle(cx);
                                                        window.on_next_frame(move |window, cx| {
                                                            focus.focus(window, cx);
                                                        });
                                                        cx.notify();
                                                        return;
                                                    }
                                                    if !this.connected
                                                        || this.closing
                                                        || !this.workspace_writes_allowed()
                                                    {
                                                        return;
                                                    }
                                                    if !this.sessions.iter().any(|s| s.id == sid_promote) {
                                                        if let Some(snapshot) = snapshot_for_promote.clone() {
                                                            if this.workspace.as_deref() == Some(snapshot.workspace_id.as_str())
                                                                && !this.removed_sessions.contains(&snapshot.id)
                                                            {
                                                                this.sessions.push(snapshot);
                                                            }
                                                        }
                                                    }
                                                    if this.sessions.iter().any(|s| s.id == sid_promote) {
                                                        this.open_member_session(sid_promote.clone(), window, cx);
                                                        if this.session.as_ref().is_some_and(|s| s.id == sid_promote) {
                                                            this.split_peek = None;
                                                        }
                                                    }
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
                            h_flex()
                                .id("split-peek-meta-strip")
                                .w_full()
                                .min_w_0()
                                .flex_wrap()
                                .items_center()
                                .gap_1p5()
                                .text_size(px(10.5))
                                .text_color(Color::Muted.get(cx))
                                .when_some(exec_tooltip, |row, tip| {
                                    row.tooltip(move |window, cx| {
                                        gpui_kit::component::tooltip::Tooltip::new(tip.clone())
                                            .build(window, cx)
                                    })
                                })
                                .child(
                                    div()
                                        .px_1p5()
                                        .py(px(0.5))
                                        .rounded_sm()
                                        .bg(Color::AccentSoft.get(cx))
                                        .text_color(Color::Accent.get(cx))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(origin_label),
                                )
                                .when(!peek.backend_label.is_empty(), |row| {
                                    row.child(div().child(peek.backend_label.clone()))
                                })
                                .when_some(state_label, |row, (label, color)| {
                                    row.child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(color)
                                            .child(format!("· {label}")),
                                    )
                                })
                                .when_some(exec_label, |row, label| {
                                    row.child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .child(format!("· {label}")),
                                    )
                                }),
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
            .when(highlight_report || state == "waiting_interaction", |row| {
                row.border_1()
                    .border_color(crate::chrome::attention_dot(cx).opacity(0.55))
            })
            .when(state == "error", |row| {
                row.border_1().border_color(Color::Error.get(cx).opacity(0.55))
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
        let _locale = crate::language::test_locale_guard();
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

    #[gpui_kit::test]
    fn split_peek_orchestrator_in_members_uses_orchestrator_badge_and_offline_promote_is_blocked(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use crate::app::{HiveApp, clear_test_bounds, get_test_bounds, init_test_globals};
        use crate::protocol::{Session, Workspace};
        use gpui_kit::{
            AppContext as _, Bounds, MouseButton, VisualTestContext, WindowBounds, WindowOptions,
            open_window,
        };
        use serde_json::json;

        let _locale = init_test_globals(cx);
        clear_test_bounds();
        let (window, app) = cx.update(|cx| {
            let bounds = Bounds::maximized(None, cx);
            open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| HiveApp::new_for_test(window, cx)),
            )
            .unwrap()
        });

        let lead_session: Session = serde_json::from_value(json!({
            "id": "s-lead",
            "title": "主会话",
            "workspaceId": "w1",
            "agentId": "w1:orchestrator",
            "originAgentId": "w1:orchestrator"
        }))
        .unwrap();
        let main_session: Session = serde_json::from_value(json!({
            "id": "s-main",
            "title": "Other Session",
            "workspaceId": "w1"
        }))
        .unwrap();

        app.update(cx, |app, cx| {
            app.workspaces = vec![Workspace {
                id: "w1".into(),
                name: "Alpha".into(),
                path: "/tmp/a".into(),
            }];
            app.workspace = Some("w1".into());
            app.members = vec![Member {
                id: "w1:orchestrator".into(),
                description: String::new(),
                name: "Lead Codex".into(),
                role: "orchestrator".into(),
                status: "idle".into(),
                pending_task_count: 0,
                configured_command: None,
                desktop_model: None,
                desktop_profile: None,
                desktop_launch_error: None,
                desktop_activity: None,
            }];
            app.sessions = vec![main_session.clone(), lead_session.clone()];
            app.session = Some(main_session);
            app.connected = true;
            app.open_split_peek_session("s-lead".into(), cx);

            let peek = app.split_peek.as_ref().expect("split_peek opened");
            assert_eq!(peek.role_key, "主控");
            let origin_zh = HiveApp::split_peek_origin_label(peek);
            assert!(
                origin_zh.starts_with("主控"),
                "expected orchestrator label starting with 主控, got {origin_zh:?}"
            );
            assert!(!origin_zh.contains("成员"));

            gpui_kit::component::set_locale("en");
            let origin_en = HiveApp::split_peek_origin_label(peek);
            assert!(
                origin_en.starts_with("Orchestrator"),
                "expected English orchestrator label, got {origin_en:?}"
            );
            gpui_kit::component::set_locale("zh-CN");

            // Simulate backend disconnect while SplitPeek is open.
            app.connected = false;
            cx.notify();
        });

        let mut vcx = VisualTestContext::from_window(window, cx);
        vcx.run_until_parked();

        let promote_bounds =
            get_test_bounds("split-peek-promote").expect("split-peek-promote button must render");
        vcx.simulate_mouse_down(promote_bounds.center(), MouseButton::Left, Default::default());
        vcx.simulate_mouse_up(promote_bounds.center(), MouseButton::Left, Default::default());
        vcx.run_until_parked();

        vcx.read(|cx| {
            let app = app.read(cx);
            assert!(
                app.split_peek.is_some(),
                "clicking split-peek-promote while disconnected must preserve split_peek"
            );
            assert_eq!(
                app.session.as_ref().map(|s| s.id.as_str()),
                Some("s-main"),
                "disconnected split-peek-promote must not switch active session"
            );
        });
    }
}
