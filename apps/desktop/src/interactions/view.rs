use super::*;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
};

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
        v_flex()
            .id(("interaction-card", i))
            .w_full()
            .min_w_0()
            .gap_3()
            .p_4()
            .rounded_xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Attention.get(cx))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(if questions {
                        tr("Codex 需要你的回答")
                    } else if files {
                        tr("Codex 请求修改文件")
                    } else {
                        tr("Codex 请求执行命令")
                    }),
            )
            .when(!waiting && card.raw.has_native_identity(), |view| {
                view.child(div().text_sm().child(
                    if card.submitting && card.raw.state != "responding" {
                        tr("正在提交…")
                    } else {
                        tr("已提交，等待 Codex 确认…")
                    },
                ))
            })
            .when_some(command, |view, command| {
                view.child(
                    div()
                        .p_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .bg(Color::Card.get(cx))
                        .font_family(Theme::global(cx).mono_font_family.clone())
                        .text_sm()
                        .child(crate::text_reading::selectable_text(
                            format!("interaction-command-{i}"),
                            command.to_string(),
                        )),
                )
            })
            .when_some(params["cwd"].as_str(), |view, cwd| {
                view.child(div().text_sm().child(if crate::language::english() {
                    format!("{}: {cwd}", tr("工作目录"))
                } else {
                    format!("{}：{cwd}", tr("工作目录"))
                }))
            })
            .when_some(params["reason"].as_str(), |view, reason| {
                view.child(div().text_sm().child(if crate::language::english() {
                    format!("{}: {reason}", tr("理由"))
                } else {
                    format!("{}：{reason}", tr("理由"))
                }))
            })
            .when_some(
                params["networkApprovalContext"]["host"].as_str(),
                |view, host| {
                    view.child(div().text_sm().child(if crate::language::english() {
                        format!("{}: {host}", tr("网络主机"))
                    } else {
                        format!("{}：{host}", tr("网络主机"))
                    }))
                },
            )
            .when_some(
                params
                    .get("additionalPermissions")
                    .filter(|value| !value.is_null()),
                |view, permissions| {
                    view.child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr("本次请求的额外权限")),
                            )
                            .child(
                                div().text_sm().child(
                                    serde_json::to_string_pretty(permissions)
                                        .unwrap_or_else(|_| permissions.to_string()),
                                ),
                            ),
                    )
                },
            )
            .when_some(params["grantRoot"].as_str(), |view, root| {
                view.child(div().text_sm().child(if crate::language::english() {
                    format!("{}: {root}", tr("请求写入目录"))
                } else {
                    format!("{}：{root}", tr("请求写入目录"))
                }))
            })
            .when(files, |view| {
                view.when(!card.has_diff(), |view| {
                    view.child(
                        div()
                            .text_sm()
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
                                .gap_2()
                                .min_w_0()
                                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(
                                    format!(
                                        "{} · {}",
                                        change["path"].as_str().unwrap_or(tr("未提供路径")),
                                        match change["kind"]["type"].as_str() {
                                            Some("add") => tr("新增"),
                                            Some("delete") => tr("删除"),
                                            Some("update") => tr("修改"),
                                            _ => tr("变更"),
                                        }
                                    ),
                                ))
                                .when_some(change["kind"]["move_path"].as_str(), |view, path| {
                                    view.child(div().text_sm().child(if crate::language::english() {
                                        format!("{}: {path}", tr("移动至"))
                                    } else {
                                        format!("{}：{path}", tr("移动至"))
                                    }))
                                })
                                .child(
                                    div()
                                        .id(("interaction-diff", i * 1000 + j))
                                        .max_h(px(260.))
                                        .overflow_y_scroll()
                                        .p_3()
                                        .rounded_lg()
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .bg(Color::Card.get(cx))
                                        .font_family(Theme::global(cx).mono_font_family.clone())
                                        .text_xs()
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
                    self.render_interaction_question(i, j, &session, &id, draft, disabled, cx)
                }))
            })
            .when_some(card.error.as_ref().map(|error|error.text()), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when(questions, |view| {
                view.child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new(("submit-interaction-answers", i))
                                .primary()
                                .rounded_full()
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
                                .rounded_full()
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
                    h_flex().flex_wrap().gap_2().children(
                        card.decisions()
                            .into_iter()
                            .enumerate()
                            .map(|(j, (decision, label))| {
                                let session = session.clone();
                                let id = id.clone();
                                let primary = j == 0 && decision.starts_with("accept");
                                Button::new(("interaction-decision", i * 3 + j))
                                    .when(primary, |btn| btn.primary())
                                    .rounded_full()
                                    .label(tr(label))
                                    .disabled(
                                        disabled
                                            || (files && decision == "accept" && !card.has_diff()),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.answer_interaction(
                                            session.clone(),
                                            id.clone(),
                                            Some(decision),
                                            cx,
                                        )
                                    }))
                            }),
                    ),
                )
                .when(card.decisions().is_empty(), |view| {
                    view.child(
                        div()
                            .text_sm()
                            .child(tr("Codex 未提供可用的单次决定。可使用输入区的停止按钮结束本轮。")),
                    )
                })
            })
            .into_any_element()
    }
}
