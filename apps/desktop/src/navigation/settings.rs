use super::*;
use crate::app::SettingsSection;
use crate::focus_scroll::{Reveal, Viewport};
use gpui_kit::component::scroll::ScrollableElement;

impl HiveApp {
    fn select_preferences_target(
        &mut self,
        section: Option<SettingsSection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences_search_open = false;
        if let Some(section) = section {
            self.select_settings_section(section, window, cx);
        } else {
            self.settings_search
                .update(cx, |input, cx| input.set_value("", window, cx));
            if !self.roles.visible {
                self.open_roles(cx);
            }
            self.preferences_focus.focus(window, cx);
            cx.notify();
        }
    }

    pub fn render_settings_tabs(
        &self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let expanded = !compact || self.preferences_search_open;
        let searching = !self.settings_search.read(cx).value().trim().is_empty();
        let groups = [
            (
                tr("偏好"),
                vec![
                    (
                        "settings-general-tab",
                        tr("通用"),
                        Glyph::SlidersHorizontal,
                        Some(SettingsSection::General),
                    ),
                    (
                        "settings-appearance-tab",
                        tr("外观"),
                        Glyph::Monitor,
                        Some(SettingsSection::Appearance),
                    ),
                    (
                        "settings-chat-tab",
                        tr("聊天"),
                        Glyph::MessageSquare,
                        Some(SettingsSection::Chat),
                    ),
                    (
                        "settings-keybindings-tab",
                        tr("快捷键"),
                        Glyph::Keyboard,
                        Some(SettingsSection::Keybindings),
                    ),
                    (
                        "settings-notifications-tab",
                        tr("通知"),
                        Glyph::Bell,
                        Some(SettingsSection::Notifications),
                    ),
                ],
            ),
            (
                "Agent",
                vec![
                    (
                        "settings-pi-accounts-tab",
                        tr("内置 Pi"),
                        Glyph::Bot,
                        Some(SettingsSection::PiAccounts),
                    ),
                    (
                        "settings-models-tab",
                        tr("模型与接口"),
                        Glyph::Cpu,
                        Some(SettingsSection::Models),
                    ),
                    (
                        "settings-profiles-tab",
                        tr("Agent 配置"),
                        Glyph::Bot,
                        Some(SettingsSection::Profiles),
                    ),
                    (
                        "settings-programs-tab",
                        tr("外部程序"),
                        Glyph::SquareTerminal,
                        Some(SettingsSection::Programs),
                    ),
                ],
            ),
            (
                tr("协作"),
                vec![
                    (
                        "settings-diffs-tab",
                        tr("Diff 与 PR"),
                        Glyph::FileDiff,
                        Some(SettingsSection::Diffs),
                    ),
                    (
                        "settings-project-scripts-tab",
                        tr("项目脚本"),
                        Glyph::SquareTerminal,
                        Some(SettingsSection::ProjectScripts),
                    ),
                    (
                        "settings-team-tab",
                        tr("团队"),
                        Glyph::Network,
                        Some(SettingsSection::Team),
                    ),
                    ("settings-roles-tab", tr("角色模板"), Glyph::Users, None),
                ],
            ),
            (
                tr("应用"),
                vec![
                    (
                        "settings-archive-tab",
                        tr("归档会话"),
                        Glyph::Archive,
                        Some(SettingsSection::Archive),
                    ),
                    (
                        "settings-about-tab",
                        tr("关于与维护"),
                        Glyph::Info,
                        Some(SettingsSection::About),
                    ),
                ],
            ),
        ];
        v_flex()
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .when(!compact, |nav| nav.w(px(240.)))
            .when(compact && !expanded, |nav| nav.w(px(56.)))
            .when(compact && expanded, |nav| nav.flex_1().w_full())
            .bg(Color::Sidebar.get(cx))
            .border_r_1()
            .border_color(Color::Divider.get(cx))
            .when(expanded, |nav| {
                nav.child(self.render_settings_search(true, window, cx))
            })
            .when(!expanded, |nav| {
                nav.child(
                    h_flex().h(px(48.)).justify_center().items_center().child(
                        Button::new("settings-open-search")
                            .ghost()
                            .compact()
                            .h(px(30.))
                            .w(px(30.))
                            .icon(Glyph::Search)
                            .tooltip(tr("搜索设置"))
                            .accessibility_label(tr("搜索设置"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.preferences_search_open = true;
                                let focus = this.settings_search.read(cx).focus_handle(cx);
                                window.on_next_frame(move |window, cx| focus.focus(window, cx));
                                cx.notify();
                            })),
                    ),
                )
            })
            .when(compact && expanded, |nav| {
                nav.child(
                    Button::new("settings-collapse-search")
                        .ghost()
                        .small()
                        .label(tr("返回设置"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.preferences_search_open = false;
                            this.settings_search
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            this.preferences_focus.focus(window, cx);
                            cx.notify();
                        })),
                )
            })
            .child(if searching && expanded {
                self.render_settings_results(cx).into_any_element()
            } else {
                Viewport::new(
                    "settings-navigation-viewport",
                    &self.preferences_nav_scroll,
                    v_flex()
                        .id("settings-navigation-list")
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.preferences_nav_scroll)
                        .vertical_scrollbar(&self.preferences_nav_scroll)
                        .px_2()
                        .pb_3()
                        .gap_3()
                        .children(groups.into_iter().map(|(heading, entries)| {
                            v_flex()
                                .flex_shrink_0()
                                .gap_0p5()
                                .when(expanded, |group| {
                                    group.child(
                                        div()
                                            .px_2()
                                            .pt_3()
                                            .pb_1()
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Color::Muted.get(cx))
                                            .child(heading),
                                    )
                                })
                                .children(entries.into_iter().map(|(id, label, glyph, section)| {
                                    let active = if self.roles.visible {
                                        section.is_none()
                                    } else {
                                        section == Some(self.settings_section)
                                    };
                                    let is_project_scoped =
                                        section == Some(SettingsSection::ProjectScripts);
                                    let nav_tip = match section {
                                        Some(SettingsSection::ProjectScripts) => {
                                            format!("{label} · {}", tr("按当前项目保存"))
                                        }
                                        Some(SettingsSection::Programs) => {
                                            format!("{label} · {}", tr("环境变量优先于应用设置"))
                                        }
                                        Some(SettingsSection::Models) => {
                                            format!("{label} · {}", tr("支持模型级覆写与继承"))
                                        }
                                        Some(SettingsSection::Profiles) => {
                                            format!("{label} · {}", tr("全局模板与会话版本锁定"))
                                        }
                                        Some(SettingsSection::Keybindings) => {
                                            format!("{label} · {}", tr("分上下文作用域"))
                                        }
                                        _ => label.to_owned(),
                                    };
                                    Reveal::new(
                                        SharedString::from(format!("{id}-reveal")),
                                        &self.preferences_nav_scroll,
                                        Button::new(id)
                                            .custom(crate::palette::sidebar_item_variant(
                                                active, cx,
                                            ))
                                            .compact()
                                            .h(px(32.))
                                            .px_2()
                                            .rounded_md()
                                            .w_full()
                                            .accessibility_label(label)
                                            .tooltip(nav_tip)
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .min_w_0()
                                                    .items_center()
                                                    .gap_2()
                                                    .when(!expanded, |row| row.justify_center())
                                                    .child(
                                                        Icon::new(glyph)
                                                            .size(px(16.))
                                                            .flex_shrink_0()
                                                            .text_color(Color::Secondary.get(cx)),
                                                    )
                                                    .when(expanded, |row| {
                                                        row.child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .text_left()
                                                                .text_size(px(13.))
                                                                .truncate()
                                                                .text_color(Color::Text.get(cx))
                                                                .child(label),
                                                        )
                                                        .when(is_project_scoped, |row| {
                                                            row.child(
                                                                div()
                                                                    .flex_shrink_0()
                                                                    .px_1p5()
                                                                    .py(px(0.5))
                                                                    .rounded_sm()
                                                                    .bg(Color::Inset.get(cx))
                                                                    .text_size(px(10.))
                                                                    .text_color(Color::Muted.get(cx))
                                                                    .child(tr("项目")),
                                                            )
                                                        })
                                                    }),
                                            )
                                            .selected(active)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.select_preferences_target(section, window, cx)
                                            })),
                                    )
                                }))
                        })),
                )
                .into_any_element()
            })
    }
}
