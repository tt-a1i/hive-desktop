use crate::{
    app::{HiveApp, SettingsSection},
    appearance::Mode as AppearanceMode,
    language::{english, tr},
    palette::{self, Color},
    protocol::{Session, Workspace},
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::{Enter, Input, InputEvent, InputState, MoveDown, MoveUp},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};

#[derive(Clone)]
enum PaletteAction {
    NewSession,
    CreateConversation {
        project: Option<String>,
        backend: &'static str,
        source_revision: u64,
    },
    CopyDraft { project: String, source_revision: u64 },
    ToggleSidebar,
    ToggleTerminal,
    OpenGit,
    OpenMembers,
    OpenCollaboration,
    OpenWorkflows,
    ToggleTheme,
    OpenSettings(SettingsSection),
    SwitchSession(Session),
    SwitchWorkspace(Workspace),
}

#[derive(Clone)]
struct PaletteItem {
    group: &'static str,
    title: String,
    subtitle: Option<String>,
    keywords: String,
    icon: Glyph,
    action: PaletteAction,
}

struct CommandPalette {
    app: WeakEntity<HiveApp>,
    query: Entity<InputState>,
    items: Vec<PaletteItem>,
    selected: usize,
    scroll: ScrollHandle,
    kind: PaletteKind,
}

#[derive(Clone, Copy, PartialEq)]
enum PaletteKind {
    Commands,
    NewConversation,
    DraftCopy,
}

impl PaletteKind {
    fn labels(self) -> (&'static str, &'static str) {
        match self {
            Self::Commands => (tr("快捷命令面板"), tr("输入命令、会话、项目或设置项…")),
            Self::NewConversation => (tr("选择新会话的项目"), tr("按名称或路径搜索项目")),
            Self::DraftCopy => (tr("复制草稿到项目"), tr("按名称或路径搜索项目")),
        }
    }
}

impl HiveApp {
    pub(crate) fn can_start_new_conversation(&self) -> bool {
        self.connected
            && !self.closing
            && !self.loading
            && !self.settings_panel
            && !self.roles.visible
            && !self.welcome_busy()
            && self.workspace_writes_allowed()
            && self.new_session_creation != Some(self.connection_generation)
    }

