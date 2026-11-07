use super::*;
use gpui_kit::{
    component::{button::Button, input::Input, *},
    prelude::FluentBuilder,
};

impl HiveApp {
    pub(super) fn render_interaction_question(
        &self,
        i: usize,
        j: usize,
        session: &str,
        interaction: &str,
        draft: &QuestionDraft,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let session = session.to_string();
        let interaction = interaction.to_string();
        let question = &draft.question;
        let field = i * 1000 + j;
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(question.header.clone()),
            )
            .child(div().text_sm().child(question.question.clone()))
            .when(question.multi_select, |view| {
                view.child(div().text_xs().child("可选择多项，也可补充其他回答。"))
            })
            .children(
                question
                    .options
                    .iter()
                    .flatten()
                    .enumerate()
                    .map(|(k, option)| {
                        let session = session.clone();
                        let interaction = interaction.clone();
                        let question_id = question.id.clone();
                        let label = option.label.clone();
                        v_flex()
                            .gap_1()
                            .child(
                                Button::new(("question-option", field * 1000 + k))
                                    .w_full()
                                    .label(option.label.clone())
                                    .selected(
                                        !question.is_secret
                                            && (if question.multi_select {
                                                draft.multiple_selected.contains(&option.label)
                                            } else {
                                                !draft.other
                                                    && draft.selected.as_deref()
                                                        == Some(option.label.as_str())
                                            }),
                                    )
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if !this.connected
                                            || this
                                                .session
                                                .as_ref()
                                                .is_none_or(|current| current.id != session)
                                        {
                                            return;
                                        }
                                        if let Some(draft) = this
                                            .interactions
                                            .sessions
                                            .get_mut(&session)
                                            .and_then(|cards| {
                                                cards.iter_mut().find(|card| {
                                                    card.raw.id == interaction && card.waiting()
                                                })
                                            })
                                            .and_then(|card| {
                                                card.questions
                                                    .iter_mut()
                                                    .find(|draft| draft.question.id == question_id)
                                            })
                                        {
                                            if draft.question.is_secret {
                                                draft.input.update(cx, |input, cx| {
                                                    input.set_value(label.clone(), window, cx)
                                                });
                                                draft.other = true;
                                            } else if draft.question.multi_select {
                                                if draft.multiple_selected.contains(&label) {
                                                    draft
                                                        .multiple_selected
                                                        .retain(|selected| selected != &label);
                                                } else {
                                                    draft.multiple_selected.push(label.clone());
                                                }
                                            } else {
                                                draft.selected = Some(label.clone());
                                                draft.other = false;
                                            }
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(div().text_xs().child(option.description.clone()))
                            .when_some(option.preview.as_ref(), |view, preview| {
                                view.child(
                                    div()
                                        .id(("question-preview", field * 1000 + k))
                                        .max_h(px(160.))
                                        .overflow_y_scroll()
                                        .p_2()
                                        .bg(rgb(0xffffff))
                                        .text_xs()
                                        .child(format!("预览（原文，不执行）\n{preview}")),
                                )
                            })
                    }),
            )
            .when(
                question.is_other
                    && question
                        .options
                        .as_ref()
                        .is_some_and(|options| !options.is_empty())
                    && !question.is_secret,
                |view| {
                    view.child(
                        Button::new(("question-other", field))
                            .label("其他回答")
                            .selected(draft.other)
                            .disabled(disabled)
                            .on_click({
                                let question_id = question.id.clone();
                                cx.listener(move |this, _, _, cx| {
                                    if !this.connected
                                        || this
                                            .session
                                            .as_ref()
                                            .is_none_or(|current| current.id != session)
                                    {
                                        return;
                                    }
                                    if let Some(draft) = this
                                        .interactions
                                        .sessions
                                        .get_mut(&session)
                                        .and_then(|cards| {
                                            cards.iter_mut().find(|card| {
                                                card.raw.id == interaction && card.waiting()
                                            })
                                        })
                                        .and_then(|card| {
                                            card.questions
                                                .iter_mut()
                                                .find(|draft| draft.question.id == question_id)
                                        })
                                    {
                                        draft.other = if draft.question.multi_select {
                                            !draft.other
                                        } else {
                                            true
                                        };
                                    }
                                    cx.notify();
                                })
                            }),
                    )
                },
            )
            .when(draft.other, |view| {
                view.child(
                    Input::new(&draft.input)
                        .aria_label(if question.is_secret {
                            format!("{}，秘密回答", question.header)
                        } else {
                            question.header.clone()
                        })
                        .disabled(disabled),
                )
            })
            .when(question.is_secret, |view| {
                view.child(
                    div()
                        .text_xs()
                        .child("秘密回答将提交给 Codex，不另存为 Hive 聊天消息。"),
                )
            })
    }
}
