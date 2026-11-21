use crate::{
    app::HiveApp,
    language::{english, tr},
    palette::Color,
};
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenuItem},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

pub(crate) mod agents;
mod projects;
mod setup;
pub(crate) use setup::Setup;

const GIT_BRANCH_TTL: Duration = Duration::from_secs(3);

fn git_branch_cache() -> &'static Mutex<HashMap<String, (Instant, Option<String>)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Option<String>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn read_git_head(project_path: &str) -> Option<String> {
    let mut current = Some(Path::new(project_path));
    for _ in 0..4 {
        let Some(dir) = current else {
            break;
        };
        let dot_git = dir.join(".git");
        let head_path = if dot_git.is_dir() {
            Some(dot_git.join("HEAD"))
        } else if dot_git.is_file() {
            fs::read_to_string(&dot_git).ok().and_then(|content| {
                content
                    .lines()
                    .find_map(|line| line.strip_prefix("gitdir:"))
                    .map(|raw| {
                        let target = PathBuf::from(raw.trim());
                        if target.is_absolute() {
                            target.join("HEAD")
                        } else {
                            dir.join(target).join("HEAD")
                        }
                    })
            })
        } else {
            None
        };
        if let Some(head_path) = head_path {
            let head = fs::read_to_string(head_path).ok()?;
            let line = head.lines().next()?.trim();
            if let Some(branch) = line.strip_prefix("ref: refs/heads/") {
                let branch = branch.trim();
                return (!branch.is_empty()).then(|| branch.to_owned());
            }
            if let Some(reference) = line.strip_prefix("ref: ") {
                let reference = reference.trim();
                return (!reference.is_empty()).then(|| reference.to_owned());
            }
            if line.len() >= 7 && line.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Some(line[..7].to_owned());
            }
            return None;
        }
        current = dir.parent();
    }
    None
}

pub(crate) fn cached_git_branch(project_path: &str) -> Option<String> {
    let now = Instant::now();
    if let Ok(cache) = git_branch_cache().lock() {
        if let Some((fetched_at, branch)) = cache.get(project_path) {
            if now.duration_since(*fetched_at) < GIT_BRANCH_TTL {
                return branch.clone();
            }
        }
    }
    let branch = read_git_head(project_path);
    if let Ok(mut cache) = git_branch_cache().lock() {
        if cache.len() > 64 {
            cache.clear();
        }
        cache.insert(project_path.to_owned(), (now, branch.clone()));
    }
    branch
}