    fn new_conversation_backend(&self) -> &'static str {
        self.session.as_ref().map_or("pi", |session| session.backend.wire())
    }

    pub(crate) fn start_scratch_conversation(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.can_start_new_conversation() {
            self.create_new_session(None, self.new_conversation_backend(), None, Some(window), cx);
        }
    }

    pub(crate) fn start_new_conversation(
        &mut self,
        current_project_only: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_start_new_conversation() || window.has_active_dialog(cx) {
            return;
        }
        let backend = self.new_conversation_backend();
        if current_project_only {
            if let Some(project) = self.workspace.clone() {
                self.create_new_session(Some(project), backend, None, Some(window), cx);
            }
            return;
        }

        let scratch_path = self.data_dir.as_ref().map(|root| root.join("scratch"));
        let canonical_scratch_path = self.data_dir.as_ref()
            .and_then(|root| root.canonicalize().ok())
            .map(|root| root.join("scratch"));
        let projects: Vec<_> = self.sorted_projects(cx).into_iter().filter(|workspace| {
            let path = std::path::Path::new(&workspace.path);
            self.workspace_removing.as_ref() != Some(&workspace.id)
                && scratch_path.as_deref() != Some(path)
                && canonical_scratch_path.as_deref() != Some(path)
        }).collect();
        if let [project] = projects.as_slice() {
            self.create_new_session(Some(project.id.clone()), backend, None, Some(window), cx);
            return;
        }

        let source_revision = self.open_request;
        let mut items: Vec<_> = projects.into_iter().map(|project| PaletteItem {
            group: "项目",
            title: project.name.clone(),
            subtitle: Some(project.path.clone()),
            keywords: String::new(),
            icon: Glyph::FolderOpen,
            action: PaletteAction::CreateConversation {
                project: Some(project.id.clone()), backend, source_revision,
            },
        }).collect();
        items.push(PaletteItem {
            group: "项目",
            title: tr("不使用项目").into(),
            subtitle: Some(tr("在独立的 Scratch 文件夹中开始").into()),
            keywords: "scratch".into(),
            icon: Glyph::SquarePen,
            action: PaletteAction::CreateConversation { project: None, backend, source_revision },
        });
        self.open_palette(items, PaletteKind::NewConversation, window, cx);
    }

    pub(crate) fn open_draft_copy_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_copy_draft(cx) || window.has_active_dialog(cx) {
            return;
        }
        let source_revision = self.open_request;
        let items = self.sorted_projects(cx).into_iter().filter(|project| {
            self.workspace.as_ref() != Some(&project.id)
                && self.workspace_removing.as_ref() != Some(&project.id)
        }).map(|project| PaletteItem {
            group: "项目",
            title: project.name.clone(),
            subtitle: Some(project.path.clone()),
            keywords: String::new(),
            icon: Glyph::FolderOpen,
            action: PaletteAction::CopyDraft { project: project.id.clone(), source_revision },
        }).collect();
        self.open_palette(items, PaletteKind::DraftCopy, window, cx);
    }

    pub(crate) fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let items = self.build_command_palette_items(cx);
        self.open_palette(items, PaletteKind::Commands, window, cx);
    }

    fn open_palette(
        &mut self,
        items: Vec<PaletteItem>,
        kind: PaletteKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (title, placeholder) = kind.labels();
        let app = cx.weak_entity();
        let palette_view = cx.new(|cx: &mut Context<CommandPalette>| {
            let query = cx.new(|cx| {
                InputState::new(window, cx).placeholder(placeholder)
            });
            cx.subscribe(&query, |this: &mut CommandPalette, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.selected = 0;
                    this.scroll = ScrollHandle::new();
                    cx.notify();
                }
            })
            .detach();
            CommandPalette {
                app,
                query,
                items,
                selected: 0,
                scroll: ScrollHandle::new(),
                kind,
            }
        });
        let focus = palette_view.read(cx).query.read(cx).focus_handle(cx);
        let palette_width = (window.viewport_size().width - px(40.)).min(px(580.));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(title)
                .w(palette_width)
                .on_ok(|_, _, _| false)
                .child(palette_view.clone())
        });
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
    }

    fn build_command_palette_items(&self, cx: &App) -> Vec<PaletteItem> {
        let mut items = Vec::new();
        let dark = Theme::global(cx).is_dark();

        items.push(PaletteItem {
            group: "操作",
            title: tr("新建会话").to_owned(),
            subtitle: Some(tr("有多个项目时先选择项目").to_owned()),
            keywords: "new chat session thread 新建 会话".into(),
            icon: Glyph::SquarePen,
            action: PaletteAction::NewSession,
        });
        items.push(PaletteItem {
            group: "操作",
            title: tr("打开或收起侧栏").to_owned(),
            subtitle: None,
            keywords: "sidebar navigation toggle 侧栏 导航".into(),
            icon: Glyph::PanelLeft,
            action: PaletteAction::ToggleSidebar,
        });
        items.push(PaletteItem {
            group: "操作",
            title: tr("工作区终端").to_owned(),
            subtitle: Some(tr("打开或切换项目终端抽屉").to_owned()),
            keywords: "terminal shell pty 终端".into(),
            icon: Glyph::Terminal,
            action: PaletteAction::ToggleTerminal,
        });
        items.push(PaletteItem {
            group: "操作",
            title: tr("Git 变更与 Diff").to_owned(),
            subtitle: Some(tr("查看当前工作区或工作树差异与提交").to_owned()),
            keywords: "git diff commit status pr 差异 提交".into(),
            icon: Glyph::GitBranch,
            action: PaletteAction::OpenGit,
        });
        items.push(PaletteItem {
            group: "操作",
            title: tr("协作概览").to_owned(),
            subtitle: Some(tr("查看团队实时态势驾驶舱与派单").to_owned()),
            keywords: "collaboration cockpit dispatch team 协作 驾驶舱 派单".into(),
            icon: Glyph::Network,
            action: PaletteAction::OpenCollaboration,
        });
        items.push(PaletteItem {
            group: "操作",
            title: tr("团队成员").to_owned(),
            subtitle: Some(tr("管理主控与 Worker 成员配置").to_owned()),
            keywords: "team members worker lead 团队 成员".into(),
            icon: Glyph::Users,
            action: PaletteAction::OpenMembers,
        });
        items.push(PaletteItem {
            group: "操作",
            title: tr("工作流").to_owned(),
            subtitle: None,
            keywords: "workflows schedule cron 工作流 定时".into(),
            icon: Glyph::Workflow,
            action: PaletteAction::OpenWorkflows,
        });
        items.push(PaletteItem {
            group: "操作",
            title: if dark {
                tr("切换为浅色主题").to_owned()
            } else {
                tr("切换为深色主题").to_owned()
            },
            subtitle: None,
            keywords: "theme light dark appearance 主题 深色 浅色 外观".into(),
            icon: if dark { Glyph::Sun } else { Glyph::Moon },
            action: PaletteAction::ToggleTheme,
        });

        for session in self.sessions.iter().filter(|s| !s.archived).take(24) {
            let title = self.session_title(session);
            let branch = session
                .execution
                .as_ref()
                .filter(|e| e.kind == "worktree")
                .map(|e| e.branch.trim())
                .filter(|b| !b.is_empty());
            let subtitle = match branch {
                Some(b) => Some(format!("{} · worktree · {b}", tr(session.backend.label()))),
                None => Some(tr(session.backend.label()).to_owned()),
            };
            items.push(PaletteItem {
                group: "会话",
                title: title.to_owned(),
                subtitle,
                keywords: format!("session thread {title} {} {}", session.backend.wire(), branch.unwrap_or("")),
                icon: Glyph::MessagesSquare,
                action: PaletteAction::SwitchSession(session.clone()),
            });
        }

        for workspace in self.workspaces.iter().take(16) {
            if self.workspace.as_deref() == Some(workspace.id.as_str()) {
                continue;
            }
            items.push(PaletteItem {
                group: "项目",
                title: workspace.name.clone(),
                subtitle: Some(workspace.path.clone()),
                keywords: format!("project workspace {} {}", workspace.name, workspace.path),
                icon: Glyph::FolderOpen,
                action: PaletteAction::SwitchWorkspace(workspace.clone()),
            });
        }

        for (label, kw, section) in [
            ("设置 → 外部程序与 MCP", "settings programs mcp readiness 外部程序 就绪度", SettingsSection::Programs),
            ("设置 → 内置 Pi", "settings pi accounts auth login 内置 账号", SettingsSection::PiAccounts),
            ("设置 → 模型与接口", "settings models providers 模型 接口", SettingsSection::Models),
            ("设置 → Diff 与 PR", "settings diff inline unified split pr 差异", SettingsSection::Diffs),
            ("设置 → 外观", "settings appearance theme font 外观 字体", SettingsSection::Appearance),
            ("设置 → 快捷键", "settings keybindings shortcuts 快捷键", SettingsSection::Keybindings),
        ] {
            items.push(PaletteItem {
                group: "设置",
                title: tr(label).to_owned(),
                subtitle: None,
                keywords: kw.into(),
                icon: Glyph::Settings,
                action: PaletteAction::OpenSettings(section),
            });
        }

        items
    }
}

