use super::*;
use crate::palette::Color;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{button::{Button, ButtonVariants}, input::Input, *},
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
            .p_3()
            .rounded_xl()
            .bg(Color::Inset.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Muted.get(cx))
                            .child(question.header.clone()),
                    )
                    .when(question.multi_select, |row| {
                        row.child(
                            div()
                                .text_size(px(11.))
                                .text_color(Color::Muted.get(cx))
                                .child(tr("可选择多项，也可补充其他回答。")),
                        )
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(Color::Text.get(cx))
                    .child(question.question.clone()),
            )
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
                        let is_selected = !question.is_secret
                            && (if question.multi_select {
                                draft.multiple_selected.contains(&option.label)
                            } else {
                                !draft.other
                                    && draft.selected.as_deref() == Some(option.label.as_str())
                            });
                        v_flex()
                            .rounded_xl()
                            .border_1()
                            .border_color(if is_selected {
                                Color::Accent.get(cx)
                            } else {
                                Color::Divider.get(cx)
                            })
                            .bg(if is_selected {
                                Color::AccentSoft.get(cx)
                            } else {
                                Color::Card.get(cx)
                            })
                            .overflow_hidden()
                            .child(
                                Button::new(("question-option", field * 1000 + k))
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .w_full()
                                    .h(px(34.))
                                    .px_3()
                                    .rounded_none()
                                    .justify_start()
                                    .selected(is_selected)
                                    .disabled(disabled)
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .items_center()
                                            .gap_2p5()
                                            .child(
                                                div()
                                                    .size(px(15.))
                                                    .flex_shrink_0()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .when(question.multi_select, |box_el| {
                                                        box_el.rounded_sm()
                                                    })
                                                    .when(!question.multi_select, |box_el| {
                                                        box_el.rounded_full()
                                                    })
                                                    .border_1()
                                                    .border_color(if is_selected {
                                                        Color::Accent.get(cx)
                                                    } else {
                                                        Color::Muted.get(cx).opacity(0.6)
                                                    })
                                                    .bg(if is_selected {
                                                        Color::Accent.get(cx)
                                                    } else {
                                                        gpui::rgba(0).into()
                                                    })
                                                    .when(is_selected, |indicator| {
                                                        indicator.child(
                                                            Icon::new(Glyph::Check)
                                                                .size(px(10.))
                                                                .text_color(gpui::white()),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(option.label.clone()),
                                            ),
                                    )
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
                            .when(!option.description.is_empty(), |card| {
                                card.child(
                                    div()
                                        .px_3()
                                        .pb_2p5()
                                        .pl_9()
                                        .text_xs()
                                        .text_color(Color::Secondary.get(cx))
                                        .child(option.description.clone()),
                                )
                            })
                            .when_some(option.preview.as_ref(), |view, preview| {
                                view.child(
                                    div()
                                        .id(("question-preview", field * 1000 + k))
                                        .mx_3()
                                        .mb_2p5()
                                        .max_h(px(160.))
                                        .overflow_y_scroll()
                                        .p_2p5()
                                        .rounded_lg()
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .bg(Color::Inset.get(cx))
                                        .font_family(Theme::global(cx).mono_font_family.clone())
                                        .text_xs()
                                        .text_color(Color::Secondary.get(cx))
                                        .child(format!(
                                            "{}\n{preview}",
                                            tr("预览（原文，不执行）")
                                        )),
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
                        div()
                            .rounded_xl()
                            .border_1()
                            .border_color(if draft.other {
                                Color::Accent.get(cx)
                            } else {
                                Color::Divider.get(cx)
                            })
                            .bg(if draft.other {
                                Color::AccentSoft.get(cx)
                            } else {
                                Color::Card.get(cx)
                            })
                            .overflow_hidden()
                            .child(
                                Button::new(("question-other", field))
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .w_full()
                                    .h(px(34.))
                                    .px_3()
                                    .rounded_none()
                                    .justify_start()
                                    .selected(draft.other)
                                    .disabled(disabled)
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .items_center()
                                            .gap_2p5()
                                            .child(
                                                div()
                                                    .size(px(15.))
                                                    .flex_shrink_0()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .when(question.multi_select, |box_el| {
                                                        box_el.rounded_sm()
                                                    })
                                                    .when(!question.multi_select, |box_el| {
                                                        box_el.rounded_full()
                                                    })
                                                    .border_1()
                                                    .border_color(if draft.other {
                                                        Color::Accent.get(cx)
                                                    } else {
                                                        Color::Muted.get(cx).opacity(0.6)
                                                    })
                                                    .bg(if draft.other {
                                                        Color::Accent.get(cx)
                                                    } else {
                                                        gpui::rgba(0).into()
                                                    })
                                                    .when(draft.other, |indicator| {
                                                        indicator.child(
                                                            Icon::new(Glyph::Check)
                                                                .size(px(10.))
                                                                .text_color(gpui::white()),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(tr("其他回答")),
                                            ),
                                    )
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
                                                    card.questions.iter_mut().find(|draft| {
                                                        draft.question.id == question_id
                                                    })
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
                            ),
                    )
                },
            )
            .when(draft.other, |view| {
                view.child(
                    Input::new(&draft.input)
                        .aria_label(if question.is_secret {
                            if english() {
                                format!("{}, secret answer", question.header)
                            } else {
                                format!("{}，秘密回答", question.header)
                            }
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
                        .text_color(Color::Muted.get(cx))
                        .child(tr("秘密回答将提交给 Codex，不另存为 Hive 聊天消息。")),
                )
            })
    }
}
