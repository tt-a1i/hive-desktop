use crate::app::{HiveApp, Message, ToolPresentation};
use crate::language::{english, tr};
use crate::palette::Color;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub(crate) fn result_status(result: &serde_json::Value, is_error: bool) -> &'static str {
    match result["toolStatus"].as_str() {
        Some("declined") => "已拒绝",
        Some("interrupted") => "已中断",
        Some("cancelled" | "canceled") => "已取消",
        Some("failed") => "失败",
        _ if is_error || result["isError"] == true => "失败",
        Some("inProgress") => "执行中",
        Some("completed") | None => "完成",
        Some(_) => "状态未知",
    }
}

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

fn tool_glyph(name: &str) -> Glyph {
    let lower = name.to_ascii_lowercase();
    if lower.contains("read") || lower.contains("file") || lower.contains("view") {
        Glyph::FileText
    } else if lower.contains("search") || lower.contains("grep") || lower.contains("glob") {
        Glyph::Search
    } else if lower.contains("bash")
        || lower.contains("exec")
        || lower.contains("command")
        || lower.contains("shell")
        || lower.contains("terminal")
    {
        Glyph::SquareTerminal
    } else if lower.contains("edit") || lower.contains("write") || lower.contains("patch") {
        Glyph::SquarePen
    } else {
        Glyph::Cpu
    }
}

