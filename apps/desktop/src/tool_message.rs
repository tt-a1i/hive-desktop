use crate::app::{HiveApp, Message, ToolOutputWindow, ToolPresentation};
use crate::language::{english, tr};
use crate::palette::Color;
use crate::protocol::ToolOutputSliceResponse;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use std::sync::Arc;

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

fn tool_glyph(name: &str, failed: bool) -> Glyph {
    if failed {
        return Glyph::CircleAlert;
    }
    let lower = name.to_ascii_lowercase();
    if lower.contains("web") || lower.contains("fetch") {
        Glyph::Globe
    } else if lower.contains("mcp") || lower.contains("dynamic") {
        Glyph::Wrench
    } else if lower.contains("edit")
        || lower.contains("write")
        || lower.contains("patch")
        || lower.contains("filechange")
    {
        Glyph::SquarePen
    } else if lower.contains("read") || lower.contains("file") || lower.contains("view") {
        Glyph::FileText
    } else if lower.contains("search")
        || lower.contains("grep")
        || lower.contains("glob")
        || lower.contains("find")
    {
        Glyph::Search
    } else if lower.contains("bash")
        || lower.contains("exec")
        || lower.contains("command")
        || lower.contains("shell")
        || lower.contains("terminal")
    {
        Glyph::SquareTerminal
    } else if lower.contains("tool") {
        Glyph::Wrench
    } else {
        Glyph::Cpu
    }
}

pub(crate) fn tool_action_label(
    name: &str,
    status: &str,
    args: Option<&serde_json::Value>,
) -> String {
    let lower = name.to_ascii_lowercase();
    let in_progress = status == "执行中";
    let failed = status == "失败";
    if lower.contains("bash")
        || lower.contains("exec")
        || lower.contains("command")
        || lower.contains("shell")
        || lower.contains("terminal")
    {
        return tr(if in_progress {
            "正在运行命令"
        } else if failed {
            "命令失败"
        } else {
            "运行命令"
        })
        .to_owned();
    }
    if lower == "filechange" {
        let count = args
            .and_then(|value| value["changes"].as_array())
            .map_or(0, |items| items.len());
        if count > 1 {
            return if in_progress {
                if english() {
                    format!("Updating {count} files")
                } else {
                    format!("正在修改 {count} 个文件")
                }
            } else if failed {
                if english() {
                    format!("Failed to update {count} files")
                } else {
                    format!("修改 {count} 个文件失败")
                }
            } else if english() {
                format!("Updated {count} files")
            } else {
                format!("修改 {count} 个文件")
            };
        }
        return tr(if in_progress {
            "正在修改文件"
        } else if failed {
            "修改文件失败"
        } else {
            "修改文件"
        })
        .to_owned();
    }
    if lower.contains("web") || lower.contains("fetch") {
        return tr(if in_progress { "正在网页搜索" } else { "网页搜索" }).to_owned();
    }
    if lower.contains("edit") || lower.contains("write") || lower.contains("patch") {
        return tr(if in_progress { "正在编辑文件" } else { "编辑文件" }).to_owned();
    }
    if lower.contains("read") || lower.contains("view") {
        return tr(if in_progress { "正在读取文件" } else { "读取文件" }).to_owned();
    }
    if lower.contains("search")
        || lower.contains("grep")
        || lower.contains("glob")
        || lower.contains("find")
    {
        return tr(if in_progress { "正在搜索代码" } else { "搜索代码" }).to_owned();
    }
    if name.trim().is_empty() || name == "工具" {
        return tr(if in_progress { "正在调用工具" } else { "调用工具" }).to_owned();
    }
    if let Some(rest) = name.strip_prefix("mcp__") {
        return rest.replace("__", " / ");
    }
    name.to_owned()
}

const TOOL_OUTPUT_SLICE_BYTES: u64 = 32_768;
const MAX_DISPLAYED_TOOL_OUTPUT_BYTES: usize = 262_144;

