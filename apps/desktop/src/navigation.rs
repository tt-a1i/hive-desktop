use crate::chrome;
use crate::focus_scroll::{Reveal, Viewport};
use crate::language::tr;
mod session_row;
mod settings;
use crate::app::HiveApp;
use crate::palette::{self, Color};
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        menu::{DropdownMenu, PopupMenuItem},
        *,
    },
    prelude::FluentBuilder,
    *,
};

impl HiveApp {
    pub(crate) fn has_active_team_context(&self) -> bool {
        let is_worker =
            |id: &str| !id.ends_with(":orchestrator") && !id.ends_with(":__workflow__");
        self.member_panel
            || self.collaboration.visible
            || !self.pending_reports.is_empty()
            || self.members.iter().any(|member| is_worker(&member.id))
            || self
                .sessions
                .iter()
                .any(|session| session.agent_id.as_deref().is_some_and(is_worker))
    }

    pub(crate) fn show_team(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_workflows();
        self.close_memory();
        self.close_settings();
        self.close_collaboration();
        self.close_roles();
        self.hide_terminal(cx);
        self.member_panel = true;
        let focus = self.member_search.read(cx).focus_handle(cx);
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
        if window.viewport_size().width < px(820.) {
            self.sidebar_open = false;
        }
        self.refresh_members(cx);
        self.load_roles(cx);
        cx.notify();
    }

    pub(crate) fn viewing_conversation(&self) -> bool {
        !(self.terminal.visible
            || self.member_panel
            || self.memory.visible
            || self.roles.visible
            || self.settings_panel
            || self.collaboration.visible
            || self.workflows.visible)
    }

    pub(crate) fn return_to_chat_button(&self, cx: &Context<Self>) -> Button {
        Button::new("return-chat")
            .ghost()
            .compact()
            .rounded_lg()
            .label(tr("返回会话"))
            .on_click(cx.listener(|this, _, window, cx| {
                this.hide_terminal(cx);
                this.close_memory();
                this.close_settings();
                this.close_roles();
                this.close_collaboration();
                this.close_workflows();
                this.member_panel = false;
                if window.viewport_size().width < px(820.) {
                    this.sidebar_open = false;
                }
                cx.notify();
            }))
    }

    /// Top bar used when the left sidebar is collapsed on non-chat subviews,
    /// or inside the left sidebar header itself (T3 Code inline traffic-lights + PanelLeft + wordmark).
    pub fn render_navigation_bar(
        &self,
        in_sidebar: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let inset = chrome::traffic_light_inset(window);
        let id = if in_sidebar {
            "sidebar-topbar"
        } else {
            "collapsed-topbar"
        };
        chrome::drag_bar(id, window, cx)
            .w_full()
            .gap_1p5()
            .when(in_sidebar, |bar| {
                bar.px_1()
                    .when(inset > px(0.), |b| b.pl(inset - px(4.)))
            })
            .when(!in_sidebar, |bar| {
                bar.px_3()
                    .border_b_1()
                    .border_color(Color::Divider.get(cx))
                    .when(inset > px(0.), |b| b.pl(inset + px(8.)))
            })
            .child(
                Button::new("toggle-navigation")
                    .custom(palette::quiet_icon_variant(cx))
                    .compact()
                    .xsmall()
                    .h(px(26.))
                    .w(px(26.))
                    .rounded_md()
                    .icon(Glyph::PanelLeft)
                    .tooltip(tr("打开或收起导航"))
                    .accessibility_label(tr("打开或收起工作区与会话导航"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_open = !this.sidebar_open;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_0p5()
                    .truncate()
                    .text_sm()
                    .font_weight(if in_sidebar {
                        FontWeight::BOLD
                    } else {
                        FontWeight::SEMIBOLD
                    })
                    .text_color(Color::Text.get(cx))
                    .child(if in_sidebar {
                        "Hive".to_owned()
                    } else {
                        self.workspaces
                            .iter()
                            .find(|workspace| Some(&workspace.id) == self.workspace.as_ref())
                            .map(|workspace| workspace.name.clone())
                            .unwrap_or_else(|| tr("桌面工作台").into())
                    }),
            )
            .when(!self.connected && self.runtime.is_some(), |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(tr("后台未连接")),
                )
            })
            .when(!self.viewing_conversation(), |view| {
                view.child(self.return_to_chat_button(cx))
            })
    }

