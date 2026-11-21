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

fn meta_member_badge_target(session: &crate::protocol::Session) -> Option<Option<&str>> {
    let lead = format!("{}:orchestrator", session.workspace_id);
    if session.identity_state.as_deref() == Some("unknown") {
        return Some(session.display_agent_id());
    }
    session
        .display_agent_id()
        .filter(|id| *id != lead && !id.ends_with(":orchestrator"))
        .map(Some)
}

fn should_show_meta_usage(usage_visible: bool, context_percent: Option<f64>) -> bool {
    usage_visible || context_percent.is_some_and(|percent| percent >= 75.0)
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
        if active_project.is_none() && self.session.is_none() {
            return div().hidden().into_any_element();
        }
        let execution = self.session.as_ref().and_then(|session| session.execution.as_ref());
        let worktree_released = self.current_worktree_released();
        let shared_worktree = execution.is_some_and(|execution| execution.owner_session_id.is_some());
        let reusable_worktree = self.shared_worktree_draft_source().map(|source| source.id.clone());
        let can_reuse_worktree = self.can_create_shared_worktree_draft();
        let scratch = self.session.as_ref().is_some_and(|session| session.scratch);
        let execution_path = execution
            .map(|execution| execution.path.clone())
            .or_else(|| self.session.as_ref().and_then(|session| session.execution_path.clone()))
            .or_else(|| {
                active_project
                    .as_ref()
                    .filter(|_| !scratch)
                    .map(|workspace| workspace.path.clone())
            });
        let branch = execution
            .map(|execution| execution.branch.clone())
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
            .session
            .as_ref()
            .and_then(meta_member_badge_target)
            .map(|agent_id| match agent_id {
                Some(id) => self.agent_name(id),
                None => tr("成员身份未知").to_owned(),
            });
        let context_percent = self
            .usage
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.context.as_ref())
            .and_then(|context| context.percent);

        let chat_width = cx.global::<crate::appearance::Appearance>().chat_width_px();
        h_flex()
            .w_full()
            .min_w_0()
            .max_w(px(chat_width))
            .mx_auto()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_x_2()
            .gap_y_1()
            .px_2()
            .pt_1p5()
            .child(
                h_flex()
                    .min_w_0()
                    .max_w_full()
                    .flex_wrap()
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
                                .max_w_full()
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
                            h_flex()
                                .id("composer-project-folder")
                                .h(px(22.))
                                .px_1p5()
                                .min_w_0()
                                .max_w_full()
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
                                        .child(tr("临时会话")),
                                )
                                .into_any_element()
                        }
                    })
                    .when_some(member_badge, |row, member| {
                        row.child(
                            h_flex()
                                .min_w_0()
                                .max_w_full()
                                .items_center()
                                .gap_1()
                                .px_1p5()
                                .py(px(1.))
                                .rounded_md()
                                .bg(Color::Inset.get(cx))
                                .child(
                                    Icon::new(Glyph::Users)
                                        .size(px(11.))
                                        .flex_shrink_0()
                                        .text_color(Color::Secondary.get(cx)),
                                )
                                .child(
                                    div()
                                        .min_w_0()
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
                    .min_w_0()
                    .max_w_full()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .when_some(
                        active_project
                            .as_ref()
                            .filter(|_| branch.is_some() || execution.is_some())
                            .map(|_| branch.unwrap_or_else(|| tr("Git 状态").to_owned())),
                        |row, branch_label| {
                            row.child(
                                Button::new("composer-git-branch")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_1p5()
                                    .rounded_md()
                                    .min_w_0()
                                    .max_w_full()
                                    .selected(self.git_drawer_open)
                                    .icon(Glyph::GitBranch)
                                    .child(
                                        div()
                                            .min_w_0()
                                            .max_w(px(180.))
                                            .truncate()
                                            .text_size(px(12.))
                                            .text_color(Color::Muted.get(cx))
                                            .child(branch_label),
                                    )
                                    .tooltip(execution_tooltip)
                                    .accessibility_label(tr("查看项目 Git 状态与差异"))
                                    .disabled(!self.connected || self.closing)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_git_workspace(window, cx)
                                    })),
                            )
                        },
                    )
                    .when_some(self.render_composer_mode_pills(running, cx), |row, pills| {
                        row.child(pills)
                    })
                    .when(
                        self.session.is_some()
                            && should_show_meta_usage(self.usage.visible, context_percent),
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
                    ),
            )
            .into_any_element()
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
        let is_en = english();
        let active_name = active_project
            .as_ref()
            .map(|ws| normalize_hero_line_text(&ws.name));
        let aria_title = if let Some(name) = &active_name {
            if is_en {
                format!("What should we build in {name}?")
            } else {
                format!("在 {name} 中构建什么？")
            }
        } else {
            normalize_hero_line_text(tr("准备构建什么？"))
        };

        let chat_width = cx.global::<crate::appearance::Appearance>().chat_width_px();
        let sidebar_w = if self.sidebar_open && !narrow {
            f32::from(self.sidebar.resolved(window.viewport_size().width))
        } else {
            0.
        };
        let right_w = if self.git_drawer_open && self.workspace.is_some() {
            self.git_drawer_width
        } else if self.split_peek.is_some() && window.viewport_size().width >= px(960.) {
            400.
        } else if self.project_files_workspace_focus(cx).is_some() {
            self.project_file_dock_width.map(f32::from).unwrap_or(280.)
        } else {
            0.
        };
        let horizontal_pad = if narrow { 32. } else { 64. };
        let available_hero_w =
            (f32::from(window.viewport_size().width) - sidebar_w - right_w - horizontal_pad)
                .min(chat_width)
                .max(200.);
        let title_style = window.text_style();
        let measure_text = |text: &str, weight: FontWeight| -> f32 {
            if text.is_empty() {
                return 0.;
            }
            let mut run = title_style.to_run(text.len());
            run.font.weight = weight;
            f32::from(
                window
                    .text_system()
                    .shape_line(text.to_owned().into(), px(hero_px), &[run], None)
                    .width(),
            )
        };
        let (inline_hero, switcher_max_w) = if let Some(name) = &active_name {
            let (prefix, suffix) = if is_en {
                ("What should we build in", "?")
            } else {
                ("在", "中构建什么？")
            };
            let prefix_suffix_w = measure_text(prefix, FontWeight::NORMAL)
                + measure_text(suffix, FontWeight::NORMAL);
            let shaped_name_w = measure_text(name, FontWeight::SEMIBOLD);
            hero_switcher_layout(available_hero_w, prefix_suffix_w, shaped_name_w, is_en)
        } else {
            let shaped_w = measure_text(&aria_title, FontWeight::MEDIUM);
            (shaped_w + 32. <= available_hero_w, available_hero_w.max(120.))
        };
        let is_scratch = self.session.as_ref().is_some_and(|s| s.scratch);

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
                                    .w_full()
                                    .min_w_0()
                                    .text_size(px(hero_px))
                                    .line_height(px(34.))
                                    .child(if let Some(ws) = active_project.as_ref() {
                                        let workspaces: Vec<(String, String)> = self
                                            .workspaces
                                            .iter()
                                            .map(|item| (item.id.clone(), item.name.clone()))
                                            .collect();
                                        let active_id = ws.id.clone();
                                        let display_name = active_name
                                            .clone()
                                            .unwrap_or_else(|| normalize_hero_line_text(&ws.name));
                                        let weak = cx.entity().downgrade();
                                        let project_switcher = Button::new("hero-project-switcher")
                                            .custom(crate::palette::quiet_icon_variant(cx))
                                            .compact()
                                            .h_auto()
                                            .min_w_0()
                                            .max_w(px(switcher_max_w))
                                            .flex_shrink_1()
                                            .when(!inline_hero, |btn| btn.px_1p5())
                                            .when(inline_hero && is_en, |btn| {
                                                btn.pl_0p5().pr(px(1.))
                                            })
                                            .when(inline_hero && !is_en, |btn| btn.px_0p5())
                                            .py_0()
                                            .rounded_md()
                                            .disabled(!self.connected || self.closing)
                                            .accessibility_label(display_name.clone())
                                            .tooltip(format!(
                                                "{}\n{}",
                                                ws.path,
                                                tr("点击切换工作区")
                                            ))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .max_w_full()
                                                    .truncate()
                                                    .text_size(px(hero_px))
                                                    .line_height(px(34.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(display_name),
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
                                        if !inline_hero {
                                            v_flex()
                                                .w_full()
                                                .min_w_0()
                                                .items_center()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(tr("准备构建什么？")),
                                                )
                                                .child(project_switcher)
                                                .into_any_element()
                                        } else if is_en {
                                            h_flex()
                                                .w_full()
                                                .min_w_0()
                                                .justify_center()
                                                .items_center()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .font_weight(FontWeight::NORMAL)
                                                        .text_color(Color::Text.get(cx))
                                                        .child("What should we build in"),
                                                )
                                                .child(
                                                    h_flex()
                                                        .min_w_0()
                                                        .max_w_full()
                                                        .items_center()
                                                        .gap_0()
                                                        .child(project_switcher)
                                                        .child(
                                                            div()
                                                                .flex_shrink_0()
                                                                .font_weight(FontWeight::NORMAL)
                                                                .text_color(Color::Text.get(cx))
                                                                .child("?"),
                                                        ),
                                                )
                                                .into_any_element()
                                        } else {
                                            h_flex()
                                                .w_full()
                                                .min_w_0()
                                                .justify_center()
                                                .items_center()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .font_weight(FontWeight::NORMAL)
                                                        .text_color(Color::Text.get(cx))
                                                        .child("在"),
                                                )
                                                .child(project_switcher)
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
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
                            .child(if active_project.is_some() && !is_scratch {
                                Button::new("welcome-start-scratch")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .disabled(!self.can_start_new_conversation())
                                    .accessibility_label(tr("或无项目开始"))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(Color::Muted.get(cx).opacity(0.8))
                                            .child(tr("或无项目开始")),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_scratch_conversation(window, cx);
                                    }))
                                    .into_any_element()
                            } else if active_project.is_some() {
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
                                h_flex()
                                    .flex_wrap()
                                    .justify_center()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("welcome-open-project")
                                            .custom(crate::palette::quiet_icon_variant(cx))
                                            .compact()
                                            .xsmall()
                                            .h(px(24.))
                                            .px_2()
                                            .rounded_md()
                                            .disabled(!self.connected)
                                            .accessibility_label(tr("选择项目文件夹"))
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(Color::Muted.get(cx).opacity(0.85))
                                                    .child(tr("选择项目文件夹")),
                                            )
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.choose_workspace_folder(window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("welcome-start-scratch")
                                            .custom(crate::palette::quiet_icon_variant(cx))
                                            .compact()
                                            .xsmall()
                                            .h(px(24.))
                                            .px_2()
                                            .rounded_md()
                                            .disabled(!self.can_start_new_conversation())
                                            .accessibility_label(tr("或无项目开始"))
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(Color::Muted.get(cx).opacity(0.8))
                                                    .child(tr("或无项目开始")),
                                            )
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.start_scratch_conversation(window, cx);
                                            })),
                                    )
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
                    }),
            )
    }
}

