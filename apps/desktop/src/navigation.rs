use crate::app::HiveApp;
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
    pub fn render_navigation_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .flex_shrink_0()
            .items_center()
            .gap_3()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(rgb(0xe5e6df))
            .child(
                Button::new("toggle-navigation")
                    .label(if self.sidebar_open {
                        "收起导航"
                    } else {
                        "打开导航"
                    })
                    .accessibility_label("打开或收起工作区与会话导航")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_open = !this.sidebar_open;
                        cx.notify();
                    })),
            )
            .child(div().font_weight(FontWeight::BOLD).child("Hive"))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(rgb(0x777c75))
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
                        .text_color(rgb(0xb54131))
                        .child("后台未连接"),
                )
            })
            .when(
                self.member_panel
                    || self.memory.visible
                    || self.roles.visible
                    || self.settings_panel
                    || self.collaboration.visible,
                |view| {
                    view.child(
                        Button::new("return-chat")
                            .label("返回会话")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_memory();
                                this.close_settings();
                                this.close_roles();
                                this.close_collaboration();
                                this.member_panel = false;
                                this.sidebar_open = false;
                                cx.notify();
                            })),
                    )
                },
            )
    }

    pub fn render_settings_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .flex_shrink_0()
            .gap_2()
            .p_3()
            .child(
                Button::new("settings-programs-tab")
                    .label("通用与外部程序")
                    .selected(self.settings_panel)
                    .on_click(cx.listener(|this, _, window, cx| {
                        if !this.settings_panel {
                            this.open_settings(window, cx);
                        }
                    })),
            )
            .child(
                Button::new("settings-roles-tab")
                    .label("角色模板")
                    .selected(self.roles.visible)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.roles.visible {
                            this.open_roles(cx);
                        }
                        cx.notify();
                    })),
            )
    }

    pub fn render_navigation(&self, narrow: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        v_flex()
            .when(narrow, |view| view.flex_1().min_w_0())
            .when(!narrow, |view| view.w(px(248.)).flex_shrink_0())
            .h_full()
            .bg(rgb(0xefefea))
            .p_3()
            .gap_3()
            .child(
                Button::new("new-session-menu")
                    .w_full()
                    .primary()
                    .label("＋ 新建会话 ▾")
                    .accessibility_label("新建会话，选择 Agent 后端")
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
                            menu = menu.item(PopupMenuItem::new(label).on_click(
                                move |_, window, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        if window.viewport_size().width < px(820.) {
                                            this.sidebar_open = false;
                                        }
                                        this.new_session(backend, cx);
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
            .child(
                v_flex()
                    .id("workspace-session-navigation")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .w_full()
                            .flex_shrink_0()
                            .gap_2()
                            .child(
                                div()
                                    .pt_2()
                                    .text_xs()
                                    .text_color(rgb(0x777c75))
                                    .child("工作区"),
                            )
                            .children(self.workspaces.iter().enumerate().map(|(i, workspace)| {
                                let id = workspace.id.clone();
                                Button::new(("workspace", i))
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
                                    .selected(self.workspace.as_ref() == Some(&workspace.id))
                                    .disabled(
                                        self.workspace_removing.as_ref() == Some(&workspace.id),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_workspace(id.clone(), cx)
                                    }))
                            }))
                            .when_some(
                                self.workspaces
                                    .iter()
                                    .find(|w| Some(&w.id) == self.workspace.as_ref()),
                                |view, workspace| {
                                    let id = workspace.id.clone();
                                    let weak = cx.weak_entity();
                                    view.child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .text_color(rgb(0x777c75))
                                            .child(workspace.path.clone()),
                                    )
                                    .child(
                                        Button::new("open-workspace-menu")
                                            .w_full()
                                            .label("打开当前项目 ▾")
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
                                                    menu = menu.item(
                                                        PopupMenuItem::new(label).on_click(
                                                            move |_, _, cx| {
                                                                let _ =
                                                                    weak.update(cx, |this, cx| {
                                                                        this.open_workspace_target(
                                                                            id.clone(),
                                                                            target,
                                                                            cx,
                                                                        )
                                                                    });
                                                            },
                                                        ),
                                                    );
                                                }
                                                menu
                                            }),
                                    )
                                },
                            )
                            .when(self.workspace.is_some(), |view| {
                                view.child(
                                    Button::new("remove-workspace")
                                        .w_full()
                                        .label(if self.workspace_removing.is_some() {
                                            "正在移除工作区…"
                                        } else {
                                            "移除当前工作区…"
                                        })
                                        .disabled(
                                            !self.connected || self.workspace_removing.is_some(),
                                        )
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.remove_workspace(window, cx)
                                        })),
                                )
                            })
                            .child(Input::new(&self.folder).aria_label("工作区文件夹的绝对路径"))
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
                            .child(
                                div()
                                    .pt_4()
                                    .text_xs()
                                    .text_color(rgb(0x777c75))
                                    .child("会话"),
                            )
                            .when(self.sessions.is_empty(), |view| {
                                view.child(div().py_3().text_sm().text_color(rgb(0x777c75)).child(
                                    if self.workspace.is_some() {
                                        "新建会话开始工作"
                                    } else {
                                        "选择工作区后新建会话"
                                    },
                                ))
                            })
                            .children(self.sessions.iter().enumerate().map(|(i, session)| {
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
                                v_flex()
                                    .min_w_0()
                                    .gap_1()
                                    .py_1()
                                    .child(div().text_xs().text_color(rgb(0x63715c)).child(
                                        format!("{role} · {}{activity}", session.backend.label()),
                                    ))
                                    .child(
                                        Button::new(("session", i))
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
                                            .tooltip(self.session_title(session).to_string())
                                            .disabled(!self.workspace_writes_allowed())
                                            .selected(
                                                self.session
                                                    .as_ref()
                                                    .is_some_and(|s| s.id == session.id),
                                            )
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                if window.viewport_size().width < px(820.) {
                                                    this.sidebar_open = false;
                                                }
                                                this.close_collaboration();
                                                this.open_session(id.clone(), cx);
                                            })),
                                    )
                            })),
                    ),
            )
            .child(
                v_flex()
                    .flex_shrink_0()
                    .gap_1()
                    .child(
                        Button::new("toggle-members")
                            .w_full()
                            .label("团队成员")
                            .selected(self.member_panel)
                            .disabled(self.workspace.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_memory();
                                this.close_settings();
                                this.close_collaboration();
                                this.close_roles();
                                this.member_panel = true;
                                if window.viewport_size().width < px(820.) {
                                    this.sidebar_open = false;
                                }
                                this.refresh_members(cx);
                                this.load_roles(cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("open-collaboration")
                            .w_full()
                            .label("协作概览")
                            .selected(self.collaboration.visible)
                            .disabled(self.workspace.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_collaboration(cx);
                                if window.viewport_size().width < px(820.) {
                                    this.sidebar_open = false;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("open-memory")
                            .w_full()
                            .label("工作区记忆")
                            .selected(self.memory.visible)
                            .disabled(self.workspace.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_memory(cx);
                                if window.viewport_size().width < px(820.) {
                                    this.sidebar_open = false;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("open-settings")
                            .w_full()
                            .label("设置")
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
}
