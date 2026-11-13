use crate::{app::{HiveApp, SettingsSection},focus_scroll::{Reveal,Viewport}, palette::Color,language::tr};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub use crate::protocol::DeliveryFailure;
pub struct Diagnostic {
    pub message: String,
    pub expanded: bool,
    pub diagnostic_write_failed: bool,
}
impl Diagnostic {
    pub(crate) fn display_message(&self) -> std::borrow::Cow<'_, str> {
        if self.message.trim().is_empty() {
            std::borrow::Cow::Borrowed(tr("Agent 回复未完成。"))
        } else if crate::language::english() {
            std::borrow::Cow::Owned(crate::language::localize_owned_error(&self.message))
        } else {
            std::borrow::Cow::Borrowed(&self.message)
        }
    }
}

fn details(owner: &str, message: &str, cx: &Context<HiveApp>) -> AnyElement {
    let display = crate::language::localize_owned_error(message);
    let text = display.clone();
    v_flex()
        .min_w_0()
        .gap_2()
        .child(
            div()
                .id(SharedString::from(format!("failure-details-{owner}")))
                .min_w_0()
                .max_h(px(180.))
                .overflow_y_scroll()
                .p_2()
                .rounded_lg()
                .border_1()
                .border_color(Color::Divider.get(cx))
                .bg(Color::Inset.get(cx))
                .font_family(Theme::global(cx).mono_font_family.clone())
                .text_xs()
                .child(crate::text_reading::selectable_text(format!("failure-text-{owner}"), display)),
        )
        .child(
            Button::new(SharedString::from(format!("copy-failure-{owner}")))
                .ghost()
                .compact()
                .rounded_full()
                .label(tr("复制错误详情"))
                .accessibility_label(tr("复制错误详情"))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                }),
        )
        .into_any_element()
}
impl HiveApp {
    pub(crate) fn render_agent_repair_actions(&self, disabled: bool, cx: &Context<Self>) -> AnyElement {
        let Some(session) = &self.session else { return div().into_any_element(); };
        self.render_backend_repair_actions(session.backend, disabled, cx)
    }
    pub(crate) fn render_backend_repair_actions(&self, backend: crate::protocol::Backend, disabled: bool, cx: &Context<Self>) -> AnyElement {
        let backend = match backend {
            crate::protocol::Backend::Pi => None,
            crate::protocol::Backend::Codex => Some("codex"),
            crate::protocol::Backend::Claude => Some("claude"),
            crate::protocol::Backend::PiRpc => Some("pi"),
        };
        h_flex().min_w_0().gap_2().flex_wrap()
            .when_some(backend, |row, backend| row.child(Button::new("repair-agent-program").track_focus(&self.repair_program_focus).ghost().compact()
                .label(tr("外部程序设置")).disabled(disabled)
                .on_click(cx.listener(move |this, _, window, cx| this.open_program_setup(backend, window, cx)))))
            .when(backend.is_none(), |row| row.children([
                ("repair-pi-accounts", SettingsSection::PiAccounts, tr("内置 Pi 账号"), self.repair_accounts_focus.clone()),
                ("repair-pi-models", SettingsSection::Models, tr("模型与接口"), self.repair_models_focus.clone()),
            ].into_iter().map(|(id, section, label, focus)| Button::new(id).track_focus(&focus).ghost().compact().label(label).disabled(disabled)
                .on_click(cx.listener(move |this, _, window, cx| { this.settings_section = section; this.open_settings(window, cx); })))))
            .into_any_element()
    }
    pub(crate) fn reset_reply_failures(&mut self) {
        if matches!(self.error, Some(crate::app::ErrorNotice::Conversation(_))) {
            self.error = None;
        }
        self.failure_scroll.set_offset(point(px(0.),px(0.)));
        self.delivery_failure = None;
        self.delivery_failure_expanded = false;
        self.agent_diagnostic = None;
    }
    pub(crate) fn set_delivery_failure(&mut self, failure: Option<DeliveryFailure>) {
        if self.delivery_failure != failure {
            self.failure_scroll.set_offset(point(px(0.),px(0.)));
            self.delivery_failure_expanded = false;
        }
        self.delivery_failure = failure;
    }
    // Explicit delivery IDs relate a current failed turn to the pending spool.
    // No parsing or matching backend error strings, and native history is retained.
    pub(crate) fn hidden_current_failure(&self, index: usize) -> bool {
        self.messages.get(index).is_some_and(|message| {
            message.failure.is_some() && !self.conversation_find_matches(&message.id)
        }) && self.delivery_failure.as_ref().is_some_and(|failure| {
            self.messages[..index]
                .iter()
                .rev()
                .find(|m| m.explicit_user || m.delivery.is_some())
                .is_some_and(|m| m.id == failure.delivery_id)
        })
    }
    pub(crate) fn render_reply_failure(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let Some(failure) = self.messages[index].failure.as_ref() else {
            return div().into_any_element();
        };
        let id = self.messages[index].id.clone();
        v_flex()
            .min_w_0()
            .gap_2()
            .p_3()
            .rounded_xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Card.get(cx))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Error.get(cx))
                            .child(tr("这次回复未完成")),
                    )
                    .child(
                        Button::new(("reply-failure", index))
                            .ghost()
                            .compact()
                            .rounded_full()
                            .label(if failure.expanded {
                                tr("收起详情")
                            } else {
                                tr("错误详情")
                            })
                            .accessibility_label(tr("查看历史回复错误详情"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.prepare_conversation_expansion(&id);
                                if let Some(message) = this.messages.iter_mut().find(|m| m.id == id)
                                {
                                    if let Some(failure) = message.failure.as_mut() {
                                        failure.expanded = !failure.expanded;
                                        message.layout_revision += 1;
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            )
            .when(failure.expanded, |view| {
                view.child(details(&format!("reply-{index}"), &failure.display_message(), cx))
            })
            .into_any_element()
    }
    pub(crate) fn render_session_failure(&self, window: &Window, running: bool, cx: &Context<Self>) -> Option<AnyElement> {
        if self.agent_diagnostic.is_none() && self.delivery_failure.is_none() { return None; }
        let height = (window.viewport_size().height / 6.).min(px(180.)).max(px(64.));
        Some(div().flex().items_stretch().flex_shrink_0().w_full().min_w_0().h(height).rounded_xl().border_1().border_color(Color::Divider.get(cx)).bg(Color::Card.get(cx))
            .child(Viewport::new("session-failure-viewport", &self.failure_scroll,
                v_flex().id("session-failure-scroll").flex_1().min_w_0().min_h_0().h(height).overflow_y_scroll().track_scroll(&self.failure_scroll).vertical_scrollbar(&self.failure_scroll).p_3()
                    .child(self.render_failure_notice(running,cx))))
            .into_any_element())
    }
    fn render_failure_notice(&self, running: bool, cx: &Context<Self>) -> AnyElement {
        let transient = self.agent_diagnostic.as_ref();
        if let Some(failure) = transient {
            return v_flex()
                .flex_shrink_0()
                .min_w_0()
                .gap_2()
                .child(
                    Reveal::new("diagnostic-header-reveal", &self.failure_scroll, h_flex()
                        .gap_2()
                        .flex_wrap()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(Color::Error.get(cx))
                                .child(if failure.diagnostic_write_failed {
                                    tr("失败详情未能保存，原消息仍保留")
                                } else {
                                    tr("Agent 报告了错误")
                                }),
                        )
                        .child(
                            Button::new("agent-diagnostic-details")
                                .ghost()
                                .compact()
                                .label(if failure.expanded {
                                    tr("收起详情")
                                } else {
                                    tr("错误详情")
                                })
                                .accessibility_label(tr("查看 Agent 错误详情"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(failure) = &mut this.agent_diagnostic {
                                        failure.expanded = !failure.expanded;
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("dismiss-agent-diagnostic")
                                .ghost()
                                .compact()
                                .label(tr("关闭"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.agent_diagnostic = None;
                                    cx.notify();
                                })),
                        )),
                )
                .child(Reveal::new("failure-repair-reveal", &self.failure_scroll, self.render_agent_repair_actions(running || !self.connected, cx)))
                .when(failure.expanded, |view| {
                    view.child(Reveal::new("agent-details-reveal", &self.failure_scroll, details("agent", &failure.message, cx)))
                })
                .into_any_element();
        }
        let Some(failure) = &self.delivery_failure else {
            return div().into_any_element();
        };
        let disabled = running
            || !self.connected
            || self.loading
            || !self.conversation_ready
            || !self.workspace_writes_allowed();
        v_flex()
            .flex_shrink_0()
            .min_w_0()
            .gap_2()
            .child(
                Reveal::new("delivery-header-reveal", &self.failure_scroll, h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if running {
                                tr("正在重新处理原消息…")
                            } else {
                                tr("消息处理未完成")
                            }),
                    )
                    .child(
                        Button::new("delivery-failure-details")
                            .ghost()
                            .compact()
                            .label(if self.delivery_failure_expanded {
                                tr("收起详情")
                            } else {
                                tr("错误详情")
                            })
                            .accessibility_label(tr("查看消息处理错误详情"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.delivery_failure_expanded = !this.delivery_failure_expanded;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("retry-delivery-failure")
                            .rounded_full()
                            .label(tr("重试原消息"))
                            .accessibility_label(tr("重试原消息及附件"))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| this.resume(cx))),
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(tr("待处理消息与附件已保留，重试将继续处理原来的内容。")),
            )
            .child(Reveal::new("failure-repair-reveal", &self.failure_scroll, self.render_agent_repair_actions(running || !self.connected, cx)))
            .when(self.delivery_failure_expanded, |view| {
                view.child(Reveal::new("delivery-details-reveal", &self.failure_scroll, details("delivery", &failure.message, cx)))
            })
            .into_any_element()
    }
}
