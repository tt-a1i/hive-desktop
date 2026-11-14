use crate::{
    app::HiveApp,
    conversation_list::CHAT_COLUMN_WIDTH,
    language::{english, tr},
    palette::Color,
};
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
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

mod agents;

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
        let branch = active_project
            .as_ref()
            .and_then(|ws| cached_git_branch(&ws.path));
        let member_badge = self
            .session
            .as_ref()
            .and_then(|s| s.agent_id.as_deref())
            .map(|id| self.agent_name(id));

        h_flex()
            .w_full()
            .min_w_0()
            .max_w(px(CHAT_COLUMN_WIDTH))
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
                        let folder_label = active_project
                            .as_ref()
                            .map(|ws| ws.name.clone())
                            .unwrap_or_else(|| tr("打开项目文件夹").to_owned());
                        let folder_tooltip = active_project
                            .as_ref()
                            .map(|ws| format!("{}\n{}", ws.path, tr("点击切换项目文件夹")))
                            .unwrap_or_else(|| tr("打开项目文件夹").to_owned());
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
                            .disabled(!self.connected)
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
                                            .child(folder_label),
                                    ),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.choose_workspace_folder(window, cx);
                            }))
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
                    .gap_2p5()
                    .when_some(branch, |row, branch| {
                        row.child(
                            h_flex()
                                .id("composer-git-branch")
                                .role(Role::Label)
                                .aria_label(format!("{}: {branch}", tr("当前 Git 分支")))
                                .min_w_0()
                                .items_center()
                                .gap_1()
                                .text_size(px(12.))
                                .text_color(Color::Muted.get(cx))
                                .child(
                                    Icon::new(Glyph::GitBranch)
                                        .size(px(12.))
                                        .text_color(Color::Muted.get(cx))
                                        .flex_shrink_0(),
                                )
                                .child(div().max_w(px(180.)).truncate().child(branch)),
                        )
                    })
                    .when(self.session.is_some(), |row| {
                        row.child(self.usage_button(window, running, cx))
                    })
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
        let hero_px = if compact { 25. } else { 28. };
        let aria_title = if let Some(ws) = &active_project {
            if english() {
                format!("What should we build in {}?", ws.name)
            } else {
                format!("在 {} 中构建什么？", ws.name)
            }
        } else {
            tr("准备构建什么？").into()
        };

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
                    .max_w(px(CHAT_COLUMN_WIDTH))
                    .mx_auto()
                    .child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .text_center()
                            .mb_6()
                            .gap_1p5()
                            .child(
                                div()
                                    .id("session-title")
                                    .role(Role::Label)
                                    .aria_label(aria_title.clone())
                                    .text_size(px(hero_px))
                                    .line_height(px(34.))
                                    .whitespace_normal()
                                    .child(if let Some(ws) = active_project.as_ref() {
                                        if english() {
                                            h_flex()
                                                .flex_wrap()
                                                .justify_center()
                                                .items_baseline()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::NORMAL)
                                                        .text_color(Color::Text.get(cx))
                                                        .child("What should we build in"),
                                                )
                                                .child(
                                                    h_flex()
                                                        .items_baseline()
                                                        .child(
                                                            div()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(Color::Text.get(cx))
                                                                .child(ws.name.clone()),
                                                        )
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
                                                .items_baseline()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::NORMAL)
                                                        .text_color(Color::Text.get(cx))
                                                        .child("在"),
                                                )
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(ws.name.clone()),
                                                )
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
                                            .text_size(px(13.))
                                            .text_color(Color::Muted.get(cx))
                                            .child(tr("切换项目文件夹")),
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
                                            .text_size(px(13.))
                                            .text_color(Color::Muted.get(cx))
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
                    }),
            )
    }
}
