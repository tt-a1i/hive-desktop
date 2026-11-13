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
        let entries = [
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
                "settings-programs-tab",
                tr("外部程序"),
                Glyph::SquareTerminal,
                Some(SettingsSection::Programs),
            ),
            (
                "settings-notifications-tab",
                tr("通知"),
                Glyph::Bell,
                Some(SettingsSection::Notifications),
            ),
            (
                "settings-remote-tab",
                tr("远程接入"),
                Glyph::Network,
                Some(SettingsSection::Remote),
            ),
            (
                "settings-team-tab",
                tr("团队"),
                Glyph::Network,
                Some(SettingsSection::Team),
            ),
            ("settings-roles-tab", tr("角色模板"), Glyph::Bot, None),
        ];
        let selected = |section: Option<SettingsSection>| {
            if self.roles.visible {
                section.is_none()
            } else {
                section == Some(self.settings_section)
            }
        };
        let (_, current_label, current_glyph, _) = entries
            .iter()
            .find(|entry| selected(entry.3))
            .unwrap_or(&entries[0]);
        let back = if self.terminal.visible {
            tr("返回工作区终端")
        } else if self.workflows.visible {
            tr("返回工作流")
        } else if self.memory.visible {
            tr("返回工作区记忆")
        } else if self.collaboration.visible && self.collaboration.selected.is_some() {
            tr("返回派单详情")
        } else if self.collaboration.visible {
            tr("返回协作概览")
        } else if self.member_panel {
            tr("返回团队成员")
        } else {
            tr("返回会话")
        };
        let weak = cx.weak_entity();
        let menu_entries = entries;
        let current = if self.roles.visible {
            None
        } else {
            Some(self.settings_section)
        };
        let inset = chrome::traffic_light_inset(window);
        div()
            .flex()
            .flex_shrink_0()
            .gap_1()
            .bg(Color::Sidebar.get(cx))
            .border_color(Color::Divider.get(cx))
            .when(compact, |bar| {
                bar.flex_row()
                    .items_center()
                    .min_h(px(chrome::TOPBAR_HEIGHT))
                    .px_3()
                    .py_1p5()
                    .when(inset > px(0.), |b| b.pl(inset + px(8.)))
                    .border_b_1()
            })
            .when(!compact, |bar| {
                bar.flex_col()
                    .h_full()
                    .min_h_0()
                    .w(px(252.))
                    .px_2()
                    .pb_3()
                    .border_r_1()
            })
            .when(!compact, |bar| {
                bar.child(
                    chrome::drag_bar("settings-sidebar-topbar", window, cx)
                        .w_full()
                        .px_1()
                        .when(inset > px(0.), |b| b.pl(inset - px(4.)))
                        .gap_1p5()
                        .child(
                            Button::new("leave-preferences")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(26.))
                                .w(px(26.))
                                .rounded_md()
                                .icon(Glyph::ArrowLeft)
                                .accessibility_label(back)
                                .tooltip(back)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.leave_preferences(window, cx)
                                })),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(Color::Text.get(cx))
                                .child(tr("设置")),
                        ),
                )
            })
            .when(compact, |bar| {
                bar.child(
                    Button::new("leave-preferences")
                        .custom(crate::palette::quiet_icon_variant(cx))
                        .compact()
                        .xsmall()
                        .h(px(28.))
                        .w(px(28.))
                        .rounded_lg()
                        .icon(Glyph::ArrowLeft)
                        .accessibility_label(back)
                        .tooltip(back)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.leave_preferences(window, cx)),
                        ),
                )
                .child(
                    Button::new("settings-section-menu")
                        .custom(crate::palette::quiet_icon_variant(cx))
                        .compact()
                        .h(px(30.))
                        .px_2()
                        .rounded_lg()
                        .flex_1()
                        .min_w_0()
                        .accessibility_label(format!("{}: {current_label}", tr("设置")))
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
                                            Icon::new(*current_glyph)
                                                .size(px(14.))
                                                .flex_shrink_0()
                                                .text_color(Color::Secondary.get(cx)),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_left()
                                                .text_size(px(13.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .truncate()
                                                .child(*current_label),
                                        ),
                                )
                                .child(
                                    Icon::new(Glyph::ChevronDown)
                                        .size(px(14.))
                                        .flex_shrink_0()
                                        .text_color(Color::Muted.get(cx)),
                                ),
                        )
                        .dropdown_menu(move |mut menu, _, _| {
                            for (_, label, glyph, section) in menu_entries {
                                let weak = weak.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(label)
                                        .icon(if current == section {
                                            Glyph::Check
                                        } else {
                                            glyph
                                        })
                                        .checked(current == section)
                                        .on_click(move |_, window, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.select_preferences_target(section, window, cx)
                                            });
                                        }),
                                );
                            }
                            menu
                        }),
                )
            })
            .when(!compact, |bar| {
                bar.child(Viewport::new(
                    "settings-navigation-viewport",
                    &self.preferences_nav_scroll,
                    v_flex()
                        .id("settings-navigation-list")
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .gap_0p5()
                        .px_0p5()
                        .pt_1()
                        .overflow_y_scroll()
                        .track_scroll(&self.preferences_nav_scroll)
                        .vertical_scrollbar(&self.preferences_nav_scroll)
                        .children(entries.into_iter().map(|(id, label, glyph, section)| {
                            let active = selected(section);
                            Reveal::new(
                                SharedString::from(format!("{id}-reveal")),
                                &self.preferences_nav_scroll,
                                Button::new(id)
                                    .custom(crate::palette::sidebar_item_variant(active, cx))
                                    .compact()
                                    .h(px(30.))
                                    .px_2()
                                    .flex_shrink_0()
                                    .rounded_lg()
                                    .w_full()
                                    .accessibility_label(label)
                                    .tooltip(label)
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Icon::new(glyph)
                                                    .size(px(14.))
                                                    .flex_shrink_0()
                                                    .text_color(if active {
                                                        Color::Text.get(cx)
                                                    } else {
                                                        Color::Muted.get(cx)
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_left()
                                                    .text_size(px(13.))
                                                    .when(active, |t| t.font_weight(FontWeight::MEDIUM))
                                                    .text_color(if active {
                                                        Color::Text.get(cx)
                                                    } else {
                                                        Color::Secondary.get(cx)
                                                    })
                                                    .truncate()
                                                    .child(label),
                                            ),
                                    )
                                    .selected(active)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select_preferences_target(section, window, cx)
                                    })),
                            )
                        })),
                ))
            })
    }
}
