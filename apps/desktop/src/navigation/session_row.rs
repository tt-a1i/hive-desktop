use super::*;
use crate::language::{english, tr};
use gpui_kit::component::menu::PopupMenu;

// Button has a stable id and forwards InteractiveElement to its base, but
// does not expose StatefulInteractiveElement's accessibility setters. This
// adapter only configures that same base; it never renders another node.
struct SemanticButton(Button);
impl InteractiveElement for SemanticButton {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.0.interactivity()
    }
}
impl StatefulInteractiveElement for SemanticButton {}

struct MenuPresentation {
    button_focus: FocusHandle,
    region_focus: FocusHandle,
    open: bool,
}

/// Keep the existing popover's mouse and keyboard paths. Accessibility Invoke
/// needs an explicit semantic action on the child Button, not a center click on
/// its normally hover-only parent.
#[derive(IntoElement)]
struct AccessibleSessionMenu {
    button: Button,
    focus_key: SharedString,
    row_group: SharedString,
    session: crate::protocol::Session,
    archive_blocked: bool,
    disabled: bool,
    app: WeakEntity<HiveApp>,
}
impl RenderOnce for AccessibleSessionMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let presentation =
            window.use_keyed_state(self.focus_key.clone(), cx, |_, cx| MenuPresentation {
                button_focus: cx.focus_handle(),
                region_focus: cx.focus_handle().tab_stop(false),
                open: false,
            });
        let focus = presentation.read(cx).button_focus.clone();
        let region = presentation.read(cx).region_focus.clone();
        let visible = presentation.read(cx).open || region.contains_focused(window, cx);
        let app = self.app.clone();
        let id = self.session.id.clone();
        let project = self.session.workspace_id.clone();
        let mut button = SemanticButton(self.button.track_focus(&focus));
        if !self.disabled {
            button = button.on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                // Recheck live state in case the queued AX request predates a
                // connection, metadata-save or workspace-removal update.
                let enabled = app.upgrade().is_some_and(|app| {
                    let app = app.read(cx);
                    app.connected
                        && !app.session_pin_loading.contains(&id)
                        && !app.session_archive_loading.contains(&id)
                        && app.workspace_removing.as_ref() != Some(&project)
                });
                cx.stop_propagation();
                if !enabled {
                    return;
                }
                focus.focus(window, cx);
                focus.dispatch_action(
                    &gpui_kit::base::actions::Confirm { secondary: false },
                    window,
                    cx,
                );
            });
        }
        let menu = button
            .0
            .dropdown_menu(move |menu, _, cx| {
                HiveApp::session_menu_items(
                    menu,
                    self.session.clone(),
                    self.archive_blocked,
                    self.app.clone(),
                    cx,
                )
            })
            .on_open_change(move |open, window, cx| {
                presentation.update(cx, |state, _| state.open = *open);
                window.refresh();
            });
        // Visibility::Hidden skips paint, including AX and keyboard listeners.
        // Opacity preserves those listeners and the existing positioned hitbox.
        // The region also covers DropdownMenu's restored trigger focus.
        div()
            .id(self.focus_key)
            .absolute()
            .right_1()
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .track_focus(&region)
            .tab_stop(false)
            .opacity(if visible { 1. } else { 0. })
            .group_hover(self.row_group, |style| style.opacity(1.))
            .child(menu)
    }
}

impl HiveApp {
    pub(crate) fn session_archive_blocked(&self, id: &str) -> bool {
        self.busy.contains(id)
            || self.pending.get(id).copied().unwrap_or(0) > 0
            || self.interaction_count(id) > 0
    }