fn format_byte_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn apply_tool_output_slice(
    text: &mut String,
    windows: &mut [ToolOutputWindow],
    position: usize,
    slice: ToolOutputSliceResponse,
) -> bool {
    let window = &mut windows[position];
    if slice.chunk.is_empty()
        || slice.chunk.len() as u64 > TOOL_OUTPUT_SLICE_BYTES
        || window.loaded_bytes.checked_add(slice.chunk.len() as u64) != Some(slice.next_byte_offset)
        || slice.next_byte_offset > slice.total_bytes
        || slice.has_more != (slice.next_byte_offset < slice.total_bytes)
    {
        return false;
    }

    let retained_bytes = (window.loaded_bytes - window.window_start_bytes) as usize;
    let range = window.display_range.clone();
    let other_bytes = text.len() - range.len();
    // If the previews already exceed the budget, a load may reuse the
    // selected block's space without expanding the combined output.
    let block_budget = MAX_DISPLAYED_TOOL_OUTPUT_BYTES
        .saturating_sub(other_bytes)
        .max(range.len());
    let (start, chunk) = if retained_bytes + slice.chunk.len() > block_budget {
        let trim = slice.chunk.len().saturating_sub(block_budget);
        let trim = slice.chunk.ceil_char_boundary(trim);
        window.window_start_bytes = window.loaded_bytes + trim as u64;
        (range.start, &slice.chunk[trim..])
    } else {
        // The runtime reports the prefix's byte length before its notice.
        // A notice-like line within real output is part of that prefix.
        (range.start + retained_bytes, slice.chunk.as_str())
    };
    let new_end = start + chunk.len();
    text.replace_range(start..range.end, chunk);
    window.display_range.end = new_end;
    window.loaded_bytes = slice.next_byte_offset;
    window.total_bytes = slice.total_bytes;
    window.has_more = slice.has_more;
    for later in &mut windows[position + 1..] {
        later.display_range = (new_end + (later.display_range.start - range.end))
            ..(new_end + (later.display_range.end - range.end));
    }
    true
}

