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
        let undisplayed: Vec<_> = self
            .pending_user_inputs
            .iter()
            .filter(|input| {
                !self.messages.iter().any(|message| message.id == input.delivery_id)
            })
            .collect();
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .when(!undisplayed.is_empty(), |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(format!("{} · {}",tr("已接收消息"),undisplayed.len())),
                )
                .child(
                    v_flex()
                        .id("queued-user-inputs")
                        .max_h(px(144.))
                        .overflow_y_scroll()
                        .gap_2()
                        .children(undisplayed.into_iter().map(|input| {
                            let text = input.text.clone();
                            let status = if input.processing {
                                tr("已接收，正在处理")
                            } else if running {
                                tr("已入队，等待当前回复结束")
                            } else {
                                tr("已入队，等待继续处理")
                            };
                            let label = if input.text.is_empty() {
                                tr("附件消息").to_string()
                            } else {
                                input.text.clone()
                            };
                            let detail = if input.attachments.is_empty() {
                                status.to_string()
                            } else {
                                format!(
                                    "{} · {}: {}", status, tr("附件"),
                                    input.attachments.iter().map(|item| item.name.as_str())
                                        .collect::<Vec<_>>().join(if english() { ", " } else { "、" })
                                )
                            };
                            h_flex()
                                .id(SharedString::from(format!("received-input-{}", input.delivery_id)))
                                .role(Role::Group)
                                .aria_label(format!("{label} · {detail}"))
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
                                        .child(div().text_sm().truncate().child(label))
                                        .child(div().text_xs().text_color(Color::Muted.get(cx))
                                            .truncate().child(detail)),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "copy-queued-{}",
                                        input.delivery_id
                                    )))
                                    .ghost()
                                    .compact()
                                    .label(tr("复制"))
                                    .accessibility_label(tr("复制已接收消息"))
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
