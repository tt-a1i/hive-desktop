use super::preferences::{ChatSettings, ComposerPreferences, SendMode, ToolActivity};
use crate::{app::HiveApp, language::tr, palette::Color};
use gpui_kit::{
    component::{button::Button, *},
    *,
};

#[derive(Clone, Copy)]
enum Change {
    Send(SendMode),
    Tools(ToolActivity),
    Thinking(bool),
}
impl HiveApp {
    pub(crate) fn render_chat_settings_notice(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let preferences = cx.global::<ComposerPreferences>();
        preferences.message().map(|message| {
            crate::settings::settings_notice_box("chat-settings-notice", message, preferences.has_error(), cx)
                .flex_shrink_0()
                .into_any_element()
        })
    }
    fn change_chat_settings(&mut self, change: Change, cx: &mut Context<Self>) {
        let previous = cx.global::<ComposerPreferences>().settings;
        let mut settings = previous;
        match change {
            Change::Send(mode) => settings.mode = mode,
            Change::Tools(activity) => settings.tool_activity = activity,
            Change::Thinking(expanded) => settings.expand_thinking = expanded,
        }
        if cx.global_mut::<ComposerPreferences>().change(settings) {
            if previous.expand_thinking != settings.expand_thinking {
                for message in &mut self.messages {
                    for block in &mut message.thinking {
                        block.expanded = !block.redacted && settings.expand_thinking;
                    }
                    if !message.thinking.is_empty() {
                        message.layout_revision += 1;
                    }
                }
            }
            self.sync_conversation_list(cx);
        }
        cx.refresh_windows();
        cx.notify();
    }
    fn chat_setting_choices(
        &self,
        id: &'static str,
        title: &'static str,
        description: &'static str,
        choices: Vec<(&'static str, &'static str, bool, Change)>,
        divider: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let ready = cx.global::<ComposerPreferences>().ready();
        crate::settings::settings_row(divider, cx).child(
            v_flex()
                .gap_2p5()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(title))
                .child(
                    div()
                        .text_sm()
                        .text_color(Color::Secondary.get(cx))
                        .child(description),
                )
                .child(crate::focus_scroll::Reveal::new(
                    id,
                    &self.chat_settings_scroll,
                    h_flex()
                        .id(SharedString::from(format!("{id}-choices")))
                        .role(Role::Group)
                        .aria_label(title)
                        .gap_2()
                        .flex_wrap()
                        .children(choices.into_iter().map(|(id, label, active, change)| {
                            Button::new(id)
                                .outline()
                                .small()
                                .rounded_lg()
                                .px_3()
                                .label(label)
                                .accessibility_label(format!("{title}: {label}"))
                                .selected(active)
                                .toggled(active)
                                .disabled(!ready)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.change_chat_settings(change, cx)
                                }))
                        })),
                )),
        )
    }
    pub(crate) fn render_chat_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let preferences = cx.global::<ComposerPreferences>();
        let ChatSettings {
            mode,
            tool_activity,
            expand_thinking,
        } = preferences.settings;
        let modifier = if cfg!(target_os = "macos") {
            tr("⌘Enter 发送")
        } else {
            tr("Ctrl+Enter 发送")
        };
        v_flex().gap_4()
            .child(div().text_sm().text_color(Color::Secondary.get(cx))
                .child(tr("设置自动保存在本机，适用于所有项目与 Agent，不修改 Agent 原生配置或聊天记录。")))
            .child(crate::settings::settings_card(cx)
                .child(self.chat_setting_choices("chat-send-mode-reveal", tr("发送消息"),
                    tr("选择习惯的发送方式。Shift+Enter 始终换行，点击发送按钮也可发送。适用于所有 Agent。"),
                    vec![
                        ("send-mode-enter", tr("Enter 发送"), mode == SendMode::Enter, Change::Send(SendMode::Enter)),
                        ("send-mode-modifier", modifier, mode == SendMode::ModifierEnter, Change::Send(SendMode::ModifierEnter)),
                    ], false, cx))
                .child(self.chat_setting_choices("chat-tools-mode-reveal", tr("工具活动"),
                    tr("连续成功的工具可合并为工作过程，也可逐项展示。失败、中断和执行中的工具始终单独显示；详情仍可展开。"),
                    vec![
                        ("tool-activity-grouped", tr("合并已完成工具"), tool_activity == ToolActivity::Grouped, Change::Tools(ToolActivity::Grouped)),
                        ("tool-activity-individual", tr("逐项展示"), tool_activity == ToolActivity::Individual, Change::Tools(ToolActivity::Individual)),
                    ], true, cx))
                .child(self.chat_setting_choices("chat-thinking-mode-reveal", tr("思考过程"),
                    tr("选择后立即应用于当前会话，并作为其他会话的默认展示方式。每段仍可单独展开或收起，后端未公开的内容不显示。"),
                    vec![
                        ("thinking-default-collapsed", tr("默认收起"), !expand_thinking, Change::Thinking(false)),
                        ("thinking-default-expanded", tr("默认展开"), expand_thinking, Change::Thinking(true)),
                    ], true, cx)))
    }
}
