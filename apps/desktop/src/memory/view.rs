use super::*;
use crate::chrome::{ResponsivePane, SubviewTopbar};
use crate::focus_scroll::{Reveal, Viewport};
use crate::palette::{self, Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
};

fn memory_status_dot(status: &str, cx: &App) -> (Rgba, Rgba) {
    match status {
        "active" => (Color::Text.get(cx), Color::Success.get(cx)),
        "candidate" => (Color::Text.get(cx), crate::chrome::attention_dot(cx)),
        "rejected" => (Color::Secondary.get(cx), Color::Error.get(cx)),
        "archived" => (Color::Secondary.get(cx), Color::Muted.get(cx)),
        _ => (Color::Text.get(cx), Color::Muted.get(cx)),
    }
}

impl HiveApp {
    pub fn render_memory(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let m = &self.memory;
        let scroll = &m.scroll;
        let disabled = !self.connected || m.loading || m.acting;

        let selected_entry = m
            .selected
            .as_ref()
            .and_then(|id| m.entries.iter().enumerate().find(|(_, entry)| &entry.id == id));

        let topbar = SubviewTopbar::new(
            "memory-topbar",
            tr("工作区记忆"),
            self.sidebar_open,
            cx.weak_entity(),
        )
        .back(self.return_to_chat_button(cx).icon(Glyph::ArrowLeft))
        .badge(format!("{}", m.entries.len()))
        .action(
            Button::new("refresh-memory")
                .custom(palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(28.))
                .w(px(28.))
                .rounded_lg()
                .icon(Glyph::RefreshCw)
                .tooltip(tr("刷新"))
                .accessibility_label(tr("刷新"))
                .disabled(disabled)
                .on_click(cx.listener(|this, _, _, cx| this.load_memory(cx))),
        );

        let left_pane = v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .bg(Color::Surface.get(cx))
            .child(Viewport::new(
                "memory-viewport",
                scroll,
                v_flex()
                    .id("memory-scroll")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(scroll)
                    .vertical_scrollbar(scroll)
                    .p_3()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_shrink_0()
                            .gap_2()
                            .p_3()
                            .rounded_xl()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(Reveal::new(
                                "memory-injection-reveal",
                                scroll,
                                h_flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .items_center()
                                            .gap_1p5()
                                            .when_some(m.enabled, |row, enabled| {
                                                row.child(div().size(px(6.)).rounded_full().bg(
                                                    if enabled {
                                                        Color::Success.get(cx)
                                                    } else {
                                                        Color::Muted.get(cx)
                                                    },
                                                ))
                                            })
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_size(px(12.5))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(match m.enabled {
                                                        Some(true) => tr("工作区记忆注入：已启用"),
                                                        Some(false) => tr("工作区记忆注入：已禁用"),
                                                        None if m.loading => tr("正在读取注入设置…"),
                                                        None => tr("注入设置暂不可用"),
                                                    }),
                                            ),
                                    )
                                    .when_some(m.enabled, |view, enabled| {
                                        view.child(
                                            Button::new("toggle-memory-injection")
                                                .when(enabled, |b| b.outline())
                                                .when(!enabled, |b| b.primary())
                                                .compact()
                                                .xsmall()
                                                .h(px(26.))
                                                .rounded_md()
                                                .label(if enabled {
                                                    tr("禁用注入")
                                                } else {
                                                    tr("启用注入")
                                                })
                                                .disabled(disabled)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.memory_action(
                                                        "PUT",
                                                        "/settings".into(),
                                                        Some(json!({"enabled": !enabled})),
                                                        cx,
                                                    )
                                                })),
                                        )
                                    }),
                            ))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("设置影响后续构建的会话与派单上下文；已发送给 Agent 的记忆不会从已有对话中移除。")),
                            )
                            .when_some(m.settings_error.clone(), |view, error| {
                                let message = format!(
                                    "{}: {}",
                                    tr("读取注入设置失败"),
                                    crate::language::localize_owned_error(&error)
                                );
                                view.child(
                                    div()
                                        .id("memory-settings-error")
                                        .role(Role::Alert)
                                        .aria_label(message.clone())
                                        .text_xs()
                                        .text_color(Color::Error.get(cx))
                                        .child(message),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_shrink_0()
                            .gap_2()
                            .child(Reveal::new(
                                "memory-search-reveal",
                                scroll,
                                h_flex()
                                    .gap_1p5()
                                    .child(
                                        div().flex_1().min_w_0().child(
                                            Input::new(&m.query)
                                                .aria_label(tr("搜索工作区记忆"))
                                                .disabled(disabled),
                                        ),
                                    )
                                    .child(
                                        Button::new("search-memory")
                                            .outline()
                                            .small()
                                            .rounded_lg()
                                            .label(tr("搜索"))
                                            .disabled(disabled)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.memory.selected = None;
                                                this.memory.scroll.set_offset(point(px(0.), px(0.)));
                                                this.load_memory(cx);
                                            })),
                                    ),
                            ))
                            .child(Reveal::new(
                                "memory-status-reveal",
                                scroll,
                                h_flex()
                                    .flex_wrap()
                                    .gap_1()
                                    .p_1()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .children(
                                        [
                                            ("active", tr("有效")),
                                            ("candidate", tr("待审核")),
                                            ("archived", tr("已归档")),
                                            ("rejected", tr("已拒绝")),
                                            ("all", tr("全部状态")),
                                        ]
                                        .into_iter()
                                        .enumerate()
                                        .map(|(i, (status, label))| {
                                            Button::new(("memory-status", i))
                                                .compact()
                                                .xsmall()
                                                .h(px(24.))
                                                .rounded_md()
                                                .label(label)
                                                .selected(m.status == status)
                                                .disabled(disabled)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.memory.status = status;
                                                    this.memory.selected = None;
                                                    this.memory
                                                        .scroll
                                                        .set_offset(point(px(0.), px(0.)));
                                                    this.load_memory(cx);
                                                }))
                                        }),
                                    ),
                            ))
                            .child(Reveal::new(
                                "memory-scope-reveal",
                                scroll,
                                h_flex()
                                    .flex_wrap()
                                    .gap_1()
                                    .p_1()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .children(
                                        [
                                            ("workspace", tr("工作区范围")),
                                            ("user", tr("用户范围")),
                                            ("all", tr("全部范围")),
                                        ]
                                        .into_iter()
                                        .enumerate()
                                        .map(|(i, (scope, label))| {
                                            Button::new(("memory-scope", i))
                                                .compact()
                                                .xsmall()
                                                .h(px(24.))
                                                .rounded_md()
                                                .label(label)
                                                .selected(m.scope == scope)
                                                .disabled(disabled)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.memory.scope = scope;
                                                    this.memory.selected = None;
                                                    this.memory
                                                        .scroll
                                                        .set_offset(point(px(0.), px(0.)));
                                                    this.load_memory(cx);
                                                }))
                                        }),
                                    ),
                            )),
                    )
                    .when(m.loading, |view| {
                        view.child(
                            div()
                                .id("memory-loading")
                                .role(Role::Status)
                                .aria_label(tr("正在读取记忆…"))
                                .px_1()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(tr("正在读取记忆…")),
                        )
                    })
                    .when_some(m.error.as_ref().map(|error| error.text()), |view, error| {
                        view.child(
                            div()
                                .id("memory-error")
                                .role(Role::Alert)
                                .aria_label(error.clone())
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .text_xs()
                                .text_color(Color::Error.get(cx))
                                .child(error),
                        )
                    })
                    .when_some(m.notice.map(tr), |view, notice| {
                        view.child(
                            div()
                                .id("memory-notice")
                                .role(Role::Status)
                                .aria_label(notice)
                                .px_3()
                                .py_1p5()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(notice),
                        )
                    })
                    .when(!m.loading && m.error.is_none() && m.entries.is_empty(), |view| {
                        view.child(
                            v_flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .py_10()
                                .px_4()
                                .rounded_xl()
                                .bg(Color::Card.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .child(
                                    Icon::new(Glyph::BookOpen)
                                        .size(px(18.))
                                        .text_color(Color::Muted.get(cx)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("没有匹配的记忆")),
                                ),
                        )
                    })
                    .child(
                        div()
                            .px_1()
                            .text_size(px(11.))
                            .text_color(Color::Muted.get(cx))
                            .child(if english() {
                                format!(
                                    "Showing {} entries (up to 50). Search or filter to narrow the list.",
                                    m.entries.len()
                                )
                            } else {
                                format!(
                                    "显示 {} 条，最多 50 条。可搜索或筛选缩小范围。",
                                    m.entries.len()
                                )
                            }),
                    )
                    .children(
                        m.entries
                            .iter()
                            .enumerate()
                            .map(|(i, entry)| self.render_memory_row(i, entry, cx)),
                    ),
            ));

        let right_pane = if let Some((idx, entry)) = selected_entry {
            self.render_memory_detail(idx, entry, cx).into_any_element()
        } else {
            v_flex()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .p_8()
                .child(
                    v_flex()
                        .items_center()
                        .gap_2()
                        .max_w(px(320.))
                        .child(
                            div()
                                .size(px(38.))
                                .rounded_xl()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(Glyph::BookOpen)
                                        .size(px(18.))
                                        .text_color(Color::Muted.get(cx)),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Color::Text.get(cx))
                                .child(tr("工作区记忆")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_center()
                                .text_color(Color::Muted.get(cx))
                                .child(if m.entries.is_empty() {
                                    tr("没有匹配的记忆")
                                } else {
                                    tr("设置影响后续构建的会话与派单上下文；已发送给 Agent 的记忆不会从已有对话中移除。")
                                }),
                        ),
                )
                .into_any_element()
        };

        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_hidden()
            .bg(Color::Surface.get(cx))
            .child(topbar)
            .when(self.error.is_some(), |view| {
                view.child(
                    div()
                        .px_4()
                        .pt_3()
                        .flex_shrink_0()
                        .child(self.render_error_banner(cx)),
                )
            })
            .child(
                ResponsivePane::new(left_pane, right_pane)
                    .left_width(px(380.))
                    .detail_active(selected_entry.is_some()),
            )
    }

    fn render_memory_row(
        &self,
        i: usize,
        entry: &MemoryEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let selected = self.memory.selected.as_ref() == Some(&entry.id);
        let id = entry.id.clone();
        let scroll = &self.memory.scroll;
        let (badge_fg, badge_dot) = memory_status_dot(&entry.status, cx);

        Reveal::new(
            ("memory-detail-reveal", i),
            scroll,
            div()
                .rounded_xl()
                .bg(if selected {
                    Color::Inset.get(cx)
                } else {
                    Color::Card.get(cx)
                })
                .border_1()
                .border_color(if selected {
                    Color::Accent.get(cx)
                } else {
                    Color::Divider.get(cx)
                })
                .child(
                    Button::new(("memory-detail", i))
                        .ghost()
                        .w_full()
                        .min_w_0()
                        .h_auto()
                        .p_3()
                        .rounded_xl()
                        .justify_start()
                        .selected(selected)
                        .accessibility_label(if english() {
                            format!(
                                "{} memory details: {}",
                                if selected { tr("收起") } else { tr("打开") },
                                entry.body.chars().take(80).collect::<String>()
                            )
                        } else {
                            format!(
                                "{}记忆详情：{}",
                                if selected { tr("收起") } else { tr("打开") },
                                entry.body.chars().take(80).collect::<String>()
                            )
                        })
                        .child(
                            v_flex()
                                .w_full()
                                .min_w_0()
                                .items_start()
                                .gap_1p5()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .flex_wrap()
                                        .items_center()
                                        .gap_1p5()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap(px(5.))
                                                .rounded_md()
                                                .bg(Color::Inset.get(cx))
                                                .border_1()
                                                .border_color(Color::Divider.get(cx))
                                                .text_color(badge_fg)
                                                .px_2()
                                                .py(px(1.))
                                                .text_size(px(11.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(
                                                    div()
                                                        .size(px(6.))
                                                        .rounded_full()
                                                        .bg(badge_dot),
                                                )
                                                .child(status_label(&entry.status).to_owned()),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_size(px(11.))
                                                .text_color(Color::Muted.get(cx))
                                                .child(format!(
                                                    "{} · {}{}{}",
                                                    kind_label(&entry.kind),
                                                    if entry.scope == "user" {
                                                        tr("用户")
                                                    } else {
                                                        tr("工作区")
                                                    },
                                                    if entry.pinned {
                                                        if english() {
                                                            " · Pinned"
                                                        } else {
                                                            " · 已固定"
                                                        }
                                                    } else {
                                                        ""
                                                    },
                                                    if entry.disabled {
                                                        if english() {
                                                            " · Disabled"
                                                        } else {
                                                            " · 已禁用"
                                                        }
                                                    } else {
                                                        ""
                                                    }
                                                )),
                                        ),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .min_w_0()
                                        .whitespace_normal()
                                        .line_clamp(2)
                                        .text_left()
                                        .text_size(px(12.5))
                                        .text_color(Color::Text.get(cx))
                                        .child(entry.body.clone()),
                                ),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.memory.selected = if this.memory.selected.as_ref() == Some(&id) {
                                None
                            } else {
                                Some(id.clone())
                            };
                            cx.notify();
                        })),
                ),
        )
    }

    fn render_memory_detail(
        &self,
        i: usize,
        entry: &MemoryEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let (badge_fg, badge_dot) = memory_status_dot(&entry.status, cx);
        let disabled = !self.connected || self.memory.acting || self.memory.loading;

        div()
            .id("memory-detail-pane")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .p_5()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(760.))
                    .mx_auto()
                    .gap_4()
                    .p_5()
                    .rounded_xl()
                    .bg(Color::Card.get(cx))
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .pb_3()
                            .border_b_1()
                            .border_color(Color::Divider.get(cx))
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .rounded_md()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .text_color(badge_fg)
                                    .px_2()
                                    .py(px(2.))
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(div().size(px(6.)).rounded_full().bg(badge_dot))
                                    .child(status_label(&entry.status).to_owned()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(format!(
                                        "{} · {}{}{}",
                                        kind_label(&entry.kind),
                                        if entry.scope == "user" {
                                            tr("用户")
                                        } else {
                                            tr("工作区")
                                        },
                                        if entry.pinned {
                                            if english() {
                                                " · Pinned"
                                            } else {
                                                " · 已固定"
                                            }
                                        } else {
                                            ""
                                        },
                                        if entry.disabled {
                                            if english() {
                                                " · Disabled"
                                            } else {
                                                " · 已禁用"
                                            }
                                        } else {
                                            ""
                                        }
                                    )),
                            )
                            .child(
                                Button::new("close-memory-detail")
                                    .custom(palette::subtle_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .rounded_md()
                                    .icon(Glyph::X)
                                    .tooltip(tr("收起"))
                                    .accessibility_label(tr("收起"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.memory.selected = None;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .p_3p5()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .text_sm()
                            .text_color(Color::Text.get(cx))
                            .child(crate::text_reading::selectable_text(
                                SharedString::from(format!("memory-body-{}", entry.id)),
                                entry.body.clone(),
                            )),
                    )
                    .when_some(entry.procedure_ref.as_ref(), |view, reference| {
                        view.child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(format!(
                                    "{}: {} · {} · {}",
                                    tr("引用"),
                                    reference.title.as_deref().unwrap_or(tr("未命名")),
                                    reference.ref_type,
                                    reference.id
                                )),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(format!(
                                "{}: {} · {}: {} · {}: {}",
                                tr("编号"),
                                entry.id,
                                tr("标签"),
                                entry.tags.join(if english() { ", " } else { "、" }),
                                tr("更新"),
                                updated_label(entry.updated_at)
                            )),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1p5()
                            .child(
                                Button::new(("copy-memory", i))
                                    .outline()
                                    .compact()
                                    .xsmall()
                                    .h(px(26.))
                                    .rounded_md()
                                    .icon(Glyph::Copy)
                                    .label(tr("复制内容"))
                                    .on_click({
                                        let body = entry.body.clone();
                                        cx.listener(move |_, _, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                body.clone(),
                                            ))
                                        })
                                    }),
                            )
                            .when(entry.status == "active", |view| {
                                view.child(
                                    Button::new(("pin-memory", i))
                                        .outline()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .rounded_md()
                                        .label(if entry.pinned {
                                            tr("取消固定")
                                        } else {
                                            tr("固定")
                                        })
                                        .disabled(disabled)
                                        .on_click({
                                            let id = entry.id.clone();
                                            let pinned = entry.pinned;
                                            cx.listener(move |this, _, _, cx| {
                                                this.memory_action(
                                                    "PATCH",
                                                    format!("/{id}"),
                                                    Some(json!({"pinned": !pinned})),
                                                    cx,
                                                )
                                            })
                                        }),
                                )
                                .child(
                                    Button::new(("disable-memory", i))
                                        .outline()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .rounded_md()
                                        .label(if entry.disabled {
                                            tr("启用此条")
                                        } else {
                                            tr("禁用此条")
                                        })
                                        .disabled(disabled)
                                        .on_click({
                                            let id = entry.id.clone();
                                            let disabled_entry = entry.disabled;
                                            cx.listener(move |this, _, _, cx| {
                                                this.memory_action(
                                                    "PATCH",
                                                    format!("/{id}"),
                                                    Some(json!({"disabled": !disabled_entry})),
                                                    cx,
                                                )
                                            })
                                        }),
                                )
                            })
                            .when(entry.status == "candidate", |view| {
                                view.child(
                                    Button::new(("approve-memory", i))
                                        .primary()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .rounded_md()
                                        .label(tr("批准"))
                                        .disabled(disabled)
                                        .on_click({
                                            let id = entry.id.clone();
                                            cx.listener(move |this, _, _, cx| {
                                                this.memory_action(
                                                    "POST",
                                                    format!("/{id}/approve"),
                                                    None,
                                                    cx,
                                                )
                                            })
                                        }),
                                )
                                .child(
                                    Button::new(("reject-memory", i))
                                        .outline()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .rounded_md()
                                        .label(tr("拒绝"))
                                        .disabled(disabled)
                                        .on_click({
                                            let id = entry.id.clone();
                                            cx.listener(move |this, _, _, cx| {
                                                this.memory_action(
                                                    "POST",
                                                    format!("/{id}/reject"),
                                                    None,
                                                    cx,
                                                )
                                            })
                                        }),
                                )
                            })
                            .when(entry.status == "active", |view| {
                                view.child(
                                    Button::new(("archive-memory", i))
                                        .outline()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .rounded_md()
                                        .label(tr("归档"))
                                        .disabled(disabled)
                                        .on_click({
                                            let id = entry.id.clone();
                                            cx.listener(move |this, _, _, cx| {
                                                this.memory_action(
                                                    "POST",
                                                    format!("/{id}/archive"),
                                                    None,
                                                    cx,
                                                )
                                            })
                                        }),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .pt_3()
                            .border_t_1()
                            .border_color(Color::Divider.get(cx))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr("来源")),
                            )
                            .when(entry.sources.is_empty(), |view| {
                                view.child(
                                    div()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("没有来源记录")),
                                )
                            })
                            .children(entry.sources.iter().map(|source| {
                                v_flex()
                                    .gap_1()
                                    .p_2p5()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Color::Secondary.get(cx))
                                            .child(format!(
                                                "{} · {} · {}{}",
                                                source_label(&source.source_type),
                                                source
                                                    .actor_name_snapshot
                                                    .as_deref()
                                                    .unwrap_or(tr("未知作者")),
                                                source
                                                    .source_id
                                                    .as_deref()
                                                    .unwrap_or(tr("无来源编号")),
                                                source
                                                    .source_sequence
                                                    .map(|seq| format!(" · #{seq}"))
                                                    .unwrap_or_default()
                                            )),
                                    )
                                    .when_some(source.excerpt.clone(), |view, excerpt| {
                                        view.child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Text.get(cx))
                                                .child(excerpt),
                                        )
                                    })
                            })),
                    ),
            )
    }
}
