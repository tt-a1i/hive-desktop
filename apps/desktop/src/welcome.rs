use crate::{
    app::{HiveApp, SettingsSection},
    chrome,
    focus_scroll::Reveal,
    language::tr,
    palette::Color,
};
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenuItem},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use std::sync::{Arc, OnceLock};
mod agents;

impl HiveApp {
    pub(crate) fn render_project_context(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let project = self
            .workspaces
            .iter()
            .find(|project| Some(&project.id) == self.workspace.as_ref());
        h_flex().min_w_0().when_some(project, |row, project| {
            let path = std::path::PathBuf::from(&project.path);
            let identity = format!("{} · {}", project.name, project.path);
            let display_path = crate::palette::display_path(&project.path);
            row.child(
                h_flex()
                    .min_w_0()
                    .max_w_full()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Button::new("home-project-folder")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .rounded_md()
                            .h(px(24.))
                            .w(px(24.))
                            .icon(Glyph::FolderOpen)
                            .accessibility_label(format!(
                                "{}: {identity}",
                                tr("在文件管理器中打开项目")
                            ))
                            .tooltip(project.path.clone())
                            .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&path))),
                    )
                    .child(
                        div()
                            .id("home-project-name")
                            .role(Role::Label)
                            .aria_label(project.name.clone())
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Secondary.get(cx))
                            .truncate()
                            .child(project.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx).opacity(0.6))
                            .child("·"),
                    )
                    .child(
                        div()
                            .id("home-project-path")
                            .role(Role::Label)
                            .aria_label(project.path.clone())
                            .max_w(px(220.))
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .truncate()
                            .child(display_path),
                    ),
            )
        })
    }

    /// Beluga-inspired centered onboarding & agent readiness view when no project is open.
    /// Opening this page never creates a session or starts an agent.
    pub(crate) fn render_welcome(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let weak = cx.weak_entity();
        static LOGO: OnceLock<Arc<Image>> = OnceLock::new();
        let logo = LOGO
            .get_or_init(|| {
                Arc::new(Image::from_bytes(
                    ImageFormat::Png,
                    include_bytes!("../assets/hive-logo.png").to_vec(),
                ))
            })
            .clone();
        let compact = window.viewport_size().width < px(760.);
        let font_scale = cx.global::<crate::appearance::Appearance>().font_size() as f32 / 16.0;
        let tall = window.viewport_size().height > px(740. * font_scale);
        let collapsed_sidebar = !self.sidebar_open;
        let left_inset = if collapsed_sidebar {
            chrome::traffic_light_inset(window)
        } else {
            px(0.)
        };
        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .child(
                chrome::drag_bar("welcome-topbar", window, cx)
                    .w_full()
                    .px_3()
                    .when(left_inset > px(0.), |bar| bar.pl(left_inset + px(8.)))
                    .gap_2()
                    .when(collapsed_sidebar, |bar| {
                        bar.child(
                            Button::new("toggle-navigation")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(28.))
                                .w(px(28.))
                                .rounded_lg()
                                .icon(Glyph::PanelLeft)
                                .tooltip(tr("打开或收起导航"))
                                .accessibility_label(tr("打开或收起工作区与会话导航"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sidebar_open = !this.sidebar_open;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(div().flex_1()),
            )
            .child(crate::focus_scroll::Viewport::new(
                "welcome-viewport",
                &self.welcome_scroll,
                div()
                    .id("welcome-page")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .h_full()
                    .when(tall, |page| page.justify_center())
                    .overflow_y_scroll()
                    .track_scroll(&self.welcome_scroll)
                    .vertical_scrollbar(&self.welcome_scroll)
                    .child(
                        v_flex()
                            .flex_shrink_0()
                            .w_full()
                            .max_w(px(620.))
                            .mx_auto()
                            .gap_6()
                            .px_6()
                            .pt_2()
                            .pb_8()
                            .child(
                                v_flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .text_center()
                                    .gap_2p5()
                                    .child(
                                        v_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .size(px(48.))
                                                    .flex_shrink_0()
                                                    .rounded_2xl()
                                                    .bg(Color::Card.get(cx))
                                                    .border_1()
                                                    .border_color(Color::Divider.get(cx))
                                                    .shadow_xs()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(img(logo).size(px(32.))),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(Color::Muted.get(cx))
                                                    .child("H I V E"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("welcome-title")
                                            .role(Role::Label)
                                            .aria_label(tr("你的项目，你的 Agent。"))
                                            .text_size(px(if compact { 26. } else { 32. }))
                                            .line_height(px(if compact { 34. } else { 40. }))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(tr("你的项目，你的 Agent。")),
                                    )
                                    .child(
                                        div()
                                            .id("welcome-description")
                                            .role(Role::Label)
                                            .aria_label(tr("在本地工作区直接连接 Claude Code、Codex 或 Pi，专注对话、代码改动与执行过程。"))
                                            .max_w(px(460.))
                                            .text_sm()
                                            .line_height(px(22.))
                                            .text_color(Color::Secondary.get(cx))
                                            .child(tr("在本地工作区直接连接 Claude Code、Codex 或 Pi，专注对话、代码改动与执行过程。")),
                                    ),
                            )
                            .when(self.error.is_some(), |view| view.child(self.render_error_banner(cx)))
                            .child(
                                v_flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_2()
                                    .child(Reveal::new(
                                        "welcome-actions-reveal",
                                        &self.welcome_scroll,
                                        h_flex()
                                            .flex_shrink_0()
                                            .justify_center()
                                            .gap_2p5()
                                            .flex_wrap()
                                            .child(
                                                Button::new("welcome-open-project")
                                                    .primary()
                                                    .rounded_xl()
                                                    .h(px(36.))
                                                    .px_4()
                                                    .icon(Glyph::FolderOpen)
                                                    .label(tr("打开项目文件夹"))
                                                    .disabled(!self.connected)
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.choose_workspace_folder(window, cx)
                                                    })),
                                            )
                                            .child(
                                                Button::new("welcome-configure")
                                                    .outline()
                                                    .rounded_xl()
                                                    .h(px(36.))
                                                    .px_3p5()
                                                    .label(tr("先配置 Agent"))
                                                    .dropdown_caret(true)
                                                    .accessibility_label(tr("选择 Agent 配置入口"))
                                                    .dropdown_menu(move |mut menu, _, _| {
                                                        for (section, label) in [
                                                            (SettingsSection::PiAccounts, tr("内置 Pi 账号")),
                                                            (SettingsSection::Models, tr("模型与接口")),
                                                            (SettingsSection::Programs, tr("外部 Agent 程序")),
                                                        ] {
                                                            let weak = weak.clone();
                                                            menu = menu.item(
                                                                PopupMenuItem::new(label).on_click(
                                                                    move |_, window, cx| {
                                                                        let _ = weak.update(
                                                                            cx,
                                                                            |this, cx| {
                                                                                this.settings_section =
                                                                                    section;
                                                                                this.open_settings(
                                                                                    window, cx,
                                                                                );
                                                                            },
                                                                        );
                                                                    },
                                                                ),
                                                            );
                                                        }
                                                        menu
                                                    }),
                                            ),
                                    ))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(if self.connected {
                                                tr("打开本地文件夹后即可选择 Agent 开始会话。")
                                            } else {
                                                tr("后台暂不可用 · 可在窗口顶部重新启动后台")
                                            }),
                                    ),
                            )
                            .child(self.render_welcome_agents(cx)),
                    ),
            ))
    }
}