    pub fn new_chat_menu(
        &self,
        id: &'static str,
        label: &'static str,
        project: Option<&crate::protocol::Workspace>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let weak = cx.weak_entity();
        let target = project.map(|workspace| workspace.id.clone());
        let project_label = project.map(|workspace| {
            if crate::language::english() {
                format!("New conversation in {}: choose an Agent", workspace.name)
            } else {
                format!("在 {} 中新建会话，选择 Agent 后端", workspace.name)
            }
        });
        Button::new(
            project
                .map(|workspace| SharedString::from(format!("new-project-chat-{}", workspace.id)))
                .unwrap_or_else(|| id.into()),
        )
        .when(project.is_some(), |button| {
            button
                .custom(palette::subtle_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(24.))
                .w(px(24.))
                .rounded_md()
        })
        .when(id == "new-session-menu", |b| {
            b.custom(palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(26.))
                .w(px(26.))
                .rounded_md()
        })
        .when(id == "home-agent", |button| {
            button
                .custom(palette::quiet_icon_variant(cx))
                .compact()
                .h(px(26.))
                .px_2()
                .rounded_lg()
                .dropdown_caret(true)
        })
        .icon(if id == "home-agent" {
            Glyph::Sparkles
        } else if project.is_some() {
            Glyph::Plus
        } else {
            Glyph::SquarePen
        })
        .when(id != "new-session-menu" && project.is_none(), |b| {
            b.label(label)
        })
        .tooltip(
            project_label
                .clone()
                .unwrap_or_else(|| tr("新建会话，选择 Agent 后端").into()),
        )
        .accessibility_label(project_label.unwrap_or_else(|| {
            if id == "new-session-menu" {
                tr("新建会话，选择 Agent 后端").into()
            } else {
                tr("选择主控 Agent").into()
            }
        }))
        .disabled(
            !self.connected
                || if let Some(project) = project {
                    self.workspace_removing.as_ref() == Some(&project.id)
                        || self
                            .attachments
                            .loading
                            .contains(&format!("workspace:{}", project.id))
                } else {
                    self.workspace.is_none()
                        || self.loading
                        || !self.workspace_writes_allowed()
                        || self.workspace.as_ref().is_some_and(|id| {
                            self.attachments
                                .loading
                                .contains(&format!("workspace:{id}"))
                        })
                },
        )
        .dropdown_menu(move |mut menu, _, _| {
            for (backend, label) in [
                ("pi", tr("内置 Pi")),
                ("codex", "Codex"),
                ("pi-rpc", tr("外部 Pi")),
                ("claude", "Claude Code"),
            ] {
                let weak = weak.clone();
                let target = target.clone();
                menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if window.viewport_size().width < px(820.) {
                            this.sidebar_open = false;
                        }
                        this.hide_terminal(cx);
                        if let Some(project) = &target {
                            this.new_session_in_project(project.clone(), backend, cx);
                        } else {
                            this.new_session(backend, cx);
                        }
                    });
                }));
            }
            menu
        })
    }

    fn render_more_navigation(&self, cx: &Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let has_workspace = self.workspace.is_some();
        let terminal_enabled = self.connected && has_workspace && self.workspace_writes_allowed();
        let connected = self.connected;
        let selected = [
            self.workflows.visible,
            self.memory.visible,
            self.terminal.visible,
        ];
        Button::new("navigation-more")
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(26.))
            .w(px(26.))
            .rounded_md()
            .icon(Glyph::Ellipsis)
            .tooltip(tr("更多工作区功能"))
            .accessibility_label(tr("更多工作区功能"))
            .selected(selected.into_iter().any(|active| active))
            .dropdown_menu(move |mut menu, _, _| {
                for (index, label, glyph, enabled) in [
                    (0, tr("工作流"), Glyph::Workflow, has_workspace),
                    (1, tr("工作区记忆"), Glyph::BookOpen, has_workspace),
                    (2, tr("工作区终端"), Glyph::SquareTerminal, terminal_enabled),
                    (3, tr("已归档会话"), Glyph::Archive, connected),
                ] {
                    if index == 3 {
                        menu = menu.separator();
                    }
                    let weak = weak.clone();
                    let current = selected.get(index).copied().unwrap_or(false);
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .icon(if current { Glyph::Check } else { glyph })
                            .checked(current)
                            .disabled(!enabled)
                            .on_click(move |_, window, cx| {
                                let _ = weak.update(cx, |this, cx| match index {
                                    0 => this.show_workflows(window, cx),
                                    1 => {
                                        this.hide_terminal(cx);
                                        this.open_memory(cx);
                                        if window.viewport_size().width < px(820.) {
                                            this.sidebar_open = false;
                                        }
                                        cx.notify();
                                    }
                                    2 => this.show_terminal(window, cx),
                                    _ => this.open_archived_sessions(window, cx),
                                });
                            }),
                    );
                }
                menu
            })
    }

    pub fn render_navigation(
        &self,
        narrow: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let show_team_footer = self.has_active_team_context();
        let terminal_enabled =
            self.connected && self.workspace.is_some() && self.workspace_writes_allowed();
        v_flex()
            .when(narrow, |view| view.flex_1().min_w_0())
            .when(!narrow, |view| view.w(px(252.)).flex_shrink_0())
            .h_full()
            .bg(Color::Sidebar.get(cx))
            .border_r_1()
            .border_color(Color::Divider.get(cx))
            .px_2()
            .pb_2()
            .gap_0p5()
            .child(self.render_navigation_bar(true, window, cx))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .px_0p5()
                    .pt_0p5()
                    .gap_0p5()
                    .child(
                        div().flex_1().min_w_0().child(
                            Button::new("search-sessions")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(28.))
                                .w_full()
                                .justify_start()
                                .rounded_md()
                                .icon(Glyph::Search)
                                .accessibility_label(tr("搜索会话"))
                                .tooltip(if cfg!(target_os = "macos") {
                                    tr("搜索会话 (⌘K)")
                                } else {
                                    tr("搜索会话 (Ctrl+K)")
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .text_left()
                                        .text_size(px(13.))
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("搜索")),
                                )
                                .disabled(!self.connected)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_session_search(window, cx)
                                })),
                        ),
                    )
                    .child(
                        Button::new("toggle-add-workspace")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_md()
                            .icon(Glyph::FolderPlus)
                            .selected(self.add_workspace_open)
                            .accessibility_label(if self.add_workspace_open {
                                tr("收起添加工作区")
                            } else {
                                tr("添加工作区")
                            })
                            .tooltip(if self.add_workspace_open {
                                tr("收起添加工作区")
                            } else {
                                tr("添加工作区")
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_workspace_open = !this.add_workspace_open;
                                cx.notify();
                            })),
                    )
                    .child(self.new_chat_menu("new-session-menu", tr("新建会话"), None, cx))
                    .child(self.render_more_navigation(cx)),
            )
            .child(Viewport::new(
                "workspace-navigation-viewport",
                &self.navigation_scroll,
                v_flex()
                    .id("workspace-session-navigation")
                    .mt_1p5()
                    .flex_1()
                    .min_w_0()
                    .w_full()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.navigation_scroll)
                    .px_0p5()
                    .py_1()
                    .child(self.render_pinned_navigation(cx))
                    .child(
                        v_flex()
                            .w_full()
                            .flex_shrink_0()
                            .gap_0p5()
                            .when(self.add_workspace_open, |section| {
                                section.child(
                                    v_flex()
                                        .mb_1p5()
                                        .p_2()
                                        .gap_1p5()
                                        .rounded_xl()
                                        .bg(Color::Card.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .child(Reveal::new(
                                            "workspace-folder-input-reveal",
                                            &self.navigation_scroll,
                                            Input::new(&self.folder)
                                                .aria_label(tr("工作区文件夹的绝对路径")),
                                        ))
                                        .child(
                                            h_flex()
                                                .gap_1p5()
                                                .child(Reveal::new(
                                                    "workspace-folder-picker-reveal",
                                                    &self.navigation_scroll,
                                                    Button::new("choose-workspace-folder")
                                                        .secondary()
                                                        .compact()
                                                        .label(tr("选择文件夹…"))
                                                        .disabled(!self.connected)
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.choose_workspace_folder(
                                                                    window, cx,
                                                                )
                                                            },
                                                        )),
                                                ))
                                                .child(Reveal::new(
                                                    "workspace-folder-add-reveal",
                                                    &self.navigation_scroll,
                                                    Button::new("add-workspace")
                                                        .primary()
                                                        .compact()
                                                        .label(tr("添加文件夹"))
                                                        .disabled(
                                                            !self.connected
                                                                || self.runtime.is_none(),
                                                        )
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.add_workspace(window, cx)
                                                            },
                                                        )),
                                                )),
                                        ),
                                )
                            })
                            .child(
                                Button::new("toggle-project-section")
                                    .custom(palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_1p5()
                                    .w_full()
                                    .rounded_md()
                                    .accessibility_label(if self.projects_collapsed {
                                        tr("展开工作区列表")
                                    } else {
                                        tr("收起工作区列表")
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
                                                    .child(tr("工作区")),
                                            )
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .when(!self.workspaces.is_empty(), |r| {
                                                        r.child(
                                                            div()
                                                                .text_size(px(11.))
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(
                                                                    self.workspaces
                                                                        .len()
                                                                        .to_string(),
                                                                ),
                                                        )
                                                    })
                                                    .child(
                                                        Icon::new(if self.projects_collapsed {
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
                                        this.projects_collapsed = !this.projects_collapsed;
                                        cx.notify();
                                    })),
                            )
                            .when(!self.projects_collapsed, |section| {
                                section.children(self.workspaces.iter().enumerate().map(
                                    |(i, workspace)| {
                                        let id = workspace.id.clone();
                                        let toggle_id = id.clone();
                                        let expanded = self.expanded_projects.contains(&id);
                                        let sessions = if self.workspace.as_ref() == Some(&id) {
                                            Some(&self.sessions)
                                        } else {
                                            self.project_sessions.get(&id)
                                        };
                                        v_flex()
                                            .min_w_0()
                                            .mt_0p5()
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .items_center()
                                                    .gap_0p5()
                                                    .child(
                                                        Button::new(("expand-project", i))
                                                            .custom(palette::subtle_icon_variant(
                                                                cx,
                                                            ))
                                                            .compact()
                                                            .xsmall()
                                                            .h(px(24.))
                                                            .w(px(20.))
                                                            .rounded_md()
                                                            .icon(if expanded {
                                                                Glyph::ChevronDown
                                                            } else {
                                                                Glyph::ChevronRight
                                                            })
                                                            .toggled(expanded)
                                                            .accessibility_label(
                                                                if crate::language::english() {
                                                                    format!(
                                                                        "{} {} conversations",
                                                                        if expanded {
                                                                            tr("收起")
                                                                        } else {
                                                                            tr("展开")
                                                                        },
                                                                        workspace.name
                                                                    )
                                                                } else {
                                                                    format!(
                                                                        "{} {} 的会话",
                                                                        if expanded {
                                                                            tr("收起")
                                                                        } else {
                                                                            tr("展开")
                                                                        },
                                                                        workspace.name
                                                                    )
                                                                },
                                                            )
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    this.toggle_project(
                                                                        toggle_id.clone(),
                                                                        cx,
                                                                    )
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        div().flex_1().min_w_0().child(
                                                            Button::new(("workspace", i))
                                                                .ghost()
                                                                .compact()
                                                                .rounded_md()
                                                                .h(px(28.))
                                                                .px_1p5()
                                                                .w_full()
                                                                .min_w_0()
                                                                .accessibility_label(
                                                                    workspace.name.clone(),
                                                                )
                                                                .child(
                                                                    h_flex()
                                                                        .flex_1()
                                                                        .min_w_0()
                                                                        .items_center()
                                                                        .gap_1p5()
                                                                        .child(
                                                                            palette::project_badge(
                                                                                &workspace.name,
                                                                                17.,
                                                                                cx,
                                                                            ),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .flex_1()
                                                                                .min_w_0()
                                                                                .text_left()
                                                                                .text_size(px(13.))
                                                                                .font_weight(
                                                                                    FontWeight::MEDIUM,
                                                                                )
                                                                                .truncate()
                                                                                .child(
                                                                                    workspace
                                                                                        .name
                                                                                        .clone(),
                                                                                ),
                                                                        ),
                                                                )
                                                                .tooltip(workspace.path.clone())
                                                                .selected(
                                                                    self.workspace.as_ref()
                                                                        == Some(&workspace.id)
                                                                        && self.session.is_none()
                                                                        && self
                                                                            .viewing_conversation(),
                                                                )
                                                                .disabled(
                                                                    self.workspace_removing.as_ref()
                                                                        == Some(&workspace.id),
                                                                )
                                                                .on_click(cx.listener(
                                                                    move |this, _, window, cx| {
                                                                        if this.terminal.visible {
                                                                            this.remember_terminal_focus(
                                                                                window, cx,
                                                                            );
                                                                        }
                                                                        this.select_workspace(
                                                                            id.clone(),
                                                                            cx,
                                                                        )
                                                                    },
                                                                )),
                                                        ),
                                                    )
                                                    .when(!expanded, |row| {
                                                        if let Some((label, detail)) = sessions
                                                            .and_then(|items| {
                                                                self.project_activity(items)
                                                            })
                                                        {
                                                            row.child(
                                                                Button::new(SharedString::from(
                                                                    format!(
                                                                        "project-activity-{}",
                                                                        workspace.id
                                                                    ),
                                                                ))
                                                                .ghost()
                                                                .compact()
                                                                .xsmall()
                                                                .h(px(22.))
                                                                .label(label)
                                                                .accessibility_label(
                                                                    if crate::language::english() {
                                                                        format!(
                                                                            "Expand {} conversations: {detail}",
                                                                            workspace.name
                                                                        )
                                                                    } else {
                                                                        format!(
                                                                            "展开 {} 的会话：{detail}",
                                                                            workspace.name
                                                                        )
                                                                    },
                                                                )
                                                                .tooltip(detail)
                                                                .on_click(cx.listener({
                                                                    let id = workspace.id.clone();
                                                                    move |this, _, _, cx| {
                                                                        this.toggle_project(
                                                                            id.clone(),
                                                                            cx,
                                                                        )
                                                                    }
                                                                })),
                                                            )
                                                        } else {
                                                            row
                                                        }
                                                    })
                                                    .child(self.new_chat_menu(
                                                        "project-new-chat",
                                                        tr("新建"),
                                                        Some(workspace),
                                                        cx,
                                                    ))
                                                    .when(
                                                        self.workspace.as_ref()
                                                            == Some(&workspace.id),
                                                        |row| {
                                                            row.child(self.project_menu(
                                                                workspace.id.clone(),
                                                                cx,
                                                            ))
                                                        },
                                                    ),
                                            )
                                            .when(expanded, |group| {
                                                let rows = v_flex()
                                                    .ml_2p5()
                                                    .pl_2()
                                                    .border_l_1()
                                                    .border_color(Color::Divider.get(cx))
                                                    .mt_0p5()
                                                    .gap(px(1.))
                                                    .min_w_0();
                                                group.child(match sessions {
                                                    Some(items)
                                                        if items
                                                            .iter()
                                                            .all(|session| session.archived) =>
                                                    {
                                                        rows.child(
                                                            div()
                                                                .px_2()
                                                                .py_1()
                                                                .text_xs()
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(tr("暂无未归档会话")),
                                                        )
                                                    }
                                                    Some(items) => rows.children(
                                                        items
                                                            .iter()
                                                            .filter(|session| !session.archived)
                                                            .map(|session| {
                                                                self.render_session_link(
                                                                    session, cx,
                                                                )
                                                            }),
                                                    ),
                                                    None => rows.child(
                                                        div()
                                                            .px_2()
                                                            .py_1()
                                                            .text_xs()
                                                            .text_color(Color::Muted.get(cx))
                                                            .child(tr("正在读取会话…")),
                                                    ),
                                                })
                                            })
                                    },
                                ))
                            }),
                    ),
            ))
            .child(
                v_flex()
                    .flex_shrink_0()
                    .pt_1()
                    .gap_1()
                    .when(show_team_footer, |footer| {
                        footer.child(
                            h_flex()
                                .w_full()
                                .gap_1()
                                .child(
                                    Button::new("toggle-members")
                                        .h(px(28.))
                                        .rounded_lg()
                                        .ghost()
                                        .compact()
                                        .icon(Glyph::Bot)
                                        .justify_start()
                                        .flex_1()
                                        .min_w_0()
                                        .accessibility_label(tr("团队成员"))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_xs()
                                                .child(tr("团队成员")),
                                        )
                                        .selected(self.member_panel)
                                        .disabled(self.workspace.is_none())
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.show_team(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("open-collaboration")
                                        .h(px(28.))
                                        .rounded_lg()
                                        .ghost()
                                        .compact()
                                        .icon(Glyph::Network)
                                        .justify_start()
                                        .flex_1()
                                        .min_w_0()
                                        .accessibility_label(tr("协作概览"))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_xs()
                                                .child(tr("协作概览")),
                                        )
                                        .selected(self.collaboration.visible)
                                        .disabled(self.workspace.is_none())
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.hide_terminal(cx);
                                            this.open_collaboration(cx);
                                            if window.viewport_size().width < px(820.) {
                                                this.sidebar_open = false;
                                            }
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .px_0p5()
                            .gap_1()
                            .child(
                                Button::new("open-settings")
                                    .custom(palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(28.))
                                    .w(px(28.))
                                    .rounded_md()
                                    .icon(Glyph::Settings)
                                    .tooltip(if cfg!(target_os = "macos") {
                                        tr("设置 (⌘,)")
                                    } else {
                                        tr("设置 (Ctrl+,)")
                                    })
                                    .accessibility_label(tr("设置"))
                                    .selected(self.settings_panel || self.roles.visible)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_settings(window, cx);
                                        if window.viewport_size().width < px(820.) {
                                            this.sidebar_open = false;
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("sidebar-quick-terminal")
                                    .custom(palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(28.))
                                    .w(px(28.))
                                    .rounded_md()
                                    .icon(Glyph::SquareTerminal)
                                    .tooltip(tr("工作区终端"))
                                    .accessibility_label(tr("工作区终端"))
                                    .selected(self.terminal.visible)
                                    .disabled(!terminal_enabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.terminal.visible {
                                            this.hide_terminal(cx);
                                        } else {
                                            this.show_terminal(window, cx);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(div().flex_1()),
                    ),
            )
    }

    fn project_menu(&self, id: String, cx: &Context<Self>) -> impl IntoElement + use<> {
        let weak = cx.weak_entity();
        Button::new("open-workspace-menu")
            .custom(palette::subtle_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(24.))
            .w(px(24.))
            .rounded_md()
            .icon(Glyph::Ellipsis)
            .tooltip(tr("项目操作"))
            .accessibility_label(tr("项目操作"))
            .disabled(!self.connected)
            .dropdown_menu(move |mut menu, _, _| {
                for (target, label) in [
                    (
                        "finder",
                        if cfg!(target_os = "macos") {
                            tr("访达")
                        } else {
                            tr("文件管理器")
                        },
                    ),
                    ("vscode", "Visual Studio Code"),
                    ("vscode-insiders", "VS Code Insiders"),
                    ("cursor", "Cursor"),
                    ("zed", "Zed"),
                ] {
                    let weak = weak.clone();
                    let id = id.clone();
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.open_workspace_target(id.clone(), target, cx)
                        });
                    }));
                }
                let weak = weak.clone();
                menu.separator().item(
                    PopupMenuItem::new(tr("移除当前工作区…")).on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| this.remove_workspace(window, cx));
                    }),
                )
            })
    }

    pub(super) fn session_activity(&self, id: &str) -> Option<&'static str> {
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

    fn project_activity(&self, sessions: &[crate::protocol::Session]) -> Option<(String, String)> {
        let summary: Vec<String> = [tr("待回应"), tr("处理中"), tr("已暂停"), tr("未读")]
            .into_iter()
            .filter_map(|label| {
                let count = sessions
                    .iter()
                    .filter(|session| {
                        !session.archived && self.session_activity(&session.id) == Some(label)
                    })
                    .count();
                (count > 0).then(|| format!("{count} {label}"))
            })
            .collect();
        summary.first().map(|first| {
            (
                first.clone(),
                summary.join(if crate::language::english() { ", " } else { "，" }),
            )
        })
    }
}
