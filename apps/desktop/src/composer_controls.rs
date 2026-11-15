use crate::{app::HiveApp, language::tr, palette, protocol::Backend};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenuItem},
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub(crate) fn backend_glyph(backend: Backend) -> Glyph {
    match backend {
        Backend::Pi => Glyph::Sparkles,
        Backend::Codex => Glyph::Cpu,
        Backend::Claude => Glyph::Bot,
        Backend::PiRpc => Glyph::SquareTerminal,
        Backend::Acp => Glyph::SquareTerminal,
    }
}

impl HiveApp {
    pub(crate) fn render_composer_controls(
        &self,
        window: &mut Window,
        _welcome: bool,
        running: bool,
        can_send: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let can_send = can_send && !self.conversation_is_read_only();
        let footer_width = window.use_keyed_state("composer-footer-width", cx, |_, _| px(0.));
        let width = *footer_width.read(cx);
        let compact = if width > px(0.) {
            width < px(480.)
        } else {
            window.viewport_size().width < px(900.)
        };
        let native_commands = self.pi_commands_available();
        let can_resume = !self.conversation_is_read_only() && !running
            && self
                .session
                .as_ref()
                .and_then(|session| self.pending.get(&session.id))
                .copied()
                .unwrap_or(0)
                > 0
            && (self.delivery_failure.is_none() || self.agent_diagnostic.is_some());

        h_flex()
            .id("composer-footer-controls")
            .on_prepaint(move |bounds, window, cx| {
                if *footer_width.read(cx) != bounds.size.width {
                    footer_width.update(cx, |width, _| *width = bounds.size.width);
                    window.refresh();
                }
            })
            .w_full()
            .min_w_0()
            .gap_2()
            .items_center()
            .pt_1()
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .child(
                        Button::new("attach-images")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(28.))
                            .w(px(28.))
                            .rounded_lg()
                            .icon(Glyph::Paperclip)
                            .tooltip(tr("添加文件或图片"))
                            .accessibility_label(tr("添加文件或图片"))
                            .disabled(
                                !self.connected
                                    || self.workspace.is_none()
                                    || self.loading
                                    || self.session.as_ref().is_some_and(|session| {
                                        self.inflight_inputs.contains_key(&session.id)
                                    })
                                    || !self.attachments.ready(self.draft_target.as_ref()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.choose_images(cx))),
                    )
                    .child(
                        Button::new("reference-project-file")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(28.))
                            .w(px(28.))
                            .rounded_lg()
                            .icon(Glyph::AtSign)
                            .tooltip(crate::keybindings::hint(
                                crate::keybindings::Target::SearchProjectFiles,
                                "引用项目文件",
                                cx,
                            ))
                            .accessibility_label(tr("引用项目文件"))
                            .disabled(!self.can_reference_project_file())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_project_file_picker(window, cx)
                            })),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .justify_end()
                    .items_center()
                    .gap_0p5()
                    .when(self.session.is_none(), |row| {
                        row.child(self.new_chat_menu("home-agent", tr("选择 Agent"), None, cx))
                    })
                    .when(self.session.is_some(), |row| {
                        row.when(!self.session.as_ref().is_some_and(|s| matches!(s.backend, Backend::Acp)) && !self.current_history_access().is_some_and(|a| a.handoff_unconsumed() && (a.handoff_passive() || !self.session.as_ref().is_some_and(|s| matches!(s.backend, Backend::Pi)))), |row| row.child(
                            div()
                                .min_w_0()
                                .max_w(px(260.))
                                .child(self.model_picker(window, running || self.conversation_is_read_only(), cx)),
                        ))
                        .when(native_commands && !compact, |row| {
                            row.child(self.command_button(cx))
                        })
                        .when(native_commands && compact, |row| {
                            row.child(
                                Button::new("composer-controls-overflow")
                                    .custom(palette::quiet_icon_variant(cx))
                                    .compact()
                                    .h(px(28.))
                                    .w(px(28.))
                                    .rounded_lg()
                                    .icon(Glyph::Ellipsis)
                                    .tooltip(tr("更多输入选项"))
                                    .accessibility_label(tr("更多输入选项"))
                                    .disabled(
                                        !self.connected || !self.conversation_ready || self.loading,
                                    )
                                    .dropdown_menu({
                                        let weak = cx.weak_entity();
                                        move |menu, _, _| {
                                            let weak = weak.clone();
                                            menu.item(
                                                PopupMenuItem::new(tr("选择原生命令")).on_click(
                                                    move |_, window, cx| {
                                                        let _ = weak.update(cx, |this, cx| {
                                                            this.toggle_native_commands(window, cx);
                                                        });
                                                    },
                                                ),
                                            )
                                        }
                                    }),
                            )
                        })
                    }),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1p5()
                    .when(running, |actions| {
                        actions.child(
                            Button::new("stop")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(28.))
                                .w(px(28.))
                                .rounded_full()
                                .icon(Glyph::Square)
                                .tooltip(tr("停止回复"))
                                .accessibility_label(tr("停止"))
                                .disabled(!self.connected)
                                .on_click(cx.listener(|this, _, _, cx| this.abort(cx))),
                        )
                    })
                    .when(can_resume, |actions| {
                        actions.child(
                            Button::new("resume")
                                .primary()
                                .compact()
                                .h(px(28.))
                                .px_3()
                                .rounded_full()
                                .label(tr("继续处理"))
                                .disabled(
                                    !self.connected || !self.conversation_ready || self.loading,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.resume(cx))),
                        )
                    })
                    .when(!can_resume, |actions| {
                        let busy_mode = crate::general::current(cx).busy_send_mode;
                        let blocked_while_running = running
                            && busy_mode == crate::general::BusySendMode::BlockUntilIdle;
                        let busy_badge = if !running {
                            None
                        } else {
                            match busy_mode {
                                crate::general::BusySendMode::Queue => Some(tr("排队")),
                                crate::general::BusySendMode::SteerWhenSupported => Some(tr("插话")),
                                crate::general::BusySendMode::BlockUntilIdle => None,
                            }
                        };
                        actions.child(
                            Button::new("send")
                                .custom(palette::send_circle_variant(can_send, cx))
                                .compact()
                                .h(px(28.))
                                .when(busy_badge.is_none(), |button| button.w(px(28.)))
                                .when(busy_badge.is_some(), |button| button.px_3())
                                .rounded_full()
                                .when(!can_send, |button| {
                                    button
                                        .bg(palette::send_circle_disabled_bg(cx))
                                        .text_color(palette::send_circle_disabled_fg(cx))
                                })
                                .icon(Glyph::ArrowUp)
                                .when_some(busy_badge, |button, label| button.label(label))
                                .accessibility_label(if blocked_while_running {
                                    tr("Agent 正在运行中；当前设置为等待空闲后再发送")
                                } else if running {
                                    match busy_mode {
                                        crate::general::BusySendMode::SteerWhenSupported => {
                                            tr("发送插话引导")
                                        }
                                        crate::general::BusySendMode::Queue => tr("加入消息队列"),
                                        crate::general::BusySendMode::BlockUntilIdle => tr("发送消息"),
                                    }
                                } else {
                                    tr("发送消息")
                                })
                                .tooltip(if self.workspace.is_none() {
                                    tr("选择项目文件夹并发送").into()
                                } else if blocked_while_running {
                                    tr("Agent 正在运行中；当前设置为等待空闲后再发送").into()
                                } else if running {
                                    match busy_mode {
                                        crate::general::BusySendMode::SteerWhenSupported => {
                                            tr("优先向当前回合发送插话引导；不支持时自动按顺序排队").into()
                                        }
                                        crate::general::BusySendMode::Queue => {
                                            tr("加入队列，当前回复结束后按顺序处理").into()
                                        }
                                        crate::general::BusySendMode::BlockUntilIdle => {
                                            tr("Agent 正在运行中；当前设置为等待空闲后再发送").into()
                                        }
                                    }
                                } else {
                                    cx.global::<crate::composer::ComposerPreferences>()
                                        .settings
                                        .mode
                                        .hint()
                                })
                                .disabled(!can_send)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if this.workspace.is_none() {
                                        this.choose_workspace_folder(window, cx);
                                    } else {
                                        this.send(window, cx);
                                    }
                                })),
                        )
                    }),
            )
    }
}