impl CommandPalette {
    fn filtered_items(&self, cx: &App) -> Vec<&PaletteItem> {
        let q = self.query.read(cx).value().trim().to_lowercase();
        if q.is_empty() {
            return self.items.iter().collect();
        }
        let terms: Vec<&str> = q.split_whitespace().collect();
        self.items
            .iter()
            .filter(|item| {
                let hay = format!(
                    "{} {} {}",
                    item.title.to_lowercase(),
                    item.subtitle.as_deref().unwrap_or("").to_lowercase(),
                    item.keywords.to_lowercase()
                );
                terms.iter().all(|t| hay.contains(t))
            })
            .collect()
    }

    fn execute(&mut self, action: PaletteAction, window: &mut Window, cx: &mut Context<Self>) {
        let app = self.app.clone();
        window.close_dialog(cx);
        let _ = app.update(cx, |this, cx| {
            match action {
                PaletteAction::NewSession => this.start_new_conversation(false, window, cx),
                PaletteAction::CreateConversation { project, backend, source_revision } => {
                    if this.open_request == source_revision && this.can_start_new_conversation() {
                        this.create_new_session(project, backend, None, Some(window), cx);
                    }
                }
                PaletteAction::CopyDraft { project, source_revision } => {
                    this.begin_draft_copy(project, source_revision, window, cx);
                }
                PaletteAction::ToggleSidebar => {
                    this.sidebar_open = !this.sidebar_open;
                    cx.notify();
                }
                PaletteAction::ToggleTerminal => {
                    if this.terminal.visible {
                        this.hide_terminal(cx);
                    } else {
                        this.show_terminal(window, cx);
                    }
                }
                PaletteAction::OpenGit => {
                    this.open_git_workspace(window, cx);
                }
                PaletteAction::OpenMembers => {
                    this.show_team(window, cx);
                }
                PaletteAction::OpenCollaboration => {
                    this.open_collaboration(cx);
                }
                PaletteAction::OpenWorkflows => {
                    this.show_workflows(window, cx);
                }
                PaletteAction::ToggleTheme => {
                    let next = if Theme::global(cx).is_dark() {
                        AppearanceMode::Light
                    } else {
                        AppearanceMode::Dark
                    };
                    crate::appearance::choose(next, window, cx);
                }
                PaletteAction::OpenSettings(section) => {
                    this.select_settings_section(section, window, cx);
                }
                PaletteAction::SwitchSession(session) => {
                    let _ = this.navigate_to_session(session, window, cx);
                }
                PaletteAction::SwitchWorkspace(workspace) => {
                    this.select_workspace(workspace.id, cx);
                }
            }
            cx.notify();
        });
    }

