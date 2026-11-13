use super::*;
use crate::focus_scroll::{Reveal, Viewport};
use crate::palette::Color;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        scroll::ScrollableElement,
        Icon, *,
    },
    prelude::FluentBuilder,
    *,
};
impl Render for Resources {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale_english = english();
        if self.locale_english != locale_english {
            self.query.update(cx, |input, cx| {
                input.set_placeholder(tr("搜索名称、完整说明、来源或路径…"), window, cx)
            });
            self.locale_english = locale_english;
        }
        let query = self.query.read(cx).value().to_lowercase();
        let visible: Vec<_> = self
            .commands
            .iter()
            .filter(|command| {
                (self.filter == "all" || command.source == self.filter)
                    && (query.is_empty()
                        || format!(
                            "{} {} {} {} {} {} {}",
                            command.name,
                            command.description.as_deref().unwrap_or(""),
                            kind(&command.source),
                            scope(command),
                            command.source_info,
                            kind_key(&command.source).unwrap_or(""),
                            tr(scope(command))
                        )
                        .to_lowercase()
                        .contains(&query))
            })
            .cloned()
            .collect();
        let selected_state = window.use_keyed_state("session-resource-selected", cx, |_, _| 0usize);
        let selected_idx = (*selected_state.read(cx)).min(visible.len().saturating_sub(1));
        let narrow = window.viewport_size().width < px(720.);
        let height = (window.viewport_size().height - px(320.))
            .max(px(160.))
            .min(px(520.));
        v_flex()
            .min_w_0()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(tr("只显示此 Pi 会话实际加载的资源。重新读取只更新目录；新安装的资源需由原生后端重新加载。")),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .rounded_lg()
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .bg(Color::Card.get(cx))
                            .px_1()
                            .child(
                                Input::new(&self.query)
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .cleanable(true)
                                    .prefix(
                                        Icon::new(Glyph::Search)
                                            .size(px(14.))
                                            .text_color(Color::Muted.get(cx)),
                                    ),
                            ),
                    )
                    .child(
                        Button::new("resources-refresh")
                            .outline()
                            .compact()
                            .h(px(30.))
                            .px_2p5()
                            .rounded_lg()
                            .icon(Glyph::RefreshCw)
                            .label(if self.loading { tr("读取中…") } else { tr("重新读取") })
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        h_flex()
                            .p_0p5()
                            .gap_0p5()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .children([
                                ("all", tr("全部")),
                                ("skill", tr("技能")),
                                ("prompt", tr("提示模板")),
                                ("extension", tr("扩展")),
                            ]
                            .into_iter()
                            .map(|(filter, label)| {
                                let count = self
                                    .commands
                                    .iter()
                                    .filter(|command| filter == "all" || command.source == filter)
                                    .count();
                                let selected_state = selected_state.clone();
                                Button::new(SharedString::from(format!("resource-filter-{filter}")))
                                    .ghost()
                                    .compact()
                                    .h(px(26.))
                                    .px_2p5()
                                    .rounded_md()
                                    .label(format!("{label} {count}"))
                                    .selected(self.filter == filter)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.filter = filter;
                                        this.scroll.set_offset(point(px(0.), px(0.)));
                                        selected_state.update(cx, |idx, _| *idx = 0);
                                        cx.notify();
                                    }))
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(if english() {
                                format!(
                                    "Showing {} of {} resources · Adding to the draft does not send or execute",
                                    visible.len(),
                                    self.commands.len()
                                )
                            } else {
                                format!(
                                    "显示 {} / {} 项 · 补入草稿不会发送或执行",
                                    visible.len(),
                                    self.commands.len()
                                )
                            }),
                    ),
            )
            .when_some(self.error.as_ref().map(|error| error.text()), |view, error| {
                view.child(
                    div()
                        .p_3()
                        .rounded_xl()
                        .bg(Color::ErrorSurface.get(cx))
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when(visible.is_empty() && !self.loading, |view| {
                view.child(
                    div()
                        .p_6()
                        .rounded_xl()
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .bg(Color::Inset.get(cx))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(Color::Muted.get(cx))
                        .child(if self.commands.is_empty() {
                            tr("当前会话没有返回可浏览的技能、模板或扩展命令。")
                        } else {
                            tr("没有匹配的资源。")
                        }),
                )
            })
            .when(!visible.is_empty(), |view| {
                let detail = visible[selected_idx].clone();
                let list_pane = div()
                    .when(!narrow, |pane| {
                        pane.w(px(270.))
                            .flex_shrink_0()
                            .h(height)
                            .border_r_1()
                            .border_color(Color::Divider.get(cx))
                    })
                    .when(narrow, |pane| pane.w_full().max_h(px(190.)))
                    .min_w_0()
                    .flex()
                    .child(Viewport::new(
                        "session-resource-viewport",
                        &self.scroll,
                        v_flex()
                            .id("session-resource-results")
                            .w_full()
                            .flex_1()
                            .min_w_0()
                            .p_1p5()
                            .gap_0p5()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .vertical_scrollbar(&self.scroll)
                            .children(visible.into_iter().enumerate().map(|(index, command)| {
                                let is_selected = index == selected_idx;
                                let selected_state = selected_state.clone();
                                let kind_label = kind(&command.source).to_owned();
                                Reveal::new(
                                    ("resource-card-reveal", index),
                                    &self.scroll,
                                    Button::new(("resource-select", index))
                                        .ghost()
                                        .compact()
                                        .w_full()
                                        .h_auto()
                                        .px_2p5()
                                        .py_2()
                                        .rounded_lg()
                                        .justify_start()
                                        .selected(is_selected)
                                        .child(
                                            v_flex()
                                                .w_full()
                                                .min_w_0()
                                                .items_start()
                                                .gap_0p5()
                                                .child(
                                                    h_flex()
                                                        .w_full()
                                                        .min_w_0()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .font_family(Theme::global(cx).mono_font_family.clone())
                                                                .text_size(px(12.5))
                                                                .font_weight(if is_selected {
                                                                    FontWeight::SEMIBOLD
                                                                } else {
                                                                    FontWeight::MEDIUM
                                                                })
                                                                .text_color(Color::Text.get(cx))
                                                                .truncate()
                                                                .child(format!("/{}", command.name)),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_shrink_0()
                                                                .px_1p5()
                                                                .py(px(1.))
                                                                .rounded_md()
                                                                .bg(Color::Inset.get(cx))
                                                                .text_size(px(10.5))
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(kind_label),
                                                        ),
                                                )
                                                .when_some(
                                                    command.description.filter(|s| !s.is_empty()),
                                                    |col, desc| {
                                                        col.child(
                                                            div()
                                                                .w_full()
                                                                .truncate()
                                                                .text_xs()
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(desc),
                                                        )
                                                    },
                                                ),
                                        )
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            selected_state.update(cx, |idx, _| *idx = index);
                                            cx.notify();
                                        })),
                                )
                            })),
                    ));
                let name = detail.name.clone();
                let copied = format!("/{}", detail.name);
                let path = local_path(&detail);
                let preview = path.clone();
                let source_origin = detail.source_info["source"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                let raw_path = detail.source_info["path"].as_str().map(str::to_owned);
                let non_local_notice =
                    detail.source_info["path"].as_str().is_some() && path.is_none();
                let detail_pane = v_flex()
                    .flex_1()
                    .min_w_0()
                    .when(!narrow, |pane| pane.h(height))
                    .p_4()
                    .gap_3()
                    .bg(Color::Inset.get(cx).opacity(0.28))
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .truncate()
                                    .child(format!("/{}", detail.name)),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .bg(Color::Card.get(cx))
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Color::Secondary.get(cx))
                                    .child(format!("{} · {}", kind(&detail.source), tr(scope(&detail)))),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_height(relative(1.5))
                            .text_color(Color::Text.get(cx))
                            .child(
                                detail
                                    .description
                                    .clone()
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or_else(|| tr("此资源未提供说明。").into()),
                            ),
                    )
                    .when_some(source_origin, |col, source| {
                        col.child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(if english() {
                                    format!(
                                        "{}: {}",
                                        tr("来源"),
                                        match source.as_str() {
                                            "auto" => tr("原生自动发现"),
                                            "builtin" => tr("原生内置"),
                                            _ => &source,
                                        }
                                    )
                                } else {
                                    format!(
                                        "{}：{}",
                                        tr("来源"),
                                        match source.as_str() {
                                            "auto" => tr("原生自动发现"),
                                            "builtin" => tr("原生内置"),
                                            _ => &source,
                                        }
                                    )
                                }),
                        )
                    })
                    .when_some(raw_path, |col, path_text| {
                        col.child(
                            div()
                                .p_2p5()
                                .rounded_lg()
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .bg(Color::Card.get(cx))
                                .child(
                                    crate::text_reading::selectable_text(
                                        format!("resource-path-{selected_idx}"),
                                        path_text,
                                    )
                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx)),
                                ),
                        )
                    })
                    .when(non_local_notice, |col| {
                        col.child(
                            div()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(tr("原生内置或非本地文件来源，不提供文件预览。")),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .pt_3()
                            .border_t_1()
                            .border_color(Color::Divider.get(cx))
                            .gap_2()
                            .flex_wrap()
                            .child(
                                Button::new(("resource-use", selected_idx))
                                    .primary()
                                    .compact()
                                    .h(px(30.))
                                    .px_3()
                                    .rounded_lg()
                                    .label(tr("补入草稿"))
                                    .accessibility_label(format!("{}: /{}", tr("补入草稿"), detail.name))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.use_command(name.clone(), window, cx)
                                    })),
                            )
                            .child(
                                Button::new(("resource-copy", selected_idx))
                                    .outline()
                                    .compact()
                                    .h(px(30.))
                                    .px_2p5()
                                    .rounded_lg()
                                    .icon(Glyph::Copy)
                                    .label(tr("复制命令"))
                                    .accessibility_label(format!("{}: /{}", tr("复制命令"), detail.name))
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
                                    })),
                            )
                            .when_some(preview, |row, path| {
                                row.child(
                                    Button::new(("resource-preview", selected_idx))
                                        .outline()
                                        .compact()
                                        .h(px(30.))
                                        .px_2p5()
                                        .rounded_lg()
                                        .icon(Glyph::Eye)
                                        .label(tr("查看来源文件"))
                                        .accessibility_label(format!("{}: /{}", tr("查看来源文件"), detail.name))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.preview(path.clone(), window, cx)
                                        })),
                                )
                            })
                            .when_some(path, |row, path| {
                                let reveal = path.clone();
                                row.child(
                                    Button::new(("resource-copy-path", selected_idx))
                                        .ghost()
                                        .compact()
                                        .h(px(30.))
                                        .px_2p5()
                                        .rounded_lg()
                                        .label(tr("复制路径"))
                                        .accessibility_label(format!("{}: /{}", tr("复制路径"), detail.name))
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))
                                        })),
                                )
                                .child(
                                    Button::new(("resource-reveal", selected_idx))
                                        .ghost()
                                        .compact()
                                        .h(px(30.))
                                        .px_2p5()
                                        .rounded_lg()
                                        .icon(Glyph::ExternalLink)
                                        .label(tr("定位文件"))
                                        .accessibility_label(format!("{}: /{}", tr("定位文件"), detail.name))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this.current(cx) {
                                                cx.reveal_path(std::path::Path::new(&reveal));
                                            } else {
                                                this.stale(cx);
                                            }
                                        })),
                                )
                            }),
                    );
                let container = div()
                    .w_full()
                    .min_w_0()
                    .rounded_xl()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Card.get(cx))
                    .overflow_hidden();
                let container = if narrow {
                    container.flex().flex_col().child(list_pane).child(
                        div()
                            .border_t_1()
                            .border_color(Color::Divider.get(cx))
                            .child(detail_pane),
                    )
                } else {
                    container
                        .flex()
                        .flex_row()
                        .items_stretch()
                        .child(list_pane)
                        .child(detail_pane)
                };
                view.child(container)
            })
    }
}
