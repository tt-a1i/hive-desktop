use super::ThinkingBlock;
use crate::{
    app::HiveApp,
    language::{english, tr},
    palette::Color,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

impl HiveApp {
    pub(crate) fn render_thinking(
        &self,
        row: usize,
        message_id: &str,
        block: &ThinkingBlock,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = message_id.to_owned();
        let index = block.index;
        let expanded = block.expanded;
        let label = if block.redacted {
            tr("思考过程未公开")
        } else if block.running {
            tr("思考中…")
        } else if block.interrupted {
            tr("思考已中断")
        } else {
            tr("思考过程")
        };
        let preview = (!expanded && !block.redacted)
            .then(|| {
                block
                    .text
                    .lines()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .map(str::to_owned)
            })
            .flatten();
        let dot_color = if block.redacted {
            Color::Muted.get(cx).opacity(0.6)
        } else if block.running {
            Color::Accent.get(cx)
        } else if block.interrupted {
            rgb(if Theme::global(cx).is_dark() {
                0xfbbf24
            } else {
                0xd97706
            })
        } else {
            Color::Muted.get(cx).opacity(0.75)
        };
        v_flex()
            .min_w_0()
            .gap_1()
            .child(
                Button::new(SharedString::from(format!("thinking-toggle-{row}-{index}")))
                    .custom(crate::palette::quiet_icon_variant(cx))
                    .compact()
                    .h(px(26.))
                    .px_1p5()
                    .rounded_md()
                    .justify_start()
                    .w_full()
                    .accessibility_label(if english() {
                        format!(
                            "{} thinking block {}: {label}",
                            if expanded { tr("收起") } else { tr("展开") },
                            index + 1
                        )
                    } else {
                        format!(
                            "{}思考过程，第 {} 段，{label}",
                            if expanded { tr("收起") } else { tr("展开") },
                            index + 1
                        )
                    })
                    .disabled(block.redacted)
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1p5()
                            .items_center()
                            .child(div().size(px(6.)).rounded_full().flex_shrink_0().bg(dot_color))
                            .child(
                                Icon::new(if block.redacted {
                                    Glyph::Lock
                                } else {
                                    Glyph::Sparkles
                                })
                                .size(px(13.))
                                .text_color(if block.running {
                                    Color::Accent.get(cx)
                                } else {
                                    Color::Muted.get(cx)
                                }),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if block.running {
                                        Color::Text.get(cx)
                                    } else {
                                        Color::Secondary.get(cx)
                                    })
                                    .child(label),
                            )
                            .when_some(preview, |row, summary| {
                                row.child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(summary),
                                )
                            })
                            .when(!block.redacted, |row| {
                                row.child(
                                    Icon::new(if expanded {
                                        Glyph::ChevronDown
                                    } else {
                                        Glyph::ChevronRight
                                    })
                                    .size(px(12.))
                                    .text_color(Color::Muted.get(cx)),
                                )
                            }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.prepare_conversation_expansion(&id);
                        if let Some(message) =
                            this.messages.iter_mut().find(|message| message.id == id)
                        {
                            if let Some(block) = message
                                .thinking
                                .iter_mut()
                                .find(|block| block.index == index && !block.redacted)
                            {
                                block.expanded = !block.expanded;
                                message.layout_revision += 1;
                            }
                        }
                        cx.notify();
                    })),
            )
            .when(expanded && !block.redacted, |view| {
                let text = block.text.clone();
                view.child(
                    v_flex()
                        .min_w_0()
                        .ml_2()
                        .pl_3()
                        .pr_1()
                        .py_1p5()
                        .gap_1p5()
                        .border_l_1()
                        .border_color(if block.running {
                            Color::Accent.get(cx).opacity(0.45)
                        } else {
                            Color::Divider.get(cx)
                        })
                        .rounded_r_lg()
                        .bg(Color::Inset.get(cx).opacity(0.55))
                        .text_size(px(12.5))
                        .line_height(rems(1.55))
                        .text_color(Color::Secondary.get(cx))
                        .child(crate::text_reading::selectable_text(
                            SharedString::from(format!("thinking-body-{row}-{index}")),
                            text.clone(),
                        ))
                        .child(
                            h_flex().justify_end().child(
                                Button::new(SharedString::from(format!(
                                    "copy-thinking-{row}-{index}"
                                )))
                                .custom(crate::palette::subtle_icon_variant(cx))
                                .compact()
                                .h(px(24.))
                                .w(px(24.))
                                .rounded_md()
                                .icon(Glyph::Copy)
                                .accessibility_label(if english() {
                                    format!("Copy thinking block {}", index + 1)
                                } else {
                                    format!("复制思考过程，第 {} 段", index + 1)
                                })
                                .tooltip(tr("复制思考过程"))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                                })),
                            ),
                        ),
                )
            })
            .into_any_element()
    }
}
