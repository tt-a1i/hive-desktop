use crate::{app::HiveApp, language::tr};
use gpui_kit::{assets::IconName as Glyph, component::{button::{Button, ButtonVariants}, *}, prelude::FluentBuilder, *};

impl HiveApp {
    pub(crate) fn render_composer_controls(&self, window: &Window, running: bool, can_send: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let can_resume = !running && self.session.as_ref().and_then(|session| self.pending.get(&session.id)).copied().unwrap_or(0) > 0
            && (self.delivery_failure.is_none() || self.agent_diagnostic.is_some());
        h_flex().w_full().min_w_0().gap_2().items_end()
            .child(h_flex().min_w_0().flex_wrap().gap_1()
                .when(self.session.is_none(), |row| row.child(self.new_chat_menu("home-agent", tr("选择 Agent"), None, cx)))
                .child(Button::new("attach-images").ghost().compact().icon(Glyph::Plus).label(tr("附件"))
                    .accessibility_label(tr("添加文件或图片"))
                    .disabled(!self.connected || self.workspace.is_none() || self.loading || self.session.as_ref().is_some_and(|session| self.inflight_inputs.contains_key(&session.id)) || !self.attachments.ready(self.draft_target.as_ref()))
                    .on_click(cx.listener(|this, _, _, cx| this.choose_images(cx))))
                .when(self.pi_commands_available(), |row| row.child(self.command_button(cx))))
            .child(h_flex().flex_1().min_w_0().flex_wrap().justify_end().items_center().gap_1()
                .when(self.session.is_some(), |row| row
                    .child(self.model_picker(window, running, cx))
                    .child(self.usage_button(window, running, cx))))
            .when(self.session.is_some(), |row| row.child(h_flex().flex_shrink_0().gap_1()
                .when(running, |actions| actions.child(Button::new("stop").rounded_full().icon(Glyph::Square)
                    .tooltip(tr("停止回复")).accessibility_label(tr("停止")).disabled(!self.connected)
                    .on_click(cx.listener(|this, _, _, cx| this.abort(cx)))))
                .when(can_resume, |actions| actions.child(Button::new("resume").primary().label(tr("继续处理"))
                    .disabled(!self.connected || !self.conversation_ready || self.loading)
                    .on_click(cx.listener(|this, _, _, cx| this.resume(cx)))))
                .when(!can_resume, |actions| actions.child(Button::new("send").primary().icon(Glyph::ArrowUp).rounded(px(20.))
                    .when(running, |button| button.label(tr("排队")))
                    .accessibility_label(if running { tr("加入消息队列") } else { tr("发送消息") })
                    .tooltip(if running { tr("加入队列，当前回复结束后按顺序处理") } else { cx.global::<crate::composer::ComposerPreferences>().mode.hint() })
                    .disabled(!can_send).on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))))))
    }
}
