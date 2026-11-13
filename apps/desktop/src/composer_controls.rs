use crate::{
    app::HiveApp,
    language::tr,
    palette::{self, Color},
    protocol::Backend,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
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
    }
}

impl HiveApp {
    pub(crate) fn render_composer_controls(
        &self,
        window: &Window,
        _welcome: bool,
        running: bool,
        can_send: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let attachment_count = self.attachments.refs(self.draft_target.as_ref()).len();
        let can_resume = !running
            && self
                .session
                .as_ref()
                .and_then(|session| self.pending.get(&session.id))
                .copied()
                .unwrap_or(0)
                > 0
            && (self.delivery_failure.is_none() || self.agent_diagnostic.is_some());

        h_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .items_center()
            .pt_1()
            .child(
                h_flex()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .gap_0p5()
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
                                    || self
                                        .session
                                        .as_ref()
                                        .is_some_and(|session| {
                                            self.inflight_inputs.contains_key(&session.id)
                                        })
                                    || !self.attachments.ready(self.draft_target.as_ref()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.choose_images(cx))),
                    )
                    .when(attachment_count > 0, |row| {
                        row.child(
                            Button::new("show-draft-attachments")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .h(px(26.))
                                .px_2()
                                .rounded_full()
                                .label(format!("{} · {}", tr("附件"), attachment_count))
                                .accessibility_label(format!(
                                    "{} · {}",
                                    tr("查看草稿附件"),
                                    attachment_count
                                ))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let offset = this.composer_scroll.offset();
                                    let delta = this.composer_scroll.bounds().top()
                                        - this.attachments.scroll.bounds().top();
                                    this.composer_scroll
                                        .set_offset(point(offset.x, offset.y + delta));
                                    this.attachments.scroll.set_offset(point(px(0.), px(0.)));
                                    window.refresh();
                                    cx.notify();
                                })),
                        )
                    })
                    .when(self.pi_commands_available(), |row| {
                        row.child(self.command_button(cx))
                    })
                    .when(self.session.is_none(), |row| {
                        row.child(self.new_chat_menu("home-agent", tr("选择 Agent"), None, cx))
                    })
                    .when_some(self.session.as_ref(), |row, session| {
                        let label = tr(session.backend.label());
                        row.child(
                            div()
                                .id("composer-backend-badge")
                                .role(Role::Label)
                                .aria_label(label)
                                .flex_shrink_0()
                                .h(px(28.))
                                .px_2()
                                .rounded_lg()
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(
                                    Icon::new(backend_glyph(session.backend))
                                        .size(px(13.))
                                        .text_color(Color::Muted.get(cx)),
                                )
                                .child(label),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .flex_wrap()
                    .justify_end()
                    .items_center()
                    .gap_1()
                    .when(self.session.is_some(), |row| {
                        row.child(self.model_picker(window, running, cx))
                            .child(self.usage_button(window, running, cx))
                    }),
            )
            .when(self.session.is_some(), |row| {
                row.child(
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
                                    .h(px(30.))
                                    .w(px(30.))
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
                                    .h(px(30.))
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
                            actions.child(
                                Button::new("send")
                                    .custom(palette::send_circle_variant(can_send, cx))
                                    .compact()
                                    .h(px(30.))
                                    .when(!running, |button| button.w(px(30.)))
                                    .when(running, |button| button.px_3())
                                    .rounded_full()
                                    .when(!can_send, |button| {
                                        button
                                            .bg(palette::send_circle_disabled_bg(cx))
                                            .text_color(palette::send_circle_disabled_fg(cx))
                                    })
                                    .icon(Glyph::ArrowUp)
                                    .when(running, |button| button.label(tr("排队")))
                                    .accessibility_label(if running {
                                        tr("加入消息队列")
                                    } else {
                                        tr("发送消息")
                                    })
                                    .tooltip(if running {
                                        tr("加入队列，当前回复结束后按顺序处理")
                                    } else {
                                        cx.global::<crate::composer::ComposerPreferences>()
                                            .mode
                                            .hint()
                                    })
                                    .disabled(!can_send)
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.send(window, cx)),
                                    ),
                            )
                        }),
                )
            })
    }
}
