use crate::chrome;
use crate::focus_scroll::{Reveal, Viewport};
use crate::language::tr;
mod search;
mod session_row;
mod width;
mod working;
pub(crate) use search::SidebarSearch;
pub(crate) use width::SidebarPreferences;
mod settings;
use crate::app::HiveApp;
use crate::palette::{self, Color};
use crate::protocol::Session;
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
use std::collections::HashSet;

fn step_visible_session<'a>(
    rows: impl IntoIterator<Item = &'a Session>,
    current: Option<&str>,
    delta: isize,
) -> Option<&'a Session> {
    let mut seen = HashSet::new();
    let visible: Vec<_> = rows
        .into_iter()
        .filter(|row| seen.insert(row.id.as_str()))
        .collect();
    match visible
        .iter()
        .position(|row| current == Some(row.id.as_str()))
    {
        Some(index) if visible.len() > 1 => {
            let next = (index as isize + delta).rem_euclid(visible.len() as isize) as usize;
            Some(visible[next])
        }
        Some(_) => None,
        None if delta < 0 => visible.last().copied(),
        None => visible.first().copied(),
    }
}

#[cfg(test)]
mod tests {
    use super::step_visible_session;
    use crate::protocol::Session;

    fn row(id: &str, workspace: &str) -> Session {
        serde_json::from_value(serde_json::json!({
            "id": id, "workspaceId": workspace, "title": "Same title",
        }))
        .unwrap()
    }

    #[test]
    fn sidebar_step_uses_visible_ids_and_keeps_the_first_appearance() {
        let alpha_pin = row("alpha-pin", "alpha");
        let beta_pin = row("beta-pin", "beta");
        let alpha_ordinary = row("alpha-ordinary", "alpha");
        let alpha_working = row("alpha-working", "alpha");
        let visible = [
            &alpha_pin,
            &beta_pin,
            &alpha_ordinary,
            &alpha_pin,
            &alpha_working,
        ];
        let next = |rows: &[&Session], current, delta| {
            step_visible_session(rows.iter().copied(), current, delta).map(|row| row.id.clone())
        };

        for (current, delta, expected) in [
            (Some("alpha-pin"), 1, "beta-pin"),
            (Some("alpha-ordinary"), 1, "alpha-working"),
            (Some("alpha-working"), 1, "alpha-pin"),
            (Some("beta-pin"), -1, "alpha-pin"),
            (None, 1, "alpha-pin"),
            (None, -1, "alpha-working"),
        ] {
            assert_eq!(next(&visible, current, delta).as_deref(), Some(expected));
        }
        let collapsed_pins = [&alpha_ordinary, &alpha_pin];
        assert_eq!(
            next(&collapsed_pins, Some("beta-pin"), 1).as_deref(),
            Some("alpha-ordinary")
        );
        assert_eq!(
            next(&collapsed_pins, Some("alpha-ordinary"), 1).as_deref(),
            Some("alpha-pin")
        );
        assert_eq!(next(&[&alpha_pin], Some("alpha-pin"), 1), None);
        assert_eq!(next(&[], None, 1), None);
    }
}

impl HiveApp {
    pub(crate) fn has_active_team_context(&self) -> bool {
        let is_worker = |id: &str| !id.ends_with(":orchestrator") && !id.ends_with(":__workflow__");
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
        !(self.member_panel
            || self.memory.visible
            || self.roles.visible
            || self.settings_panel
            || self.collaboration.visible
            || self.workflows.visible)
    }