impl HiveApp {
    /// T3 Code `WorkLog` compact group disclosure row.
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
        v_flex().min_w_0().child(
            Button::new(("tool-group", start))
                .custom(crate::palette::quiet_icon_variant(cx))
                .compact()
                .h(px(24.))
                .px_1p5()
                .rounded_md()
                .w_full()
                .accessibility_label(if english() {
                    format!(
                        "{} work activity, {count} tools completed",
                        if expanded { tr("收起") } else { tr("展开") }
                    )
                } else {
                    format!(
                        "{}工作过程，{count} 项工具已完成",
                        if expanded { tr("收起") } else { tr("展开") }
                    )
                })
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .gap_1p5()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if expanded {
                                    Color::Secondary.get(cx)
                                } else {
                                    Color::Muted.get(cx)
                                })
                                .child(tr("工作过程")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(Color::Muted.get(cx).opacity(0.85))
                                .child(if english() {
                                    format!("· {count} completed")
                                } else {
                                    format!("· {count} 项已完成")
                                }),
                        )
                        .child(
                            Icon::new(if expanded {
                                Glyph::ChevronDown
                            } else {
                                Glyph::ChevronRight
                            })
                            .size(px(11.5))
                            .text_color(Color::Muted.get(cx).opacity(0.8)),
                        )
                        .child(div().flex_1()),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.prepare_conversation_expansion(&key);
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
        let action = if tool.expanded {
            tr("收起")
        } else {
            tr("展开")
        };
        let input = self.tool_inputs.get(&id).filter(|args| !args.is_null());
        let summary = input.and_then(input_summary);
        let failed = tool.status == "失败";
        let in_progress = tool.status == "执行中";
        let status_color = match tool.status {
            "失败" => Color::Error.get(cx),
            "完成" => Color::Muted.get(cx),
            "执行中" => Color::Accent.get(cx),
            _ => Color::Secondary.get(cx),
        };
        let status_bg = match tool.status {
            "失败" => Color::ErrorSurface.get(cx),
            "执行中" => Color::AccentSoft.get(cx),
            _ => Color::Inset.get(cx),
        };
        v_flex()
            .id(("tool-message-row", index))
            .when(self.conversation_find_matches(&message.id), |view| {
                view.bg(Color::AccentSoft.get(cx))
                    .border_1()
                    .border_color(Color::Accent.get(cx).opacity(0.28))
                    .rounded_lg()
                    .px_1()
            })
            .min_w_0()
            .gap_1()
            .child(
                Button::new(("tool-details", index))
                    .custom(crate::palette::quiet_icon_variant(cx))
                    .compact()
                    .h(px(24.))
                    .px_1p5()
                    .rounded_md()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .accessibility_label(if english() {
                        format!(
                            "{action} {} tool details · {} {}",
                            tool.name,
                            tr(tool.status),
                            summary.as_deref().unwrap_or("")
                        )
                    } else {
                        format!(
                            "{action} {} 工具详情，{} {}",
                            tool.name,
                            tr(tool.status),
                            summary.as_deref().unwrap_or("")
                        )
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(tool_glyph(&tool.name))
                                    .flex_shrink_0()
                                    .size(px(12.5))
                                    .text_color(if failed {
                                        Color::Error.get(cx)
                                    } else if in_progress {
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
                                    .text_color(if tool.expanded || in_progress {
                                        Color::Text.get(cx)
                                    } else {
                                        Color::Secondary.get(cx)
                                    })
                                    .child(tool.name.clone()),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .text_left()
                                    .overflow_hidden()
                                    .truncate()
                                    .text_size(px(11.5))
                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                    .text_color(Color::Muted.get(cx).opacity(0.85))
                                    .child(summary.unwrap_or_default()),
                            )
                            .when(tool.status != "完成", |row| {
                                row.child(
                                    div()
                                        .flex_shrink_0()
                                        .px_1p5()
                                        .py(px(1.))
                                        .rounded_full()
                                        .bg(status_bg)
                                        .text_size(px(10.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(status_color)
                                        .child(tr(tool.status)),
                                )
                            })
                            .child(
                                Icon::new(if tool.expanded {
                                    Glyph::ChevronDown
                                } else {
                                    Glyph::ChevronRight
                                })
                                .flex_shrink_0()
                                .size(px(11.5))
                                .text_color(Color::Muted.get(cx).opacity(0.8)),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.prepare_conversation_expansion(&toggle_id);
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
                    Some(input) if !text.is_empty() => {
                        format!("{}\n{input}\n\n{}\n{text}", tr("输入参数"), tr("输出"))
                    }
                    Some(input) => input.clone(),
                    None => text.clone(),
                };
                let diff_available = crate::tool_diff::data::available(
                    &tool.name,
                    input.unwrap_or(&serde_json::Value::Null),
                    tool.patch.as_deref(),
                );
                let paths = input
                    .map(|value| crate::file_viewer::tool_paths(&tool.name, value))
                    .unwrap_or_default();
                let has_header_bar = !paths.is_empty() || diff_available || !details.is_empty();
                let args_for_diff = input.cloned().unwrap_or_default();
                let name_for_diff = tool.name.clone();
                let patch_for_diff = tool.patch.clone();
                let status_for_diff = tool.status;
                let patch_match = self.conversation_find_patch_match(&id, tool.patch.as_deref());
                view.child(
                    div()
                        .ml(px(12.))
                        .pl_3()
                        .py_0p5()
                        .border_l_1()
                        .border_color(if failed {
                            Color::Error.get(cx).opacity(0.45)
                        } else if in_progress {
                            Color::Accent.get(cx).opacity(0.45)
                        } else {
                            Color::Divider.get(cx)
                        })
                        .child(
                            v_flex()
                                .tab_group()
                                .rounded_lg()
                                .bg(Color::Card.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .overflow_hidden()
                                .when(has_header_bar, |card| {
                                    card.child(
                                        h_flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .px_3()
                                            .py_1p5()
                                            .border_b_1()
                                            .border_color(Color::Divider.get(cx))
                                            .child(
                                                h_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .flex_wrap()
                                                    .gap_1()
                                                    .children(paths.into_iter().enumerate().map(
                                                        |(i, path)| {
                                                            let file = path.clone();
                                                            Button::new(SharedString::from(
                                                                format!("tool-file-{id}-{i}"),
                                                            ))
                                                            .custom(
                                                                crate::palette::subtle_icon_variant(
                                                                    cx,
                                                                ),
                                                            )
                                                            .compact()
                                                            .h(px(22.))
                                                            .px_1p5()
                                                            .rounded_md()
                                                            .max_w(px(360.))
                                                            .min_w_0()
                                                            .child(
                                                                h_flex()
                                                                    .min_w_0()
                                                                    .items_center()
                                                                    .gap_1()
                                                                    .child(
                                                                        Icon::new(Glyph::FileText)
                                                                            .size(px(12.))
                                                                            .text_color(
                                                                                Color::Muted
                                                                                    .get(cx),
                                                                            ),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .min_w_0()
                                                                            .truncate()
                                                                            .text_xs()
                                                                            .font_family(
                                                                                Theme::global(cx)
                                                                                    .mono_font_family
                                                                                    .clone(),
                                                                            )
                                                                            .text_color(
                                                                                Color::Secondary
                                                                                    .get(cx),
                                                                            )
                                                                            .child(path.clone()),
                                                                    ),
                                                            )
                                                            .accessibility_label(format!(
                                                                "{}: {path}",
                                                                tr("预览文件")
                                                            ))
                                                            .tooltip(format!(
                                                                "{}: {path}",
                                                                tr("预览当前磁盘内容")
                                                            ))
                                                            .on_click(cx.listener(
                                                                move |this, _, window, cx| {
                                                                    this.open_file_preview(
                                                                        file.clone(),
                                                                        window,
                                                                        cx,
                                                                    )
                                                                },
                                                            ))
                                                        },
                                                    )),
                                            )
                                            .child(
                                                h_flex()
                                                    .flex_shrink_0()
                                                    .items_center()
                                                    .gap_1()
                                                    .when(diff_available, |row| {
                                                        row.child(
                                                            Button::new(SharedString::from(
                                                                format!("tool-diff-{id}"),
                                                            ))
                                                            .custom(
                                                                crate::palette::subtle_icon_variant(
                                                                    cx,
                                                                ),
                                                            )
                                                            .compact()
                                                            .h(px(24.))
                                                            .px_2()
                                                            .rounded_md()
                                                            .icon(Glyph::FileDiff)
                                                            .label(if patch_match {
                                                                tr("查看改动 · 匹配")
                                                            } else {
                                                                tr("查看改动")
                                                            })
                                                            .selected(patch_match)
                                                            .accessibility_label(format!(
                                                                "{}: {}{}",
                                                                tr("查看改动"),
                                                                tool.name,
                                                                if patch_match {
                                                                    if english() {
                                                                        ", includes search matches"
                                                                    } else {
                                                                        "，包含查找匹配"
                                                                    }
                                                                } else {
                                                                    ""
                                                                }
                                                            ))
                                                            .on_click(cx.listener(
                                                                move |this, _, window, cx| {
                                                                    this.open_tool_diff(
                                                                        name_for_diff.clone(),
                                                                        args_for_diff.clone(),
                                                                        patch_for_diff.clone(),
                                                                        status_for_diff,
                                                                        window,
                                                                        cx,
                                                                    );
                                                                },
                                                            )),
                                                        )
                                                    })
                                                    .child(
                                                        Button::new(("copy-tool", index))
                                                            .custom(
                                                                crate::palette::subtle_icon_variant(
                                                                    cx,
                                                                ),
                                                            )
                                                            .compact()
                                                            .h(px(24.))
                                                            .w(px(24.))
                                                            .rounded_md()
                                                            .icon(Glyph::Copy)
                                                            .disabled(details.is_empty())
                                                            .tooltip(tr("复制详情"))
                                                            .accessibility_label(if english() {
                                                                format!(
                                                                    "Copy {} tool details",
                                                                    tool.name
                                                                )
                                                            } else {
                                                                format!(
                                                                    "复制 {} 工具详情",
                                                                    tool.name
                                                                )
                                                            })
                                                            .on_click(cx.listener(
                                                                move |_, _, _, cx| {
                                                                    cx.write_to_clipboard(
                                                                        ClipboardItem::new_string(
                                                                            details.clone(),
                                                                        ),
                                                                    )
                                                                },
                                                            )),
                                                    ),
                                            ),
                                    )
                                })
                                .child({
                                    let has_args = args.is_some();
                                    crate::text_reading::region(
                                        format!("tool-details-body-{id}"),
                                        if english() {
                                            format!(
                                                "{} tool input and output; scroll with arrow or page keys",
                                                tool.name
                                            )
                                        } else {
                                            format!(
                                                "{} 工具输入与输出，可用方向键或翻页键滚动",
                                                tool.name
                                            )
                                        },
                                        &tool.details_scroll,
                                        px(300.),
                                        cx,
                                    )
                                    .border_0()
                                    .rounded_none()
                                    .px_3()
                                    .py_2()
                                    .gap_2()
                                    .when_some(args, |card, input| {
                                        card.child(
                                            v_flex()
                                                .min_w_0()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(tr("输入参数")),
                                                )
                                                .child(
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "tool-input-text-{id}"
                                                        )))
                                                        .role(Role::Label)
                                                        .aria_value(format!(
                                                            "{}: {input}",
                                                            tr("输入参数")
                                                        ))
                                                        .min_w_0()
                                                        .font_family(
                                                            Theme::global(cx)
                                                                .mono_font_family
                                                                .clone(),
                                                        )
                                                        .text_size(px(12.))
                                                        .line_height(px(18.5))
                                                        .text_color(Color::Secondary.get(cx))
                                                        .child(gpui_kit::base::SelectableText::new(
                                                            SharedString::from(format!(
                                                                "tool-input-{id}"
                                                            )),
                                                            input,
                                                        )),
                                                ),
                                        )
                                    })
                                    .when(!text.is_empty(), |card| {
                                        card.child(
                                            v_flex()
                                                .min_w_0()
                                                .gap_1()
                                                .when(has_args, |section| {
                                                    section
                                                        .pt_2()
                                                        .border_t_1()
                                                        .border_color(Color::Divider.get(cx).opacity(0.65))
                                                })
                                                .child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(if failed {
                                                            Color::Error.get(cx)
                                                        } else {
                                                            Color::Muted.get(cx)
                                                        })
                                                        .child(tr("输出")),
                                                )
                                                .child(
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "tool-output-text-{id}"
                                                        )))
                                                        .role(Role::Label)
                                                        .aria_value(format!(
                                                            "{}: {text}",
                                                            tr("输出")
                                                        ))
                                                        .min_w_0()
                                                        .font_family(
                                                            Theme::global(cx)
                                                                .mono_font_family
                                                                .clone(),
                                                        )
                                                        .text_size(px(12.))
                                                        .line_height(px(18.5))
                                                        .text_color(if failed {
                                                            Color::Error.get(cx)
                                                        } else {
                                                            Color::Text.get(cx)
                                                        })
                                                        .child(gpui_kit::base::SelectableText::new(
                                                            SharedString::from(format!(
                                                                "tool-output-{id}"
                                                            )),
                                                            text,
                                                        )),
                                                ),
                                        )
                                    })
                                }),
                        ),
                )
            })
            .when(failed && !tool.expanded, |view| {
                view.child(
                    div()
                        .ml(px(12.))
                        .pl_2p5()
                        .pr_2()
                        .py_1()
                        .border_l_2()
                        .border_color(Color::Error.get(cx).opacity(0.5))
                        .bg(Color::ErrorSurface.get(cx).opacity(0.6))
                        .rounded_r_md()
                        .text_xs()
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

#[cfg(test)]
mod status_tests {
    use super::result_status;
    use serde_json::json;
    #[test]
    fn terminal_statuses_do_not_turn_rejections_or_interruptions_into_success() {
        for (native, label) in [
            ("declined", "已拒绝"),
            ("interrupted", "已中断"),
            ("cancelled", "已取消"),
            ("failed", "失败"),
            ("inProgress", "执行中"),
            ("completed", "完成"),
            ("future-status", "状态未知"),
        ] {
            assert_eq!(result_status(&json!({"toolStatus":native}), false), label);
        }
        assert_eq!(
            result_status(&json!({"toolStatus":"completed"}), true),
            "失败"
        );
        assert_eq!(result_status(&json!({"isError":true}), false), "失败");
        assert_eq!(result_status(&json!({"content":[]}), false), "完成");
    }
}
