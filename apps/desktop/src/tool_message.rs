use crate::app::{HiveApp, Message, ToolPresentation};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

impl HiveApp {
    pub fn render_tool_message(
        &self,
        index: usize,
        message: &Message,
        tool: &ToolPresentation,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = message.id.clone();
        let action = if tool.expanded { "收起" } else { "展开" };
        let failed = tool.status == "失败";
        v_flex()
            .min_w_0()
            .gap_1()
            .child(
                Button::new(("tool-details", index))
                    .ghost()
                    .w_full()
                    .accessibility_label(format!(
                        "{action} {} 工具详情，{}",
                        tool.name, tool.status
                    ))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child(if tool.expanded { "▾" } else { "▸" })
                            .child(div().min_w_0().flex_1().truncate().child(tool.name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if failed { rgb(0xb54131) } else { rgb(0x586653) })
                                    .child(tool.status),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(tool) = this
                            .messages
                            .iter_mut()
                            .find(|msg| msg.id == id)
                            .and_then(|msg| msg.tool.as_mut())
                        {
                            tool.expanded = !tool.expanded;
                            cx.notify();
                        }
                    })),
            )
            .when(tool.expanded, |view| {
                let text = message.text.clone();
                view.child(
                    v_flex()
                        .gap_2()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0xf0f2ed))
                        .child(div().min_w_0().text_sm().child(text.clone()))
                        .child(
                            Button::new(("copy-tool", index))
                                .label("复制详情")
                                .accessibility_label(format!("复制 {} 工具详情", tool.name))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                                })),
                        ),
                )
            })
            .when(failed && !tool.expanded, |view| {
                view.child(
                    div()
                        .px_3()
                        .text_sm()
                        .text_color(rgb(0xb54131))
                        .child(message.text.chars().take(160).collect::<String>()),
                )
            })
    }
}