impl HiveApp {
    /// T3 Code Layer-2 borderless metadata strip directly below the Composer card.
    pub(crate) fn render_composer_meta_bar(
        &self,
        window: &Window,
        running: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let active_project = self
            .workspaces
            .iter()
            .find(|ws| Some(&ws.id) == self.workspace.as_ref())
            .cloned();
        let execution = self.session.as_ref().and_then(|session| session.execution.as_ref());
        let worktree_released = self.current_worktree_released();
        let shared_worktree = execution.is_some_and(|execution| execution.owner_session_id.is_some());
        let reusable_worktree = self.shared_worktree_draft_source().map(|source| source.id.clone());
        let can_reuse_worktree = self.can_create_shared_worktree_draft();
        let scratch = self.session.as_ref().is_some_and(|session| session.scratch);
        let execution_path = execution.map(|execution| execution.path.clone())
            .or_else(|| self.session.as_ref().and_then(|session| session.execution_path.clone()))
            .or_else(|| active_project.as_ref().filter(|_| !scratch).map(|workspace| workspace.path.clone()));
        let branch = execution.map(|execution| execution.branch.clone())
            .or_else(|| execution_path.as_deref().and_then(cached_git_branch));
        let git_hint = crate::keybindings::hint(
            crate::keybindings::Target::OpenGitWorkspace,
            "查看项目 Git 状态与差异",
            cx,
        );
        let execution_tooltip = execution_path
            .as_ref()
            .map(|path| {
                format!(
                    "{} · {}\n{}",
                    tr(if scratch {
                        "临时会话目录"
                    } else if shared_worktree {
                        "共用工作树执行目录"
                    } else if execution.is_some() {
                        "独立工作树执行目录"
                    } else {
                        "会话执行目录"
                    }),
                    path,
                    git_hint
                )
            })
            .unwrap_or(git_hint);
        let member_badge = self
            .has_active_team_context()
            .then(|| {
                self.session
                    .as_ref()
                    .and_then(|s| s.agent_id.as_deref())
                    .map(|id| self.agent_name(id))
            })
            .flatten();

        let chat_width = cx.global::<crate::appearance::Appearance>().chat_width_px();
        h_flex()
            .w_full()
            .min_w_0()
            .max_w(px(chat_width))
            .mx_auto()
            .items_center()
            .justify_between()
            .gap_2()
            .px_2()
            .pt_1p5()
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .child({
                        if let Some(ws) = active_project.as_ref() {
                            let folder_label = if worktree_released {
                                format!("{} · {}", tr("已解除共用工作树"), ws.name)
                            } else if shared_worktree {
                                format!("{} · {}", tr("共用工作树"), ws.name)
                            } else if execution.is_some() {
                                format!("{} · {}", tr("独立工作树"), ws.name)
                            } else {
                                tr("当前检出").to_owned()
                            };
                            let folder_tooltip =
                                format!("{}\n{}", ws.path, tr("点击切换工作区或新建工作树"));
                            let ws_id = ws.id.clone();
                            let workspaces: Vec<(String, String)> = self
                                .workspaces
                                .iter()
                                .map(|item| (item.id.clone(), item.name.clone()))
                                .collect();
                            let weak = cx.entity().downgrade();
                            let can_copy_draft = self.can_copy_draft(cx);
                            let retry_copies = self.pending_draft_copy_targets();
                            Button::new("composer-project-folder")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(22.))
                                .px_1p5()
                                .rounded_md()
                                .min_w_0()
                                .tooltip(folder_tooltip)
                                .accessibility_label(tr("打开项目文件夹"))
                                .disabled(!self.connected || self.closing)
                                .child(
                                    h_flex()
                                        .min_w_0()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            Icon::new(Glyph::Folder)
                                                .size(px(12.5))
                                                .text_color(Color::Muted.get(cx))
                                                .flex_shrink_0(),
                                        )
                                        .child(
                                            div()
                                                .min_w_0()
                                                .max_w(px(210.))
                                                .truncate()
                                                .text_size(px(12.))
                                                .text_color(Color::Muted.get(cx))
                                                .child(folder_label),
                                        ),
                                )
                                .dropdown_menu(move |mut menu, _, _| {
                                    if can_copy_draft {
                                        let owner = weak.clone();
                                        menu = menu.item(PopupMenuItem::new(tr("复制草稿到项目…")).on_click(
                                            move |_, window, cx| {
                                                let owner = owner.clone();
                                                window.defer(cx, move |window, cx| {
                                                    let _ = owner.update(cx, |app, cx| app.open_draft_copy_picker(window, cx));
                                                });
                                            },
                                        ));
                                    }
                                    for (id, project) in &retry_copies {
                                        let owner = weak.clone();
                                        let id = id.clone();
                                        let title = format!("{} · {project}", tr("重试复制草稿"));
                                        menu = menu.item(PopupMenuItem::new(title).on_click(
                                            move |_, window, cx| {
                                                let _ = owner.update(cx, |app, cx| app.run_draft_copy(id.clone(), Some(window), cx));
                                            },
                                        ));
                                    }
                                    if can_copy_draft || !retry_copies.is_empty() { menu = menu.separator(); }
                                    if let Some(source) = &reusable_worktree {
                                        let owner = weak.clone();
                                        let source = source.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(tr("在此工作树新建会话"))
                                                .disabled(!can_reuse_worktree)
                                                .on_click(move |_, _, cx| {
                                                    let _ = owner.update(cx, |app, cx| app.create_shared_worktree_draft(&source, cx));
                                                }),
                                        );
                                    }
                                    let weak_new = weak.clone();
                                    let new_ws = ws_id.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(tr("新建独立工作树会话…")).on_click(
                                            move |_, window, cx| {
                                                let _ = weak_new.update(cx, |this, cx| {
                                                    this.open_session_execution(
                                                        Some(new_ws.clone()),
                                                        false,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            },
                                        ),
                                    );
                                    let weak_recover = weak.clone();
                                    let recover_ws = ws_id.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(tr("查看待恢复工作树…")).on_click(
                                            move |_, window, cx| {
                                                let _ = weak_recover.update(cx, |this, cx| {
                                                    this.open_session_execution(
                                                        Some(recover_ws.clone()),
                                                        true,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            },
                                        ),
                                    );
                                    if workspaces.len() > 1 {
                                        menu = menu.separator();
                                        for (id, name) in &workspaces {
                                            let weak_ws = weak.clone();
                                            let target = id.clone();
                                            let selected = *id == ws_id;
                                            menu = menu.item(
                                                PopupMenuItem::new(name.clone())
                                                    .checked(selected)
                                                    .on_click(move |_, _, cx| {
                                                        let _ = weak_ws.update(cx, |this, cx| {
                                                            this.select_workspace(
                                                                target.clone(),
                                                                cx,
                                                            );
                                                        });
                                                    }),
                                            );
                                        }
                                    }
                                    let weak_browse = weak.clone();
                                    menu.separator().item(
                                        PopupMenuItem::new(tr("打开其他项目文件夹…")).on_click(
                                            move |_, window, cx| {
                                                let _ = weak_browse.update(cx, |this, cx| {
                                                    this.choose_workspace_folder(window, cx);
                                                });
                                            },
                                        ),
                                    )
                                })
                                .into_any_element()
                        } else {
                            Button::new("composer-project-folder")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(22.))
                                .px_1p5()
                                .rounded_md()
                                .min_w_0()
                                .tooltip(tr("打开项目文件夹"))
                                .accessibility_label(tr("打开项目文件夹"))
                                .disabled(!self.connected || self.closing)
                                .child(
                                    h_flex()
                                        .min_w_0()
                                        .items_center()
                                        .gap_1p5()
                                        .child(
                                            Icon::new(Glyph::Folder)
                                                .size(px(12.5))
                                                .text_color(Color::Muted.get(cx))
                                                .flex_shrink_0(),
                                        )
                                        .child(
                                            div()
                                                .min_w_0()
                                                .max_w(px(220.))
                                                .truncate()
                                                .text_size(px(12.))
                                                .text_color(Color::Muted.get(cx))
                                                .child(tr("打开项目文件夹")),
                                        ),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.choose_workspace_folder(window, cx);
                                }))
                                .into_any_element()
                        }
                    })
                    .when_some(member_badge, |row, member| {
                        row.child(
                            h_flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap_1()
                                .px_1p5()
                                .py(px(1.))
                                .rounded_md()
                                .bg(Color::Inset.get(cx))
                                .child(
                                    Icon::new(Glyph::Users)
                                        .size(px(11.))
                                        .text_color(Color::Secondary.get(cx)),
                                )
                                .child(
                                    div()
                                        .max_w(px(140.))
                                        .truncate()
                                        .text_size(px(11.5))
                                        .text_color(Color::Secondary.get(cx))
                                        .child(member),
                                ),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .when(active_project.is_some(), |row| {
                        row.child(
                            Button::new("composer-git-branch")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(22.))
                                .px_1p5()
                                .rounded_md()
                                .min_w_0()
                                .selected(self.git_drawer_open)
                                .icon(Glyph::GitBranch)
                                .child(
                                    div()
                                        .min_w_0()
                                        .max_w(px(180.))
                                        .truncate()
                                        .text_size(px(12.))
                                        .text_color(Color::Muted.get(cx))
                                        .child(branch.unwrap_or_else(|| tr("Git 状态").to_owned())),
                                )
                                .tooltip(execution_tooltip)
                                .accessibility_label(tr("查看项目 Git 状态与差异"))
                                .disabled(!self.connected || self.closing)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_git_workspace(window, cx)
                                })),
                        )
                    })
                    .when_some(self.render_composer_mode_pills(running, cx), |row, pills| {
                        row.child(pills)
                    })
                    .when(
                        self.session.is_some()
                            && (!self.messages.is_empty()
                                || self.usage.visible
                                || self.usage.snapshot.is_some()),
                        |row| {
                            row.when(
                                !self
                                    .session
                                    .as_ref()
                                    .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Acp))
                                    && !self
                                        .current_history_access()
                                        .is_some_and(|a| a.handoff_unconsumed()),
                                |row| row.child(self.usage_button(window, running, cx)),
                            )
                        },
                    )
                    .when(self.session.is_none() && active_project.is_some(), |row| {
                        row.child(
                            div()
                                .text_size(px(12.))
                                .text_color(Color::Muted.get(cx))
                                .child(tr("未选择 Agent")),
                        )
                    }),
            )
    }

    /// Unified T3 Code centered Hero stage for both no-project home and empty threads.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_hero_stage(
        &self,
        window: &mut Window,
        narrow: bool,
        running: bool,
        can_send: bool,
        pending: usize,
        needs_pi_model: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let active_project = self
            .workspaces
            .iter()
            .find(|ws| Some(&ws.id) == self.workspace.as_ref())
            .cloned();
        let compact = window.viewport_size().width < px(1000.);
        let tall = window.viewport_size().height > px(560.);
        let hero_px = if compact { 24. } else { 27. };
        let aria_title = if let Some(ws) = &active_project {
            if english() {
                format!("What should we build in {}?", ws.name)
            } else {
                format!("在 {} 中构建什么？", ws.name)
            }
        } else {
            tr("准备构建什么？").into()
        };

        let chat_width = cx.global::<crate::appearance::Appearance>().chat_width_px();
        v_flex()
            .id("welcome-draft-scroll")
            .track_scroll(&self.welcome_scroll)
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .px(if narrow { px(16.) } else { px(32.) })
            .when(tall, |v| v.justify_center().pb(px(88.)))
            .when(!tall, |v| v.pt_6().pb_8())
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(chat_width))
                    .mx_auto()
                    .child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .text_center()
                            .mb_5()
                            .gap_1()
                            .child(
                                div()
                                    .id("session-title")
                                    .role(Role::Label)
                                    .aria_value(aria_title.clone())
                                    .text_size(px(hero_px))
                                    .line_height(px(34.))
                                    .whitespace_normal()
                                    .child(if let Some(ws) = active_project.as_ref() {
                                        let workspaces: Vec<(String, String)> = self
                                            .workspaces
                                            .iter()
                                            .map(|item| (item.id.clone(), item.name.clone()))
                                            .collect();
                                        let active_id = ws.id.clone();
                                        let weak = cx.entity().downgrade();
                                        let project_switcher = Button::new("hero-project-switcher")
                                            .custom(crate::palette::quiet_icon_variant(cx))
                                            .compact()
                                            .px_1()
                                            .py_0p5()
                                            .rounded_md()
                                            .disabled(!self.connected || self.closing)
                                            .accessibility_label(ws.name.clone())
                                            .tooltip(format!(
                                                "{}\n{}",
                                                ws.path,
                                                tr("点击切换工作区")
                                            ))
                                            .child(
                                                div()
                                                    .text_size(px(hero_px))
                                                    .line_height(px(34.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(ws.name.clone()),
                                            )
                                            .dropdown_menu(move |mut menu, _, _| {
                                                for (id, name) in &workspaces {
                                                    let weak_ws = weak.clone();
                                                    let target = id.clone();
                                                    let selected = *id == active_id;
                                                    menu = menu.item(
                                                        PopupMenuItem::new(name.clone())
                                                            .checked(selected)
                                                            .on_click(move |_, _, cx| {
                                                                let _ = weak_ws.update(
                                                                    cx,
                                                                    |this, cx| {
                                                                        this.select_workspace(
                                                                            target.clone(),
                                                                            cx,
                                                                        );
                                                                    },
                                                                );
                                                            }),
                                                    );
                                                }
                                                let weak_browse = weak.clone();
                                                menu.separator().item(
                                                    PopupMenuItem::new(tr("打开其他项目文件夹…"))
                                                        .on_click(move |_, window, cx| {
                                                            let _ = weak_browse.update(
                                                                cx,
                                                                |this, cx| {
                                                                    this.choose_workspace_folder(
                                                                        window, cx,
                                                                    );
                                                                },
                                                            );
                                                        }),
                                                )
                                            });
                                        if english() {
                                            h_flex()
                                                .flex_wrap()
                                                .justify_center()
                                                .items_center()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::NORMAL)
                                                        .text_color(Color::Text.get(cx))
                                                        .child("What should we build in"),
                                                )
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .child(project_switcher)
                                                        .child(
                                                            div()
                                                                .font_weight(FontWeight::NORMAL)
                                                                .text_color(Color::Text.get(cx))
                                                                .child("?"),
                                                        ),
                                                )
                                                .into_any_element()
                                        } else {
                                            h_flex()
                                                .flex_wrap()
                                                .justify_center()
                                                .items_center()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::NORMAL)
                                                        .text_color(Color::Text.get(cx))
                                                        .child("在"),
                                                )
                                                .child(project_switcher)
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::NORMAL)
                                                        .text_color(Color::Text.get(cx))
                                                        .child("中构建什么？"),
                                                )
                                                .into_any_element()
                                        }
                                    } else {
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Color::Text.get(cx))
                                            .child(aria_title)
                                            .into_any_element()
                                    }),
                            )
                            .when_some(active_project.as_ref().filter(|workspace| self.has_scratch_project_origin(&workspace.id)), |hero, workspace| {
                                let workspace = workspace.id.clone();
                                hero.child(Button::new("hero-scratch-origin")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(26.))
                                    .px_2()
                                    .rounded_md()
                                    .label(tr("打开原 Scratch 会话"))
                                    .disabled(!self.connected || self.closing || !self.workspace_writes_allowed())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_scratch_project_origin(workspace.clone(), window, cx);
                                    })))
                            })
                            .child(if active_project.is_some() {
                                Button::new("welcome-switch-project")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .disabled(!self.connected)
                                    .accessibility_label(tr("切换项目文件夹"))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(Color::Muted.get(cx).opacity(0.8))
                                            .child(tr("或打开其他项目文件夹")),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_workspace_folder(window, cx);
                                    }))
                                    .into_any_element()
                            } else {
                                Button::new("welcome-open-project")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .disabled(!self.connected)
                                    .accessibility_label(tr("选择项目文件夹，或在下方直接输入"))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(Color::Muted.get(cx).opacity(0.8))
                                            .child(tr("选择项目文件夹，或在下方直接输入")),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_workspace_folder(window, cx);
                                    }))
                                    .into_any_element()
                            }),
                    )
                    .child(
                        self.render_composer_card(window, true, running, can_send, cx),
                    )
                    .child(self.render_composer_meta_bar(window, running, cx))
                    .child(
                        div()
                            .pt_1p5()
                            .child(self.render_composer_status(
                                running,
                                pending,
                                needs_pi_model,
                                cx,
                            )),
                    )
                    .when(active_project.is_none(), |col| {
                        col.children(self.render_agent_setup_link(cx))
                    })
                    .child(self.render_welcome_starter_prompts(active_project.is_some(), cx))
                    .child(self.render_welcome_discoverability_strip(cx)),
            )
    }

    fn render_welcome_starter_prompts(
        &self,
        has_project: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let draft_empty = self.input.read(cx).value().trim().is_empty();
        if !draft_empty || !self.connected {
            return div().into_any_element();
        }
        let cards: [(&'static str, &'static str, &'static str, Glyph); 4] = if has_project {
            [
                (
                    "梳理架构",
                    "快速了解核心模块、入口与数据流",
                    "请先浏览当前项目结构，梳理核心模块、入口文件与主要数据流，并用简洁列表总结。",
                    Glyph::Layers,
                ),
                (
                    "审查改动",
                    "排查未提交代码中的隐患与边界条件",
                    "请检查当前工作区未提交的 Git 改动，重点排查潜在 Bug、边界条件和可维护性改进点。",
                    Glyph::GitBranch,
                ),
                (
                    "修复报错",
                    "分析构建或测试失败并给出最小修复",
                    "请帮我排查当前项目的构建或测试失败，先定位根因，再给出最小且安全的修复方案。",
                    Glyph::SquareTerminal,
                ),
                (
                    "协作拆解",
                    "将复杂需求拆分为并行任务派发",
                    "请把接下来的开发需求拆分为可并行的子任务清单，明确各成员职责边界与验收标准。",
                    Glyph::Users,
                ),
            ]
        } else {
            [
                (
                    "方案设计",
                    "评估架构权衡与落地步骤",
                    "请帮我设计一个清晰的技术方案，对比可选实现路径的优缺点并给出分阶段落地计划：",
                    Glyph::Sparkles,
                ),
                (
                    "代码重构",
                    "分析瓶颈并输出更简洁的实现",
                    "请帮我审阅并重构下面这段代码，重点提升可读性、健壮性与性能：",
                    Glyph::FileText,
                ),
                (
                    "排查报错",
                    "快速定位异常原因与修复步骤",
                    "我遇到了下面这个报错，请帮我分析可能根因并提供逐步排查与修复方案：",
                    Glyph::SquareTerminal,
                ),
                (
                    "撰写文档",
                    "生成结构化说明与变更清单",
                    "请根据以下改动要点，帮我整理一份结构清晰的文档或 PR 说明：",
                    Glyph::SquarePen,
                ),
            ]
        };

        let render_chip = |idx: usize,
                           title: &'static str,
                           desc: &'static str,
                           prompt: &'static str,
                           icon: Glyph| {
            let prompt_text = if english() {
                match idx {
                    0 if has_project => "Explore this repository and summarize the core modules, entry points, and data flow.",
                    1 if has_project => "Review my current uncommitted Git changes for bugs, edge cases, and potential improvements.",
                    2 if has_project => "Help diagnose the failing test or build in this project, identify the root cause, and propose a minimal fix.",
                    3 if has_project => "Break down my upcoming feature into parallelizable tasks with clear responsibilities for multi-agent collaboration.",
                    0 => "Help me design a technical architecture, compare trade-offs, and outline a phased implementation plan for: ",
                    1 => "Review and refactor the following code for clarity, safety, and performance: ",
                    2 => "Diagnose the root cause of this error stack trace and suggest step-by-step fixes: ",
                    _ => "Draft a structured technical summary or PR description from the following notes: ",
                }
            } else {
                prompt
            };
            Button::new(("welcome-starter-card", idx))
                .custom(crate::palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(28.))
                .px_2p5()
                .rounded_full()
                .bg(Color::Inset.get(cx).opacity(0.42))
                .tooltip(tr(desc))
                .accessibility_label(tr(title))
                .child(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .child(
                            Icon::new(icon)
                                .size(px(12.))
                                .text_color(Color::Muted.get(cx)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(Color::Secondary.get(cx))
                                .child(tr(title)),
                        ),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let input = this.input.clone();
                    input.update(cx, |state, cx| {
                        state.set_value(prompt_text, window, cx);
                    });
                    let focus = this.input.read(cx).focus_handle(cx);
                    window.on_next_frame(move |window, cx| focus.focus(window, cx));
                    cx.notify();
                }))
        };

        h_flex()
            .w_full()
            .flex_wrap()
            .items_center()
            .justify_center()
            .pt_5()
            .gap_2()
            .children(cards.into_iter().enumerate().map(|(idx, (t, d, p, ic))| {
                render_chip(idx, t, d, p, ic)
            }))
            .into_any_element()
    }

    fn render_welcome_discoverability_strip(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let search_key = crate::keybindings::chord_label(crate::keybindings::Target::Search, cx)
            .unwrap_or_else(|| "⌘K".into());
        let palette_key =
            crate::keybindings::chord_label(crate::keybindings::Target::CommandPalette, cx)
                .unwrap_or_else(|| "⌘⇧P".into());
        let hint = if english() {
            format!("{search_key} Search  ·  {palette_key} Commands  ·  Shortcuts")
        } else {
            format!("{search_key} 搜索  ·  {palette_key} 命令面板  ·  快捷键速查")
        };
        h_flex()
            .w_full()
            .items_center()
            .justify_center()
            .pt_2p5()
            .child(
                Button::new("welcome-open-shortcuts-guide")
                    .custom(crate::palette::quiet_icon_variant(cx))
                    .compact()
                    .xsmall()
                    .h(px(22.))
                    .px_2()
                    .rounded_md()
                    .accessibility_label(tr("快捷键与效率速查"))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(Color::Muted.get(cx).opacity(0.6))
                            .child(hint),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_shortcuts_guide(window, cx);
                    })),
            )
    }
}