impl HiveApp {
    fn load_more_tool_output(
        &mut self,
        rendered_id: String,
        block_index: usize,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let Some(session_id) = self.session.as_ref().map(|s| s.id.clone()) else {
            return;
        };
        let generation = self.connection_generation;
        let open_request = self.open_request;
        let Some((history_message_id, byte_offset, output_generation)) =
            self.message_mut(&rendered_id).and_then(|msg| {
                let tool = msg.tool.as_mut()?;
                let win = tool
                    .output_windows
                    .iter_mut()
                    .find(|win| win.block_index == block_index)?;
                if win.loading || !win.has_more {
                    return None;
                }
                win.loading = true;
                win.error = None;
                msg.layout_revision += 1;
                Some((
                    win.history_message_id.clone(),
                    win.loaded_bytes,
                    win.generation.clone(),
                ))
            })
        else {
            return;
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let res = runtime
                .request(
                    "chat.toolOutputSlice",
                    serde_json::json!({
                        "sessionId": session_id,
                        "messageId": history_message_id,
                        "blockIndex": block_index,
                        "byteOffset": byte_offset,
                        "byteLength": TOOL_OUTPUT_SLICE_BYTES,
                    }),
                )
                .await
                .and_then(|v| {
                    serde_json::from_value::<ToolOutputSliceResponse>(v).map_err(|e| e.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != generation || this.open_request != open_request {
                    return;
                }
                let Some(msg) = this.message_mut(&rendered_id) else {
                    return;
                };
                let Some(tool) = &mut msg.tool else {
                    return;
                };
                let Some(position) = tool.output_windows.iter().position(|win| {
                    win.block_index == block_index
                        && Arc::ptr_eq(&win.generation, &output_generation)
                        && win.loading
                        && win.loaded_bytes == byte_offset
                }) else {
                    return;
                };
                tool.output_windows[position].loading = false;
                let applied = res.and_then(|slice| {
                    if apply_tool_output_slice(
                        &mut msg.text,
                        &mut tool.output_windows,
                        position,
                        slice,
                    ) {
                        Ok(())
                    } else {
                        Err(if english() {
                            "The tool output changed while loading."
                        } else {
                            "加载期间工具输出发生了变化。"
                        }
                        .to_owned())
                    }
                });
                msg.layout_revision += 1;
                match applied {
                    Ok(()) => this.refresh_conversation_find(&rendered_id, cx),
                    Err(error) => tool.output_windows[position].error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_tool_output_window(
        &self,
        id: &str,
        window: &ToolOutputWindow,
        copy_text: Option<&str>,
        multiple_blocks: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let loaded = format_byte_size(window.loaded_bytes);
        let total = format_byte_size(window.total_bytes);
        let range_label = if window.window_start_bytes > 0 {
            let start = format_byte_size(window.window_start_bytes);
            if english() {
                format!("Showing {start}–{loaded} of {total}")
            } else {
                format!("当前窗口 {start}–{loaded} / 共 {total}")
            }
        } else if english() {
            format!("Loaded {loaded} / {total}")
        } else {
            format!("已加载 {loaded} / {total}")
        };
        let block_index = window.block_index;
        let block_label = if english() {
            format!("Text block {}", block_index + 1)
        } else {
            format!("文本块 {}", block_index + 1)
        };
        let range_label = if multiple_blocks {
            format!("{block_label} · {range_label}")
        } else {
            range_label
        };
        let load_label = if multiple_blocks {
            format!("{} · {block_label}", tr("加载更多输出"))
        } else {
            tr("加载更多输出").to_owned()
        };
        let load_id = id.to_owned();
        v_flex()
            .id(("tool-output-window", block_index))
            .w_full()
            .gap_1()
            .pt_2()
            .border_t_1()
            .border_color(Color::Divider.get(cx).opacity(0.65))
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(Color::Muted.get(cx))
                            .child(range_label),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .when_some(copy_text, |row, text| {
                                let text = text.to_owned();
                                let copy_key = format!("tool-loaded:{id}:{block_index}");
                                let copied = self.is_copied(&copy_key);
                                row.child(
                                    Button::new("tool-output-copy-loaded")
                                        .custom(crate::palette::subtle_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(22.))
                                        .px_2()
                                        .rounded_sm()
                                        .icon(if copied { Glyph::Check } else { Glyph::Copy })
                                        .label(if copied { tr("已复制") } else { tr("复制已加载内容") })
                                        .accessibility_label(tr("复制已加载内容"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                text.clone(),
                                            ));
                                            this.mark_copied(copy_key.clone(), cx);
                                        })),
                                )
                            })
                            .when(window.has_more, |row| {
                                row.child(
                                    Button::new("tool-output-load-more")
                                        .outline()
                                        .compact()
                                        .xsmall()
                                        .h(px(22.))
                                        .px_2()
                                        .rounded_sm()
                                        .disabled(window.loading || !self.connected)
                                        .label(if window.loading {
                                            tr("正在加载输出…")
                                        } else {
                                            tr("加载更多输出 (+32 KB)")
                                        })
                                        .accessibility_label(load_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.load_more_tool_output(
                                                load_id.clone(),
                                                block_index,
                                                cx,
                                            );
                                        })),
                                )
                            }),
                    ),
            )
            .when_some(window.error.clone(), |col, error| {
                col.child(
                    div()
                        .text_size(px(11.))
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
    }

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
        let known_durations: Vec<u64> = messages
            .iter()
            .filter_map(|m| m.tool.as_ref()?.duration_ms)
            .collect();
        let group_duration = (!known_durations.is_empty())
            .then(|| crate::live_activity::format_duration_ms(known_durations.iter().sum()));
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
                            Icon::new(Glyph::Layers)
                                .flex_shrink_0()
                                .size(px(12.))
                                .text_color(if expanded {
                                    Color::Secondary.get(cx)
                                } else {
                                    Color::Muted.get(cx)
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
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
                                .text_size(px(12.))
                                .text_color(Color::Muted.get(cx).opacity(0.8))
                                .child(if english() {
                                    format!("· {count} completed")
                                } else {
                                    format!("· {count} 项已完成")
                                }),
                        )
                        .child(div().flex_1())
                        .when_some(group_duration, |row, dur| {
                            row.child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(11.))
                                    .font_features(crate::palette::tabular_nums())
                                    .text_color(Color::Muted.get(cx).opacity(0.8))
                                    .child(dur),
                            )
                        })
                        .child(
                            Icon::new(if expanded {
                                Glyph::ChevronDown
                            } else {
                                Glyph::ChevronRight
                            })
                            .size(px(11.))
                            .text_color(Color::Muted.get(cx).opacity(0.65)),
                        ),
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
        let expand_err_id = id.clone();
        let action = if tool.expanded {
            tr("收起")
        } else {
            tr("展开")
        };
        let input = self.tool_inputs.get(&id).filter(|args| !args.is_null());
        let summary = input.and_then(input_summary);
        let failed = tool.status == "失败";
        let in_progress = tool.status == "执行中";
        let action_label = tool_action_label(&tool.name, tool.status, input);
        let shine_key = format!("tool:{id}");
        let shine_label = in_progress
            .then(|| self.live_activity.shine_labels.get(&shine_key).cloned())
            .flatten();
        let has_shine = shine_label.is_some();
        let duration_text = if in_progress {
            tool.started_at
                .map(|started| crate::live_activity::format_elapsed_seconds(started.elapsed().as_secs()))
        } else {
            tool.duration_ms.map(crate::live_activity::format_duration_ms)
        };
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
                                Icon::new(tool_glyph(&tool.name, failed))
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
                            .when_some(shine_label, |row, entity| row.child(entity))
                            .when(!has_shine, |row| {
                                row.child(
                                    div()
                                        .flex_shrink_0()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(if failed {
                                            Color::Error.get(cx)
                                        } else if tool.expanded || in_progress {
                                            Color::Text.get(cx)
                                        } else {
                                            Color::Secondary.get(cx)
                                        })
                                        .child(action_label),
                                )
                            })
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
                            .when_some(duration_text, |row, dur| {
                                row.child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(px(11.))
                                        .font_features(crate::palette::tabular_nums())
                                        .text_color(if in_progress {
                                            Color::Accent.get(cx)
                                        } else {
                                            Color::Muted.get(cx).opacity(0.8)
                                        })
                                        .child(dur),
                                )
                            })
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
                        if let Some(msg) = this
                            .messages
                            .iter_mut()
                            .find(|msg| msg.id == toggle_id)
                        {
                            if let Some(tool) = msg.tool.as_mut() {
                                tool.expanded = !tool.expanded;
                                msg.layout_revision += 1;
                                cx.notify();
                            }
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
                let has_args = args.is_some();
                let has_header_bar = !paths.is_empty() || diff_available;
                // Avoid duplicate copy buttons when a byte-windowed output footer already
                // provides "复制已加载内容" for output-only tool cards.
                let show_top_copy = !details.is_empty() && (has_args || tool.output_windows.is_empty());
                let mut top_copy_button = show_top_copy.then(|| {
                    let tool_copy_key = format!("tool:{id}");
                    let copied = self.is_copied(&tool_copy_key);
                    let copy_details = details.clone();
                    Button::new(("copy-tool", index))
                        .custom(crate::palette::subtle_icon_variant(cx))
                        .compact()
                        .h(px(22.))
                        .w(px(22.))
                        .rounded_md()
                        .icon(if copied { Glyph::Check } else { Glyph::Copy })
                        .tooltip(if copied { tr("已复制") } else { tr("复制详情") })
                        .accessibility_label(if english() {
                            format!("Copy {} tool details", tool.name)
                        } else {
                            format!("复制 {} 工具详情", tool.name)
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_details.clone()));
                            this.mark_copied(tool_copy_key.clone(), cx);
                        }))
                });
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
                                .border_color(if failed {
                                    Color::Error.get(cx).opacity(0.45)
                                } else {
                                    Color::Divider.get(cx)
                                })
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
                                                            let source = self.session.as_ref().map(|session| crate::artifacts::ArtifactSource::session(&session.workspace_id, message.source_session_id.as_deref().unwrap_or(&session.id), &id, &path));
                                                            let target = source.as_ref().and_then(|source| self.artifact_target(&source.workspace_id, cx));
                                                            let unavailable = target.is_none();
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
                                                            .disabled(unavailable)
                                                            .on_click(cx.listener(
                                                                move |this, _, window, cx| {
                                                                    if let (Some(source), Some(target)) = (source.clone(), target.clone()) {
                                                                        this.open_session_file_preview(source, target, window, cx);
                                                                    }
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
                                                    .when_some(
                                                        has_header_bar.then(|| top_copy_button.take()).flatten(),
                                                        |row, btn| row.child(btn),
                                                    ),
                                            ),
                                    )
                                })
                                .child({
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
                                                    h_flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_size(px(11.))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(tr("输入参数")),
                                                        )
                                                        .when_some(
                                                            (!has_header_bar).then(|| top_copy_button.take()).flatten(),
                                                            |row, btn| row.child(btn),
                                                        ),
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
                                        let err_copy_key = format!("tool-err:{id}");
                                        let err_copied = self.is_copied(&err_copy_key);
                                        let err_text = text.clone();
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
                                                    h_flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_size(px(11.))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(if failed {
                                                                    Color::Error.get(cx)
                                                                } else {
                                                                    Color::Muted.get(cx)
                                                                })
                                                                .child(tr(if failed {
                                                                    "错误输出"
                                                                } else {
                                                                    "输出"
                                                                })),
                                                        )
                                                        .when(failed, |row| {
                                                            row.child(
                                                                Button::new(("copy-tool-error", index))
                                                                    .custom(crate::palette::subtle_icon_variant(cx))
                                                                    .compact()
                                                                    .xsmall()
                                                                    .h(px(22.))
                                                                    .px_2()
                                                                    .rounded_md()
                                                                    .icon(if err_copied {
                                                                        Glyph::Check
                                                                    } else {
                                                                        Glyph::Copy
                                                                    })
                                                                    .label(if err_copied {
                                                                        tr("已复制")
                                                                    } else {
                                                                        tr("复制完整错误")
                                                                    })
                                                                    .accessibility_label(tr("复制完整错误"))
                                                                    .tooltip(if err_copied {
                                                                        tr("已复制")
                                                                    } else {
                                                                        tr("复制错误详情")
                                                                    })
                                                                    .on_click(cx.listener(
                                                                        move |this, _, _, cx| {
                                                                            cx.write_to_clipboard(
                                                                                ClipboardItem::new_string(
                                                                                    err_text.clone(),
                                                                                ),
                                                                            );
                                                                            this.mark_copied(
                                                                                err_copy_key.clone(),
                                                                                cx,
                                                                            );
                                                                        },
                                                                    )),
                                                            )
                                                        })
                                                        .when_some(
                                                            (!failed && !has_header_bar && !has_args)
                                                                .then(|| top_copy_button.take())
                                                                .flatten(),
                                                            |row, btn| row.child(btn),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "tool-output-text-{id}"
                                                        )))
                                                        .role(Role::Label)
                                                        .aria_value(format!(
                                                            "{}: {text}",
                                                            tr(if failed { "错误输出" } else { "输出" })
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
                                                            text.clone(),
                                                        )),
                                                ),
                                        )
                                    })
                                    .children(tool.output_windows.iter().enumerate().map(|(position, window)| {
                                        self.render_tool_output_window(
                                            &id,
                                            window,
                                            (position == 0).then_some(text.as_str()),
                                            tool.output_windows.len() > 1,
                                            cx,
                                        )
                                    }))
                                }),
                        ),
                )
            })
            .when(failed && !tool.expanded && !message.text.trim().is_empty(), |view| {
                let err_preview: String = message
                    .text
                    .lines()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .unwrap_or(message.text.trim())
                    .chars()
                    .take(160)
                    .collect();
                let copy_key = format!("tool-err:{id}");
                let copied = self.is_copied(&copy_key);
                let full_err = message.text.clone();
                view.child(
                    h_flex()
                        .ml(px(12.))
                        .pl_2p5()
                        .pr_1p5()
                        .py_1()
                        .gap_1p5()
                        .items_center()
                        .border_l_2()
                        .border_color(Color::Error.get(cx).opacity(0.55))
                        .bg(Color::ErrorSurface.get(cx).opacity(0.65))
                        .rounded_r_md()
                        .child(
                            h_flex()
                                .id(("tool-error-preview", index))
                                .flex_1()
                                .min_w_0()
                                .items_center()
                                .gap_1p5()
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.prepare_conversation_expansion(&expand_err_id);
                                    if let Some(msg) = this
                                        .messages
                                        .iter_mut()
                                        .find(|msg| msg.id == expand_err_id)
                                    {
                                        if let Some(tool) = msg.tool.as_mut() {
                                            tool.expanded = true;
                                            msg.layout_revision += 1;
                                            cx.notify();
                                        }
                                    }
                                }))
                                .child(
                                    Icon::new(Glyph::CircleAlert)
                                        .flex_shrink_0()
                                        .size(px(12.))
                                        .text_color(Color::Error.get(cx)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_xs()
                                        .text_color(Color::Error.get(cx))
                                        .child(err_preview),
                                ),
                        )
                        .child(
                            Button::new(("copy-tool-err-collapsed", index))
                                .custom(crate::palette::subtle_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(20.))
                                .w(px(20.))
                                .rounded_sm()
                                .icon(if copied { Glyph::Check } else { Glyph::Copy })
                                .tooltip(if copied { tr("已复制") } else { tr("复制完整错误") })
                                .accessibility_label(tr("复制错误详情"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        full_err.clone(),
                                    ));
                                    this.mark_copied(copy_key.clone(), cx);
                                })),
                        ),
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
mod tool_output_tests {
    use super::{MAX_DISPLAYED_TOOL_OUTPUT_BYTES, apply_tool_output_slice};
    use crate::conversation::{history_tool_output, message_text};
    use crate::protocol::ToolOutputSliceResponse;
    use serde_json::json;