    fn has_marked_text(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.query.update(cx, |input, cx| input.marked_text_range(window, cx).is_some())
    }

    fn select_project_row(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_marked_text(window, cx) {
            return;
        }
        let len = self.filtered_items(cx).len();
        if len > 0 {
            self.selected = (self.selected + if forward { 1 } else { len - 1 }) % len;
            self.scroll.scroll_to_item(self.selected);
            cx.notify();
        }
        cx.stop_propagation();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let m = event.keystroke.modifiers;
        if m.control || m.platform || m.alt || (m.shift && event.keystroke.key.as_str() != "tab") {
            return;
        }
        let matches: Vec<PaletteAction> = self
            .filtered_items(cx)
            .into_iter()
            .map(|i| i.action.clone())
            .collect();
        match event.keystroke.key.as_str() {
            "up" | "down" | "tab" if !matches.is_empty() => {
                let len = matches.len();
                let forward = event.keystroke.key == "down"
                    || (event.keystroke.key == "tab" && !m.shift);
                self.selected = if forward {
                    (self.selected + 1) % len
                } else {
                    (self.selected + len - 1) % len
                };
                self.scroll.scroll_to_item(self.selected);
                cx.notify();
            }
            "enter" => {
                if let Some(action) = matches.get(self.selected).cloned() {
                    self.execute(action, window, cx);
                }
            }
            "escape" => {
                let composer_focus = self
                    .app
                    .upgrade()
                    .map(|app| app.read(cx).input.read(cx).focus_handle(cx));
                window.close_dialog(cx);
                if let Some(focus) = composer_focus {
                    window.on_next_frame(move |window, cx| focus.focus(window, cx));
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered: Vec<PaletteItem> = self.filtered_items(cx).into_iter().cloned().collect();
        let selected = self.selected.min(filtered.len().saturating_sub(1));
        v_flex()
            .w_full()
            .gap_2p5()
            .when(self.kind == PaletteKind::Commands, |view| view.on_key_down(cx.listener(Self::on_key_down)))
            .child(
                div()
                    .when(self.kind != PaletteKind::Commands, |view| {
                        view.capture_action(cx.listener(|this, action: &Enter, window, cx| {
                            if !action.secondary && !action.shift && !this.has_marked_text(window, cx) {
                                cx.stop_propagation();
                                let action = this.filtered_items(cx).get(this.selected).map(|item| item.action.clone());
                                if let Some(action) = action {
                                    this.execute(action, window, cx);
                                }
                            }
                        }))
                        .capture_action(cx.listener(|this, _: &MoveDown, window, cx| this.select_project_row(true, window, cx)))
                        .capture_action(cx.listener(|this, _: &MoveUp, window, cx| this.select_project_row(false, window, cx)))
                    })
                    .child(Input::new(&self.query)
                        .aria_label(self.kind.labels().1)
                        .cleanable(true)
                        .prefix(Icon::new(Glyph::Search).size(px(14.)).text_color(Color::Muted.get(cx)))),
            )
            .child(
                div()
                    .id("command-palette-list")
                    .w_full()
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .vertical_scrollbar(&self.scroll)
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .when(filtered.is_empty(), |col| {
                                col.child(
                                    div()
                                        .py_6()
                                        .text_center()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(if self.kind != PaletteKind::Commands { tr("没有匹配的项目") } else { tr("没有匹配的命令、会话或设置项") }),
                                )
                            })
                            .children(filtered.into_iter().enumerate().map(|(ix, item)| {
                                let is_sel = ix == selected;
                                let action = item.action.clone();
                                let accessible_name = match &item.subtitle {
                                    Some(detail) => format!("{} · {detail}", item.title),
                                    None => item.title.clone(),
                                };
                                Button::new(("palette-item", ix))
                                    .custom(palette::sidebar_item_variant(is_sel, cx))
                                    .accessibility_label(accessible_name)
                                    .selected(is_sel)
                                    .w_full()
                                    .h_auto()
                                    .px_2p5()
                                    .py_2()
                                    .rounded_lg()
                                    .justify_start()
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
                                                    .gap_2()
                                                    .child(
                                                        Icon::new(item.icon)
                                                            .size(px(14.))
                                                            .flex_shrink_0()
                                                            .text_color(if is_sel {
                                                                Color::Accent.get(cx)
                                                            } else {
                                                                Color::Muted.get(cx)
                                                            }),
                                                    )
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .items_start()
                                                            .gap_0p5()
                                                            .child(
                                                                div()
                                                                    .w_full()
                                                                    .truncate()
                                                                    .text_left()
                                                                    .text_size(px(13.))
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(Color::Text.get(cx))
                                                                    .child(item.title),
                                                            )
                                                            .when_some(item.subtitle, |c, sub| {
                                                                c.child(
                                                                    div()
                                                                        .w_full()
                                                                        .truncate()
                                                                        .text_left()
                                                                        .text_size(px(11.))
                                                                        .text_color(Color::Muted.get(cx))
                                                                        .child(sub),
                                                                )
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .px_1p5()
                                                    .py(px(1.))
                                                    .rounded_sm()
                                                    .bg(Color::Inset.get(cx))
                                                    .text_size(px(10.5))
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(tr(item.group).to_owned()),
                                            ),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.execute(action.clone(), window, cx);
                                    }))
                            })),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .pt_1()
                    .border_t_1()
                    .border_color(Color::Divider.get(cx))
                    .text_size(px(11.))
                    .text_color(Color::Muted.get(cx))
                    .child(if english() {
                        "↑↓ / Tab to select · Enter to run · Esc to close"
                    } else {
                        "↑↓ / Tab 选择 · Enter 执行 · Esc 关闭"
                    })
                    .child(crate::keybindings::hint(
                        crate::keybindings::Target::CommandPalette,
                        tr("快捷命令面板"),
                        cx,
                    )),
            )
    }
}
