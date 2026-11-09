use crate::{app::HiveApp, palette::Color,language::{english,tr}};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        text::TextView,
        *,
    },
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
            .map(|delivery| delivery.heading_text())
            .unwrap_or_else(|| {
                match message.role.as_str() {
                    "user" if message.explicit_user => tr("你"),
                    "user" => tr("对话输入"),
                    "assistant" => self
                        .session
                        .as_ref()
                        .map(|s| tr(s.backend.label()))
                        .unwrap_or(tr("助手")),
                    "notice" => tr("会话通知"),
                    _ => tr("工具"),
                }
                .into()
            });
        v_flex()
            .id(("message", i))
            .role(Role::Group)
            .aria_label(heading.clone())
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
                                    tr("处理回合已完成")
                                } else {
                                    self.pending_reports
                                        .iter()
                                        .find(|report| report.delivery_id == message.id)
                                        .map(|report| tr(report.status()))
                                        .unwrap_or(tr("已进入会话"))
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
                                tr("成员会话"),
                                cx,
                            ),
                            |row, link| row.child(link),
                        )
                        .when(message.delivery.is_some(), |view| {
                            view.child(
                                Button::new(("expand-delivery", i))
                                    .accessibility_label(format!("{} {}",if message.delivery.as_ref().is_some_and(|d|d.expanded) {tr("收起")} else {tr("查看")},tr("消息原文")))
                                    .label(
                                        if message.delivery.as_ref().is_some_and(|d| d.expanded) {
                                            tr("收起原文")
                                        } else {
                                            tr("查看原文")
                                        },
                                    )
                                    .on_click({
                                        let id = message.id.clone();
                                        cx.listener(move |this, _, _, cx| {
                                            this.prepare_conversation_expansion(&id);
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
            .when(
                message.failure.is_some() && !self.hidden_current_failure(i),
                |view| view.child(self.render_reply_failure(i, cx)),
            )
            .children(
                message
                    .thinking
                    .iter()
                    .map(|block| self.render_thinking(i, &message.id, block, cx)),
            )
            .when_some(
                message
                    .markdown
                    .as_ref()
                    .filter(|_| !message.text.is_empty()),
                |view, markdown| {
                    view.child(div()
                        .id(("message-body", i))
                        .role(Role::Label)
                        .aria_label(markdown.read(cx).rendered_text().as_str().to_string())
                        .child(TextView::new(markdown).selectable(true).stream_fade(true)))
                },
            )
            .when(
                message.markdown.is_none() && !message.text.is_empty(),
                |view| {
                    view.child(
                        div()
                            .id(("message-body", i))
                            .role(Role::Label)
                            .aria_label(message.text.clone())
                            .min_w_0()
                            .text_size(rems(15. / 16.))
                            .line_height(rems(1.5))
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
                },
            )
            .when(message.images_loading, |view| {
                view.child(div().text_xs().child(tr("读取图片…")))
            })
            .when_some(message.images_error.as_ref(), |view, error| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(format!("{}: {error}",tr("图片预览失败"))),
                )
            })
            .when(!message.images.is_empty(), |view| {
                view.child(
                    h_flex().min_w_0().flex_wrap().gap_2().children(
                        message
                            .images
                            .iter()
                            .cloned()
                            .enumerate()
                            .map(|(index, image)| {
                                let original = message.image_sources[index].clone();
                                let name =
                                    format!("{}-{}.{}",if english() {"Image"} else {"图片"},index+1,original.format.extension());
                                Button::new(SharedString::from(format!("view-image-{i}-{index}")))
                                    .ghost()
                                    .w(px(144.))
                                    .h(px(112.))
                                    .accessibility_label(format!("{} {}",tr("查看图片"),index+1))
                                    .child(
                                        img(image)
                                            .w(px(144.))
                                            .h(px(112.))
                                            .object_fit(ObjectFit::Contain),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_image(original.clone(), name.clone(), window, cx)
                                    }))
                            }),
                    ),
                )
            })
            .when(!message.files.is_empty(), |view| {
                view.child(
                    h_flex().w_full().min_w_0().flex_wrap().gap_2().children(
                        message
                            .files
                            .iter()
                            .cloned()
                            .map(|item| self.file_attachment_link(&message.id, item, cx)),
                    ),
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
                            .id(("delivery-raw", i))
                            .role(Role::Label)
                            .aria_label(message.delivery.as_ref().map(|d| d.raw.clone()).unwrap_or_default())
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
            .when(
                !message.text.is_empty()
                    || !message.images.is_empty()
                    || message.delivery.is_some(),
                |view| view.child(self.message_actions(i, message, cx)),
            )
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
                            tr("正在读取…")
                        } else if !self.connected {
                            tr("后台连接已断开，当前草稿仍保留。可在窗口顶部重新启动后台。")
                        } else if running {
                            tr("正在等待 Agent 回复…")
                        } else if needs_pi_model {
                            tr("还没有消息。")
                        } else {
                            tr("会话已就绪。发送第一条消息开始工作。")
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
                                                "{} · {}",
                                                report.worker_name.as_deref().unwrap_or("Worker"),tr("成员汇报")
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(tr(report.status())),
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
                                            tr("成员会话"),
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
