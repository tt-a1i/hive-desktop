use super::Dispatch;
use crate::focus_scroll::Reveal;
use crate::{
    app::HiveApp,
    language::{english, tr},
    palette::Color,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

fn excerpt(text: &str) -> String {
    let mut chars = text.chars();
    let prefix: String = chars.by_ref().take(320).collect();
    let preview = prefix.split_whitespace().collect::<Vec<_>>().join(" ");
    if chars.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

impl HiveApp {
    pub(crate) fn render_dispatch_card(
        &self,
        index: usize,
        dispatch: &Dispatch,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let item = dispatch.clone();
        let owner = self.agent_name(&dispatch.to_agent_id);
        let state = super::dispatch_state(&dispatch.state);
        let (_badge_bg, badge_fg, badge_dot) = super::dispatch_badge_colors(&dispatch.state, cx);
        let dot_color = badge_dot.unwrap_or_else(|| match dispatch.state.as_str() {
            "queued" | "submitted" => crate::chrome::attention_dot(cx),
            _ => Color::Muted.get(cx),
        });
        let is_selected = self
            .collaboration
            .selected
            .as_ref()
            .is_some_and(|selected| selected.id == dispatch.id);
        let text = excerpt(&dispatch.text);
        let is_unread_report = dispatch.state == "submitted";
        let assignee_member = self.members.iter().find(|m| m.id == dispatch.to_agent_id);
        let assignee_worktree = assignee_member
            .and_then(|m| m.desktop_activity.as_ref())
            .and_then(|a| a.worktree_branch.as_deref())
            .filter(|b| !b.is_empty())
            .map(str::to_owned);
        let assignee_session = self.member_session_id(&dispatch.to_agent_id);
        let owner_for_button = owner.clone();
        Reveal::new(
            format!("dispatch-{}-reveal", dispatch.id),
            &self.collaboration_scroll,
            v_flex()
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .rounded_xl()
                .bg(if is_selected {
                    Color::Inset.get(cx)
                } else {
                    Color::Card.get(cx)
                })
                .border_1()
                .border_color(if is_selected {
                    Color::Accent.get(cx)
                } else if is_unread_report {
                    crate::chrome::attention_dot(cx).opacity(0.6)
                } else {
                    Color::Divider.get(cx)
                })
                .child(
                    Button::new(("dispatch", index))
                        .ghost()
                        .w_full()
                        .min_w_0()
                        .h_auto()
                        .p_3()
                        .rounded_xl()
                        .justify_start()
                        .accessibility_label(format!(
                            "{}: {state} · {owner} · {}",
                            tr("打开派单"),
                            text.chars().take(160).collect::<String>()
                        ))
                        .child(
                            v_flex()
                                .flex_1()
                                .w_full()
                                .min_w_0()
                                .overflow_hidden()
                                .items_start()
                                .gap_1p5()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            h_flex()
                                                .flex_shrink_0()
                                                .items_center()
                                                .gap(px(5.))
                                                .rounded_md()
                                                .bg(Color::Inset.get(cx))
                                                .border_1()
                                                .border_color(Color::Divider.get(cx))
                                                .text_color(badge_fg)
                                                .px_2()
                                                .py(px(1.))
                                                .text_size(px(11.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(
                                                    div()
                                                        .size(px(6.))
                                                        .rounded_full()
                                                        .bg(dot_color),
                                                )
                                                .child(state.to_owned()),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_size(px(11.5))
                                                .text_color(Color::Muted.get(cx))
                                                .child(format!("{} · {owner}", tr("负责人"))),
                                        )
                                        .when(is_unread_report, |row| {
                                            row.child(
                                                div()
                                                    .flex_shrink_0()
                                                    .px_1p5()
                                                    .py(px(1.))
                                                    .rounded_sm()
                                                    .bg(crate::chrome::attention_dot(cx).opacity(0.14))
                                                    .text_size(px(10.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(crate::chrome::attention_dot(cx))
                                                    .child(tr("待阅汇报").to_owned()),
                                            )
                                        })
                                        .when_some(assignee_worktree, |row, branch| {
                                            row.child(
                                                div()
                                                    .flex_shrink_0()
                                                    .px_1p5()
                                                    .py(px(1.))
                                                    .rounded_sm()
                                                    .bg(Color::Inset.get(cx))
                                                    .border_1()
                                                    .border_color(Color::Divider.get(cx))
                                                    .text_size(px(10.5))
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(format!("worktree · {branch}")),
                                            )
                                        })
                                        .when(!dispatch.artifacts.is_empty(), |row| {
                                            row.child(
                                                div()
                                                    .flex_shrink_0()
                                                    .text_size(px(11.))
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(if english() {
                                                        format!(
                                                            "{} {}",
                                                            dispatch.artifacts.len(),
                                                            if dispatch.artifacts.len() == 1 {
                                                                "artifact"
                                                            } else {
                                                                "artifacts"
                                                            }
                                                        )
                                                    } else {
                                                        format!(
                                                            "{} 项产物",
                                                            dispatch.artifacts.len()
                                                        )
                                                    }),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .min_w_0()
                                        .whitespace_normal()
                                        .line_clamp(2)
                                        .text_left()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Color::Text.get(cx))
                                        .child(text),
                                )
                                .when_some(
                                    dispatch
                                        .report_text
                                        .as_ref()
                                        .filter(|text| !text.trim().is_empty()),
                                    |view, report| {
                                        view.child(
                                            div()
                                                .w_full()
                                                .min_w_0()
                                                .overflow_hidden()
                                                .truncate()
                                                .text_left()
                                                .text_xs()
                                                .text_color(Color::Muted.get(cx))
                                                .child(format!(
                                                    "{} · {}",
                                                    if dispatch.state == "cancelled" {
                                                        tr("取消原因")
                                                    } else {
                                                        tr("汇报")
                                                    },
                                                    excerpt(report)
                                                )),
                                        )
                                    },
                                ),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_dispatch(item.clone(), cx)
                        })),
                )
                .when_some(assignee_session, |card, sid| {
                    card.child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_end()
                            .px_3()
                            .pb_2()
                            .pt_0()
                            .child(
                                Button::new(("dispatch-open-member", index))
                                    .ghost()
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .label(if english() {
                                        format!("Open {owner_for_button} session →")
                                    } else {
                                        format!("直达 {owner_for_button} 会话 →")
                                    })
                                    .accessibility_label(if english() {
                                        format!("Open member session: {owner_for_button}")
                                    } else {
                                        format!("打开成员会话：{owner_for_button}")
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_member_session(sid.clone(), window, cx);
                                    })),
                            ),
                    )
                }),
        )
    }
}
