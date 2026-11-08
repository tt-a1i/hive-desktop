use crate::{app::HiveApp, palette::Color,language::{english,tr}};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    *,
};

impl HiveApp {
    pub(crate) fn render_pending_user_inputs(&self, cx: &Context<Self>) -> impl IntoElement {
        let running = self
            .session
            .as_ref()
            .is_some_and(|session| self.busy.contains(&session.id));
        let waiting: Vec<_> = self
            .pending_user_inputs
            .iter()
            .filter(|input| {
                !input.processing
                    && !self
                        .messages
                        .iter()
                        .any(|message| message.id == input.delivery_id)
            })
            .collect();
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .when(!waiting.is_empty(), |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(format!("{} · {}",tr("待处理消息"),waiting.len())),
                )
                .child(
                    v_flex()
                        .id("queued-user-inputs")
                        .max_h(px(144.))
                        .overflow_y_scroll()
                        .gap_2()
                        .children(waiting.into_iter().map(|input| {
                            let text = input.text.clone();
                            h_flex()
                                .min_w_0()
                                .gap_2()
                                .p_2()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_1()
                                        .child(div().text_sm().truncate().child(
                                            if input.text.is_empty() {
                                                tr("附件消息").into()
                                            } else {
                                                input.text.clone()
                                            },
                                        ))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Muted.get(cx))
                                                .truncate()
                                                .child(if input.attachments.is_empty() {
                                                    if running {
                                                        tr("已入队，等待当前回复结束").into()
                                                    } else {
                                                        tr("已入队，等待继续处理").into()
                                                    }
                                                } else {
                                                    format!(
                                                        "{}: {}",tr("附件"),
                                                        input
                                                            .attachments
                                                            .iter()
                                                            .map(|item| item.name.as_str())
                                                            .collect::<Vec<_>>()
                                                            .join(if english() {", "} else {"、"})
                                                    )
                                                }),
                                        ),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "copy-queued-{}",
                                        input.delivery_id
                                    )))
                                    .ghost()
                                    .compact()
                                    .label(tr("复制"))
                                    .accessibility_label(tr("复制待处理消息"))
                                    .disabled(input.text.is_empty())
                                    .on_click(
                                        move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                text.clone(),
                                            ))
                                        },
                                    ),
                                )
                        })),
                )
            })
    }
}
