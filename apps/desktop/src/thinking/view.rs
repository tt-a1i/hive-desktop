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
                    .map(|line| {
                        let mut chars = line.chars();
                        let head: String = chars.by_ref().take(42).collect();
                        if chars.next().is_some() {
                            format!("{head}…")
                        } else {
                            head
                        }
                    })
            })
            .flatten();
        let accent_color = if block.redacted {
            Color::Muted.get(cx).opacity(0.7)
        } else if block.running {
            Color::Accent.get(cx)
        } else if block.interrupted {
            rgb(if Theme::global(cx).is_dark() {
                0xfbbf24
            } else {
                0xd97706
            })
        } else {
            Color::Muted.get(cx)
        };
        v_flex()
            .min_w_0()
            .gap_1()
            .child(
                Button::new(SharedString::from(format!("thinking-toggle-{row}-{index}")))
                    .custom(crate::palette::quiet_icon_variant(cx))
                    .compact()
                    .h(px(24.))
                    .px_1p5()
                    .rounded_md()
                    .justify_start()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
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
                            .w_full()
                            .min_w_0()
                            .gap_1p5()
                            .items_center()
                            .child(
                                Icon::new(if block.redacted {
                                    Glyph::Lock
                                } else {
                                    Glyph::Sparkles
                                })
                                .flex_shrink_0()
                                .size(px(12.5))
                                .text_color(accent_color),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if block.running {
                                        Color::Text.get(cx)
                                    } else if expanded {
                                        Color::Secondary.get(cx)
                                    } else {
                                        Color::Muted.get(cx)
                                    })
                                    .child(label),
                            )
                            .when_some(preview, |row, summary| {
                                row.child(
                                    div()
                                        .min_w_0()
                                        .text_left()
                                        .overflow_hidden()
                                        .truncate()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx).opacity(0.8))
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
                                    .flex_shrink_0()
                                    .size(px(11.5))
                                    .text_color(Color::Muted.get(cx).opacity(0.8)),
                                )
                            })
                            .child(div().flex_1()),
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
                        .ml(px(12.))
                        .pl_3()
                        .pr_1()
                        .py_1()
                        .gap_1()
                        .border_l_1()
                        .border_color(if block.running {
                            Color::Accent.get(cx).opacity(0.45)
                        } else if block.interrupted {
                            accent_color.opacity(0.45)
                        } else {
                            Color::Divider.get(cx)
                        })
                        .text_size(px(13.))
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
                                .h(px(22.))
                                .w(px(22.))
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
