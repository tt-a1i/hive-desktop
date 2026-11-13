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

fn meta_row(label: &'static str, value: &str, cx: &App) -> AnyElement {
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
                .child(value.to_string()),
        )
        .into_any_element()
}

impl HiveApp {
    pub fn render_interactions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self
            .session
            .as_ref()
            .map(|session| session.id.clone())
            .unwrap_or_default();
        v_flex().w_full().min_w_0().gap_3().children(
            self.interactions
                .sessions
                .get(&session)
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(i, card)| self.render_interaction_card(i, &session, card, cx)),
        )
    }
    fn render_interaction_card(
        &self,
        i: usize,
        session: &str,
        card: &InteractionCard,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        if card.is_claude() {
            return self
                .render_claude_interaction(i, session, card, cx)
                .into_any_element();
        }
        if card.is_pi() {
            return self
                .render_pi_interaction(i, session, card, cx)
                .into_any_element();
        }
        let method = card.raw.method.as_str();
        let questions = method == "item/tool/requestUserInput";
        let files = method == "item/fileChange/requestApproval";
        let waiting = card.waiting();
        let disabled = !waiting || !self.connected;
        let session = session.to_string();
        let id = card.raw.id.clone();
        let params = &card.raw.params;
        let command = params["command"].as_str().or_else(|| {
            card.raw
                .item
                .as_ref()
                .and_then(|item| item["command"].as_str())
        });
        let title = if questions {
            tr("Codex 需要你的回答")
        } else if files {
            tr("Codex 请求修改文件")
        } else {
            tr("Codex 请求执行命令")
        };
        v_flex()
            .id(("interaction-card", i))
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
                                    .child(title),
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
                            .child("Codex"),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .p_4()
                    .gap_3()
                    .when(!waiting && card.raw.has_native_identity(), |view| {
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
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(if card.submitting && card.raw.state != "responding" {
                                    tr("正在提交…")
                                } else {
                                    tr("已提交，等待 Codex 确认…")
                                }),
                        )
                    })
                    .when_some(command, |view, command| {
                        view.child(
                            div()
                                .p_3()
                                .rounded_xl()
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .bg(Color::Inset.get(cx))
                                .font_family(Theme::global(cx).mono_font_family.clone())
                                .text_xs()
                                .line_height(px(18.))
                                .text_color(Color::Text.get(cx))
                                .child(crate::text_reading::selectable_text(
                                    format!("interaction-command-{i}"),
                                    command.to_string(),
                                )),
                        )
                    })
                    .when_some(params["cwd"].as_str(), |view, cwd| {
                        view.child(meta_row(tr("工作目录"), cwd, cx))
                    })
                    .when_some(params["reason"].as_str(), |view, reason| {
                        view.child(meta_row(tr("理由"), reason, cx))
                    })
                    .when_some(
                        params["networkApprovalContext"]["host"].as_str(),
                        |view, host| view.child(meta_row(tr("网络主机"), host, cx)),
                    )
                    .when_some(
                        params
                            .get("additionalPermissions")
                            .filter(|value| !value.is_null()),
                        |view, permissions| {
                            view.child(
                                v_flex()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Color::Secondary.get(cx))
                                            .child(tr("本次请求的额外权限")),
                                    )
                                    .child(
                                        div()
                                            .p_2p5()
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(Color::Divider.get(cx))
                                            .bg(Color::Inset.get(cx))
                                            .font_family(Theme::global(cx).mono_font_family.clone())
                                            .text_xs()
                                            .text_color(Color::Secondary.get(cx))
                                            .child(
                                                serde_json::to_string_pretty(permissions)
                                                    .unwrap_or_else(|_| permissions.to_string()),
                                            ),
                                    ),
                            )
                        },
                    )
                    .when_some(params["grantRoot"].as_str(), |view, root| {
                        view.child(meta_row(tr("请求写入目录"), root, cx))
                    })
                    .when(files, |view| {
                        view.when(!card.has_diff(), |view| {
                            view.child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr("尚未收到 Codex 提供的原生文件差异，收到后才能批准。")),
                            )
                        })
                        .children(
                            card.raw
                                .item
                                .as_ref()
                                .and_then(|item| item["changes"].as_array())
                                .into_iter()
                                .flatten()
                                .enumerate()
                                .map(|(j, change)| {
                                    v_flex()
                                        .min_w_0()
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .bg(Color::Inset.get(cx))
                                        .overflow_hidden()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .justify_between()
                                                .gap_2()
                                                .px_3()
                                                .py_1p5()
                                                .border_b_1()
                                                .border_color(Color::Divider.get(cx))
                                                .bg(Color::Card.get(cx).opacity(0.6))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .truncate()
                                                        .text_xs()
                                                        .font_family(
                                                            Theme::global(cx)
                                                                .mono_font_family
                                                                .clone(),
                                                        )
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(
                                                            change["path"]
                                                                .as_str()
                                                                .unwrap_or(tr("未提供路径"))
                                                                .to_string(),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .text_size(px(11.))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(
                                                            match change["kind"]["type"].as_str() {
                                                                Some("add") => tr("新增"),
                                                                Some("delete") => tr("删除"),
                                                                Some("update") => tr("修改"),
                                                                _ => tr("变更"),
                                                            },
                                                        ),
                                                ),
                                        )
                                        .when_some(
                                            change["kind"]["move_path"].as_str(),
                                            |view, path| {
                                                view.child(
                                                    div()
                                                        .px_3()
                                                        .pt_1p5()
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(if crate::language::english() {
                                                            format!("{}: {path}", tr("移动至"))
                                                        } else {
                                                            format!("{}：{path}", tr("移动至"))
                                                        }),
                                                )
                                            },
                                        )
                                        .child(
                                            div()
                                                .id(("interaction-diff", i * 1000 + j))
                                                .max_h(px(260.))
                                                .overflow_y_scroll()
                                                .p_3()
                                                .font_family(
                                                    Theme::global(cx).mono_font_family.clone(),
                                                )
                                                .text_xs()
                                                .line_height(px(18.))
                                                .child(crate::text_reading::selectable_text(
                                                    format!("interaction-diff-text-{i}-{j}"),
                                                    change["diff"]
                                                        .as_str()
                                                        .unwrap_or(tr("Codex 未提供此文件的差异文本"))
                                                        .to_string(),
                                                )),
                                        )
                                }),
                        )
                    })
                    .when(questions && waiting, |view| {
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
                            h_flex()
                                .flex_wrap()
                                .gap_2()
                                .pt_1()
                                .child(
                                    Button::new(("submit-interaction-answers", i))
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
                                                this.answer_interaction(
                                                    session.clone(),
                                                    id.clone(),
                                                    None,
                                                    cx,
                                                )
                                            })
                                        }),
                                )
                                .child(
                                    Button::new(("cancel-interaction-questions", i))
                                        .custom(crate::palette::subtle_icon_variant(cx))
                                        .compact()
                                        .h(px(30.))
                                        .px_3()
                                        .rounded_lg()
                                        .label(tr("取消本轮"))
                                        .disabled(disabled)
                                        .on_click({
                                            let session = session.clone();
                                            let id = id.clone();
                                            cx.listener(move |this, _, _, cx| {
                                                this.cancel_interaction_turn(
                                                    session.clone(),
                                                    id.clone(),
                                                    cx,
                                                )
                                            })
                                        }),
                                ),
                        )
                    })
                    .when(!questions, |view| {
                        view.child(
                            h_flex().flex_wrap().gap_2().pt_1().children(
                                card.decisions().into_iter().enumerate().map(
                                    |(j, (decision, label))| {
                                        let session = session.clone();
                                        let id = id.clone();
                                        let primary = j == 0 && decision.starts_with("accept");
                                        let btn = Button::new(("interaction-decision", i * 3 + j))
                                            .compact()
                                            .h(px(30.))
                                            .px_3()
                                            .rounded_lg()
                                            .label(tr(label))
                                            .disabled(
                                                disabled
                                                    || (files
                                                        && decision == "accept"
                                                        && !card.has_diff()),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.answer_interaction(
                                                    session.clone(),
                                                    id.clone(),
                                                    Some(decision),
                                                    cx,
                                                )
                                            }));
                                        if primary {
                                            btn.primary()
                                        } else {
                                            btn.custom(crate::palette::subtle_icon_variant(cx))
                                        }
                                    },
                                ),
                            ),
                        )
                        .when(card.decisions().is_empty(), |view| {
                            view.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("Codex 未提供可用的单次决定。可使用输入区的停止按钮结束本轮。")),
                            )
                        })
                    }),
            )
            .into_any_element()
    }
}
