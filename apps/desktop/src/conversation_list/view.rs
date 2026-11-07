use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    component::{button::Button, text::TextView, *},
    prelude::FluentBuilder,
    *,
};

impl HiveApp {
    pub(crate) fn render_conversation_message(
        &self,
        i: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let message = &self.messages[i];
        if let Some(tool) = &message.tool {
            return self
                .render_tool_message(i, message, tool, cx)
                .into_any_element();
        }
        let heading = message
            .delivery
            .as_ref()
            .map(|delivery| delivery.heading.clone())
            .unwrap_or_else(|| {
                match message.role.as_str() {
                    "user" if message.explicit_user => "你",
                    "user" => "对话输入",
                    "assistant" => self
                        .session
                        .as_ref()
                        .map(|s| s.backend.label())
                        .unwrap_or("助手"),
                    "notice" => "会话通知",
                    _ => "工具",
                }
                .into()
            });
        v_flex()
            .id(("message", i))
            .min_w_0()
            .gap_2()
            .px_3()
            .py_2()
            .rounded_lg()
            .when(self.conversation_find_matches(&message.id), |view| {
                view.bg(Color::Attention.get(cx))
            })
            .when(message.role == "user", |bubble| {
                bubble.ml_auto().max_w(px(680.))
            })
            .when(!message.explicit_user, |view| {
                view.child(
                    h_flex()
                        .min_w_0()
                        .gap_2()
                        .justify_between()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Color::Secondary.get(cx))
                                .child(heading),
                        )
                        .when(
                            message.delivery.as_ref().is_some_and(|card| card.report),
                            |row| {
                                let state = if self.completed_deliveries.contains(&message.id) {
                                    "处理回合已完成"
                                } else {
                                    self.pending_reports
                                        .iter()
                                        .find(|report| report.delivery_id == message.id)
                                        .map(|report| report.status())
                                        .unwrap_or("已进入会话")
                                };
                                row.child(
                                    div()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(state),
                                )
                            },
                        )
                        .when_some(
                            self.dispatch_link(
                                &message.id,
                                message
                                    .delivery
                                    .as_ref()
                                    .and_then(|d| d.dispatch_id.as_deref()),
                                cx,
                            ),
                            |row, link| row.child(link),
                        )
                        .when_some(
                            self.member_session_link(
                                &message.id,
                                message
                                    .delivery
                                    .as_ref()
                                    .and_then(|card| card.worker_id.as_deref()),
                                "成员会话",
                                cx,
                            ),
                            |row, link| row.child(link),
                        )
                        .when(message.delivery.is_some(), |view| {
                            view.child(
                                Button::new(("expand-delivery", i))
                                    .accessibility_label(format!(
                                        "{}消息原文",
                                        if message.delivery.as_ref().is_some_and(|d| d.expanded) {
                                            "收起"
                                        } else {
                                            "查看"
                                        }
                                    ))
                                    .label(
                                        if message.delivery.as_ref().is_some_and(|d| d.expanded) {
                                            "收起原文"
                                        } else {
                                            "查看原文"
                                        },
                                    )
                                    .on_click({
                                        let id = message.id.clone();
                                        cx.listener(move |this, _, _, cx| {
                                            if let Some(delivery) = this
                                                .messages
                                                .iter_mut()
                                                .find(|m| m.id == id)
                                                .and_then(|m| m.delivery.as_mut())
                                            {
                                                delivery.expanded = !delivery.expanded;
                                            }
                                            cx.notify();
                                        })
                                    }),
                            )
                        }),
                )
            })
            .when_some(message.markdown.as_ref(), |view, markdown| {
                view.child(TextView::new(markdown).selectable(true).stream_fade(true))
            })
            .when(message.markdown.is_none(), |view| {
                view.child(
                    div()
                        .min_w_0()
                        .text_size(px(15.))
                        .line_height(px(24.))
                        .when(message.role == "user", |bubble| {
                            bubble
                                .px_4()
                                .py_2()
                                .rounded_xl()
                                .bg(Color::Card.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                        })
                        .child(message.text.clone()),
                )
            })
            .when_some(message.delivery.as_ref(), |view, delivery| {
                view.children(
                    delivery
                        .artifacts
                        .iter()
                        .enumerate()
                        .map(|(i, path)| self.artifact_link(&message.id, i, path, cx)),
                )
            })
            .when(
                message.delivery.as_ref().is_some_and(|d| d.expanded),
                |view| {
                    view.child(
                        div()
                            .min_w_0()
                            .p_3()
                            .rounded_lg()
                            .bg(Color::Card.get(cx))
                            .text_sm()
                            .child(
                                message
                                    .delivery
                                    .as_ref()
                                    .map(|d| d.raw.clone())
                                    .unwrap_or_default(),
                            ),
                    )
                },
            )
            .child(self.message_actions(i, message, cx))
            .into_any_element()
    }
    pub(crate) fn render_conversation_tail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self
            .session
            .as_ref()
            .is_some_and(|session| self.busy.contains(&session.id));
        let needs_pi_model = self
            .session
            .as_ref()
            .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Pi))
            && self.model.is_none()
            && !self.loading;
        v_flex()
            .id("conversation-tail")
            .track_focus(&self.conversation_list.tail_focus)
            .w_full()
            .min_w_0()
            .gap_5()
            .when(
                self.messages.is_empty()
                    && self.pending_reports.is_empty()
                    && (self.interaction_count(
                        self.session.as_ref().map(|s| s.id.as_str()).unwrap_or(""),
                    ) == 0
                        || !self.connected),
                |view| {
                    view.child(div().py_8().text_color(Color::Muted.get(cx)).child(
                        if self.loading {
                            "正在读取…"
                        } else if !self.connected {
                            "后台连接已断开，当前草稿仍保留。可在窗口顶部重新启动后台。"
                        } else if running {
                            "正在等待 Agent 回复…"
                        } else if needs_pi_model {
                            "还没有消息。"
                        } else {
                            "会话已就绪。发送第一条消息开始工作。"
                        },
                    ))
                },
            )
            .children(
                self.pending_reports
                    .iter()
                    .filter(|report| {
                        !self
                            .messages
                            .iter()
                            .any(|message| message.id == report.delivery_id)
                    })
                    .map(|report| {
                        v_flex()
                            .min_w_0()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(format!(
                                                "{} · 成员汇报",
                                                report.worker_name.as_deref().unwrap_or("Worker")
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(report.status()),
                                    )
                                    .when_some(
                                        self.dispatch_link(
                                            &report.delivery_id,
                                            report.dispatch_id.as_deref(),
                                            cx,
                                        ),
                                        |row, link| row.child(link),
                                    )
                                    .when_some(
                                        self.member_session_link(
                                            &report.delivery_id,
                                            report.worker_id.as_deref(),
                                            "成员会话",
                                            cx,
                                        ),
                                        |row, link| row.child(link),
                                    ),
                            )
                            .child(div().text_sm().child(report.text.clone()))
                            .children(report.artifacts.iter().enumerate().map(|(i, path)| {
                                self.artifact_link(&report.delivery_id, i, path, cx)
                            }))
                    }),
            )
            .child(self.render_interactions(cx))
    }
}