    fn preview(head: &str, tail: &str) -> serde_json::Value {
        json!({
            "type": "text",
            "text": format!("{head}\n… [已截断 {} 字符]", tail.chars().count()),
            "totalBytes": head.len() + tail.len(),
            "loadedBytes": head.len(),
            "hasMoreBytes": true,
        })
    }

    fn slice(head: &str, tail: &str) -> ToolOutputSliceResponse {
        let end = (head.len() + tail.len()) as u64;
        ToolOutputSliceResponse {
            chunk: tail.to_owned(),
            next_byte_offset: end,
            total_bytes: end,
            has_more: false,
        }
    }

    #[test]
    fn tool_output_updates_independent_utf8_blocks_and_preserves_joined_text() {
        let head_a = "第一😀\n… [已截断 原始内容]";
        let head_b = "第二🧵";
        let message = json!({
            "role": "toolResult",
            "content": [
                {"type": "text", "text": "first"},
                {"type": "image"},
                {"type": "text", "text": ""},
                preview(head_a, "结束🧪"),
                {"type": "text", "text": null},
                {"type": "toolCall", "name": "inspect", "arguments": {}},
                preview(head_b, "结束🎉"),
                {"type": "text", "text": "last"},
            ],
        });
        let (mut text, mut windows) = history_tool_output("saved-tool", &message);
        assert_eq!(text, message_text(&message));
        assert_eq!(
            windows
                .iter()
                .map(|win| win.block_index)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        assert_eq!(windows[0].loaded_bytes, head_a.len() as u64);
        assert_eq!(windows[1].loaded_bytes, head_b.len() as u64);

        assert!(apply_tool_output_slice(
            &mut text,
            &mut windows,
            1,
            slice(head_b, "结束🎉")
        ));
        assert!(windows[0].has_more);
        assert!(!windows[1].has_more);
        assert_eq!(
            &text[windows[1].display_range.clone()],
            format!("{head_b}结束🎉")
        );
        assert!(apply_tool_output_slice(
            &mut text,
            &mut windows,
            0,
            slice(head_a, "结束🧪")
        ));
        assert_eq!(
            text,
            format!("first\n\n\n\n{head_a}结束🧪\n\ninspect\n{{}}\n\n{head_b}结束🎉\n\nlast")
        );
        assert_eq!(
            &text[windows[1].display_range.clone()],
            format!("{head_b}结束🎉")
        );
    }

    #[test]
    fn tool_output_window_rotates_just_the_selected_block_at_the_display_limit() {
        let prefix = "界".repeat(32_768);
        let neighbor = "🐝".repeat(12_000);
        let complete = "文".repeat(32_768);
        let new_chunk = "🧪".repeat(8_192);
        let message = json!({
            "role": "toolResult",
            "content": [
                {"type": "text", "text": "before"},
                preview(&prefix, &new_chunk),
                preview(&neighbor, "末尾"),
                {"type": "text", "text": complete},
            ],
        });
        let (mut text, mut windows) = history_tool_output("saved-tool", &message);
        let neighbor_preview = text[windows[1].display_range.clone()].to_owned();
        let mut invalid = slice(&prefix, &new_chunk);
        invalid.next_byte_offset -= 1;
        assert!(!apply_tool_output_slice(
            &mut text,
            &mut windows,
            0,
            invalid
        ));
        assert_eq!(text, message_text(&message));
        assert_eq!(windows[0].loaded_bytes, prefix.len() as u64);
        assert!(apply_tool_output_slice(
            &mut text,
            &mut windows,
            0,
            slice(&prefix, &new_chunk)
        ));
        assert_eq!(windows[0].window_start_bytes, prefix.len() as u64);
        assert_eq!(
            text,
            format!("before\n\n{new_chunk}\n\n{neighbor_preview}\n\n{complete}")
        );
        assert_eq!(&text[windows[1].display_range.clone()], neighbor_preview);
        assert!(windows[1].has_more);
        assert!(text.len() <= MAX_DISPLAYED_TOOL_OUTPUT_BYTES);
    }

    #[test]
    fn tool_output_load_keeps_an_oversized_preview_bounded_at_a_utf8_boundary() {
        let prefix = "a".repeat(32_768);
        let first = "a".repeat(32_765);
        let second = "🧪".repeat(8_192);
        let neighbor = "界".repeat(32_768);
        let message = json!({
            "role": "toolResult",
            "content": [
                preview(&prefix, &format!("{first}{second}")),
                {"type": "text", "text": neighbor},
                {"type": "text", "text": neighbor},
                {"type": "text", "text": neighbor},
            ],
        });
        let (mut text, mut windows) = history_tool_output("saved-tool", &message);
        let original_size = text.len();
        let first_slice = ToolOutputSliceResponse {
            total_bytes: windows[0].total_bytes,
            has_more: true,
            ..slice(&prefix, &first)
        };
        assert!(apply_tool_output_slice(
            &mut text,
            &mut windows,
            0,
            first_slice
        ));
        let second_slice = slice(&format!("{prefix}{first}"), &second);
        assert!(apply_tool_output_slice(
            &mut text,
            &mut windows,
            0,
            second_slice
        ));
        assert!(text.len() < original_size);
        assert_eq!(
            windows[0].window_start_bytes as usize,
            prefix.len() + first.len() + 4
        );
        assert_eq!(
            text,
            format!("{}\n\n{neighbor}\n\n{neighbor}\n\n{neighbor}", &second[4..])
        );
    }
}

#[cfg(test)]
mod status_tests {
    use super::{result_status, tool_action_label, tool_glyph};
    use gpui_kit::assets::IconName as Glyph;
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

