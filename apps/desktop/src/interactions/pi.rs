use super::*;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::Button,
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
            .unwrap_or("输入回答…")
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
            actions.push(("确认".into(), json!({"confirmed":true})));
            actions.push(("否".into(), json!({"confirmed":false})));
        }
        actions.push(("取消请求".into(), json!({"cancelled":true})));
        v_flex()
            .id(("pi-interaction-card", i))
            .w_full()
            .min_w_0()
            .gap_3()
            .p_4()
            .rounded_lg()
            .bg(Color::Attention.get(cx))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("外部 Pi 需要你的回应"),
            )
            .when_some(card.raw.params["title"].as_str(), |view, title| {
                view.child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string()),
                )
            })
            .when_some(card.raw.params["message"].as_str(), |view, message| {
                view.child(div().text_sm().child(message.to_string()))
            })
            .when(!card.waiting(), |view| {
                view.child(div().text_sm().child("正在发送回应…"))
            })
            .when_some(card.pi_input.as_ref(), |view, input| {
                view.child(
                    Input::new(input)
                        .aria_label("外部 Pi 回答")
                        .disabled(disabled),
                )
            })
            .when_some(card.pi_editor.as_ref(), |view, input| {
                view.child(
                    Textarea::new(input)
                        .aria_label("外部 Pi 编辑内容")
                        .disabled(disabled),
                )
            })
            .when_some(card.error.clone(), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when(text_entry, |view| {
                view.child(
                    Button::new(("pi-submit", i))
                        .label("提交回答")
                        .disabled(disabled)
                        .on_click({
                            let session = session.clone();
                            let id = id.clone();
                            cx.listener(move |this, _, _, cx| {
                                let Some(card) =
                                    this.interactions.sessions.get(&session).and_then(|cards| {
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
                )
            })
            .child(
                v_flex()
                    .w_full()
                    .gap_2()
                    .children(
                        actions
                            .into_iter()
                            .enumerate()
                            .map(|(j, (label, response))| {
                                let session = session.clone();
                                let id = id.clone();
                                Button::new(("pi-option", i * 1000 + j))
                                    .w_full()
                                    .label(label)
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.submit_interaction(
                                            session.clone(),
                                            id.clone(),
                                            response.clone(),
                                            cx,
                                        )
                                    }))
                            }),
                    ),
            )
    }
}
