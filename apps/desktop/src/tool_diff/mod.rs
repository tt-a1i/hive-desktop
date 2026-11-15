pub(crate) mod data;
use crate::{app::HiveApp, palette::Color, language::{english,tr}};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonCustomVariant, ButtonVariants},
        diff::{
            Diff, DiffChangeIndicator, DiffFile, DiffHoverHighlight, DiffHunkSeparator, DiffMode,
            DiffState,
        },
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::Value;

struct ToolDiff {
    state: Entity<DiffState>,
    document: Option<data::Document>,
    provenance: &'static str,
    status: &'static str,
    raw: bool,
    ignore_whitespace: bool,
    loading: bool,
    scroll: ScrollHandle,
    raw_focus: FocusHandle,
}

fn compute_diff_files(document: &data::Document, ignore_whitespace: bool) -> Vec<DiffFile> {
    if !ignore_whitespace || document.raw.is_empty() {
        return document.files.clone();
    }
    let filtered = crate::diffs_settings::filter_patch_whitespace(&document.raw);
    if filtered == document.raw {
        return document.files.clone();
    }
    if filtered.trim().is_empty() {
        return Vec::new();
    }
    DiffFile::parse(&filtered).unwrap_or_else(|_| document.files.clone())
}

fn apply_tool_diff_files(
    state: &Entity<DiffState>,
    document: &data::Document,
    ignore_whitespace: bool,
    collapse_multi: bool,
    cx: &mut App,
) {
    let files = compute_diff_files(document, ignore_whitespace);
    let paths: Vec<String> = files.iter().map(|f| f.path().to_string()).collect();
    state.update(cx, |state, cx| {
        state.set_files(files, cx);
        if collapse_multi && paths.len() > 1 {
            for path in paths {
                state.set_file_collapsed(&path, true, cx);
            }
        }
    });
}
impl HiveApp {
    pub(crate) fn retain_tool_patch(&mut self, id: &str, result: &Value, cx: &App) {
        if let Some(patch) = result["details"]["patch"].as_str() {
            if let Some(tool) = self
                .messages
                .iter_mut()
                .find(|m| m.id == id)
                .and_then(|m| m.tool.as_mut())
            {
                tool.patch = Some(patch.into());
            }
            self.refresh_conversation_find(id, cx);
        }
    }
    pub(crate) fn open_tool_diff(
        &mut self,
        name: String,
        args: Value,
        patch: Option<String>,
        status: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let provenance = if patch.is_some() {
            "工具返回的实际补丁"
        } else if name == "fileChange" {
            "Codex 返回的文件改动记录"
        } else {
            "工具请求的改动片段；行号从片段起点计算，不代表编辑已成功"
        };
        let english_labels = english();
        let prefs = crate::diffs_settings::current(cx);
        let initial_mode = prefs.default_diff_mode.into_diff_mode();
        let initial_ignore_ws = prefs.ignore_whitespace;
        let collapse_multi = prefs.collapse_multi_file_diffs;
        let viewer = cx.new(|cx: &mut Context<ToolDiff>| {
            let state = cx.new(|cx| DiffState::new([], cx).with_mode(initial_mode));
            let view = ToolDiff {
                state,
                document: None,
                provenance,
                status,
                raw: false,
                ignore_whitespace: initial_ignore_ws,
                loading: true,
                scroll: ScrollHandle::new(),
                raw_focus: cx.focus_handle().tab_stop(true),
            };
            cx.spawn(async move |this, cx| {
                let document = smol::unblock(move || {
                    data::build(data::changes(&name, &args, patch.as_deref(), english_labels))
                })
                .await;
                let _ = this.update(cx, |this, cx| {
                    apply_tool_diff_files(
                        &this.state,
                        &document,
                        this.ignore_whitespace,
                        collapse_multi,
                        cx,
                    );
                    this.raw = document.error.is_some();
                    this.document = Some(document);
                    this.loading = false;
                    cx.notify();
                });
            })
            .detach();
            view
        });
        let width = (window.viewport_size().width - px(48.)).min(px(1120.));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(tr("工具改动")).w(width).child(viewer.clone())
        });
    }
}
fn segment_tab_variant(cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(gpui::rgba(0).into())
        .foreground(Color::Text.get(cx).into())
        .hover(Color::Card.get(cx).opacity(0.65).into())
        .active(Color::Card.get(cx).into())
}

