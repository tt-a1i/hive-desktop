use crate::{
    app::HiveApp, composer::SendIntent, general::BusySendMode, language::tr, palette,
    protocol::Backend,
};
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
        Backend::Codex => Glyph::Aperture,
        Backend::Claude => Glyph::Asterisk,
        Backend::PiRpc => Glyph::SquareTerminal,
        Backend::Acp => Glyph::SquareTerminal,
    }
}

pub(crate) fn composer_control_layout(width: f32) -> (bool, bool) {
    (width < 520., width < 380.)
}

fn busy_send_hint(mode: BusySendMode, shifted: bool) -> SharedString {
    let (default, alternate) = match mode {
        BusySendMode::Queue => (
            tr("加入队列，当前回复结束后按顺序处理"),
            tr("仅本条尝试插话，不可用时排队"),
        ),
        BusySendMode::SteerWhenSupported => (
            tr("优先向当前回合发送插话引导；不支持时自动按顺序排队"),
            tr("仅本条排队"),
        ),
        BusySendMode::BlockUntilIdle => {
            return tr("Agent 正在运行中；当前设置为等待空闲后再发送").into();
        }
    };
    let shortcut = match (cfg!(target_os = "macos"), shifted) {
        (true, false) => "⌘Enter",
        (true, true) => "⌘⇧Enter",
        (false, false) => "Ctrl+Enter",
        (false, true) => "Ctrl+Shift+Enter",
    };
    format!("{default}\n{shortcut} · {alternate}").into()
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
        let (compact, tight) = if width > px(0.) {
            composer_control_layout(f32::from(width))
        } else {
            let vw = f32::from(window.viewport_size().width);
            (vw < 900., vw < 640.)
        };
        let native_commands = self.pi_commands_available();
        let can_resume = !self.conversation_is_read_only()
            && !running
            && self
                .session
                .as_ref()
                .and_then(|session| self.pending.get(&session.id))
                .copied()
                .unwrap_or(0)
                > 0
            && (self.delivery_failure.is_none() || self.agent_diagnostic.is_some());
        let show_send_and_draft = self.session.is_some()
            && !running
            && !can_resume
            && can_send
            && self.can_send_and_start_draft(cx);
        let can_reference = self.can_reference_project_file();
        let can_stash = self.can_open_prompt_stash();
        let stash_count = self.stashed_draft_count();
        let can_usage = self.session.is_some()
            && self.connected
            && self.conversation_ready
            && !self.conversation_is_read_only()
            && !self
                .session
                .as_ref()
                .is_some_and(|s| matches!(s.backend, Backend::Acp))
            && !self
                .current_history_access()
                .is_some_and(|a| a.handoff_unconsumed());

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
            .gap(if tight { px(4.) } else { px(8.) })
            .items_center()
            .pt_1()
            .child(
                h_flex().flex_shrink_0().items_center().child(
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
                                || self.current_worktree_released()
                                || self.session.as_ref().is_some_and(|session| {
                                    self.inflight_inputs.contains_key(&session.id)
                                })
                                || !self.attachments.ready(self.draft_target.as_ref()),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.choose_images(cx))),
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
                        row.when(
                            !self
                                .session
                                .as_ref()
                                .is_some_and(|s| matches!(s.backend, Backend::Acp))
                                && !self.current_history_access().is_some_and(|a| {
                                    a.handoff_unconsumed()
                                        && (a.handoff_passive()
                                            || !self
                                                .session
                                                .as_ref()
                                                .is_some_and(|s| matches!(s.backend, Backend::Pi)))
                                }),
                            |row| {
                                row.child(
                                    div()
                                        .min_w_0()
                                        .max_w(px(if compact { 190. } else { 260. }))
                                        .child(self.model_picker(
                                            window,
                                            running || self.conversation_is_read_only(),
                                            cx,
                                        )),
                                )
                            },
                        )
                        .when(native_commands && !compact, |row| {
                            row.child(self.command_button(cx))
                        })
                    })
                    .when(self.connected && !self.closing, |row| {
                        row.child(
                            Button::new("composer-controls-overflow")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(28.))
                                .w(px(28.))
                                .rounded_lg()
                                .icon(Glyph::Ellipsis)
                                .tooltip(tr("更多输入选项"))
                                .accessibility_label(tr("更多输入选项"))
                                .disabled(!self.connected || self.loading)
                                .dropdown_menu({
                                    let weak = cx.weak_entity();
                                    move |mut menu, _, _| {
                                        let weak_ref = weak.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(format!(
                                                "{} (@)",
                                                tr("引用项目文件")
                                            ))
                                            .disabled(!can_reference)
                                            .on_click(move |_, window, cx| {
                                                let _ = weak_ref.update(cx, |this, cx| {
                                                    this.open_project_file_picker(window, cx);
                                                });
                                            }),
                                        );
                                        let stash_label = if stash_count > 0 {
                                            format!("{} ({stash_count})", tr("草稿暂存"))
                                        } else {
                                            tr("草稿暂存").to_owned()
                                        };
                                        let weak_stash = weak.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(stash_label)
                                                .disabled(!can_stash)
                                                .on_click(move |_, window, cx| {
                                                    let _ = weak_stash.update(cx, |this, cx| {
                                                        this.open_prompt_stash(window, cx);
                                                    });
                                                }),
                                        );
                                        if native_commands {
                                            let weak_cmd = weak.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(format!(
                                                    "{} (/)",
                                                    tr("选择原生命令")
                                                ))
                                                .on_click(move |_, window, cx| {
                                                    let _ = weak_cmd.update(cx, |this, cx| {
                                                        this.toggle_native_commands(window, cx);
                                                    });
                                                }),
                                            );
                                        }
                                        if can_usage {
                                            let weak_usage = weak.clone();
                                            menu = menu.separator().item(
                                                PopupMenuItem::new(tr("原生用量与上下文")).on_click(
                                                    move |_, _, cx| {
                                                        let _ = weak_usage.update(cx, |this, cx| {
                                                            this.usage.visible =
                                                                !this.usage.visible;
                                                            if this.usage.visible {
                                                                this.load_usage(cx);
                                                            }
                                                            cx.notify();
                                                        });
                                                    },
                                                ),
                                            );
                                        }
                                        if compact && show_send_and_draft {
                                            let weak_draft = weak.clone();
                                            menu = menu.separator().item(
                                                PopupMenuItem::new(tr("发送并开始新草稿"))
                                                    .on_click(move |_, window, cx| {
                                                        let _ = weak_draft.update(cx, |this, cx| {
                                                            this.send_and_start_draft(window, cx);
                                                        });
                                                    }),
                                            );
                                        }
                                        menu
                                    }
                                }),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(if tight { px(4.) } else { px(6.) })
                    .when(running, |actions| {
                        actions.child(
                            Button::new("stop")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(30.))
                                .w(px(30.))
                                .rounded_full()
                                .bg(palette::Color::Inset.get(cx))
                                .border_1()
                                .border_color(palette::Color::Divider.get(cx))
                                .child(
                                    div()
                                        .size(px(9.))
                                        .rounded(px(2.))
                                        .bg(palette::Color::Text.get(cx)),
                                )
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
                    .when(!compact && show_send_and_draft, |actions| {
                        actions.child(
                            Button::new("send-and-start-draft")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(28.))
                                .w(px(28.))
                                .rounded_lg()
                                .icon(Glyph::SquarePen)
                                .tooltip(crate::keybindings::hint(
                                    crate::keybindings::Target::SendAndStartDraft,
                                    "发送并开始新草稿",
                                    cx,
                                ))
                                .accessibility_label(tr("发送并开始新草稿"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.send_and_start_draft(window, cx)
                                })),
                        )
                    })
                    .when(!can_resume, |actions| {
                        let busy_mode = crate::general::current(cx).busy_send_mode;
                        let blocked_while_running =
                            running && busy_mode == BusySendMode::BlockUntilIdle;
                        let busy_badge = if !running {
                            None
                        } else {
                            match busy_mode {
                                BusySendMode::Queue => Some(tr("排队")),
                                BusySendMode::SteerWhenSupported => Some(tr("插话")),
                                BusySendMode::BlockUntilIdle => None,
                            }
                        };
                        actions.child(
                            Button::new("send")
                                .custom(palette::send_circle_variant(can_send, cx))
                                .compact()
                                .h(px(30.))
                                .when(busy_badge.is_none(), |button| button.w(px(30.)))
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
                                        BusySendMode::SteerWhenSupported => {
                                            tr("发送插话引导")
                                        }
                                        BusySendMode::Queue => tr("加入消息队列"),
                                        BusySendMode::BlockUntilIdle => tr("发送消息"),
                                    }
                                } else {
                                    tr("发送消息")
                                })
                                .tooltip(if self.workspace.is_none() {
                                    tr("选择项目文件夹并发送").into()
                                } else if running {
                                    let shifted = cx
                                        .global::<crate::composer::ComposerPreferences>()
                                        .settings
                                        .mode
                                        .uses_shift_for_alternate(&self.input.read(cx).value());
                                    busy_send_hint(busy_mode, shifted)
                                } else {
                                    cx.global::<crate::composer::ComposerPreferences>()
                                        .settings
                                        .mode
                                        .hint()
                                        .into()
                                })
                                .disabled(!can_send)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if this.workspace.is_none() {
                                        this.choose_workspace_folder(window, cx);
                                    } else {
                                        this.send(SendIntent::Configured, window, cx);
                                    }
                                })),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::composer_control_layout;

    #[test]
    fn composer_controls_compact_thresholds() {
        assert_eq!(composer_control_layout(600.), (false, false));
        assert_eq!(composer_control_layout(520.), (false, false));
        assert_eq!(composer_control_layout(519.), (true, false));
        assert_eq!(composer_control_layout(380.), (true, false));
        assert_eq!(composer_control_layout(379.), (true, true));
    }
}
