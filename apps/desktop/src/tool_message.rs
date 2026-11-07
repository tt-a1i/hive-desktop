use crate::app::{HiveApp, Message, ToolPresentation};
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

fn input_summary(args: &serde_json::Value) -> Option<String> {
    let line = ["command", "file_path", "path", "query", "pattern"]
        .iter()
        .find_map(|key| args.get(key)?.as_str())
        .or_else(|| args.as_str());
    if let Some(line) = line {
        return line
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(|line| line.chars().take(140).collect());
    }
    let argv = args.get("args")?.as_array()?;
    Some(
        argv.iter()
            .filter_map(|arg| arg.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .lines()
            .next()?
            .chars()
            .take(140)
            .collect(),
    )
}

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
        let input = self.tool_inputs.get(&id).filter(|args| !args.is_null());
        let summary = input.and_then(input_summary);
        let failed = tool.status == "失败";
        v_flex()
            .min_w_0()
            .gap_1()
            .child(
                Button::new(("tool-details", index))
                    .ghost()
                    .w_full()
                    .accessibility_label(format!(
                        "{action} {} 工具详情，{} {}",
                        tool.name,
                        tool.status,
                        summary.as_deref().unwrap_or("")
                    ))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child(if tool.expanded { "▾" } else { "▸" })
                            .child(div().flex_shrink_0().child(tool.name.clone()))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .truncate()
                                    .text_sm()
                                    .text_color(Color::Muted.get(cx))
                                    .child(summary.unwrap_or_default()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if failed {
                                        Color::Error.get(cx)
                                    } else {
                                        Color::Secondary.get(cx)
                                    })
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
                let args = input.map(|value| {
                    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
                });
                let text = message.text.clone();
                let details = match &args {
                    Some(input) if !text.is_empty() => format!("输入参数\n{input}\n\n输出\n{text}"),
                    Some(input) => input.clone(),
                    None => text.clone(),
                };
                view.child(
                    v_flex()
                        .gap_2()
                        .p_3()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .when_some(args, |card, input| {
                            card.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child("输入参数"),
                            )
                            .child(div().text_sm().child(input))
                        })
                        .when(!text.is_empty(), |card| {
                            card.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child("输出"),
                            )
                            .child(div().min_w_0().text_sm().child(text))
                        })
                        .child(
                            Button::new(("copy-tool", index))
                                .disabled(details.is_empty())
                                .label("复制详情")
                                .accessibility_label(format!("复制 {} 工具详情", tool.name))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        details.clone(),
                                    ))
                                })),
                        ),
                )
            })
            .when(failed && !tool.expanded, |view| {
                view.child(
                    div()
                        .px_3()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(message.text.chars().take(160).collect::<String>()),
                )
            })
    }
}
