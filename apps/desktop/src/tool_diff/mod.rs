pub(crate) mod data;
use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        diff::{Diff, DiffMode, DiffState},
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
    loading: bool,
    scroll: ScrollHandle,
    raw_focus: FocusHandle,
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
        let viewer = cx.new(|cx: &mut Context<ToolDiff>| {
            let state = cx.new(|cx| DiffState::new([], cx));
            let view = ToolDiff {
                state,
                document: None,
                provenance,
                status,
                raw: false,
                loading: true,
                scroll: ScrollHandle::new(),
                raw_focus: cx.focus_handle().tab_stop(true),
            };
            cx.spawn(async move |this, cx| {
                let document = smol::unblock(move || {
                    data::build(data::changes(&name, &args, patch.as_deref()))
                })
                .await;
                let _ = this.update(cx, |this, cx| {
                    this.state
                        .update(cx, |state, cx| state.set_files(document.files.clone(), cx));
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
            dialog.title("工具改动").w(width).child(viewer.clone())
        });
    }
}
impl Render for ToolDiff {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = (window.viewport_size().height - px(310.))
            .max(px(120.))
            .min(px(600.));
        let rows = self
            .state
            .read(cx)
            .files()
            .iter()
            .map(|f| f.additions() + f.deletions() + 10)
            .sum::<usize>();
        let diff_height = px((rows as f32 * 22.).max(180.)).min(height);
        let has_files = self.document.as_ref().is_some_and(|d| !d.files.is_empty());
        v_flex()
            .min_w_0()
            .gap_3()
            .tab_group()
            .child(
                div()
                    .text_sm()
                    .text_color(Color::Secondary.get(cx))
                    .child(format!("{} · {}", self.provenance, self.status)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child("只读历史记录，与当前磁盘文件可能不同；不会应用或撤销改动。"),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("diff-unified")
                            .ghost()
                            .compact()
                            .label("合并视图")
                            .selected(!self.raw && self.state.read(cx).mode() == DiffMode::Unified)
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
                            .ghost()
                            .compact()
                            .label("并排视图")
                            .selected(!self.raw && self.state.read(cx).mode() == DiffMode::Split)
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
                        Button::new("diff-previous")
                            .ghost()
                            .compact()
                            .label("上一处")
                            .disabled(!has_files || self.raw)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.update(cx, |s, cx| s.previous_change(cx))
                            })),
                    )
                    .child(
                        Button::new("diff-next")
                            .ghost()
                            .compact()
                            .label("下一处")
                            .disabled(!has_files || self.raw)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.update(cx, |s, cx| s.next_change(cx))
                            })),
                    )
                    .child(
                        Button::new("diff-raw")
                            .ghost()
                            .compact()
                            .label("原始补丁")
                            .selected(self.raw)
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.raw = true;
                                window.focus(&this.raw_focus, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("diff-copy")
                            .ghost()
                            .compact()
                            .label("复制补丁")
                            .disabled(self.document.as_ref().is_none_or(|d| d.raw.is_empty()))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(d) = &this.document {
                                    cx.write_to_clipboard(ClipboardItem::new_string(d.raw.clone()));
                                }
                            })),
                    ),
            )
            .when(self.loading, |view| {
                view.child(div().text_sm().child("正在准备改动…"))
            })
            .when_some(
                self.document.as_ref().and_then(|d| d.error.clone()),
                |view, error| {
                    view.child(
                        div()
                            .text_sm()
                            .text_color(Color::Error.get(cx))
                            .child(error),
                    )
                },
            )
            .when(
                !self.loading
                    && !has_files
                    && self.document.as_ref().is_some_and(|d| d.error.is_none()),
                |view| view.child(div().text_sm().child("这段记录没有文本差异。")),
            )
            .when(!self.raw && has_files, |view| {
                view.child(
                    div()
                        .h(diff_height)
                        .min_w_0()
                        .child(Diff::new(&self.state).soft_wrap(true)),
                )
            })
            .when(self.raw, |view| {
                view.child(
                    crate::text_reading::region(
                        "tool-diff-raw".into(),
                        "原始补丁，可用方向键或翻页键滚动".into(),
                        &self.scroll,
                        height,
                        cx,
                    )
                    .track_focus(&self.raw_focus)
                    .font_family(Theme::global(cx).mono_font_family.clone())
                    .text_size(px(12.))
                    .line_height(px(19.))
                    .child(gpui_kit::base::SelectableText::new(
                        "tool-diff-source",
                        self.document
                            .as_ref()
                            .map(|d| d.raw.clone())
                            .unwrap_or_default(),
                    )),
                )
            })
    }
}
