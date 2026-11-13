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
        Reveal::new(
            format!("dispatch-{}-reveal", dispatch.id),
            &self.collaboration_scroll,
            div()
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
                ),
        )
    }
}
