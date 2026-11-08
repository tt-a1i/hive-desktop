use crate::app::{HiveApp, Message, ToolPresentation};
use crate::palette::Color;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        scroll::ScrollableElement,
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
    pub fn render_completed_tools_header(
        &self,
        start: usize,
        messages: &[Message],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let key = messages[0].id.clone();
        let expanded = self.expanded_tool_groups.contains(&key)
            || messages
                .iter()
                .any(|m| m.tool.as_ref().is_some_and(|t| t.expanded));
        let count = messages.len();
        v_flex().min_w_0().gap_1().child(
            Button::new(("tool-group", start))
                .ghost()
                .w_full()
                .accessibility_label(format!(
                    "{}工作过程，{count} 项工具已完成",
                    if expanded { "收起" } else { "展开" }
                ))
                .child(
                    h_flex()
                        .flex_1()
                        .gap_2()
                        .child(div().text_sm().child("工作过程"))
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(format!("{count} 项已完成")),
                        )
                        .child(if expanded { "▾" } else { "▸" }),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if expanded {
                        this.expanded_tool_groups.remove(&key);
                        // The stream may have grown between drawing and clicking.
                        if let Some(index) = this.messages.iter().position(|m| m.id == key) {
                            for m in this.messages[index..]
                                .iter_mut()
                                .take_while(|m| m.completed_tool())
                            {
                                if let Some(tool) = &mut m.tool {
                                    tool.expanded = false;
                                }
                            }
                        }
                    } else {
                        this.expanded_tool_groups.insert(key.clone());
                    }
                    cx.notify();
                })),
        )
    }

    pub fn render_tool_message(
        &self,
        index: usize,
        message: &Message,
        tool: &ToolPresentation,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = message.id.clone();
        let toggle_id = id.clone();
        let action = if tool.expanded { "收起" } else { "展开" };
        let input = self.tool_inputs.get(&id).filter(|args| !args.is_null());
        let summary = input.and_then(input_summary);
        let failed = tool.status == "失败";
        v_flex()
            .id(("tool-message-row", index))
            .when(self.conversation_find_matches(&message.id), |view| {
                view.bg(Color::Attention.get(cx)).rounded_lg()
            })
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
                            .find(|msg| msg.id == toggle_id)
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
                        .tab_group()
                        .gap_2()
                        .p_3()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .child({
                            let scroll = tool.details_scroll.clone();
                            v_flex()
                                .id(SharedString::from(format!("tool-details-body-{id}")))
                                .min_w_0()
                                .max_h(px(320.))
                                .overflow_y_scroll()
                                .track_scroll(&tool.details_scroll)
                                .relative()
                                .vertical_scrollbar(&tool.details_scroll)
                                .tab_index(0)
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .rounded_md()
                                .p_2()
                                .focus(|style| style.border_color(Color::Secondary.get(cx)))
                                .role(Role::Region)
                                .aria_label(format!(
                                    "{} 工具输入与输出，可用方向键或翻页键滚动",
                                    tool.name
                                ))
                                .gap_2()
                                .on_key_down(move |event, window, cx| {
                                    let modifiers = event.keystroke.modifiers;
                                    if modifiers.control
                                        || modifiers.platform
                                        || modifiers.alt
                                        || modifiers.shift
                                    {
                                        return;
                                    }
                                    let offset = scroll.offset();
                                    let page = (scroll.bounds().size.height - px(24.)).max(px(24.));
                                    match event.keystroke.key.as_str() {
                                        "up" => scroll.set_offset(point(
                                            offset.x,
                                            (offset.y + px(24.)).min(px(0.)),
                                        )),
                                        "down" => {
                                            scroll.set_offset(point(offset.x, offset.y - px(24.)))
                                        }
                                        "pageup" => scroll.set_offset(point(
                                            offset.x,
                                            (offset.y + page).min(px(0.)),
                                        )),
                                        "pagedown" => {
                                            scroll.set_offset(point(offset.x, offset.y - page))
                                        }
                                        "home" => scroll.set_offset(point(offset.x, px(0.))),
                                        "end" => scroll.scroll_to_bottom(),
                                        _ => return,
                                    }
                                    window.refresh();
                                    cx.stop_propagation();
                                })
                                .when_some(args, |card, input| {
                                    card.child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child("输入参数"),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .font_family(Theme::global(cx).mono_font_family.clone())
                                            .text_size(px(12.))
                                            .line_height(px(19.))
                                            .child(gpui_kit::base::SelectableText::new(
                                                SharedString::from(format!("tool-input-{id}")),
                                                input,
                                            )),
                                    )
                                })
                                .when(!text.is_empty(), |card| {
                                    card.child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child("输出"),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .font_family(Theme::global(cx).mono_font_family.clone())
                                            .text_size(px(12.))
                                            .line_height(px(19.))
                                            .child(gpui_kit::base::SelectableText::new(
                                                SharedString::from(format!("tool-output-{id}")),
                                                text,
                                            )),
                                    )
                                })
                        })
                        .child(
                            h_flex().justify_end().child(
                                Button::new(("copy-tool", index))
                                    .ghost()
                                    .compact()
                                    .icon(Glyph::Copy)
                                    .disabled(details.is_empty())
                                    .tooltip("复制详情")
                                    .accessibility_label(format!("复制 {} 工具详情", tool.name))
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            details.clone(),
                                        ))
                                    })),
                            ),
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

impl Message {
    pub(crate) fn completed_tool(&self) -> bool {
        self.tool.as_ref().is_some_and(|tool| tool.status == "完成")
    }
}
