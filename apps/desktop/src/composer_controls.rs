use crate::{
    app::HiveApp,
    language::tr,
    palette,
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
        window: &mut Window,
        _welcome: bool,
        running: bool,
        can_send: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let _ = Self::command_button;
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
                                    || self
                                        .session
                                        .as_ref()
                                        .is_some_and(|session| {
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
                    .flex_wrap()
                    .justify_end()
                    .items_center()
                    .gap_1()
                    .when(self.session.is_none(), |row| {
                        row.child(self.new_chat_menu("home-agent", tr("选择 Agent"), None, cx))
                    })
                    .when(self.session.is_some(), |row| {
                        row.child(self.model_picker(window, running, cx))
                            .children(self.reasoning_picker_trigger(window, running, cx))
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
                        actions.child(
                            Button::new("send")
                                .custom(palette::send_circle_variant(can_send, cx))
                                .compact()
                                .h(px(28.))
                                .when(!running, |button| button.w(px(28.)))
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
                                .tooltip(if self.workspace.is_none() {
                                    tr("选择项目文件夹并发送").into()
                                } else if running {
                                    tr("加入队列，当前回复结束后按顺序处理").into()
                                } else {
                                    cx.global::<crate::composer::ComposerPreferences>()
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
