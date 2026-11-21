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
use crate::git_workspace::patch_reader::PatchReader;
use serde_json::Value;

struct ThemeDiffColorGuard<'a> {
    cx: &'a mut App,
    prev_success: Hsla,
    prev_danger: Hsla,
}

impl<'a> ThemeDiffColorGuard<'a> {
    fn enter(palette: crate::diffs_settings::DiffPalette, cx: &'a mut App) -> Self {
        let (prev_success, prev_danger) = {
            let theme = Theme::global_mut(cx);
            let prev_success = theme.colors.success;
            let prev_danger = theme.colors.danger;
            theme.colors.success = palette.added.into();
            theme.colors.danger = palette.removed.into();
            (prev_success, prev_danger)
        };
        Self {
            cx,
            prev_success,
            prev_danger,
        }
    }
}

impl Drop for ThemeDiffColorGuard<'_> {
    fn drop(&mut self) {
        let theme = Theme::global_mut(self.cx);
        theme.colors.success = self.prev_success;
        theme.colors.danger = self.prev_danger;
    }
}

pub(crate) struct DiffPaletteScope {
    child: AnyElement,
    palette: crate::diffs_settings::DiffPalette,
}

impl DiffPaletteScope {
    pub(crate) fn wrap(child: impl IntoElement, cx: &App) -> AnyElement {
        let scheme = cx
            .try_global::<crate::diffs_settings::DiffPreferences>()
            .map(|prefs| prefs.settings.color_scheme)
            .unwrap_or_default();
        if scheme == crate::diffs_settings::DiffColorScheme::RedGreen {
            return child.into_any_element();
        }
        Self {
            child: child.into_any_element(),
            palette: crate::diffs_settings::palette(scheme, cx),
        }
        .into_any_element()
    }
}