impl Render for ToolDiff {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = (window.viewport_size().height - px(310.))
            .max(px(120.))
            .min(px(600.));
        let files = self.state.read(cx).files();
        let file_count = files.len();
        let total_additions: usize = files.iter().map(|f| f.additions()).sum();
        let total_deletions: usize = files.iter().map(|f| f.deletions()).sum();
        let rows: usize = files
            .iter()
            .map(|f| f.additions() + f.deletions() + 10)
            .sum();
        let diff_height = px((rows as f32 * 22.).max(180.)).min(height);
        let has_raw_files = self.document.as_ref().is_some_and(|d| !d.files.is_empty());
        let has_files = !files.is_empty();
        let mode = self.state.read(cx).mode();
        let code_size = f32::from(cx.global::<crate::appearance::Appearance>().code_font_size());
        let code_wrap = cx.global::<crate::appearance::Appearance>().code_word_wrap();
        let status_failed = self.status == "失败";
        let status_running = self.status == "执行中";
        v_flex()
            .min_w_0()
            .gap_2p5()
            .tab_group()
            .child(
                v_flex()
                    .gap_1()
                    .pb_2p5()
                    .border_b_1()
                    .border_color(Color::Divider.get(cx))
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .child(
                                        Icon::new(Glyph::FileDiff)
                                            .size(px(14.))
                                            .text_color(Color::Muted.get(cx)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Color::Text.get(cx))
                                            .child(tr(self.provenance)),
                                    )
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py(px(1.5))
                                            .rounded_md()
                                            .bg(if status_failed {
                                                Color::ErrorSurface.get(cx)
                                            } else if status_running {
                                                Color::AccentSoft.get(cx)
                                            } else {
                                                Color::Inset.get(cx)
                                            })
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if status_failed {
                                                Color::Error.get(cx)
                                            } else if status_running {
                                                Color::Accent.get(cx)
                                            } else {
                                                Color::Secondary.get(cx)
                                            })
                                            .child(tr(self.status)),
                                    ),
                            )
                            .when(has_files, |header| {
                                header.child(
                                    h_flex()
                                        .items_center()
                                        .gap_2()
                                        .font_family(Theme::global(cx).mono_font_family.clone())
                                        .text_xs()
                                        .child(
                                            div()
                                                .text_color(Color::Muted.get(cx))
                                                .child(if english() {
                                                    format!(
                                                        "{file_count} {}",
                                                        if file_count == 1 { "file" } else { "files" }
                                                    )
                                                } else {
                                                    format!("{file_count} 个文件")
                                                }),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Success.get(cx))
                                                .child(format!("+{total_additions}")),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Error.get(cx))
                                                .child(format!("−{total_deletions}")),
                                        ),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("只读历史记录，与当前磁盘文件可能不同；不会应用或撤销改动。")),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .p(px(2.))
                            .gap(px(2.))
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(
                                Button::new("diff-unified")
                                    .custom(segment_tab_variant(cx))
                                    .compact()
                                    .h(px(26.))
                                    .px_2p5()
                                    .rounded_md()
                                    .icon(Glyph::Rows2)
                                    .label(tr("合并视图"))
                                    .selected(!self.raw && mode == DiffMode::Unified)
                                    .disabled(!has_files)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.raw = false;
                                        window.focus(&this.state.read(cx).focus_handle(cx), cx);
                                        this.state
                                            .update(cx, |s, cx| s.set_mode(DiffMode::Unified, cx));
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("diff-split")
                                    .custom(segment_tab_variant(cx))
                                    .compact()
                                    .h(px(26.))
                                    .px_2p5()
                                    .rounded_md()
                                    .icon(Glyph::Columns2)
                                    .label(tr("并排视图"))
                                    .selected(!self.raw && mode == DiffMode::Split)
                                    .disabled(!has_files)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.raw = false;
                                        window.focus(&this.state.read(cx).focus_handle(cx), cx);
                                        this.state
                                            .update(cx, |s, cx| s.set_mode(DiffMode::Split, cx));
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("diff-raw")
                                    .custom(segment_tab_variant(cx))
                                    .compact()
                                    .h(px(26.))
                                    .px_2p5()
                                    .rounded_md()
                                    .icon(Glyph::Code)
                                    .label(tr("原始补丁"))
                                    .selected(self.raw)
                                    .disabled(self.loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.raw = true;
                                        window.focus(&this.raw_focus, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("diff-ignore-ws")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .h(px(26.))
                                    .px_2()
                                    .rounded_md()
                                    .label(tr("忽略空白"))
                                    .selected(self.ignore_whitespace)
                                    .toggled(self.ignore_whitespace)
                                    .disabled(self.loading || self.document.as_ref().is_none_or(|d| d.raw.is_empty()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.ignore_whitespace = !this.ignore_whitespace;
                                        if let Some(doc) = &this.document {
                                            apply_tool_diff_files(
                                                &this.state,
                                                doc,
                                                this.ignore_whitespace,
                                                false,
                                                cx,
                                            );
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .p(px(2.))
                                    .gap(px(2.))
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(
                                        Button::new("diff-previous")
                                            .custom(crate::palette::quiet_icon_variant(cx))
                                            .compact()
                                            .h(px(26.))
                                            .px_2()
                                            .rounded_md()
                                            .icon(Glyph::ChevronUp)
                                            .label(tr("上一处"))
                                            .disabled(!has_files || self.raw)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.state
                                                    .update(cx, |s, cx| s.previous_change(cx))
                                            })),
                                    )
                                    .child(
                                        Button::new("diff-next")
                                            .custom(crate::palette::quiet_icon_variant(cx))
                                            .compact()
                                            .h(px(26.))
                                            .px_2()
                                            .rounded_md()
                                            .icon(Glyph::ChevronDown)
                                            .label(tr("下一处"))
                                            .disabled(!has_files || self.raw)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.state.update(cx, |s, cx| s.next_change(cx))
                                            })),
                                    ),
                            )
                            .child(
                                Button::new("diff-copy")
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .h(px(30.))
                                    .px_2p5()
                                    .rounded_lg()
                                    .icon(Glyph::Copy)
                                    .label(tr("复制补丁"))
                                    .disabled(self.document.as_ref().is_none_or(|d| d.raw.is_empty()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(d) = &this.document {
                                            let content = if this.ignore_whitespace {
                                                crate::diffs_settings::filter_patch_whitespace(&d.raw)
                                            } else {
                                                d.raw.clone()
                                            };
                                            cx.write_to_clipboard(ClipboardItem::new_string(content));
                                        }
                                    })),
                            ),
                    ),
            )
            .when(self.loading, |view| {
                view.child(
                    div()
                        .p_4()
                        .rounded_xl()
                        .bg(Color::Inset.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .text_sm()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("正在准备改动…")),
                )
            })
            .when_some(
                self.document
                    .as_ref()
                    .and_then(|document| document.error.as_ref())
                    .map(|error| match error {
                        data::ParseFailure::NoFiles => {
                            tr("补丁未包含可显示的文件；下方保留原始内容。").to_owned()
                        }
                        data::ParseFailure::Invalid(error) => {
                            format!("{}: {error}", tr("无法解析补丁，保留原始内容"))
                        }
                    }),
                |view, error| {
                    view.child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .bg(Color::ErrorSurface.get(cx))
                            .border_1()
                            .border_color(Color::Error.get(cx).opacity(0.24))
                            .child(
                                Icon::new(Glyph::CircleAlert)
                                    .size(px(14.))
                                    .text_color(Color::Error.get(cx)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Error.get(cx))
                                    .child(error),
                            ),
                    )
                },
            )
            .when(
                !self.loading
                    && !has_files
                    && self.document.as_ref().is_some_and(|d| d.error.is_none()),
                |view| {
                    view.child(
                        div()
                            .p_4()
                            .rounded_xl()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .text_sm()
                            .text_color(Color::Muted.get(cx))
                            .child(if self.ignore_whitespace && has_raw_files {
                                tr("忽略空白后没有代码内容变更。")
                            } else {
                                tr("这段记录没有文本差异。")
                            }),
                    )
                },
            )
            .when(!self.raw && has_files, |view| {
                view.child(
                    div()
                        .h(diff_height)
                        .min_w_0()
                        .rounded_xl()
                        .bg(Color::Card.get(cx))
                        .overflow_hidden()
                        .child(
                            Diff::new(&self.state)
                                .soft_wrap(code_wrap)
                                .hover_highlight(DiffHoverHighlight::Both)
                                .change_indicator(DiffChangeIndicator::Signs)
                                .hunk_separator(DiffHunkSeparator::Metadata)
                                .rounded_xl()
                                .overflow_hidden()
                                .border_color(Color::Divider.get(cx)),
                        ),
                )
            })
            .when(self.raw, |view| {
                view.child(
                    div()
                        .min_w_0()
                        .rounded_xl()
                        .bg(Color::Card.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .overflow_hidden()
                        .child(
                            crate::text_reading::region(
                                "tool-diff-raw".into(),
                                tr("原始补丁，可用方向键或翻页键滚动").into(),
                                &self.scroll,
                                height,
                                cx,
                            )
                            .border_0()
                            .rounded_none()
                            .px_3p5()
                            .py_3()
                            .track_focus(&self.raw_focus)
                            .font_family(Theme::global(cx).mono_font_family.clone())
                            .text_size(px(code_size))
                            .line_height(px((code_size * 1.6).round()))
                            .when(code_wrap, |v| v.whitespace_normal())
                            .text_color(Color::Text.get(cx))
                            .child(crate::text_reading::selectable_text(
                                "tool-diff-source",
                                self.document
                                    .as_ref()
                                    .map(|d| {
                                        if self.ignore_whitespace {
                                            let filtered = crate::diffs_settings::filter_patch_whitespace(&d.raw);
                                            if filtered.trim().is_empty() && !d.raw.trim().is_empty() {
                                                tr("忽略空白后没有代码内容变更。").into()
                                            } else {
                                                filtered
                                            }
                                        } else {
                                            d.raw.clone()
                                        }
                                    })
                                    .unwrap_or_default(),
                            )),
                        ),
                )
            })
    }
}