    pub(crate) fn return_to_chat_button(&self, cx: &Context<Self>) -> Button {
        Button::new("return-chat")
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .small()
            .h(px(28.))
            .px_2()
            .rounded_lg()
            .icon(Glyph::ArrowLeft)
            .label(tr("返回会话"))
            .on_click(cx.listener(|this, _, window, cx| {
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
                bar.px_1().when(inset > px(0.), |b| b.pl(inset - px(4.)))
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
                    .tooltip(crate::keybindings::hint(
                        crate::keybindings::Target::ToggleSidebar,
                        "打开或收起导航",
                        cx,
                    ))
                    .accessibility_label(tr("打开或收起工作区与会话导航"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_open = !this.sidebar_open;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .when(!in_sidebar, |view| view.flex_1())
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
            .when(in_sidebar, |view| {
                view.child(
                    h_flex()
                        .flex_1()
                        .items_center()
                        .gap_0p5()
                        .pl_1()
                        .child(
                            Button::new("nav-history-back")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(24.))
                                .w(px(24.))
                                .rounded_md()
                                .icon(Glyph::ArrowLeft)
                                .tooltip(tr("后退"))
                                .accessibility_label(tr("后退"))
                                .disabled(self.nav_back.is_empty())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_history_back(cx);
                                })),
                        )
                        .child(
                            Button::new("nav-history-forward")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(24.))
                                .w(px(24.))
                                .rounded_md()
                                .icon(Glyph::ArrowRight)
                                .tooltip(tr("前进"))
                                .accessibility_label(tr("前进"))
                                .disabled(self.nav_forward.is_empty())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_history_forward(cx);
                                })),
                        ),
                )
            })
            .child(self.global_search_button(cx))
            .child(self.command_palette_button(cx))
            .when(!self.connected && self.runtime.is_some(), |view| {
                view.child(
                    h_flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap_1()
                        .px_1p5()
                        .py_0p5()
                        .rounded_full()
                        .bg(Color::ErrorSurface.get(cx))
                        .child(div().size(px(6.)).rounded_full().bg(Color::Error.get(cx)))
                        .child(
                            div()
                                .text_xs()
                                .text_color(Color::Error.get(cx))
                                .child(tr("后台未连接")),
                        ),
                )
            })
            .when(!in_sidebar && !self.viewing_conversation(), |view| {
                view.child(self.return_to_chat_button(cx))
            })
    }

    pub(crate) fn global_search_button(&self, cx: &Context<Self>) -> Button {
        Button::new("open-global-conversation-search")
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(24.))
            .w(px(24.))
            .rounded_md()
            .icon(Glyph::Search)
            .tooltip(crate::keybindings::hint(
                crate::keybindings::Target::Search,
                "搜索所有项目的会话",
                cx,
            ))
            .accessibility_label(tr("搜索所有项目的会话"))
            .disabled(!self.connected)
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_session_search(window, cx);
            }))
    }

    pub(crate) fn command_palette_button(&self, cx: &Context<Self>) -> Button {
        Button::new("open-command-palette")
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(24.))
            .w(px(24.))
            .rounded_md()
            .icon(Glyph::Sparkles)
            .tooltip(crate::keybindings::hint(
                crate::keybindings::Target::CommandPalette,
                "快捷命令面板",
                cx,
            ))
            .accessibility_label(tr("快捷命令面板"))
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_command_palette(window, cx);
            }))
    }

    pub fn new_chat_menu(
        &self,
        id: &'static str,
        label: &'static str,
        project: Option<&crate::protocol::Workspace>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let weak = cx.weak_entity();
        let profiles = self.profiles.snapshot.as_ref().map(|s| s.profiles.clone()).unwrap_or_default();
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
                .h(px(28.))
                .px_2()
                .rounded_lg()
                .dropdown_caret(true)
        })
        .icon(if id == "home-agent" {
            Glyph::Sparkles
        } else if id == "new-session-menu" {
            Glyph::ChevronDown
        } else if project.is_some() {
            Glyph::Plus
        } else {
            Glyph::SquarePen
        })
        .when(id != "new-session-menu" && project.is_none(), |b| {
            b.label(label)
        })
        .tooltip(project_label.clone().unwrap_or_else(|| tr("新建会话，选择 Agent 后端").into()))
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
                        if let Some(project) = &target {
                            this.new_session_in_project(project.clone(), backend, cx);
                        } else {
                            this.new_session(backend, cx);
                        }
                    });
                }));
            }
            for profile in profiles.iter().filter(|p| p.enabled && !p.id.starts_with("default-")) {
                let backend = match profile.driver.as_str() { "pi" => "pi", "codex" => "codex", "pi-rpc" => "pi-rpc", "claude" => "claude", "acp" => "acp", _ => continue };
                let reference = crate::agent_profiles::ProfileRef { id: profile.id.clone(), revision: profile.current_revision };
                let weak = weak.clone(); let target = target.clone();
                menu = menu.item(PopupMenuItem::new(format!("{} · {} · r{}", profile.label, backend, profile.current_revision)).on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if window.viewport_size().width < px(820.) { this.sidebar_open = false; }
                        if let Some(project) = &target {
                            this.create_new_session(Some(project.clone()), backend, Some(reference.clone()), None, cx);
                        } else {
                            this.new_session_with_profile(backend, Some(reference.clone()), cx);
                        }
                    });
                }));
            }
            if id == "new-session-menu" {
                let scratch = weak.clone();
                menu = menu.separator().item(PopupMenuItem::new(tr("不使用项目")).on_click(move |_, window, cx| {
                    let _ = scratch.update(cx, |this, cx| this.start_scratch_conversation(window, cx));
                }));
            }
            let isolated = weak.clone();
            let isolated_project = target.clone();
            let pending = weak.clone();
            let pending_project = target.clone();
            let operations = weak.clone();
            let operations_project = target.clone();
            menu.separator()
                .item(PopupMenuItem::new(tr("查看历史操作…")).on_click(move |_, window, cx| {
                    let _ = operations.update(cx, |this, cx| this.open_history_operations(operations_project.clone(), window, cx));
                }))
                .item(PopupMenuItem::new(tr("新建独立工作树会话…")).on_click(move |_, window, cx| {
                    let _ = isolated.update(cx, |this, cx| this.open_session_execution(isolated_project.clone(), false, window, cx));
                }))
                .item(PopupMenuItem::new(tr("查看待恢复工作树…")).on_click(move |_, window, cx| {
                    let _ = pending.update(cx, |this, cx| this.open_session_execution(pending_project.clone(), true, window, cx));
                }))
        })
    }

    fn render_filter_navigation(
        &self,
        selected_filter: Option<String>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let weak = cx.weak_entity();
        let has_workspace = self.workspace.is_some();
        let workspaces: Vec<crate::protocol::Workspace> =
            self.sorted_projects(cx).into_iter().cloned().collect();
        let active_workspace = self.workspace.clone();
        let scratch_origin = active_workspace
            .as_ref()
            .filter(|workspace| self.has_scratch_project_origin(workspace))
            .cloned();
        let connected = self.connected;
        let selected = [self.workflows.visible, self.memory.visible];
        let filter_active = selected_filter.is_some() || selected.into_iter().any(|active| active);
        Button::new("navigation-more")
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(26.))
            .w(px(26.))
            .rounded_md()
            .icon(Glyph::ListFilter)
            .tooltip(tr("更多工作区功能"))
            .accessibility_label(tr("更多工作区功能"))
            .selected(filter_active)
            .dropdown_menu(move |mut menu, window, cx| {
                if workspaces.len() > 1 {
                    let weak_all = weak.clone();
                    let is_all = selected_filter.is_none();
                    menu = menu.item(
                        PopupMenuItem::new(tr("全部"))
                            .icon(if is_all { Glyph::Check } else { Glyph::Layers })
                            .checked(is_all)
                            .on_click(move |_, _, cx| {
                                let _ = weak_all.update(cx, |this, cx| {
                                    this.sidebar_workspace_filter = None;
                                    let missing: Vec<String> = this
                                        .workspaces
                                        .iter()
                                        .filter(|ws| {
                                            this.workspace.as_ref() != Some(&ws.id)
                                                && !this.project_sessions.contains_key(&ws.id)
                                                && !this.expanded_projects.contains(&ws.id)
                                        })
                                        .map(|ws| ws.id.clone())
                                        .collect();
                                    for id in missing {
                                        this.toggle_project(id, cx);
                                    }
                                    cx.notify();
                                });
                            }),
                    );
                    for ws in &workspaces {
                        let ws_id = ws.id.clone();
                        let ws_id_click = ws_id.clone();
                        let weak_ws = weak.clone();
                        let is_checked = selected_filter.as_deref() == Some(ws_id.as_str());
                        menu = menu.item(
                            PopupMenuItem::new(ws.name.clone())
                                .icon(if is_checked {
                                    Glyph::Check
                                } else {
                                    Glyph::Folder
                                })
                                .checked(is_checked)
                                .on_click(move |_, _, cx| {
                                    let _ = weak_ws.update(cx, |this, cx| {
                                        this.sidebar_workspace_filter = Some(ws_id_click.clone());
                                        this.select_workspace(ws_id_click.clone(), cx);
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                    menu = menu.separator();
                }
                for (index, label, glyph, enabled) in [
                    (0, tr("工作流"), Glyph::Workflow, has_workspace),
                    (1, tr("工作区记忆"), Glyph::BookOpen, has_workspace),
                    (2, tr("已归档会话"), Glyph::Archive, connected),
                ] {
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
                                    _ => this.open_archived_sessions(window, cx),
                                });
                            }),
                    );
                }
                if let Some(ws_id) = active_workspace.clone() {
                    menu = menu.separator();
                    if let Some(workspace) = scratch_origin.clone() {
                        let owner = weak.clone();
                        menu = menu.item(
                            PopupMenuItem::new(tr("打开原 Scratch 会话")).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |this, cx| {
                                        this.open_scratch_project_origin(workspace.clone(), window, cx);
                                    });
                                },
                            ),
                        );
                    }
                    let owner = weak.clone();
                    menu = menu.submenu(tr("打开方式"), window, cx, move |menu, _, cx| {
                        crate::open_targets::workspace_menu(menu, owner.clone(), ws_id.clone(), None, cx)
                    });
                    let weak = weak.clone();
                    menu =
                        menu.separator()
                            .item(PopupMenuItem::new(tr("移除当前工作区…")).on_click(
                                move |_, window, cx| {
                                    let _ = weak
                                        .update(cx, |this, cx| this.remove_workspace(window, cx));
                                },
                            ));
                }
                menu
            })
    }

    fn sidebar_filtered_workspace(&self) -> Option<&str> {
        self.sidebar_workspace_filter
            .as_deref()
            .filter(|id| self.workspaces.iter().any(|workspace| workspace.id == *id))
    }

    fn sidebar_conversation_sections(&self, cx: &App) -> working::SidebarSessions<'_> {
        let selected_filter = self.sidebar_filtered_workspace();
        // Collect flat unarchived sessions across workspaces (or filtered to a workspace).
        let mut flat_sessions: Vec<&Session> = Vec::new();
        let mut seen_ids = HashSet::new();
        let general = crate::general::current(cx);
        for workspace in self.sorted_projects(cx) {
            if selected_filter.is_some_and(|filter| filter != workspace.id) {
                continue;
            }
            let items: Option<&[Session]> =
                if Some(&workspace.id) == self.workspace.as_ref() && !self.sessions.is_empty() {
                    Some(self.sessions.as_slice())
                } else {
                    self.project_sessions.get(&workspace.id).map(Vec::as_slice)
                };
            if let Some(items) = items {
                for session in items {
                    if !session.archived
                        && !self.removed_sessions.contains(&session.id)
                        && seen_ids.insert(session.id.as_str())
                    {
                        flat_sessions.push(session);
                    }
                }
            }
        }
        if self.workspaces.len() > 1
            && selected_filter.is_none()
            && general.project_sort_order == crate::general::ProjectSortOrder::RecentActivity
        {
            flat_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        }
        if general.pin_active_sessions_top {
            let mut active = Vec::new();
            let mut inactive = Vec::new();
            for session in flat_sessions {
                if self.is_session_active(&session.id) {
                    active.push(session);
                } else {
                    inactive.push(session);
                }
            }
            active.extend(inactive);
            flat_sessions = active;
        }

        self.sidebar_session_sections(flat_sessions, general.fold_working_sessions)
    }

    pub(crate) fn sidebar_session_step(&self, delta: isize, cx: &App) -> Option<Session> {
        // Search owns its own selection and body-match open action. When the
        // whole sidebar is hidden, its normal ordering still works by keyboard.
        if self.sidebar_open && self.sidebar_search.read(cx).active(cx) {
            return None;
        }
        let mut rows: Vec<_> = self
            .visible_pinned_navigation_hits()
            .map(|hit| &hit.session)
            .collect();
        if !self.projects_collapsed {
            let sections = self.sidebar_conversation_sections(cx);
            rows.extend(sections.regular);
            rows.extend(
                sections
                    .working
                    .iter()
                    .flat_map(|project| self.visible_working_sessions(project)),
            );
        }
        let available = rows.into_iter().filter(|session| {
            !session.archived
                && !self.removed_sessions.contains(&session.id)
                && self.workspace_removing.as_deref() != Some(session.workspace_id.as_str())
                && self
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.id == session.workspace_id)
        });
        step_visible_session(
            available,
            self.session.as_ref().map(|session| session.id.as_str()),
            delta,
        )
        .cloned()
    }

    pub fn render_navigation(
        &self,
        narrow: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let terminal_enabled =
            self.connected && self.workspace.is_some() && self.workspace_writes_allowed();
        let selected_filter = self.sidebar_filtered_workspace();
        let search = self.sidebar_search.clone();
        search.update(cx, |search, cx| {
            search.sync(
                self.runtime.clone().filter(|_| self.connected),
                self.connection_generation,
                selected_filter.map(str::to_owned),
                window,
                cx,
            )
        });
        let searching = search.read(cx).active(cx);
        let query = search.read(cx).query.clone();
        let search_keys = search.clone();
        let preloaded_ws_state = window.use_keyed_state(
            SharedString::from("sidebar-preloaded-workspaces"),
            cx,
            |_, _| std::collections::HashSet::<String>::new(),
        );
        if !searching
            && self.connected
            && self.runtime.is_some()
            && self.workspaces.len() > 1
            && selected_filter.is_none()
        {
            let missing_ws: Vec<String> = {
                let attempted = preloaded_ws_state.read(cx);
                self.workspaces
                    .iter()
                    .filter(|ws| {
                        self.workspace.as_ref() != Some(&ws.id)
                            && !self.project_sessions.contains_key(&ws.id)
                            && !self.expanded_projects.contains(&ws.id)
                            && !attempted.contains(&ws.id)
                    })
                    .map(|ws| ws.id.clone())
                    .collect()
            };
            if !missing_ws.is_empty() {
                preloaded_ws_state.update(cx, |attempted, _| {
                    attempted.extend(missing_ws.iter().cloned());
                });
                let weak = cx.weak_entity();
                window.on_next_frame(move |_, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        for id in missing_ws {
                            if this.workspace.as_ref() != Some(&id)
                                && !this.project_sessions.contains_key(&id)
                                && !this.expanded_projects.contains(&id)
                            {
                                this.toggle_project(id, cx);
                            }
                        }
                    });
                });
            }
        }

        let sessions = self.sidebar_conversation_sections(cx);
        let session_count = sessions.len();
        let active_workspace = self
            .workspace
            .as_ref()
            .and_then(|id| self.workspaces.iter().find(|ws| &ws.id == id))
            .or_else(|| self.workspaces.first());
        let filtered_workspace =
            selected_filter.and_then(|id| self.workspaces.iter().find(|ws| ws.id == id));
        let new_thread_workspace = filtered_workspace.or(active_workspace);
        let in_new_thread =
            self.workspace.is_some() && self.session.is_none() && self.viewing_conversation();
        let show_new_thread_row =
            new_thread_workspace.is_some() && (in_new_thread || session_count == 0);
        let show_working_project = self.workspaces.len() > 1 && selected_filter.is_none();

        v_flex()
            .relative()
            .when(narrow, |view| view.flex_1().min_w_0())
            .when(!narrow, |view| {
                view.w(self.sidebar.resolved(window.viewport_size().width))
                    .flex_shrink_0()
            })
            .h_full()
            .bg(Color::Sidebar.get(cx))
            .border_r_1()
            .border_color(Color::Divider.get(cx))
            .px_2()
            .pb_2()
            .gap_0p5()
            .child(self.render_navigation_bar(true, window, cx))
            .child(self.render_sidebar_brand_probe(window, cx))
            .child(h_flex().px_0p5().child(self.global_inbox_button("sidebar-global-inbox", cx)))
            .children(self.render_sidebar_width_notice(cx))
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
                            div()
                                .id("sidebar-inline-search-input")
                                .capture_key_down(move |event, window, cx| {
                                    search_keys
                                        .update(cx, |search, cx| search.key(event, window, cx));
                                })
                                .child(
                                    Input::new(&query)
                                        .small()
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .cleanable(true)
                                        .prefix(
                                            Icon::new(Glyph::Search)
                                                .size(px(14.))
                                                .text_color(Color::Muted.get(cx)),
                                        )
                                        .aria_label(tr("搜索会话名称、项目路径或 Agent")),
                                ),
                        ),
                    )
                    .child(self.render_filter_navigation(selected_filter.map(str::to_owned), cx))
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
                            .on_click(cx.listener(move |this, _, window, cx| {
                                query.update(cx, |input, cx| input.set_value("", window, cx));
                                this.add_workspace_open = !this.add_workspace_open;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("new-conversation")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_md()
                            .icon(Glyph::SquarePen)
                            .accessibility_label(tr("全局新建对话"))
                            .tooltip(crate::keybindings::hint(
                                crate::keybindings::Target::NewSession,
                                "新建对话，Shift+点击在当前项目中创建",
                                cx,
                            ))
                            .disabled(!self.can_start_new_conversation())
                            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                                this.start_new_conversation(event.modifiers().shift, window, cx);
                            })),
                    )
                    .child(self.new_chat_menu("new-session-menu", tr("新建会话"), None, cx)),
            )
            .when(searching, |sidebar| sidebar.child(search.clone()))
            .when(!searching, |sidebar| {
                sidebar.child(Viewport::new(
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
                        .when(self.add_workspace_open, |section| {
                            section.child(
                                v_flex()
                                    .mb_2()
                                    .p_2p5()
                                    .gap_2()
                                    .rounded_xl()
                                    .bg(Color::Card.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(Reveal::new(
                                        "workspace-folder-input-reveal",
                                        &self.navigation_scroll,
                                        Input::new(&self.folder)
                                            .small()
                                            .aria_label(tr("工作区文件夹的绝对路径")),
                                    ))
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .child(Reveal::new(
                                                "workspace-folder-picker-reveal",
                                                &self.navigation_scroll,
                                                Button::new("choose-workspace-folder")
                                                    .outline()
                                                    .compact()
                                                    .small()
                                                    .h(px(28.))
                                                    .rounded_lg()
                                                    .label(tr("选择文件夹…"))
                                                    .disabled(!self.connected)
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.choose_workspace_folder(window, cx)
                                                        },
                                                    )),
                                            ))
                                            .child(Reveal::new(
                                                "workspace-folder-add-reveal",
                                                &self.navigation_scroll,
                                                Button::new("add-workspace")
                                                    .primary()
                                                    .compact()
                                                    .small()
                                                    .h(px(28.))
                                                    .rounded_lg()
                                                    .label(tr("添加文件夹"))
                                                    .disabled(
                                                        !self.connected || self.runtime.is_none(),
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
                        .when_some(filtered_workspace, |section, ws| {
                            section.child(
                                h_flex().w_full().px_1().pb_1p5().child(
                                    Button::new("clear-workspace-filter")
                                        .custom(palette::sidebar_item_variant(true, cx))
                                        .compact()
                                        .h(px(24.))
                                        .px_2()
                                        .rounded_md()
                                        .tooltip(tr("清除筛选"))
                                        .accessibility_label(tr("清除筛选"))
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_1p5()
                                                .child(palette::project_badge(&ws.name, 14., cx))
                                                .child(
                                                    div()
                                                        .max_w(px(150.))
                                                        .text_size(px(12.))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .truncate()
                                                        .child(ws.name.clone()),
                                                )
                                                .child(
                                                    Icon::new(Glyph::X)
                                                        .size(px(11.))
                                                        .text_color(Color::Muted.get(cx)),
                                                ),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.sidebar_workspace_filter = None;
                                            cx.notify();
                                        })),
                                ),
                            )
                        })
                        .child(self.render_pinned_navigation(cx))
                        .child(
                            v_flex()
                                .w_full()
                                .flex_shrink_0()
                                .gap_0p5()
                                .child(
                                    Button::new("toggle-project-section")
                                        .custom(palette::quiet_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(24.))
                                        .px_2()
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
                                                        .child(tr("全部会话")),
                                                )
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .gap_1()
                                                        .when(session_count > 0, |r| {
                                                            r.child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .text_color(
                                                                        Color::Muted.get(cx),
                                                                    )
                                                                    .child(session_count.to_string()),
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
                                    if self.workspaces.is_empty() {
                                        section.child(
                                            Button::new("empty-open-workspace")
                                                .custom(palette::sidebar_item_variant(false, cx))
                                                .compact()
                                                .h(px(30.))
                                                .px_2()
                                                .w_full()
                                                .rounded_md()
                                                .accessibility_label(tr("添加工作区"))
                                                .child(
                                                    h_flex()
                                                        .w_full()
                                                        .min_w_0()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            Icon::new(Glyph::FolderPlus)
                                                                .size(px(14.))
                                                                .flex_shrink_0()
                                                                .text_color(Color::Muted.get(cx)),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .text_left()
                                                                .text_size(px(13.))
                                                                .text_color(Color::Muted.get(cx))
                                                                .truncate()
                                                                .child(tr("添加工作区")),
                                                        ),
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.add_workspace_open = true;
                                                    cx.notify();
                                                })),
                                        )
                                    } else {
                                        let section =
                                            if show_new_thread_row {
                                                if let Some(ws) = new_thread_workspace {
                                                    let ws_id = ws.id.clone();
                                                    let ws_selected = self.workspace.as_ref()
                                                        == Some(&ws.id)
                                                        && self.session.is_none()
                                                        && self.viewing_conversation();
                                                    section.child(
                                                Button::new("empty-project-new-thread")
                                                    .custom(palette::sidebar_item_variant(
                                                        ws_selected,
                                                        cx,
                                                    ))
                                                    .compact()
                                                    .h(px(30.))
                                                    .px_2()
                                                    .w_full()
                                                    .rounded_md()
                                                    .accessibility_label(format!(
                                                        "{} · {}",
                                                        ws.name,
                                                        tr("新对话")
                                                    ))
                                                    .tooltip(ws.path.clone())
                                                    .selected(ws_selected)
                                                    .disabled(
                                                        self.workspace_removing.as_ref()
                                                            == Some(&ws.id),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .min_w_0()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(palette::project_badge(
                                                                &ws.name, 18., cx,
                                                            ))
                                                            .child(
                                                                div()
                                                                    .flex_1()
                                                                    .min_w_0()
                                                                    .text_left()
                                                                    .text_size(px(13.))
                                                                    .when(ws_selected, |t| {
                                                                        t.font_weight(
                                                                            FontWeight::MEDIUM,
                                                                        )
                                                                    })
                                                                    .text_color(if ws_selected {
                                                                        Color::Text.get(cx)
                                                                    } else {
                                                                        Color::Secondary.get(cx)
                                                                    })
                                                                    .truncate()
                                                                    .child(tr("新对话")),
                                                            ),
                                                    )
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            if this.terminal.visible {
                                                                this.remember_terminal_focus(
                                                                    window, cx,
                                                                );
                                                            }
                                                            this.select_workspace(
                                                                ws_id.clone(),
                                                                cx,
                                                            );
                                                        },
                                                    )),
                                            )
                                                } else {
                                                    section
                                                }
                                            } else {
                                                section
                                            };
                                        section
                                            .children(sessions.regular.into_iter().map(|session| {
                                                self.render_session_link(session, cx)
                                            }))
                                            .children(sessions.working.iter().map(|project| {
                                                self.render_working_project(
                                                    project, show_working_project, cx,
                                                )
                                            }))
                                    }
                                }),
                        ),
                ))
            })
            .child(
                h_flex()
                    .flex_shrink_0()
                    .w_full()
                    .h(px(34.))
                    .items_center()
                    .border_t_1()
                    .border_color(Color::Divider.get(cx))
                    .px_0p5()
                    .gap_0p5()
                    .child(
                        Button::new("open-settings")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_md()
                            .icon(Glyph::Settings)
                            .tooltip(crate::keybindings::hint(
                                crate::keybindings::Target::Preferences,
                                "设置",
                                cx,
                            ))
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
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_md()
                            .icon(Glyph::SquareTerminal)
                            .tooltip(crate::keybindings::hint(
                                crate::keybindings::Target::ToggleTerminal,
                                "工作区终端",
                                cx,
                            ))
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
                    .child(
                        Button::new("sidebar-git-drawer")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_md()
                            .icon(Glyph::GitBranch)
                            .tooltip(crate::keybindings::hint(
                                crate::keybindings::Target::OpenGitWorkspace,
                                "Git 状态与差异",
                                cx,
                            ))
                            .accessibility_label(tr("Git 状态与差异"))
                            .selected(self.git_drawer_open)
                            .disabled(!terminal_enabled || self.workspace.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_git_workspace(window, cx);
                            })),
                    )
                    .when(self.workspace.is_some(), |row| {
                        row.child(self.project_file_browser_button("sidebar-project-files", window, cx))
                    })
                    .child(div().flex_1())
                    .when(!self.pending_reports.is_empty(), |row| {
                        row.child(
                            div()
                                .px_1p5()
                                .py(px(1.))
                                .rounded_full()
                                .bg(Color::AccentSoft.get(cx))
                                .text_size(px(10.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Color::Accent.get(cx))
                                .child(self.pending_reports.len().to_string()),
                        )
                    })
                    .child(
                        Button::new("toggle-members")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_md()
                            .icon(Glyph::Bot)
                            .tooltip(tr("团队成员"))
                            .accessibility_label(tr("团队成员"))
                            .selected(self.member_panel)
                            .disabled(!terminal_enabled)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_team(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("open-collaboration")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_md()
                            .icon(Glyph::Network)
                            .tooltip(tr("协作概览"))
                            .accessibility_label(tr("协作概览"))
                            .selected(self.collaboration.visible)
                            .disabled(!terminal_enabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_collaboration(cx);
                                if window.viewport_size().width < px(820.) {
                                    this.sidebar_open = false;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("sidebar-workflows")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_md()
                            .icon(Glyph::Workflow)
                            .tooltip(tr("工作流"))
                            .accessibility_label(tr("工作流"))
                            .selected(self.workflows.visible)
                            .disabled(!terminal_enabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_workflows(window, cx);
                            })),
                    ),
            )
            .when(!narrow, |sidebar| {
                sidebar.child(self.render_sidebar_resize(window, cx))
            })
    }
}
