use super::ThinkingBlock;
use crate::{app::HiveApp, palette::Color, language::{english,tr}};
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
        v_flex()
            .min_w_0()
            .gap_1()
            .child(
                Button::new(SharedString::from(format!("thinking-toggle-{row}-{index}")))
                    .ghost()
                    .compact()
                    .justify_start()
                    .w_full()
                    .accessibility_label(if english() {format!("{} thinking block {}: {label}",if expanded {tr("收起")} else {tr("展开")},index+1)} else {format!("{}思考过程，第 {} 段，{label}",if expanded {tr("收起")} else {tr("展开")},index+1)})
                    .disabled(block.redacted)
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(if block.redacted {
                                    Glyph::Lock
                                } else {
                                    Glyph::Sparkles
                                })
                                .size_4(),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(Color::Muted.get(cx))
                                    .child(label),
                            )
                            .when(!block.redacted, |row| {
                                row.child(
                                    Icon::new(if expanded {
                                        Glyph::ChevronDown
                                    } else {
                                        Glyph::ChevronRight
                                    })
                                    .size_3(),
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
                        .py_1()
                        .gap_1()
                        .border_l_2()
                        .border_color(Color::Divider.get(cx))
                        .text_sm()
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
                                .ghost()
                                .compact()
                                .icon(Glyph::Copy)
                                .accessibility_label(if english() {format!("Copy thinking block {}",index+1)} else {format!("复制思考过程，第 {} 段",index+1)})
                                .tooltip(tr("复制思考过程"))
                                .on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            text.clone(),
                                        ))
                                    },
                                )),
                            ),
                        ),
                )
            })
            .into_any_element()
    }
}
