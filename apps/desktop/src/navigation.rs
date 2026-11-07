use crate::app::{HiveApp, SettingsSection};
use crate::palette::Color;
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
            || self.collaboration.visible)
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
                    .tooltip("打开或收起导航")
                    .accessibility_label("打开或收起工作区与会话导航")
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
                            .unwrap_or_else(|| "桌面工作台".into()),
                    ),
            )
            .when(!self.connected && self.runtime.is_some(), |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child("后台未连接"),
                )
            })
            .when(!self.viewing_conversation(), |view| {
                view.child(
                    Button::new("return-chat")
                        .ghost()
                        .compact()
                        .label("返回会话")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.hide_terminal(cx);
                            this.close_memory();
                            this.close_settings();
                            this.close_roles();
                            this.close_collaboration();
                            this.member_panel = false;
                            if window.viewport_size().width < px(820.) {
                                this.sidebar_open = false;
                            }
                            cx.notify();
                        })),
                )
            })
    }

    pub fn render_settings_tabs(&self, compact: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let back = if self.terminal.visible {
            "返回工作区终端"
        } else if self.memory.visible {
            "返回工作区记忆"
        } else if self.collaboration.visible && self.collaboration.selected.is_some() {
            "返回派单详情"
        } else if self.collaboration.visible {
            "返回协作概览"
        } else if self.member_panel {
            "返回团队成员"
        } else {
            "返回会话"
        };
        div()
            .flex()
            .flex_shrink_0()
            .p_3()
            .gap_1()
            .bg(Color::Surface.get(cx))
            .when(compact, |bar| bar.flex_row().flex_wrap().border_b_1())
            .when(!compact, |bar| {
                bar.flex_col().w(px(260.)).pl_6().pr_4().pt_7()
            })
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .when(!compact, |row| row.px_3().pb_4())
                    .child(
                        Button::new("leave-preferences")
                            .ghost()
                            .compact()
                            .rounded_full()
                            .icon(Glyph::ArrowLeft)
                            .accessibility_label(back)
                            .tooltip(back)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.leave_preferences(window, cx)
                            })),
                    )
                    .when(!compact, |row| {
                        row.child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::MEDIUM)
                                .child("设置"),
                        )
                    }),
            )
            .children(
                [
                    (
                        "settings-appearance-tab",
                        "外观",
                        Glyph::Monitor,
                        SettingsSection::Appearance,
                    ),
                    (
                        "settings-pi-accounts-tab",
                        "内置 Pi",
                        Glyph::Bot,
                        SettingsSection::PiAccounts,
                    ),
                    (
                        "settings-team-tab",
                        "团队",
                        Glyph::Network,
                        SettingsSection::Team,
                    ),
                    (
                        "settings-notifications-tab",
                        "通知",
                        Glyph::Bell,
                        SettingsSection::Notifications,
                    ),
                    (
                        "settings-programs-tab",
                        "外部程序",
                        Glyph::SquareTerminal,
                        SettingsSection::Programs,
                    ),
                ]
                .into_iter()
                .map(|(id, label, glyph, section)| {
                    Button::new(id)
                        .ghost()
                        .h(px(36.))
                        .rounded_full()
                        .icon(glyph)
                        .when(!compact, |row| row.w_full().justify_start())
                        .accessibility_label(label)
                        .child(div().when(!compact, |text| text.flex_1()).child(label))
                        .selected(self.settings_panel && self.settings_section == section)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.settings_section = section;
                            if section == SettingsSection::PiAccounts {
                                this.load_pi_accounts(window.window_handle(), cx);
                            }
                            if !this.settings_panel {
                                this.open_settings(window, cx);
                            }
                            cx.notify();
                        }))
                }),
            )
            .child(
                Button::new("settings-roles-tab")
                    .ghost()
                    .h(px(36.))
                    .rounded_full()
                    .icon(Glyph::Bot)
                    .when(!compact, |row| row.w_full().justify_start())
                    .accessibility_label("角色模板")
                    .child(div().when(!compact, |text| text.flex_1()).child("角色模板"))
                    .selected(self.roles.visible)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.roles.visible {
                            this.open_roles(cx);
                        }
                        cx.notify();
                    })),
            )
    }

    pub fn new_chat_menu(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let weak = cx.weak_entity();
        Button::new(id)
            .when(id == "new-session-menu", |b| {
                b.h(px(32.)).rounded_full().w_full().justify_start()
            })
            .ghost()
            .icon(Glyph::Plus)
            .when(id == "new-session-menu", |b| {
                b.child(div().flex_1().font_weight(FontWeight::NORMAL).child(label))
            })
            .when(id != "new-session-menu", |b| b.label(label))
            .accessibility_label(if id == "new-session-menu" {
                "新建会话，选择 Agent 后端"
            } else {
                "选择主控 Agent"
            })
            .disabled(
                !self.connected
                    || self.workspace.is_none()
                    || self.loading
                    || !self.workspace_writes_allowed(),
            )
            .dropdown_menu(move |mut menu, _, _| {
                for (backend, label) in [
                    ("pi", "内置 Pi"),
                    ("codex", "Codex"),
                    ("pi-rpc", "外部 Pi"),
                    ("claude", "Claude Code"),
                ] {
                    let weak = weak.clone();
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            if window.viewport_size().width < px(820.) {
                                this.sidebar_open = false;
                            }
                            this.hide_terminal(cx);
                            this.new_session(backend, cx);
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
            .child(self.new_chat_menu("new-session-menu", "新建", cx))
            .child(Button::new("search-sessions").ghost().h(px(32.)).rounded_full().icon(Glyph::Search)
                .justify_start().w_full().accessibility_label("搜索会话")
                .tooltip(if cfg!(target_os = "macos") { "搜索会话 (⌘K)" } else { "搜索会话 (Ctrl+K)" })
                .child(div().flex_1().font_weight(FontWeight::NORMAL).child("搜索会话"))
                .disabled(!self.connected).on_click(cx.listener(|this, _, window, cx| this.open_session_search(window, cx))))
            .child(
                v_flex()
                    .flex_shrink_0()
                    .gap_1()
                    .child(
                        Button::new("open-terminal")
                            .h(px(32.))
                            .rounded_full()
                            .ghost()
                            .icon(Glyph::SquareTerminal)
                            .justify_start()
                            .w_full()
                            .accessibility_label("工作区终端")
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::NORMAL)
                                    .child("工作区终端"),
                            )
                            .selected(self.terminal.visible)
                            .disabled(
                                !self.connected
                                    || self.workspace.is_none()
                                    || !self.workspace_writes_allowed(),
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_terminal(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("toggle-members")
                            .h(px(32.))
                            .rounded_full()
                            .ghost()
                            .icon(Glyph::Bot)
                            .justify_start()
                            .w_full()
                            .accessibility_label("团队成员")
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::NORMAL)
                                    .child("团队成员"),
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
                            .accessibility_label("协作概览")
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::NORMAL)
                                    .child("协作概览"),
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
                    .child(
                        Button::new("open-memory")
                            .h(px(32.))
                            .rounded_full()
                            .ghost()
                            .icon(Glyph::BookOpen)
                            .justify_start()
                            .w_full()
                            .accessibility_label("工作区记忆")
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::NORMAL)
                                    .child("工作区记忆"),
                            )
                            .selected(self.memory.visible)
                            .disabled(self.workspace.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.hide_terminal(cx);
                                this.open_memory(cx);
                                if window.viewport_size().width < px(820.) {
                                    this.sidebar_open = false;
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .id("workspace-session-navigation")
                    .mt_4()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
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
                                            .label("工作区")
                                            .accessibility_label(if self.projects_collapsed { "展开工作区列表" } else { "收起工作区列表" })
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
                                                "收起添加工作区"
                                            } else {
                                                "添加工作区"
                                            })
                                            .tooltip(if self.add_workspace_open {
                                                "收起添加工作区"
                                            } else {
                                                "添加工作区"
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
                                        Input::new(&self.folder)
                                            .aria_label("工作区文件夹的绝对路径"),
                                    )
                                    .child(
                                        Button::new("choose-workspace-folder")
                                            .label("选择文件夹…")
                                            .disabled(!self.connected)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.choose_workspace_folder(window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("add-workspace")
                                            .label("添加文件夹")
                                            .disabled(!self.connected || self.runtime.is_none())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.add_workspace(window, cx)
                                            })),
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
                                                    .accessibility_label(format!(
                                                        "{} {} 的会话",
                                                        if expanded { "收起" } else { "展开" },
                                                        workspace.name
                                                    ))
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
                                            Some(items) if items.is_empty() => rows.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Color::Muted.get(cx))
                                                    .child("暂无会话"),
                                            ),
                                            Some(items) => {
                                                rows.children(items.iter().map(|session| {
                                                    self.render_session_link(session, cx)
                                                }))
                                            }
                                            None => rows.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Color::Muted.get(cx))
                                                    .child("正在读取会话…"),
                                            ),
                                        })
                                    })
                            }))),
                    ),
            )
            .child(
                v_flex().flex_shrink_0().child(
                    Button::new("open-settings")
                        .h(px(32.))
                        .rounded_full()
                        .ghost()
                        .icon(Glyph::Settings)
                        .justify_start()
                        .w_full()
                        .tooltip(if cfg!(target_os = "macos") { "设置 (⌘,)" } else { "设置 (Ctrl+,)" }).accessibility_label("设置")
                        .child(div().flex_1().font_weight(FontWeight::NORMAL).child("设置"))
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
            .tooltip("项目操作")
            .accessibility_label("项目操作")
            .disabled(!self.connected)
            .dropdown_menu(move |mut menu, _, _| {
                for (target, label) in [
                    (
                        "finder",
                        if cfg!(target_os = "macos") {
                            "访达"
                        } else {
                            "文件管理器"
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
                        PopupMenuItem::new("移除当前工作区…").on_click(move |_, window, cx| {
                            let _ = weak.update(cx, |this, cx| this.remove_workspace(window, cx));
                        }),
                    )
            })
    }
    fn render_session_link(
        &self,
        session: &crate::protocol::Session,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let project = session.workspace_id.clone();
        let id = session.id.clone();
        let role = session
            .agent_id
            .as_deref()
            .map(|agent| {
                if agent.ends_with(":orchestrator") {
                    "主控"
                } else {
                    "成员"
                }
            })
            .unwrap_or("会话");
        let activity = if self.interaction_count(&id) > 0 {
            " · 待回应"
        } else if self.busy.contains(&id) {
            " · 处理中"
        } else if self.pending.get(&id).copied().unwrap_or(0) > 0 {
            " · 已暂停"
        } else if self.unread.contains(&id) {
            " · 未读"
        } else {
            ""
        };
        Button::new(SharedString::from(format!("session-{id}")))
            .ghost()
            .h(px(30.))
            .rounded_lg()
            .accessibility_label(format!(
                "{} · {} · {role}{activity}",
                session.backend.label(),
                self.session_title(session)
            ))
            .w_full()
            .min_w_0()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(self.session_title(session).to_string()),
            )
            .when(!activity.is_empty(), |row| {
                row.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(activity.trim_start_matches(" · ")),
                )
            })
            .tooltip(format!(
                "{} · {role} · {}",
                self.session_title(session),
                session.backend.label()
            ))
            .disabled(self.workspace_removing.as_ref() == Some(&project))
            .selected(
                self.viewing_conversation()
                    && self.session.as_ref().is_some_and(|s| s.id == session.id),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                if window.viewport_size().width < px(820.) {
                    this.sidebar_open = false;
                }
                this.hide_terminal(cx);
                this.close_collaboration();
                if this.workspace.as_ref() != Some(&project) {
                    this.select_workspace(project.clone(), cx);
                }
                this.open_session(id.clone(), cx);
            }))
    }
}
