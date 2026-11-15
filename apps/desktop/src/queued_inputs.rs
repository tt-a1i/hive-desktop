mod editor;
use crate::{
    app::HiveApp,
    language::{english, tr},
    palette::Color,
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

impl HiveApp {
    pub(crate) fn render_pending_user_inputs(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let running = self
            .session
            .as_ref()
            .is_some_and(|session| self.busy.contains(&session.id));
        let undisplayed: Vec<_> = self
            .pending_user_inputs
            .iter()
            .filter(|input| {
                !self
                    .messages
                    .iter()
                    .any(|message| message.id == input.delivery_id)
            })
            .collect();
        if undisplayed.is_empty() {
            return None;
        }
        let count = undisplayed.len();
        Some(
            v_flex()
                .w_full()
                .min_w_0()
                .gap_1p5()
                .p_2p5()
                .rounded_xl()
                .bg(Color::Inset.get(cx))
                .border_1()
                .border_color(Color::Divider.get(cx))
                .child(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .px_0p5()
                        .child(
                            div()
                                .size(px(6.))
                                .rounded_full()
                                .flex_shrink_0()
                                .bg(if running {
                                    Color::Accent.get(cx)
                                } else {
                                    Color::Muted.get(cx)
                                }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Color::Secondary.get(cx))
                                .child(tr("已接收消息")),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py(px(1.))
                                .rounded_full()
                                .bg(Color::Card.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .text_size(px(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Color::Muted.get(cx))
                                .child(count.to_string()),
                        ),
                )
                .child(v_flex().id("queued-user-inputs").gap_1p5().children(
                    undisplayed.into_iter().map(|input| {
                        let text = input.text.clone();
                        let edit_input = input.clone();
                        let cancel_input = input.clone();
                        let status = match input.reason.as_deref() {
                            Some("completed") => tr("消息已处理，不能编辑或撤回"),
                            Some("attempted") => tr("消息已尝试发送，不能编辑或撤回"),
                            Some("legacy") => tr("旧版待发消息缺少原始记录，不能编辑或撤回"),
                            _ if input.processing => tr("已接收，正在处理"),
                            _ if running => tr("已入队，等待当前回复结束"),
                            _ => tr("已入队，等待继续处理"),
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
                                "{} · {}: {}",
                                status,
                                tr("附件"),
                                input
                                    .attachments
                                    .iter()
                                    .map(|item| item.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(if english() { ", " } else { "、" })
                            )
                        };
                        crate::focus_scroll::Reveal::new(
                            SharedString::from(format!(
                                "received-input-reveal-{}",
                                input.delivery_id
                            )),
                            &self.composer_scroll,
                            h_flex()
                                .id(SharedString::from(format!(
                                    "received-input-{}",
                                    input.delivery_id
                                )))
                                .role(Role::Group)
                                .aria_label(format!("{label} · {detail}"))
                                .items_center()
                                .min_w_0()
                                .gap_2()
                                .px_2p5()
                                .py_1p5()
                                .rounded_lg()
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .bg(Color::Card.get(cx))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .truncate()
                                                .child(label),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(Color::Muted.get(cx))
                                                .truncate()
                                                .child(detail),
                                        ),
                                )
                                .when(input.editable && !input.processing, |row| {
                                    row.child(
                                        Button::new(SharedString::from(format!(
                                            "edit-queued-{}",
                                            input.delivery_id
                                        )))
                                        .custom(crate::palette::subtle_icon_variant(cx))
                                        .compact()
                                        .h(px(24.))
                                        .w(px(24.))
                                        .rounded_md()
                                        .icon(Glyph::Pencil)
                                        .tooltip(tr("编辑待发消息"))
                                        .accessibility_label(tr("编辑待发消息"))
                                        .disabled(!self.connected || self.closing)
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.open_queue_editor(
                                                    edit_input.clone(),
                                                    false,
                                                    window,
                                                    cx,
                                                )
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "cancel-queued-{}",
                                            input.delivery_id
                                        )))
                                        .custom(crate::palette::subtle_icon_variant(cx))
                                        .compact()
                                        .h(px(24.))
                                        .w(px(24.))
                                        .rounded_md()
                                        .icon(Glyph::X)
                                        .tooltip(tr("撤回待发消息"))
                                        .accessibility_label(tr("撤回待发消息"))
                                        .disabled(!self.connected || self.closing)
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                if crate::general::current(cx)
                                                    .confirm_cancel_queued_input
                                                {
                                                    this.open_queue_editor(
                                                        cancel_input.clone(),
                                                        true,
                                                        window,
                                                        cx,
                                                    );
                                                } else {
                                                    this.cancel_queued_input_now(
                                                        cancel_input.clone(),
                                                        cx,
                                                    );
                                                }
                                            }),
                                        ),
                                    )
                                })
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "copy-queued-{}",
                                        input.delivery_id
                                    )))
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .h(px(24.))
                                    .w(px(24.))
                                    .rounded_md()
                                    .icon(Glyph::Copy)
                                    .tooltip(tr("复制"))
                                    .accessibility_label(tr("复制已接收消息"))
                                    .disabled(input.text.is_empty())
                                    .on_click(
                                        move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                text.clone(),
                                            ))
                                        },
                                    ),
                                ),
                        )
                    }),
                ))
                .into_any_element(),
        )
    }

    pub(super) fn cancel_queued_input_now(
        &mut self,
        input: crate::protocol::PendingUserInput,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self.closing
            || !self.workspace_writes_allowed()
            || !input.editable
            || input.processing
        {
            return;
        }
        let Some(session_id) = self.session.as_ref().map(|session| session.id.clone()) else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.queue.cancel",
                    serde_json::json!({
                        "sessionId": session_id,
                        "deliveryId": input.delivery_id,
                        "expectedText": input.text,
                    }),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                match result {
                    Ok(value) if value["ok"] == true => {}
                    Ok(_) => {
                        this.error = Some("后台未确认待发消息操作。".into());
                    }
                    Err(error) => {
                        this.error = Some(crate::app::ErrorNotice::Detail(
                            "撤回待发消息失败",
                            error,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
