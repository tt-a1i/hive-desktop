use super::*;
use crate::palette::Color;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        switch::Switch,
        Icon, *,
    },
    prelude::FluentBuilder,
    *,
};
impl Render for ConversationExport {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = (window.viewport_size().height - px(360.))
            .max(px(120.))
            .min(px(480.));
        let preview: String = self.text.chars().take(10000).collect();
        let description = if english() {
            format!(
                "Fixed snapshot of {} loaded records in Markdown. Includes attachment names and metadata and available image counts. Unsent drafts are excluded.",
                self.snapshot.count()
            )
        } else {
            format!(
                "{} 条已加载记录的固定快照 · Markdown 格式。只记录普通附件名称及元数据、可用的图片数量，未发送草稿不会导出。",
                self.snapshot.count()
            )
        };
        v_flex()
            .min_w_0()
            .gap_3()
            .child(
                v_flex()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .id("export-title")
                            .role(Role::Label)
                            .aria_label(self.snapshot.title.clone())
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .truncate()
                            .child(self.snapshot.title.clone()),
                    )
                    .child(
                        div()
                            .id("export-description")
                            .role(Role::Label)
                            .aria_label(description.clone())
                            .text_xs()
                            .line_height(relative(1.45))
                            .text_color(Color::Muted.get(cx))
                            .child(description),
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_4()
                    .items_center()
                    .px_3()
                    .py_2()
                    .rounded_xl()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Card.get(cx))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Secondary.get(cx))
                            .child(
                                Switch::new("export-thinking")
                                    .accessibility_label(tr("导出时包含思考过程"))
                                    .checked(self.thinking)
                                    .disabled(self.saving)
                                    .on_change(cx.listener(|this, value, _, cx| {
                                        this.thinking = *value;
                                        this.refresh(cx);
                                    })),
                            )
                            .child(tr("包含思考过程")),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Secondary.get(cx))
                            .child(
                                Switch::new("export-tools")
                                    .accessibility_label(tr("导出时包含工具过程"))
                                    .checked(self.tools)
                                    .disabled(self.saving)
                                    .on_change(cx.listener(|this, value, _, cx| {
                                        this.tools = *value;
                                        this.refresh(cx);
                                    })),
                            )
                            .child(tr("包含工具过程")),
                    ),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .rounded_xl()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Inset.get(cx).opacity(0.6))
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .px_3()
                            .py_1p5()
                            .border_b_1()
                            .border_color(Color::Divider.get(cx))
                            .bg(Color::Card.get(cx))
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        Icon::new(Glyph::FileText)
                                            .size(px(13.))
                                            .text_color(Color::Muted.get(cx)),
                                    )
                                    .child(
                                        div()
                                            .font_family(Theme::global(cx).mono_font_family.clone())
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Color::Secondary.get(cx))
                                            .child("Markdown"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(Color::Muted.get(cx))
                                    .child(format!("{} chars", preview.chars().count())),
                            ),
                    )
                    .child(
                        crate::text_reading::region(
                            "export-preview".into(),
                            tr("对话快照预览，可用方向键或翻页键滚动").into(),
                            &self.scroll,
                            height,
                            cx,
                        )
                        .border_0()
                        .rounded_none()
                        .p_3()
                        .font_family(Theme::global(cx).mono_font_family.clone())
                        .text_xs()
                        .line_height(px(19.))
                        .child(crate::text_reading::selectable_text("export-preview-text", preview)),
                    ),
            )
    }
}
impl Render for ExportFooter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let saving = self.export.read(cx).saving;
        let truncated = self.export.read(cx).text.chars().nth(10000).is_some();
        let notice = self.export.read(cx).notice.as_ref().map(ExportNotice::text);
        let error = self
            .export
            .read(cx)
            .error
            .as_ref()
            .map(|(key, error)| {
                format!("{}: {}", tr(key), crate::language::localize_owned_error(error))
            });
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .when(truncated, |view| {
                view.child(
                    div()
                        .id("export-preview-limit")
                        .role(Role::Label)
                        .aria_label(tr("预览仅显示前 10,000 字；复制和保存包含完整快照。"))
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("预览仅显示前 10,000 字；复制和保存包含完整快照。")),
                )
            })
            .when_some(notice, |view, notice| {
                view.child(
                    div()
                        .id("export-notice")
                        .role(Role::Status)
                        .aria_label(notice.clone())
                        .px_2p5()
                        .py_1p5()
                        .rounded_lg()
                        .bg(Color::AccentSoft.get(cx))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Color::Text.get(cx))
                        .child(notice),
                )
            })
            .when_some(error, |view, error| {
                view.child(
                    div()
                        .id("export-error")
                        .role(Role::Alert)
                        .aria_label(error.clone())
                        .px_2p5()
                        .py_1p5()
                        .rounded_lg()
                        .bg(Color::ErrorSurface.get(cx))
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_end()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("export-copy")
                            .outline()
                            .compact()
                            .h(px(32.))
                            .px_3()
                            .rounded_lg()
                            .icon(Glyph::Copy)
                            .label(tr("复制 Markdown"))
                            .accessibility_label(tr("复制完整对话快照"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.export.update(cx, |export, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        export.text.clone(),
                                    ));
                                    export.notice = Some(ExportNotice::Copied);
                                    cx.notify();
                                });
                            })),
                    )
                    .child(
                        Button::new("export-save")
                            .primary()
                            .compact()
                            .h(px(32.))
                            .px_3p5()
                            .rounded_lg()
                            .icon(Glyph::FileDown)
                            .label(if saving {
                                tr("正在保存…")
                            } else {
                                tr("保存 Markdown…")
                            })
                            .disabled(saving)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.export.update(cx, |export, cx| export.save(cx))
                            })),
                    ),
            )
    }
}
