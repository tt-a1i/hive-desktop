use super::*;
use crate::palette::Color;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
};
use serde_json::json;

impl InteractionCard {
    pub(super) fn prepare_claude(&mut self, window: &mut Window, cx: &mut Context<HiveApp>) {
        if self.initialized || !self.raw.has_native_identity() || self.raw.state != "pending" {
            return;
        }
        self.initialized = true;
        if self.raw.method != "ask-user-question" {
            return;
        }
        let parsed =
            serde_json::from_value::<Vec<Question>>(self.raw.params["input"]["questions"].clone());
        match parsed {
            Ok(questions)
                if (1..=4).contains(&questions.len())
                    && questions.iter().all(|q| {
                        !q.question.trim().is_empty()
                            && q.options
                                .as_ref()
                                .is_some_and(|options| (2..=4).contains(&options.len()))
                    }) =>
            {
                for mut question in questions {
                    question.id = question.question.clone();
                    question.is_other = true;
                    question.is_secret = false;
                    let input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("其他回答…")));
                    let subscription =
                        cx.subscribe_in(&input, window, |_, _, event: &InputEvent, _, cx| {
                            if matches!(event, InputEvent::Change) {
                                cx.notify();
                            }
                        });
                    self.questions.push(QuestionDraft {
                        question,
                        input,
                        selected: None,
                        multiple_selected: vec![],
                        other: false,
                        _subscription: subscription,
                    });
                }
            }
            _ => self.error = Some("无法读取 Claude 问题，请取消请求或停止本轮。".into()),
        }
    }
}

impl HiveApp {
    fn answer_claude_questions(&mut self, session: String, id: String, cx: &mut Context<Self>) {
        let Some(card) = self
            .interactions
            .sessions
            .get_mut(&session)
            .and_then(|cards| cards.iter_mut().find(|card| card.raw.id == id))
        else {
            return;
        };
        if !card.waiting() || !card.is_claude() || card.questions.is_empty() {
            return;
        }
        let mut answers = serde_json::Map::new();
        for draft in &card.questions {
            let mut values = if draft.question.multi_select {
                draft.multiple_selected.clone()
            } else if draft.other {
                vec![]
            } else {
                draft.selected.iter().cloned().collect()
            };
            if draft.other {
                let other = draft.input.read(cx).value().to_string();
                if other.trim().is_empty() {
                    card.error = Some("请填写其他回答，或取消选择其他回答。".into());
                    cx.notify();
                    return;
                }
                if !values.contains(&other) {
                    values.push(other);
                }
            }
            if values.is_empty() {
                card.error = Some("请回答每个问题；没有默认选择。".into());
                cx.notify();
                return;
            }
            answers.insert(draft.question.question.clone(), json!(values));
        }
        self.submit_interaction(session, id, json!({"answers":answers}), cx);
    }

