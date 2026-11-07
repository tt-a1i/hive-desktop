use super::*;
use crate::palette::Color;
use gpui_kit::{
    component::{button::Button, *},
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
                    let input = cx.new(|cx| InputState::new(window, cx).placeholder("其他回答…"));
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
            vec![("cancel", "取消请求")]
        } else if params["defaultToNo"] == true {
            vec![
                ("decline", "拒绝本次"),
                ("accept", "仅允许本次"),
                ("cancel", "取消请求"),
            ]
        } else {
            vec![
                ("accept", "仅允许本次"),
                ("decline", "拒绝本次"),
                ("cancel", "取消请求"),
            ]
        };
        v_flex()
            .id(("claude-interaction", i))
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
                    .child(if questions {
                        "Claude 需要你的回答"
                    } else {
                        "Claude 请求使用工具"
                    }),
            )
            .children(
                [
                    ("title", "标题"),
                    ("displayName", "名称"),
                    ("description", "说明"),
                    ("toolName", "工具"),
                    ("decisionReason", "请求原因"),
                    ("blockedPath", "受限路径"),
                ]
                .into_iter()
                .filter_map(|(key, label)| {
                    params[key]
                        .as_str()
                        .map(|text| div().text_sm().child(format!("{label}：{text}")))
                }),
            )
            .when_some(params["mcpServer"].as_object(), |view, _| {
                view.child(
                    div()
                        .text_sm()
                        .child(format!("MCP：{}", params["mcpServer"])),
                )
            })
            .when(params["defaultToNo"] == true, |view| {
                view.child(
                    div()
                        .text_sm()
                        .child("Claude 建议默认拒绝；请明确决定，本界面不会自动批准。"),
                )
            })
            .when(!card.waiting() && card.raw.has_native_identity(), |view| {
                view.child(div().text_sm().child("正在提交回应…"))
            })
            .when(!questions, |view| {
                view.child(
                    v_flex()
                        .min_w_0()
                        .gap_2()
                        .child(div().text_sm().child("原生工具输入（完整原文）"))
                        .children(
                            params["input"]
                                .as_object()
                                .into_iter()
                                .flat_map(|input| input.iter())
                                .filter_map(|(key, value)| value.as_str().map(|text| (key, text)))
                                .enumerate()
                                .map(|(j, (key, text))| {
                                    v_flex()
                                        .min_w_0()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(key.clone()),
                                        )
                                        .child(
                                            div()
                                                .id(("claude-input-field", i * 1000 + j))
                                                .max_h(px(200.))
                                                .overflow_y_scroll()
                                                .p_2()
                                                .bg(Color::Card.get(cx))
                                                .text_sm()
                                                .child(text.to_string()),
                                        )
                                }),
                        )
                        .child(
                            div()
                                .id(("claude-native-input", i))
                                .max_h(px(240.))
                                .overflow_y_scroll()
                                .p_3()
                                .bg(Color::Card.get(cx))
                                .text_sm()
                                .child(
                                    serde_json::to_string_pretty(&params["input"])
                                        .unwrap_or_else(|_| params["input"].to_string()),
                                ),
                        ),
                )
            })
            .when(questions && card.waiting(), |view| {
                view.children(card.questions.iter().enumerate().map(|(j, draft)| {
                    self.render_interaction_question(i, j, &session, &id, draft, disabled, cx)
                }))
            })
            .when_some(card.error.clone(), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when(questions, |view| {
                view.child(
                    Button::new(("claude-answer", i))
                        .label("提交回答")
                        .disabled(disabled || card.questions.is_empty())
                        .on_click({
                            let session = session.clone();
                            let id = id.clone();
                            cx.listener(move |this, _, _, cx| {
                                this.answer_claude_questions(session.clone(), id.clone(), cx)
                            })
                        }),
                )
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .children(
                        actions
                            .into_iter()
                            .enumerate()
                            .map(|(j, (decision, label))| {
                                let session = session.clone();
                                let id = id.clone();
                                Button::new(("claude-decision", i * 3 + j))
                                    .label(label)
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.submit_interaction(
                                            session.clone(),
                                            id.clone(),
                                            json!({"decision":decision}),
                                            cx,
                                        )
                                    }))
                            }),
                    ),
            )
    }
}