    #[test]
    fn tool_glyph_and_action_labels_reflect_semantics_and_failure() {
        let _locale = crate::language::test_locale_guard();
        assert_eq!(tool_glyph("commandExecution", true), Glyph::CircleAlert);
        assert_eq!(tool_glyph("webSearch", false), Glyph::Globe);
        assert_eq!(tool_glyph("mcp__docs__search", false), Glyph::Wrench);
        assert_eq!(tool_glyph("fileChange", false), Glyph::SquarePen);
        assert_eq!(tool_glyph("read", false), Glyph::FileText);

        assert_eq!(
            tool_action_label("commandExecution", "执行中", None),
            "正在运行命令"
        );
        assert_eq!(
            tool_action_label("commandExecution", "失败", None),
            "命令失败"
        );
        assert_eq!(tool_action_label("read", "完成", None), "读取文件");
        assert_eq!(tool_action_label("webSearch", "完成", None), "网页搜索");
        assert_eq!(
            tool_action_label("fileChange", "完成", Some(&json!({"changes": [{}, {}]}))),
            "修改 2 个文件"
        );
        assert_eq!(tool_action_label("fileChange", "失败", None), "修改文件失败");
        assert_eq!(
            tool_action_label("fileChange", "失败", Some(&json!({"changes": [{}, {}]}))),
            "修改 2 个文件失败"
        );
        assert_eq!(tool_action_label("mcp__fs__stat", "完成", None), "fs / stat");
    }
}
