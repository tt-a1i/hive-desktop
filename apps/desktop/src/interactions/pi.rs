use super::*;
use crate::palette::Color;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::{Input, Textarea, TextareaState},
        *,
    },
    prelude::FluentBuilder,
};
use serde_json::json;

impl InteractionCard {
    pub(super) fn prepare_pi(&mut self, window: &mut Window, cx: &mut Context<HiveApp>) {
        if self.initialized || self.raw.state != "pending" {
            return;
        }
        self.initialized = true;
        let placeholder = self.raw.params["placeholder"]
            .as_str()
            .unwrap_or(tr("输入回答…"))
            .to_string();
        match self.raw.method.as_str() {
            "input" => {
                self.pi_input =
                    Some(cx.new(|cx| InputState::new(window, cx).placeholder(placeholder)));
            }
            "editor" => {
                let prefill = self.raw.params["prefill"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                self.pi_editor = Some(cx.new(|cx| {
                    let mut input = TextareaState::new(window, cx)
                        .auto_grow(4, 10)
                        .placeholder(placeholder);
                    input.set_value(prefill, window, cx);
                    input
                }));
            }
            _ => {}
        }
    }
}

impl HiveApp {
    pub(super) fn render_pi_interaction(
        &self,
        i: usize,
        session: &str,
        card: &InteractionCard,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let disabled = !card.waiting() || !self.connected;
        let session = session.to_string();
        let id = card.raw.id.clone();
        let text_entry = matches!(card.raw.method.as_str(), "input" | "editor");
        let mut actions: Vec<(String, serde_json::Value)> = vec![];
        if card.raw.method == "select" {
            actions.extend(
                card.raw.params["options"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|option| option.as_str())
                    .map(|option| (option.to_string(), json!({"value":option}))),
            );
        } else if card.raw.method == "confirm" {
            actions.push((tr("确认").into(), json!({"confirmed":true})));
            actions.push((tr("否").into(), json!({"confirmed":false})));
        }
        actions.push((tr("取消请求").into(), json!({"cancelled":true})));
        let action_count = actions.len();
        v_flex()
            .id(("pi-interaction-card", i))
            .w_full()
            .min_w_0()
            .rounded_2xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Card.get(cx))
            .shadow_xs()
            .overflow_hidden()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Inset.get(cx).opacity(0.6))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .size(px(7.))
                                    .rounded_full()
                                    .flex_shrink_0()
                                    .bg(rgb(if Theme::global(cx).is_dark() {
                                        0xfbbf24
                                    } else {
                                        0xd97706
                                    })),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .truncate()
                                    .child(tr("外部 Pi 需要你的回应")),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_2()
                            .py(px(2.))
                            .rounded_full()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .text_size(px(10.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Muted.get(cx))
                            .child("Pi"),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .p_4()
                    .gap_3()
                    .when_some(card.raw.params["title"].as_str(), |view, title| {
                        view.child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Color::Text.get(cx))
                                .child(title.to_string()),
                        )
                    })
                    .when_some(card.raw.params["message"].as_str(), |view, message| {
                        view.child(
                            div()
                                .text_sm()
                                .text_color(Color::Secondary.get(cx))
                                .child(message.to_string()),
                        )
                    })
                    .when(!card.waiting(), |view| {
                        view.child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(tr("正在发送回应…")),
                        )
                    })
                    .when_some(card.pi_input.as_ref(), |view, input| {
                        view.child(
                            Input::new(input)
                                .aria_label(tr("外部 Pi 回答"))
                                .disabled(disabled),
                        )
                    })
                    .when_some(card.pi_editor.as_ref(), |view, input| {
                        view.child(
                            Textarea::new(input)
                                .aria_label(tr("外部 Pi 编辑内容"))
                                .disabled(disabled),
                        )
                    })
                    .when_some(card.error.as_ref().map(|error| error.text()), |view, error| {
                        view.child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .border_1()
                                .border_color(Color::Error.get(cx).opacity(0.24))
                                .child(
                                    Icon::new(Glyph::CircleAlert)
                                        .size(px(14.))
                                        .text_color(Color::Error.get(cx)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Color::Error.get(cx))
                                        .child(error),
                                ),
                        )
                    })
                    .when(text_entry, |view| {
                        view.child(
                            h_flex().child(
                                Button::new(("pi-submit", i))
                                    .primary()
                                    .compact()
                                    .h(px(30.))
                                    .px_3()
                                    .rounded_lg()
                                    .label(tr("提交回答"))
                                    .disabled(disabled)
                                    .on_click({
                                        let session = session.clone();
                                        let id = id.clone();
                                        cx.listener(move |this, _, _, cx| {
                                            let Some(card) = this
                                                .interactions
                                                .sessions
                                                .get(&session)
                                                .and_then(|cards| {
                                                    cards.iter().find(|card| card.raw.id == id)
                                                })
                                            else {
                                                return;
                                            };
                                            let value = if let Some(input) = &card.pi_input {
                                                input.read(cx).value().to_string()
                                            } else if let Some(input) = &card.pi_editor {
                                                input.read(cx).value().to_string()
                                            } else {
                                                return;
                                            };
                                            this.submit_interaction(
                                                session.clone(),
                                                id.clone(),
                                                json!({"value":value}),
                                                cx,
                                            );
                                        })
                                    }),
                            ),
                        )
                    })
                    .child(
                        v_flex().w_full().gap_1p5().children(
                            actions.into_iter().enumerate().map(|(j, (label, response))| {
                                let session = session.clone();
                                let id = id.clone();
                                let is_cancel = j + 1 == action_count;
                                let is_confirm_primary =
                                    card.raw.method == "confirm" && j == 0 && !is_cancel;
                                let btn = Button::new(("pi-option", i * 1000 + j))
                                    .w_full()
                                    .compact()
                                    .h(px(32.))
                                    .px_3()
                                    .rounded_lg()
                                    .label(label)
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.submit_interaction(
                                            session.clone(),
                                            id.clone(),
                                            response.clone(),
                                            cx,
                                        )
                                    }));
                                if is_confirm_primary {
                                    btn.primary()
                                } else if is_cancel {
                                    btn.custom(crate::palette::quiet_icon_variant(cx))
                                } else {
                                    btn.custom(crate::palette::subtle_icon_variant(cx))
                                }
                            }),
                        ),
                    ),
            )
    }
}