    /// Shared by sidebar and title menus, with the original session identity
    /// and archive guard captured for this menu opening.
    pub(crate) fn session_menu_items(
        menu: PopupMenu,
        session: crate::protocol::Session,
        archive_blocked: bool,
        app: WeakEntity<Self>,
        cx: &App,
    ) -> PopupMenu {
        let archive_session = session.clone();
        let archive_app = app.clone();
        let worktree_session = session.clone();
        let worktree_app = app.clone();
        let scratch_session = session.clone();
        let scratch_app = app.clone();
        let linked = app
            .upgrade()
            .is_some_and(|app| app.read(cx).scratch_project_target(&session).is_some());
        menu.item(
            PopupMenuItem::new(if session.pinned {
                tr("取消置顶")
            } else {
                tr("置顶会话")
            })
            .on_click(move |_, window, cx| {
                let _ = app.update(cx, |this, cx| {
                    this.request_session_pinned(session.clone(), !session.pinned, window, cx)
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
            .on_click(move |_, window, cx| {
                let _ = archive_app.update(cx, |this, cx| {
                    this.request_session_archived(
                        archive_session.clone(),
                        !archive_session.archived,
                        window,
                        cx,
                    )
                });
            }),
        )
        .when(
            worktree_session.execution.as_ref().is_some_and(|execution| execution.kind == "worktree"),
            |menu| menu.item(
                PopupMenuItem::new(tr("清理独立工作树…"))
                    .on_click(move |_, window, cx| {
                        let _ = worktree_app.update(cx, |this, cx|
                            this.open_worktree_removal(worktree_session.clone(), window, cx));
                    }),
            ),
        )
        .when(scratch_session.scratch, |menu| {
            menu.item(PopupMenuItem::new(tr(if linked { "打开 Scratch 项目" } else { "将 Scratch 文件夹添加为项目…" }))
                .on_click(move |_, window, cx| {
                    let app = scratch_app.clone();
                    let session = scratch_session.clone();
                    window.defer(cx, move |window, cx| {
                        let _ = app.update(cx, |this, cx| this.show_scratch_project(session, window, cx));
                    });
                }))
        })
    }

    fn session_activity(&self, id: &str) -> Option<&'static str> {
        if self.interaction_count(id) > 0 {
            Some(tr("待回应"))
        } else if self.busy.contains(id) {
            Some(tr("处理中"))
        } else if self.pending.get(id).copied().unwrap_or(0) > 0 {
            Some(tr("已暂停"))
        } else if self.unread.contains(id) {
            Some(tr("未读"))
        } else {
            None
        }
    }

    fn workspace_display_name(&self, workspace_id: &str) -> String {
        let matches = self
            .workspaces
            .iter()
            .filter(|workspace| workspace.id == workspace_id)
            .collect::<Vec<_>>();
        let Some(workspace) = matches.first() else {
            return "Hive".to_owned();
        };
        let duplicate = self
            .workspaces
            .iter()
            .filter(|item| item.name == workspace.name)
            .count()
            > 1;
        if duplicate {
            std::path::Path::new(&workspace.path)
                .components()
                .rev()
                .take(2)
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join(" · ")
        } else {
            workspace.name.clone()
        }
    }

    pub(super) fn render_session_link(
        &self,
        session: &crate::protocol::Session,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let workspace_name = self.workspace_display_name(&session.workspace_id);
        self.render_session_row(
            session,
            self.session_title(session).to_owned(),
            workspace_name,
            false,
            cx,
        )
    }

    fn render_session_row(
        &self,
        session: &crate::protocol::Session,
        title: String,
        workspace_name: String,
        is_pinned_group: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let scope = if is_pinned_group { "pinned" } else { "session" };
        let project = session.workspace_id.clone();
        let id = session.id.clone();
        let row_group = SharedString::from(format!("{scope}-row-{id}"));
        let role = tr(session.display_role_key());
        let activity_label = self.session_activity(&id);
        let activity = activity_label
            .map(|label| format!(" · {label}"))
            .unwrap_or_default();
        let dot_color = if self.interaction_count(&id) > 0 {
            Some(Color::Error.get(cx))
        } else if self.busy.contains(&id) {
            Some(Color::Accent.get(cx))
        } else if self.pending.get(&id).copied().unwrap_or(0) > 0 {
            Some(Color::Muted.get(cx))
        } else if self.unread.contains(&id) {
            Some(Color::Success.get(cx))
        } else {
            None
        };
        let archive_blocked = self.session_archive_blocked(&id);
        let menu_session = session.clone();
        let weak = cx.weak_entity();
        let pinning =
            self.session_pin_loading.contains(&id) || self.session_archive_loading.contains(&id);
        let project_tooltip = self
            .workspaces
            .iter()
            .find(|workspace| workspace.id == project)
            .map(|workspace| format!(" · {workspace_name} · {}", workspace.path))
            .unwrap_or_else(|| format!(" · {workspace_name}"));
        let is_selected = self.viewing_conversation()
            && self.session.as_ref().is_some_and(|s| s.id == session.id);
        let worktree_branch = session
            .execution
            .as_ref()
            .filter(|e| e.kind == "worktree")
            .map(|e| e.branch.trim())
            .filter(|b| !b.is_empty())
            .map(str::to_owned);
        let relative_time = palette::format_session_time(
            session.updated_at,
            crate::general::current(cx).session_time_format,
            english(),
        );
        let menu_disabled =
            !self.connected || pinning || self.workspace_removing.as_ref() == Some(&project);
        let menu_button = Button::new(SharedString::from(format!("{scope}-menu-{id}")))
            .custom(palette::subtle_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(22.))
            .w(px(22.))
            .rounded_md()
            .icon(Glyph::Ellipsis)
            .tooltip(tr("会话操作"))
            .accessibility_label(if is_pinned_group {
                if english() {
                    format!("Pinned conversation actions: {title} · {workspace_name}")
                } else {
                    format!("{title} · {workspace_name} 的置顶会话操作")
                }
            } else if english() {
                format!("Conversation actions: {title}")
            } else {
                format!("{title} 的会话操作")
            })
            .disabled(menu_disabled);
        let menu = AccessibleSessionMenu {
            button: menu_button,
            focus_key: format!("session-menu-focus:{scope}:{id}").into(),
            row_group: row_group.clone(),
            session: menu_session,
            archive_blocked,
            disabled: menu_disabled,
            app: weak,
        };
        h_flex()
            .relative()
            .group(row_group.clone())
            .min_w_0()
            .w_full()
            .items_center()
            .child(
                div().flex_1().min_w_0().child(
                    Button::new(SharedString::from(format!("{scope}-{id}")))
                        .custom(palette::sidebar_item_variant(is_selected, cx))
                        .compact()
                        .h(px(30.))
                        .px_2()
                        .rounded_md()
                        .accessibility_label(format!(
                            "{}{} · {} · {role}{activity}{}",
                            if is_pinned_group {
                                format!("{} · {workspace_name} · ", tr("置顶"))
                            } else {
                                String::new()
                            },
                            tr(session.backend.label()),
                            title,
                            if session.pinned {
                                if english() {
                                    " · Pinned"
                                } else {
                                    " · 已置顶"
                                }
                            } else {
                                ""
                            }
                        ))
                        .w_full()
                        .min_w_0()
                        .child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .child(palette::project_badge(&workspace_name, 18., cx))
                                .when_some(dot_color, |row, color| {
                                    row.child(
                                        div().size(px(5.)).flex_shrink_0().rounded_full().bg(color),
                                    )
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_left()
                                        .text_size(px(13.))
                                        .text_color(if is_selected {
                                            Color::Text.get(cx)
                                        } else {
                                            Color::Secondary.get(cx)
                                        })
                                        .when(is_selected, |t| t.font_weight(FontWeight::MEDIUM))
                                        .truncate()
                                        .child(title.clone()),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1()
                                        .group_hover(row_group.clone(), |s| s.invisible())
                                        .when(
                                            session.pinned
                                                && !is_pinned_group
                                                && activity_label.is_none(),
                                            |row| {
                                                row.child(
                                                    Icon::new(Glyph::Pin)
                                                        .size(px(11.))
                                                        .flex_shrink_0()
                                                        .text_color(
                                                            Color::Muted.get(cx).opacity(0.65),
                                                        ),
                                                )
                                            },
                                        )
                                        .when_some(worktree_branch, |row, branch| {
                                            row.child(
                                                div()
                                                    .flex_shrink_0()
                                                    .max_w(px(64.))
                                                    .truncate()
                                                    .px_1()
                                                    .py(px(0.5))
                                                    .rounded_sm()
                                                    .bg(Color::Inset.get(cx))
                                                    .text_size(px(10.))
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(branch),
                                            )
                                        })
                                        .when_some(activity_label, |row, label| {
                                            row.child(
                                                div()
                                                    .flex_shrink_0()
                                                    .text_size(px(11.5))
                                                    .text_color(if is_selected {
                                                        Color::Secondary.get(cx)
                                                    } else {
                                                        Color::Muted.get(cx)
                                                    })
                                                    .child(label),
                                            )
                                        })
                                        .when(activity_label.is_none(), |row| {
                                            row.when_some(relative_time, |row, ts| {
                                                row.child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .text_size(px(11.5))
                                                        .text_color(if is_selected {
                                                            Color::Secondary.get(cx)
                                                        } else {
                                                            Color::Muted.get(cx)
                                                        })
                                                        .child(ts),
                                                )
                                            })
                                        }),
                                ),
                        )
                        .tooltip(format!(
                            "{} · {role} · {}{project_tooltip}",
                            title,
                            tr(session.backend.label())
                        ))
                        .disabled(self.workspace_removing.as_ref() == Some(&project))
                        .selected(is_selected)
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
        v_flex().min_w_0().gap_0p5().when(
            !state.records.is_empty() || state.loading || state.error.is_some(),
            |section| {
                section
                    .pb_2()
                    .child(
                        h_flex()
                            .group("pinned-section-header")
                            .w_full()
                            .items_center()
                            .gap_0p5()
                            .child(
                                div().flex_1().min_w_0().child(
                                    Button::new("toggle-pinned-section")
                                        .custom(palette::quiet_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(24.))
                                        .px_2()
                                        .w_full()
                                        .rounded_md()
                                        .accessibility_label(if state.collapsed {
                                            tr("展开置顶会话")
                                        } else {
                                            tr("收起置顶会话")
                                        })
                                        .child(
                                            h_flex()
                                                .flex_1()
                                                .w_full()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(tr("置顶")),
                                                )
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .gap_1()
                                                        .when(!state.records.is_empty(), |r| {
                                                            r.child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .text_color(
                                                                        Color::Muted.get(cx),
                                                                    )
                                                                    .child(
                                                                        state
                                                                            .records
                                                                            .len()
                                                                            .to_string(),
                                                                    ),
                                                            )
                                                        })
                                                        .child(
                                                            Icon::new(if state.collapsed {
                                                                Glyph::ChevronRight
                                                            } else {
                                                                Glyph::ChevronDown
                                                            })
                                                            .size(px(12.))
                                                            .text_color(Color::Muted.get(cx)),
                                                        ),
                                                ),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.pinned_sessions.collapsed =
                                                !this.pinned_sessions.collapsed;
                                            cx.notify();
                                        })),
                                ),
                            )
                            .child(
                                div()
                                    .invisible()
                                    .group_hover("pinned-section-header", |s| s.visible())
                                    .child(
                                        Button::new("refresh-pinned-section")
                                            .custom(palette::subtle_icon_variant(cx))
                                            .compact()
                                            .xsmall()
                                            .h(px(20.))
                                            .w(px(20.))
                                            .rounded_md()
                                            .icon(Glyph::RefreshCw)
                                            .tooltip(tr("刷新置顶会话"))
                                            .accessibility_label(tr("刷新置顶会话"))
                                            .disabled(!self.connected || state.loading)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.refresh_pinned_sessions(cx);
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .when(!state.collapsed, |section| {
                        section
                            .when(state.loading, |section| {
                                section.child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("正在更新置顶会话…")),
                                )
                            })
                            .when_some(state.error.as_ref(), |section, error| {
                                section.child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .text_xs()
                                        .text_color(Color::Error.get(cx))
                                        .child(format!(
                                            "{}: {}",
                                            tr("读取置顶失败"),
                                            crate::language::localize_owned_error(error)
                                        )),
                                )
                            })
                            .children(state.records.iter().map(|hit| {
                                let project =
                                    self.workspace_display_name(&hit.session.workspace_id);
                                self.render_session_row(
                                    &hit.session,
                                    crate::view::untitled_session_label(&hit.session)
                                        .unwrap_or(&hit.title)
                                        .to_owned(),
                                    project,
                                    true,
                                    cx,
                                )
                            }))
                    })
            },
        )
    }
}
