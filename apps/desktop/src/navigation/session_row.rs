use crate::language::{english, tr};
use super::*;

impl HiveApp {
    pub(super) fn render_session_link(
        &self,
        session: &crate::protocol::Session,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        self.render_session_row(session, self.session_title(session).to_owned(), None, cx)
    }

    fn render_session_row(
        &self,
        session: &crate::protocol::Session,
        title: String,
        project_name: Option<String>,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let scope = if project_name.is_some() {
            "pinned"
        } else {
            "session"
        };
        let project = session.workspace_id.clone();
        let id = session.id.clone();
        let role = session
            .agent_id
            .as_deref()
            .map(|agent| {
                if agent.ends_with(":orchestrator") {
                    tr("主控")
                } else {
                    tr("成员")
                }
            })
            .unwrap_or(tr("会话"));
        let activity = self
            .session_activity(&id)
            .map(|label| format!(" · {label}"))
            .unwrap_or_default();
        let archive_blocked = self.busy.contains(&id)
            || self.pending.get(&id).copied().unwrap_or(0) > 0
            || self.interaction_count(&id) > 0;
        let menu_session = session.clone();
        let weak = cx.weak_entity();
        let pinning =
            self.session_pin_loading.contains(&id) || self.session_archive_loading.contains(&id);
        let project_tooltip = project_name
            .as_ref()
            .map(|name| {
                self.workspaces
                    .iter()
                    .find(|workspace| workspace.id == project)
                    .map(|workspace| format!(" · {name} · {}", workspace.path))
                    .unwrap_or_else(|| format!(" · {name}"))
            })
            .unwrap_or_default();
        let menu = Button::new(SharedString::from(format!("{scope}-menu-{id}")))
            .ghost()
            .compact()
            .h(px(28.))
            .w(px(28.))
            .icon(Glyph::Ellipsis)
            .tooltip(tr("会话操作"))
            .accessibility_label(
                project_name
                    .as_ref()
                    .map(|name| if english() { format!("Pinned conversation actions: {title} · {name}") } else { format!("{title} · {name} 的置顶会话操作") })
                    .unwrap_or_else(|| if english() { format!("Conversation actions: {title}") } else { format!("{title} 的会话操作") }),
            )
            .disabled(
                !self.connected || pinning || self.workspace_removing.as_ref() == Some(&project),
            )
            .dropdown_menu(move |menu, _, _| {
                let session = menu_session.clone();
                let weak = weak.clone();
                let archive_session = session.clone();
                let archive_app = weak.clone();
                menu.item(
                    PopupMenuItem::new(if session.pinned {
                        tr("取消置顶")
                    } else {
                        tr("置顶会话")
                    })
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.set_session_pinned(session.clone(), !session.pinned, cx)
                        });
                    }),
                )
                .item(
                    PopupMenuItem::new(if archive_session.archived {
                        tr("恢复会话")
                    } else if archive_blocked {
                        tr("归档会话（需先处理当前任务）")
                    } else {
                        tr("归档会话")
                    })
                    .disabled(!archive_session.archived && archive_blocked)
                    .on_click(move |_, _, cx| {
                        let _ = archive_app.update(cx, |this, cx| {
                            this.set_session_archived(
                                archive_session.clone(),
                                !archive_session.archived,
                                cx,
                            )
                        });
                    }),
                )
            });
        h_flex()
            .min_w_0()
            .w_full()
            .child(
                div().flex_1().min_w_0().child(
                    Button::new(SharedString::from(format!("{scope}-{id}")))
                        .ghost()
                        .h(if project_name.is_some() {
                            px(46.)
                        } else {
                            px(30.)
                        })
                        .rounded_lg()
                        .accessibility_label(format!(
                            "{}{} · {} · {role}{activity}{}",
                            project_name
                                .as_ref()
                                .map(|name| format!("{} · {name} · ", tr("置顶")))
                                .unwrap_or_default(),
                            tr(session.backend.label()),
                            title,
                            if session.pinned { if english() { " · Pinned" } else { " · 已置顶" } } else { "" }
                        ))
                        .w_full()
                        .min_w_0()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .items_start()
                                .child(div().w_full().truncate().child(title.clone()))
                                .when_some(project_name, |row, name| {
                                    row.child(
                                        div()
                                            .text_xs()
                                            .w_full()
                                            .truncate()
                                            .text_color(Color::Muted.get(cx))
                                            .child(name),
                                    )
                                }),
                        )
                        .when(session.pinned && scope != "pinned", |row| {
                            row.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("置顶")),
                            )
                        })
                        .when(!activity.is_empty(), |row| {
                            row.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(activity.trim_start_matches(" · ").to_owned()),
                            )
                        })
                        .tooltip(format!(
                            "{} · {role} · {}{project_tooltip}",
                            title,
                            tr(session.backend.label())
                        ))
                        .disabled(self.workspace_removing.as_ref() == Some(&project))
                        .selected(
                            self.viewing_conversation()
                                && self.session.as_ref().is_some_and(|s| s.id == session.id),
                        )
                        .on_click(cx.listener({
                            let session = session.clone();
                            move |this, _, window, cx| {
                                if !this.navigate_to_session(session.clone(), window, cx) {
                                    this.error =
                                        Some("工作区正在移除或已不存在，请重新读取。".into());
                                    this.refresh_pinned_sessions(cx);
                                    cx.notify();
                                }
                            }
                        })),
                ),
            )
            .child(menu)
    }

    pub(super) fn render_pinned_navigation(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let state = &self.pinned_sessions;
        v_flex().min_w_0().gap_1().when(
            !state.records.is_empty() || state.loading || state.error.is_some(),
            |section| {
                section
                    .pb_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                Button::new("toggle-pinned-section")
                                    .ghost()
                                    .compact()
                                    .icon(if state.collapsed {
                                        Glyph::ChevronRight
                                    } else {
                                        Glyph::ChevronDown
                                    })
                                    .label(tr("置顶"))
                                    .toggled(!state.collapsed)
                                    .accessibility_label(if state.collapsed {
                                        tr("展开置顶会话")
                                    } else {
                                        tr("收起置顶会话")
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.pinned_sessions.collapsed =
                                            !this.pinned_sessions.collapsed;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("refresh-pinned-section")
                                    .ghost()
                                    .compact()
                                    .icon(Glyph::RefreshCw)
                                    .tooltip(tr("刷新置顶会话"))
                                    .accessibility_label(tr("刷新置顶会话"))
                                    .disabled(!self.connected || state.loading)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.refresh_pinned_sessions(cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .when(!state.collapsed, |section| {
                        section
                            .when(state.loading, |section| {
                                section.child(
                                    div()
                                        .px_2()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("正在更新置顶会话…")),
                                )
                            })
                            .when_some(state.error.as_ref(), |section, error| {
                                section.child(
                                    div()
                                        .px_2()
                                        .text_xs()
                                        .text_color(Color::Error.get(cx))
                                        .child(format!("{}: {error}", tr("读取置顶失败"))),
                                )
                            })
                            .children(state.records.iter().map(|hit| {
                                let duplicate = self
                                    .workspaces
                                    .iter()
                                    .filter(|workspace| workspace.name == hit.workspace_name)
                                    .count()
                                    > 1;
                                let project = if duplicate {
                                    self.workspaces
                                        .iter()
                                        .find(|workspace| workspace.id == hit.session.workspace_id)
                                        .map(|workspace| {
                                            std::path::Path::new(&workspace.path)
                                                .components()
                                                .rev()
                                                .take(2)
                                                .map(|part| part.as_os_str().to_string_lossy())
                                                .collect::<Vec<_>>()
                                                .join(" · ")
                                        })
                                        .unwrap_or_else(|| hit.workspace_name.clone())
                                } else {
                                    hit.workspace_name.clone()
                                };
                                self.render_session_row(
                                    &hit.session,
                                    hit.title.clone(),
                                    Some(project),
                                    cx,
                                )
                            }))
                    })
            },
        )
    }
}
