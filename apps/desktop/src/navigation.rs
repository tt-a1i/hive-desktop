use crate::language::tr;
mod session_row;
mod settings;
use crate::app::{HiveApp, SettingsSection};
use crate::palette::Color;
use crate::focus_scroll::{Reveal, Viewport};
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
    pub(crate) fn show_team(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_workflows();
        self.close_memory();
        self.close_settings();
        self.close_collaboration();
        self.close_roles();
        self.hide_terminal(cx);
        self.member_panel = true;
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

    pub fn render_navigation_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .min_h(px(34.))
            .when(!self.sidebar_open, |bar| bar.px_3())
            .child(
                Button::new("toggle-navigation")
                    .ghost()
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
                    .truncate()
                    .text_sm()
                    .text_color(Color::Muted.get(cx))
                    .child(
                        self.workspaces
                            .iter()
                            .find(|workspace| Some(&workspace.id) == self.workspace.as_ref())
                            .map(|workspace| workspace.name.clone())
                            .unwrap_or_else(|| tr("桌面工作台").into()),
                    ),
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
                view.child(
                    Button::new("return-chat")
                        .ghost()
                        .compact()
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
                        })),
                )
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
        let project_label =
            project.map(|workspace| if crate::language::english() { format!("New conversation in {}: choose an Agent", workspace.name) } else { format!("在 {} 中新建会话，选择 Agent 后端", workspace.name) });
        Button::new(
            project
                .map(|workspace| SharedString::from(format!("new-project-chat-{}", workspace.id)))
                .unwrap_or_else(|| id.into()),
        )
        .when(project.is_some(), |button| {
            button.compact().h(px(28.)).w(px(28.))
        })
        .when(id == "new-session-menu", |b| {
            b.h(px(32.)).rounded_full().w_full().justify_start()
        })
        .ghost()
        .when(id == "home-agent", |button| button.primary().dropdown_caret(true))
        .icon(if id == "home-agent" { Glyph::Network } else { Glyph::Plus })
        .when(id == "new-session-menu", |b| {
            b.child(div().flex_1().font_weight(FontWeight::NORMAL).child(label))
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
        let selected = [self.workflows.visible, self.memory.visible, self.terminal.visible];
        Button::new("navigation-more").ghost().h(px(32.)).rounded_full()
            .icon(Glyph::Ellipsis).justify_start().w_full()
            .accessibility_label(tr("更多工作区功能"))
            .selected(selected.into_iter().any(|active| active))
            .child(div().flex_1().font_weight(FontWeight::NORMAL).child(tr("更多")))
            .dropdown_menu(move |mut menu, _, _| {
                for (index, label, glyph, enabled) in [
                    (0, tr("工作流"), Glyph::Workflow, has_workspace),
                    (1, tr("工作区记忆"), Glyph::BookOpen, has_workspace),
                    (2, tr("工作区终端"), Glyph::SquareTerminal, terminal_enabled),
                    (3, tr("已归档会话"), Glyph::Archive, connected),
                ] {
                    if index == 3 { menu = menu.separator(); }
                    let weak = weak.clone();
                    let current = selected.get(index).copied().unwrap_or(false);
                    menu = menu.item(PopupMenuItem::new(label)
                        .icon(if current { Glyph::Check } else { glyph })
                        .checked(current).disabled(!enabled)
                        .on_click(move |_, window, cx| {
                            let _ = weak.update(cx, |this, cx| match index {
                                0 => this.show_workflows(window, cx),
                                1 => {
                                    this.hide_terminal(cx);
                                    this.open_memory(cx);
                                    if window.viewport_size().width < px(820.) { this.sidebar_open = false; }
                                    cx.notify();
                                }
                                2 => this.show_terminal(window, cx),
                                _ => this.open_archived_sessions(window, cx),
                            });
                        }));
                }
                menu
            })
    }

    pub fn render_navigation(&self, narrow: bool, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .when(narrow, |view| view.flex_1().min_w_0())
            .when(!narrow, |view| view.w(px(260.)).flex_shrink_0())
            .h_full()
            .bg(Color::Sidebar.get(cx))
            .border_r_1()
            .border_color(Color::Divider.get(cx))
            .p_3()
            .gap_1()
            .child(self.render_navigation_bar(cx))
            .child(self.new_chat_menu("new-session-menu", tr("新建"), None, cx))
            .child(Button::new("search-sessions").ghost().h(px(32.)).rounded_full().icon(Glyph::Search)
                .justify_start().w_full().accessibility_label(tr("搜索会话"))
                .tooltip(if cfg!(target_os = "macos") { tr("搜索会话 (⌘K)") } else { tr("搜索会话 (Ctrl+K)") })
                .child(div().flex_1().font_weight(FontWeight::NORMAL).child(tr("搜索会话")))
                .disabled(!self.connected).on_click(cx.listener(|this, _, window, cx| this.open_session_search(window, cx))))
            .child(
                v_flex()
                    .flex_shrink_0()
                    .gap_1()
                    .child(
                        Button::new("toggle-members")
                            .h(px(32.))
                            .rounded_full()
                            .ghost()
                            .icon(Glyph::Bot)
                            .justify_start()
                            .w_full()
                            .accessibility_label(tr("团队成员"))
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::NORMAL)
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
                            .h(px(32.))
                            .rounded_full()
                            .ghost()
                            .icon(Glyph::Network)
                            .justify_start()
                            .w_full()
                            .accessibility_label(tr("协作概览"))
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::NORMAL)
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
                    )
                    .child(self.render_more_navigation(cx)),
            )
            .child(Viewport::new("workspace-navigation-viewport", &self.navigation_scroll,
                v_flex()
                    .id("workspace-session-navigation")
                    .mt_4()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.navigation_scroll)
                    .p_1()
                    .child(self.render_pinned_navigation(cx))
                    .child(
                        v_flex()
                            .w_full()
                            .flex_shrink_0()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        Button::new("toggle-project-section").ghost().compact()
                                            .icon(if self.projects_collapsed { Glyph::ChevronRight } else { Glyph::ChevronDown })
                                            .label(tr("工作区"))
                                            .accessibility_label(if self.projects_collapsed { tr("展开工作区列表") } else { tr("收起工作区列表") })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.projects_collapsed = !this.projects_collapsed;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("toggle-add-workspace")
                                            .ghost()
                                            .compact()
                                            .icon(if self.add_workspace_open {
                                                Glyph::PanelLeft
                                            } else {
                                                Glyph::Plus
                                            })
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
                                    ),
                            )
                            .when(self.add_workspace_open, |section| {
                                section
                                    .child(
                                        Reveal::new("workspace-folder-input-reveal", &self.navigation_scroll, Input::new(&self.folder)
                                            .aria_label(tr("工作区文件夹的绝对路径"))),
                                    )
                                    .child(
                                        Reveal::new("workspace-folder-picker-reveal", &self.navigation_scroll, Button::new("choose-workspace-folder")
                                            .label(tr("选择文件夹…"))
                                            .disabled(!self.connected)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.choose_workspace_folder(window, cx)
                                            }))),
                                    )
                                    .child(
                                        Reveal::new("workspace-folder-add-reveal", &self.navigation_scroll, Button::new("add-workspace")
                                            .label(tr("添加文件夹"))
                                            .disabled(!self.connected || self.runtime.is_none())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.add_workspace(window, cx)
                                            }))),
                                    )
                            })
                            .when(!self.projects_collapsed, |section| section.children(self.workspaces.iter().enumerate().map(|(i, workspace)| {
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
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .child(
                                                Button::new(("expand-project", i))
                                                    .ghost()
                                                    .compact()
                                                    .h(px(28.)).w(px(24.)).icon(if expanded { Glyph::ChevronDown } else { Glyph::ChevronRight })
                                                    .toggled(expanded)
                                                    .accessibility_label(if crate::language::english() {
                                                        format!("{} {} conversations", if expanded { tr("收起") } else { tr("展开") }, workspace.name)
                                                    } else { format!("{} {} 的会话", if expanded { tr("收起") } else { tr("展开") }, workspace.name) })
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
                                                        .icon(if expanded { Glyph::FolderOpen } else { Glyph::Folder }).h(px(28.))
                                                        .w_full()
                                                        .min_w_0()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .truncate()
                                                                .child(workspace.name.clone()),
                                                        )
                                                        .tooltip(workspace.path.clone())
                                                        .selected(
                                                            self.workspace.as_ref()
                                                                == Some(&workspace.id)
                                                                && self.session.is_none()
                                                                && self.viewing_conversation(),
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
                                                if let Some((label, detail)) = sessions.and_then(|items| self.project_activity(items)) {
                                                    row.child(Button::new(SharedString::from(format!("project-activity-{}", workspace.id)))
                                                        .ghost().compact().h(px(28.)).label(label)
                                                        .accessibility_label(if crate::language::english() { format!("Expand {} conversations: {detail}", workspace.name) } else { format!("展开 {} 的会话：{detail}", workspace.name) })
                                                        .tooltip(detail)
                                                        .on_click(cx.listener({ let id=workspace.id.clone(); move |this,_,_,cx| this.toggle_project(id.clone(),cx) })))
                                                } else { row }
                                            })
                                            .child(self.new_chat_menu("project-new-chat", tr("新建"), Some(workspace), cx))
                                            .when(
                                                self.workspace.as_ref() == Some(&workspace.id),
                                                |row| {
                                                    row.child(
                                                        self.project_menu(workspace.id.clone(), cx),
                                                    )
                                                },
                                            ),
                                    )
                                    .when(expanded, |group| {
                                        let rows = v_flex().pl_4().pt(px(2.)).gap(px(2.)).min_w_0();
                                        group.child(match sessions {
                                            Some(items) if items.iter().all(|session|session.archived) => rows.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(tr("暂无未归档会话")),
                                            ),
                                            Some(items) => {
                                                rows.children(items.iter().filter(|session|!session.archived).map(|session| {
                                                    self.render_session_link(session, cx)
                                                }))
                                            }
                                            None => rows.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(tr("正在读取会话…")),
                                            ),
                                        })
                                    })
                            }))),
                    ),
            ))
            .child(
                v_flex().flex_shrink_0().child(
                    Button::new("open-settings")
                        .h(px(32.))
                        .rounded_full()
                        .ghost()
                        .icon(Glyph::Settings)
                        .justify_start()
                        .w_full()
                        .tooltip(if cfg!(target_os = "macos") { tr("设置 (⌘,)") } else { tr("设置 (Ctrl+,)") }).accessibility_label(tr("设置"))
                        .child(div().flex_1().font_weight(FontWeight::NORMAL).child(tr("设置")))
                        .selected(self.settings_panel || self.roles.visible)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_settings(window, cx);
                            if window.viewport_size().width < px(820.) {
                                this.sidebar_open = false;
                            }
                            cx.notify();
                        })),
                ),
            )
    }

    fn project_menu(&self, id: String, cx: &Context<Self>) -> impl IntoElement + use<> {
        let weak = cx.weak_entity();
        Button::new("open-workspace-menu")
            .ghost()
            .compact()
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
                menu.separator()
                    .item(
                        PopupMenuItem::new(tr("移除当前工作区…")).on_click(move |_, window, cx| {
                            let _ = weak.update(cx, |this, cx| this.remove_workspace(window, cx));
                        }),
                    )
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
        summary
            .first()
            .map(|first| (first.clone(), summary.join("，")))
    }
}