    pub(super) fn render_claude_interaction(
        &self,
        i: usize,
        session: &str,
        card: &InteractionCard,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let questions = card.raw.method == "ask-user-question";
        let disabled = !self.connected || !card.waiting();
        let session = session.to_string();
        let id = card.raw.id.clone();
        let params = &card.raw.params;
        let actions = if questions {
            vec![("cancel", tr("取消请求"))]
        } else if params["defaultToNo"] == true {
            vec![
                ("decline", tr("拒绝本次")),
                ("accept", tr("仅允许本次")),
                ("cancel", tr("取消请求")),
            ]
        } else {
            vec![
                ("accept", tr("仅允许本次")),
                ("decline", tr("拒绝本次")),
                ("cancel", tr("取消请求")),
            ]
        };
        v_flex()
            .id(("claude-interaction", i))
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
                                    .bg(Color::Attention.get(cx)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .truncate()
                                    .child(if questions {
                                        tr("Claude 需要你的回答")
                                    } else {
                                        tr("Claude 请求使用工具")
                                    }),
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
                            .child("Claude"),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .p_4()
                    .gap_3()
                    .children(
                        [
                            ("title", tr("标题")),
                            ("displayName", tr("名称")),
                            ("description", tr("说明")),
                            ("toolName", tr("工具")),
                            ("decisionReason", tr("请求原因")),
                            ("blockedPath", tr("受限路径")),
                        ]
                        .into_iter()
                        .filter_map(|(key, label)| {
                            params[key].as_str().map(|text| {
                                h_flex()
                                    .min_w_0()
                                    .items_baseline()
                                    .gap_2()
                                    .text_xs()
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Color::Muted.get(cx))
                                            .child(label),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_color(Color::Secondary.get(cx))
                                            .child(text.to_string()),
                                    )
                            })
                        }),
                    )
                    .when_some(params["mcpServer"].as_object(), |view, _| {
                        view.child(
                            h_flex()
                                .min_w_0()
                                .items_baseline()
                                .gap_2()
                                .text_xs()
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Color::Muted.get(cx))
                                        .child("MCP"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_color(Color::Secondary.get(cx))
                                        .child(format!("{}", params["mcpServer"])),
                                ),
                        )
                    })
                    .when(params["defaultToNo"] == true, |view| {
                        view.child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .child(
                                    Icon::new(Glyph::Info)
                                        .size(px(13.))
                                        .text_color(Color::Attention.get(cx)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Color::Secondary.get(cx))
                                        .child(tr("Claude 建议默认拒绝；请明确决定，本界面不会自动批准。")),
                                ),
                        )
                    })
                    .when(!card.waiting() && card.raw.has_native_identity(), |view| {
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
                                .child(tr("正在提交回应…")),
                        )
                    })
                    .when(!questions, |view| {
                        view.child(
                            v_flex()
                                .min_w_0()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("原生工具输入（完整原文）")),
                                )
                                .children(
                                    params["input"]
                                        .as_object()
                                        .into_iter()
                                        .flat_map(|input| input.iter())
                                        .filter_map(|(key, value)| {
                                            value.as_str().map(|text| (key, text))
                                        })
                                        .enumerate()
                                        .map(|(j, (key, text))| {
                                            v_flex()
                                                .min_w_0()
                                                .rounded_xl()
                                                .border_1()
                                                .border_color(Color::Divider.get(cx))
                                                .bg(Color::Inset.get(cx))
                                                .overflow_hidden()
                                                .child(
                                                    div()
                                                        .px_3()
                                                        .py_1p5()
                                                        .border_b_1()
                                                        .border_color(Color::Divider.get(cx))
                                                        .bg(Color::Card.get(cx).opacity(0.6))
                                                        .text_xs()
                                                        .font_family(
                                                            Theme::global(cx)
                                                                .mono_font_family
                                                                .clone(),
                                                        )
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Secondary.get(cx))
                                                        .child(key.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .id(("claude-input-field", i * 1000 + j))
                                                        .max_h(px(200.))
                                                        .overflow_y_scroll()
                                                        .p_2p5()
                                                        .font_family(
                                                            Theme::global(cx)
                                                                .mono_font_family
                                                                .clone(),
                                                        )
                                                        .text_xs()
                                                        .line_height(px(18.))
                                                        .child(crate::text_reading::selectable_text(
                                                            format!(
                                                                "claude-input-field-text-{i}-{j}"
                                                            ),
                                                            text.to_string(),
                                                        )),
                                                )
                                        }),
                                )
                                .child(
                                    div()
                                        .id(("claude-native-input", i))
                                        .max_h(px(240.))
                                        .overflow_y_scroll()
                                        .p_3()
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .bg(Color::Inset.get(cx))
                                        .font_family(Theme::global(cx).mono_font_family.clone())
                                        .text_xs()
                                        .line_height(px(18.))
                                        .child(crate::text_reading::selectable_text(
                                            format!("claude-native-input-text-{i}"),
                                            serde_json::to_string_pretty(&params["input"])
                                                .unwrap_or_else(|_| params["input"].to_string()),
                                        )),
                                ),
                        )
                    })
                    .when(questions && card.waiting(), |view| {
                        view.children(card.questions.iter().enumerate().map(|(j, draft)| {
                            self.render_interaction_question(
                                i, j, &session, &id, draft, disabled, cx,
                            )
                        }))
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
                    .when(questions, |view| {
                        view.child(
                            h_flex().pt_1().child(
                                Button::new(("claude-answer", i))
                                    .primary()
                                    .compact()
                                    .h(px(30.))
                                    .px_3()
                                    .rounded_lg()
                                    .label(tr("提交回答"))
                                    .disabled(disabled || card.questions.is_empty())
                                    .on_click({
                                        let session = session.clone();
                                        let id = id.clone();
                                        cx.listener(move |this, _, _, cx| {
                                            this.answer_claude_questions(
                                                session.clone(),
                                                id.clone(),
                                                cx,
                                            )
                                        })
                                    }),
                            ),
                        )
                    })
                    .child(
                        h_flex().flex_wrap().gap_2().pt_1().children(
                            actions
                                .into_iter()
                                .enumerate()
                                .map(|(j, (decision, label))| {
                                    let session = session.clone();
                                    let id = id.clone();
                                    let primary = j == 0 && decision == "accept";
                                    let btn = Button::new(("claude-decision", i * 3 + j))
                                        .compact()
                                        .h(px(30.))
                                        .px_3()
                                        .rounded_lg()
                                        .label(tr(label))
                                        .disabled(disabled)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.submit_interaction(
                                                session.clone(),
                                                id.clone(),
                                                json!({"decision":decision}),
                                                cx,
                                            )
                                        }));
                                    if primary {
                                        btn.primary()
                                    } else {
                                        btn.custom(crate::palette::subtle_icon_variant(cx))
                                    }
                                }),
                        ),
                    ),
            )
    }
}