impl IntoElement for DiffPaletteScope {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for DiffPaletteScope {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let guard = ThemeDiffColorGuard::enter(self.palette, cx);
        (self.child.request_layout(window, guard.cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let guard = ThemeDiffColorGuard::enter(self.palette, cx);
        self.child.prepaint(window, guard.cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let guard = ThemeDiffColorGuard::enter(self.palette, cx);
        self.child.paint(window, guard.cx);
    }
}

struct ToolDiff {
    state: Entity<DiffState>,
    document: Option<data::Document>,
    provenance: &'static str,
    status: &'static str,
    raw: bool,
    ignore_whitespace: bool,
    loading: bool,
    raw_reader: Option<PatchReader>,
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
    pub(crate) fn retain_tool_patch(&mut self, id: &str, result: &Value, cx: &mut Context<Self>) {
        if let Some(patch) = result["details"]["patch"].as_str() {
            if let Some(tool) = self
                .message_mut(id)
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
        let initial_inline = prefs.inline_diff.into_inline_unit();
        let initial_ignore_ws = prefs.ignore_whitespace;
        let collapse_multi = prefs.collapse_multi_file_diffs;
        let viewer = cx.new(|cx: &mut Context<ToolDiff>| {
            let state = cx.new(|cx| {
                DiffState::new([], cx)
                    .with_mode(initial_mode)
                    .with_inline_unit(initial_inline)
                    .with_inline_max_line_length(crate::diffs_settings::INLINE_DIFF_MAX_LINE_BYTES)
            });
            let view = ToolDiff {
                state,
                document: None,
                provenance,
                status,
                raw: false,
                ignore_whitespace: initial_ignore_ws,
                loading: true,
                raw_reader: None,
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

    pub(crate) fn open_turn_diff(
        &mut self,
        tool_changes: Vec<(String, Value, Option<String>)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let english_labels = english();
        let prefs = crate::diffs_settings::current(cx);
        let initial_mode = prefs.default_diff_mode.into_diff_mode();
        let initial_inline = prefs.inline_diff.into_inline_unit();
        let initial_ignore_ws = prefs.ignore_whitespace;
        let collapse_multi = prefs.collapse_multi_file_diffs;
        let viewer = cx.new(|cx: &mut Context<ToolDiff>| {
            let state = cx.new(|cx| {
                DiffState::new([], cx)
                    .with_mode(initial_mode)
                    .with_inline_unit(initial_inline)
                    .with_inline_max_line_length(crate::diffs_settings::INLINE_DIFF_MAX_LINE_BYTES)
            });
            let view = ToolDiff {
                state,
                document: None,
                provenance: "本轮工具改动汇总",
                status: "完成",
                raw: false,
                ignore_whitespace: initial_ignore_ws,
                loading: true,
                raw_reader: None,
                raw_focus: cx.focus_handle().tab_stop(true),
            };
            cx.spawn(async move |this, cx| {
                let document = smol::unblock(move || {
                    let combined: Vec<data::Change> = tool_changes
                        .into_iter()
                        .flat_map(|(name, args, patch)| {
                            data::changes(&name, &args, patch.as_deref(), english_labels)
                        })
                        .collect();
                    data::build(combined)
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
            dialog.title(tr("本轮改动")).w(width).child(viewer.clone())
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
                                let diff_palette = crate::diffs_settings::current_palette(cx);
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
                                                .text_color(diff_palette.added)
                                                .child(format!("+{total_additions}")),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(diff_palette.removed)
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
                                        this.raw_reader = None;
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
                                        this.raw_reader = None;
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
                            .child({
                                let inline_mode = crate::diffs_settings::InlineDiffMode::from_inline_unit(
                                    self.state.read(cx).inline_unit(),
                                );
                                Button::new("diff-inline-unit")
                                    .custom(segment_tab_variant(cx))
                                    .compact()
                                    .h(px(26.))
                                    .px_2p5()
                                    .rounded_md()
                                    .label(inline_mode.label())
                                    .tooltip(tr("切换行内局部差异高亮粒度（词级 / 字符级 / 关闭）"))
                                    .selected(!self.raw && inline_mode != crate::diffs_settings::InlineDiffMode::Off)
                                    .disabled(!has_files || self.raw)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let next = inline_mode.next();
                                        this.state.update(cx, |s, cx| {
                                            s.set_inline_unit(next.into_inline_unit(), cx);
                                        });
                                        cx.notify();
                                    }))
                            })
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
                                        this.raw_reader = None;
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
                        .child(DiffPaletteScope::wrap(
                            Diff::new(&self.state)
                                .soft_wrap(code_wrap)
                                .hover_highlight(DiffHoverHighlight::Both)
                                .change_indicator(DiffChangeIndicator::Signs)
                                .hunk_separator(DiffHunkSeparator::Metadata)
                                .rounded_xl()
                                .overflow_hidden()
                                .border_color(Color::Divider.get(cx)),
                            cx,
                        )),
                )
            })
            .when(self.raw, |view| {
                let reader = self.raw_reader.get_or_insert_with(|| {
                    let text = raw_patch_text(
                        self.document.as_ref().map(|d| d.raw.as_str()).unwrap_or_default(),
                        self.ignore_whitespace,
                    );
                    PatchReader::new(text.into()).without_copy_button()
                });
                let region = crate::text_reading::region(
                    "tool-diff-raw".into(),
                    tr("原始补丁，可用方向键或翻页键滚动").into(),
                    &reader.scroll,
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
                .text_color(Color::Text.get(cx));
                view.child(
                    div()
                        .min_w_0()
                        .rounded_xl()
                        .bg(Color::Card.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .overflow_hidden()
                        .child(
                            reader.view("tool-diff-source", region, |this| this.raw_reader.as_mut(), cx)
                        ),
                )
            })
    }
}

fn raw_patch_text(raw: &str, ignore_whitespace: bool) -> String {
    if !ignore_whitespace {
        return raw.to_owned();
    }
    let filtered = crate::diffs_settings::filter_patch_whitespace(raw);
    if filtered.trim().is_empty() && !raw.trim().is_empty() {
        tr("忽略空白后没有代码内容变更。").into()
    } else {
        filtered
    }
}


#[cfg(test)]
mod tests {
    use super::raw_patch_text;
    use crate::text_reading::TextPages;

    #[test]
    fn raw_tool_patch_pages_preserve_history_bytes_including_invalid_patch() {
        let raw = format!("invalid header\r\n{}+last", "+中文🦀\r\n".repeat(1200));
        let mut pages = TextPages::new(raw_patch_text(&raw, false).into());
        assert!(pages.count() > 1);
        let mut restored = String::new();
        for page in 0..pages.count() {
            pages.show(page);
            assert!(pages.visible.len() <= 32 * 1024);
            assert!(pages.visible.lines().count() <= 256);
            restored.push_str(&pages.visible);
        }
        assert_eq!(restored.as_bytes(), raw.as_bytes());
        assert_eq!(pages.full.as_bytes(), raw.as_bytes());
        assert_eq!(raw_patch_text(&raw, true), raw);
    }

    #[test]
    fn raw_tool_patch_keeps_existing_whitespace_filter_semantics() {
        let _locale = crate::language::test_locale_guard();
        let raw = "--- a/a\n+++ b/a\n@@ -1 +1 @@\n-old\n+new\n";
        assert_eq!(raw_patch_text(raw, false), raw);
        assert_eq!(raw_patch_text(raw, true), crate::diffs_settings::filter_patch_whitespace(raw));
        let whitespace = "--- a/a\n+++ b/a\n@@ -1 +1 @@\n-old\n+ old\n";
        assert_eq!(raw_patch_text(whitespace, true), crate::language::tr("忽略空白后没有代码内容变更。"));
        assert!(raw_patch_text("", true).is_empty());
    }

    #[gpui_kit::test]
    fn diff_palette_scope_restores_global_theme_colors_after_render(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use gpui_kit::{AppContext as _, BorrowAppContext as _, Bounds, ParentElement as _, Styled as _, WindowBounds, WindowOptions, component::{Theme, diff::*}, div, px};
        let _locale = crate::app::init_test_globals(cx);

        struct ScopeProbe {
            state: gpui_kit::Entity<DiffState>,
            captured_inside: std::rc::Rc<std::cell::Cell<Option<(gpui_kit::Hsla, gpui_kit::Hsla)>>>,
        }
        impl gpui_kit::Render for ScopeProbe {
            fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
                let captured = self.captured_inside.clone();
                div().size(px(400.)).child(super::DiffPaletteScope::wrap(
                    div()
                        .size_full()
                        .child(Diff::new(&self.state))
                        .child(gpui_kit::canvas(
                            move |_, _, cx| {
                                let t = Theme::global(cx);
                                captured.set(Some((t.colors.success, t.colors.danger)));
                            },
                            |_, _, _, _| {},
                        )),
                    cx,
                ))
            }
        }

        let captured = std::rc::Rc::new(std::cell::Cell::new(None));
        let (orig_success, orig_danger) = cx.update(|cx| {
            cx.update_global::<crate::diffs_settings::DiffPreferences, _>(|prefs, _| {
                prefs.settings.color_scheme = crate::diffs_settings::DiffColorScheme::BlueOrange;
            });
            let t = Theme::global(cx);
            (t.colors.success, t.colors.danger)
        });
        let captured_clone = captured.clone();
        let (window, _) = cx.update(|cx| {
            let bounds = Bounds::maximized(None, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    let files = DiffFile::parse("--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n").unwrap();
                    let state = cx.new(|cx| DiffState::new(files, cx));
                    cx.new(|_| ScopeProbe {
                        state,
                        captured_inside: captured_clone,
                    })
                },
            )
            .unwrap()
        });
        let vcx = gpui_kit::VisualTestContext::from_window(window, cx).into_mut();
        vcx.run_until_parked();

        let (expected_added, expected_removed) = cx.read(|cx| {
            let pal = crate::diffs_settings::current_palette(cx);
            (gpui_kit::Hsla::from(pal.added), gpui_kit::Hsla::from(pal.removed))
        });
        assert_eq!(captured.get(), Some((expected_added, expected_removed)));
        cx.read(|cx| {
            let t = Theme::global(cx);
            assert_eq!(t.colors.success, orig_success);
            assert_eq!(t.colors.danger, orig_danger);
        });
    }
}
