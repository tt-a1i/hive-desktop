use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

#[derive(Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryFailure {
    pub delivery_id: String,
    pub message: String,
}
pub struct Diagnostic {
    pub message: String,
    pub expanded: bool,
    pub diagnostic_write_failed: bool,
}

fn details(owner: &str, message: &str, cx: &Context<HiveApp>) -> AnyElement {
    let text = message.to_owned();
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
                .bg(Color::Card.get(cx))
                .text_xs()
                .child(message.to_owned()),
        )
        .child(
            Button::new(SharedString::from(format!("copy-failure-{owner}")))
                .ghost()
                .compact()
                .label("复制错误详情")
                .accessibility_label("复制错误详情")
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                }),
        )
        .into_any_element()
}
impl HiveApp {
    pub(crate) fn reset_reply_failures(&mut self) {
        self.delivery_failure = None;
        self.delivery_failure_expanded = false;
        self.agent_diagnostic = None;
    }
    pub(crate) fn set_delivery_failure(&mut self, failure: Option<DeliveryFailure>) {
        if self.delivery_failure != failure {
            self.delivery_failure_expanded = false;
        }
        self.delivery_failure = failure;
    }
    // Explicit delivery IDs relate a current failed turn to the pending spool.
    // No parsing or matching backend error strings, and native history is retained.
    pub(crate) fn hidden_current_failure(&self, index: usize) -> bool {
        self.messages
            .get(index)
            .is_some_and(|message| message.failure.is_some())
            && self.delivery_failure.as_ref().is_some_and(|failure| {
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
            .rounded_lg()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(Color::Error.get(cx))
                            .child("这次回复未完成"),
                    )
                    .child(
                        Button::new(("reply-failure", index))
                            .ghost()
                            .compact()
                            .label(if failure.expanded {
                                "收起详情"
                            } else {
                                "错误详情"
                            })
                            .accessibility_label("查看历史回复错误详情")
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
                view.child(details(&format!("reply-{index}"), &failure.message, cx))
            })
            .into_any_element()
    }
    pub(crate) fn render_session_failure(&self, running: bool, cx: &Context<Self>) -> AnyElement {
        let transient = self.agent_diagnostic.as_ref();
        if let Some(failure) = transient {
            return v_flex()
                .min_w_0()
                .gap_2()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(Color::Divider.get(cx))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(Color::Error.get(cx))
                                .child(if failure.diagnostic_write_failed {
                                    "失败详情未能保存，原消息仍保留"
                                } else {
                                    "Agent 报告了错误"
                                }),
                        )
                        .child(
                            Button::new("agent-diagnostic-details")
                                .ghost()
                                .compact()
                                .label(if failure.expanded {
                                    "收起详情"
                                } else {
                                    "错误详情"
                                })
                                .accessibility_label("查看 Agent 错误详情")
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
                                .label("关闭")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.agent_diagnostic = None;
                                    cx.notify();
                                })),
                        ),
                )
                .when(failure.expanded, |view| {
                    view.child(details("agent", &failure.message, cx))
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
            .min_w_0()
            .gap_2()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if running {
                                "正在重新处理原消息…"
                            } else {
                                "消息处理未完成"
                            }),
                    )
                    .child(
                        Button::new("delivery-failure-details")
                            .ghost()
                            .compact()
                            .label(if self.delivery_failure_expanded {
                                "收起详情"
                            } else {
                                "错误详情"
                            })
                            .accessibility_label("查看消息处理错误详情")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.delivery_failure_expanded = !this.delivery_failure_expanded;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("retry-delivery-failure")
                            .rounded_full()
                            .label("重试原消息")
                            .accessibility_label("重试原消息及附件")
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| this.resume(cx))),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child("待处理消息与附件已保留，重试将继续处理原来的内容。"),
            )
            .when(self.delivery_failure_expanded, |view| {
                view.child(details("delivery", &failure.message, cx))
            })
            .into_any_element()
    }
}
