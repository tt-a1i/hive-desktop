use super::*;

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

    pub fn render_settings_tabs(&self, compact: bool, cx: &mut Context<Self>) -> impl IntoElement {
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
                "settings-team-tab",
                tr("团队"),
                Glyph::Network,
                Some(SettingsSection::Team),
            ),
            (
                "settings-remote-tab",
                tr("远程接入"),
                Glyph::Network,
                Some(SettingsSection::Remote),
            ),
            (
                "settings-notifications-tab",
                tr("通知"),
                Glyph::Bell,
                Some(SettingsSection::Notifications),
            ),
            (
                "settings-programs-tab",
                tr("外部程序"),
                Glyph::SquareTerminal,
                Some(SettingsSection::Programs),
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
        let menu_entries = entries.clone();
        let current = if self.roles.visible {
            None
        } else {
            Some(self.settings_section)
        };
        div()
            .flex()
            .flex_shrink_0()
            .p_3()
            .gap_1()
            .bg(Color::Surface.get(cx))
            .when(compact, |bar| bar.flex_row().items_center().border_b_1())
            .when(!compact, |bar| {
                bar.flex_col().w(px(260.)).pl_6().pr_4().pt_7()
            })
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .flex_shrink_0()
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
                                .child(tr("设置")),
                        )
                    }),
            )
            .when(compact, |bar| {
                bar.child(
                    Button::new("settings-section-menu")
                        .ghost()
                        .h(px(36.))
                        .rounded_full()
                        .flex_1()
                        .min_w_0()
                        .icon(*current_glyph)
                        .accessibility_label(format!("{}: {current_label}", tr("设置")))
                        .child(div().flex_1().min_w_0().truncate().child(*current_label))
                        .child(Icon::new(Glyph::ChevronDown).size_4())
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
                bar.children(entries.into_iter().map(|(id, label, glyph, section)| {
                    Button::new(id)
                        .ghost()
                        .h(px(36.))
                        .rounded_full()
                        .icon(glyph)
                        .w_full()
                        .justify_start()
                        .accessibility_label(label)
                        .child(div().flex_1().child(label))
                        .selected(selected(section))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_preferences_target(section, window, cx)
                        }))
                }))
            })
    }
}