fn normalize_hero_line_text(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

fn hero_switcher_layout(
    available_hero_w: f32,
    prefix_suffix_w: f32,
    shaped_name_w: f32,
    is_english: bool,
) -> (bool, f32) {
    let inline_chrome = if is_english { 16. } else { 20. };
    let inline_safety = 32.;
    let inline_hero =
        prefix_suffix_w + shaped_name_w + inline_chrome + inline_safety <= available_hero_w;
    let switcher_max_w = if inline_hero {
        let prefix_suffix_reserve = prefix_suffix_w + inline_chrome;
        (available_hero_w - prefix_suffix_reserve)
            .max(120.)
            .min(available_hero_w)
    } else {
        available_hero_w.max(120.)
    };
    (inline_hero, switcher_max_w)
}

#[cfg(test)]
mod tests {
    use super::{
        hero_switcher_layout, meta_member_badge_target, normalize_hero_line_text,
        should_show_meta_usage,
    };
    use crate::protocol::Session;

    fn sample_session(
        workspace_id: &str,
        agent_id: Option<&str>,
        origin_agent_id: Option<&str>,
        identity_state: Option<&str>,
    ) -> Session {
        let mut value = serde_json::json!({
            "id": "s-1",
            "title": "Test",
            "workspaceId": workspace_id,
            "agentId": agent_id,
            "originAgentId": origin_agent_id,
            "identityState": identity_state,
        });
        serde_json::from_value(value.take()).unwrap()
    }

    #[test]
    fn normalize_hero_line_text_strips_cr_and_lf() {
        assert_eq!(
            normalize_hero_line_text("alpha\nbeta\r\ngamma\rdelta"),
            "alpha beta gamma delta"
        );
        assert!(normalize_hero_line_text("a\nb\rc").find('\n').is_none());
        assert!(normalize_hero_line_text("a\nb\rc").find('\r').is_none());
    }

    #[test]
    fn hero_switcher_layout_caps_width_in_both_inline_and_two_line_modes() {
        // Short name stays inline and caps switcher width below available_hero_w.
        let (inline, max_w) = hero_switcher_layout(768., 260., 120., true);
        assert!(inline);
        assert!((max_w - (768. - 276.)).abs() < f32::EPSILON);

        // Borderline name falls back to two-line mode with exact available_hero_w cap.
        let (inline, max_w) = hero_switcher_layout(468., 242., 200., true);
        assert!(!inline);
        assert!((max_w - 468.).abs() < f32::EPSILON);

        // Very long 120-char name stays bounded to available_hero_w (never overflows).
        let (inline, max_w) = hero_switcher_layout(288., 242., 1600., true);
        assert!(!inline);
        assert!((max_w - 288.).abs() < f32::EPSILON);

        // Narrow floor never dips below 120px when available_hero_w >= 200.
        let (inline, max_w) = hero_switcher_layout(200., 180., 500., false);
        assert!(!inline);
        assert!((max_w - 200.).abs() < f32::EPSILON);
    }

    #[test]
    fn meta_bar_hides_member_badge_for_orchestrator_but_keeps_worker_identity() {
        let lead = sample_session(
            "ws-1",
            Some("ws-1:orchestrator"),
            Some("ws-1:orchestrator"),
            Some("known"),
        );
        assert_eq!(meta_member_badge_target(&lead), None);

        let worker = sample_session(
            "ws-1",
            Some("ws-1:coder-1"),
            Some("ws-1:coder-1"),
            Some("known"),
        );
        assert_eq!(meta_member_badge_target(&worker), Some(Some("ws-1:coder-1")));

        let unknown_origin = sample_session("ws-1", None, None, Some("unknown"));
        assert_eq!(meta_member_badge_target(&unknown_origin), Some(None));
    }

    #[test]
    fn meta_bar_usage_visibility_threshold() {
        assert!(!should_show_meta_usage(false, None));
        assert!(!should_show_meta_usage(false, Some(0.0)));
        assert!(!should_show_meta_usage(false, Some(74.9)));
        assert!(should_show_meta_usage(false, Some(75.0)));
        assert!(should_show_meta_usage(false, Some(95.0)));
        assert!(should_show_meta_usage(true, None));
        assert!(should_show_meta_usage(true, Some(10.0)));
    }
}
