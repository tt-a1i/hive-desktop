use super::*;
use crate::focus_scroll::{Reveal, Viewport};
use crate::palette::Color;
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

fn memory_status_colors(status: &str, cx: &App) -> (Rgba, Rgba, Option<Rgba>) {
    match status {
        "active" => (
            Color::Inset.get(cx),
            Color::Text.get(cx),
            Some(Color::Success.get(cx)),
        ),
        "candidate" => (Color::Attention.get(cx), Color::Text.get(cx), None),
        "rejected" | "archived" => (Color::Inset.get(cx), Color::Secondary.get(cx), None),
        _ => (Color::Inset.get(cx), Color::Text.get(cx), None),
    }
}

impl HiveApp {
    pub fn render_memory(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let m = &self.memory;
        let scroll = &m.scroll;
        let disabled = !self.connected || m.loading || m.acting;
        v_flex().flex_1().min_w_0().min_h_0().h_full().px_5().py_6().gap_4()
            .child(h_flex().w_full().max_w(px(960.)).mx_auto().items_center().justify_between().gap_2()
                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(tr("工作区记忆")))
                .child(Button::new("refresh-memory").ghost().icon(Glyph::RefreshCw).label(tr("刷新")).disabled(disabled).on_click(cx.listener(|this, _, _, cx| this.load_memory(cx)))))
            .when(self.error.is_some(), |view| view.child(div().w_full().max_w(px(960.)).mx_auto().child(self.render_error_banner(cx))))
            .child(Viewport::new("memory-viewport", scroll, v_flex().id("memory-scroll").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(scroll).vertical_scrollbar(scroll).p_1().child(v_flex().w_full().max_w(px(960.)).mx_auto().min_w_0().flex_shrink_0().gap_3()
                .child(v_flex().gap_2().p_4().rounded_xl().bg(Color::Card.get(cx)).border_1().border_color(Color::Divider.get(cx))
                    .child(Reveal::new("memory-injection-reveal", scroll, h_flex().flex_wrap().gap_2().items_center()
                        .child(div().flex_1().min_w_0().text_sm().font_weight(FontWeight::SEMIBOLD).child(match m.enabled { Some(true) => tr("工作区记忆注入：已启用"), Some(false) => tr("工作区记忆注入：已禁用"), None if m.loading => tr("正在读取注入设置…"), None => tr("注入设置暂不可用") }))
                        .when_some(m.enabled, |view, enabled| view.child(Button::new("toggle-memory-injection").rounded_full().label(if enabled { tr("禁用注入") } else { tr("启用注入") }).disabled(disabled).on_click(cx.listener(move |this, _, _, cx| this.memory_action("PUT", "/settings".into(), Some(json!({"enabled":!enabled})), cx)))))))
                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(tr("设置影响后续构建的会话与派单上下文；已发送给 Agent 的记忆不会从已有对话中移除。")))
                    .when_some(m.settings_error.clone(), |view, error| {
                        let message = format!("{}: {}", tr("读取注入设置失败"), crate::language::localize_owned_error(&error));
                        view.child(div().id("memory-settings-error").role(Role::Alert).aria_label(message.clone()).text_sm().text_color(Color::Error.get(cx)).child(message))
                    }))
                .child(Reveal::new("memory-search-reveal", scroll, h_flex().gap_2()
                    .child(div().flex_1().min_w_0().child(Input::new(&m.query).aria_label(tr("搜索工作区记忆")).disabled(disabled)))
                    .child(Button::new("search-memory").rounded_full().label(tr("搜索")).disabled(disabled).on_click(cx.listener(|this, _, _, cx| { this.memory.selected = None; this.memory.scroll.set_offset(point(px(0.), px(0.))); this.load_memory(cx); })))))
                .child(Reveal::new("memory-status-reveal", scroll, h_flex().flex_wrap().gap_1().children([("active", tr("有效")), ("candidate", tr("待审核")), ("archived", tr("已归档")), ("rejected", tr("已拒绝")), ("all", tr("全部状态"))].into_iter().enumerate().map(|(i, (status, label))| Button::new(("memory-status", i)).ghost().compact().rounded_full().label(label).selected(m.status == status).disabled(disabled).on_click(cx.listener(move |this, _, _, cx| { this.memory.status = status; this.memory.selected = None; this.memory.scroll.set_offset(point(px(0.), px(0.))); this.load_memory(cx); }))))))
                .child(Reveal::new("memory-scope-reveal", scroll, h_flex().flex_wrap().gap_1().children([("workspace", tr("工作区范围")), ("user", tr("用户范围")), ("all", tr("全部范围"))].into_iter().enumerate().map(|(i, (scope, label))| Button::new(("memory-scope", i)).ghost().compact().rounded_full().label(label).selected(m.scope == scope).disabled(disabled).on_click(cx.listener(move |this, _, _, cx| { this.memory.scope = scope; this.memory.selected = None; this.memory.scroll.set_offset(point(px(0.), px(0.))); this.load_memory(cx); }))))))
                .when(m.loading, |view| view.child(div().id("memory-loading").role(Role::Status).aria_label(tr("正在读取记忆…")).text_sm().text_color(Color::Secondary.get(cx)).child(tr("正在读取记忆…"))))
                .when_some(m.error.as_ref().map(|error| error.text()), |view, error| view.child(div().id("memory-error").role(Role::Alert).aria_label(error.clone()).p_3().rounded_lg().bg(Color::ErrorSurface.get(cx)).text_sm().text_color(Color::Error.get(cx)).child(error)))
                .when_some(m.notice.map(tr), |view, notice| view.child(div().id("memory-notice").role(Role::Status).aria_label(notice).text_sm().text_color(Color::Secondary.get(cx)).child(notice)))
                .when(!m.loading && m.error.is_none() && m.entries.is_empty(), |view| view.child(div().p_4().rounded_xl().bg(Color::Card.get(cx)).border_1().border_color(Color::Divider.get(cx)).text_sm().text_color(Color::Muted.get(cx)).child(tr("没有匹配的记忆"))))
                .child(div().text_xs().text_color(Color::Muted.get(cx)).child(if english() { format!("Showing {} entries (up to 50). Search or filter to narrow the list.", m.entries.len()) } else { format!("显示 {} 条，最多 50 条。可搜索或筛选缩小范围。", m.entries.len()) }))
                .children(m.entries.iter().enumerate().map(|(i, entry)| self.render_memory_entry(i, entry, cx))))))
    }

    fn render_memory_entry(
        &self,
        i: usize,
        entry: &MemoryEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let selected = self.memory.selected.as_ref() == Some(&entry.id);
        let id = entry.id.clone();
        let scroll = &self.memory.scroll;
        let (badge_bg, badge_fg, badge_dot) = memory_status_colors(&entry.status, cx);
        let disabled = !self.connected || self.memory.acting || self.memory.loading;
        v_flex()
            .min_w_0()
            .gap_2()
            .p_4()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(5.))
                            .rounded_full()
                            .bg(badge_bg)
                            .text_color(badge_fg)
                            .px(px(9.))
                            .py(px(2.))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .when_some(badge_dot, |pill, dot| {
                                pill.child(div().size(px(6.)).rounded_full().bg(dot))
                            })
                            .child(status_label(&entry.status).to_owned()),
                    )
                    .child(
                        div()
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
                                if entry.pinned { if english() { " · Pinned" } else { " · 已固定" } } else { "" },
                                if entry.disabled { if english() { " · Disabled" } else { " · 已禁用" } } else { "" }
                            )),
                    ),
            )
            .child(Reveal::new(("memory-detail-reveal", i), scroll,
                Button::new(("memory-detail", i))
                    .ghost()
                    .w_full()
                    .min_w_0()
                    .justify_start()
                    .accessibility_label(if english() {
                        format!("{} memory details: {}", if selected { tr("收起") } else { tr("打开") }, entry.body.chars().take(80).collect::<String>())
                    } else {
                        format!("{}记忆详情：{}", if selected { tr("收起") } else { tr("打开") }, entry.body.chars().take(80).collect::<String>())
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(entry.body.clone()),
                    )
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.memory.selected = if this.memory.selected.as_ref() == Some(&id) {
                            None
                        } else {
                            Some(id.clone())
                        };
                        cx.notify();
                    })),
            ))
            .when(selected, |view| {
                view.child(div().text_sm().child(entry.body.clone()))
                    .when_some(entry.procedure_ref.as_ref(), |view, reference| {
                        view.child(div().text_sm().child(format!(
                            "{}: {} · {} · {}", tr("引用"),
                            reference.title.as_deref().unwrap_or(tr("未命名")),
                            reference.ref_type,
                            reference.id
                        )))
                    })
                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!(
                        "{}: {} · {}: {} · {}: {}", tr("编号"), entry.id,
                        tr("标签"), entry.tags.join(if english() { ", " } else { "、" }),
                        tr("更新"), updated_label(entry.updated_at)
                    )))
                    .child(Reveal::new(("memory-actions-reveal", i), scroll,
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(Button::new(("copy-memory", i)).label(tr("复制内容")).on_click({
                                let body = entry.body.clone();
                                cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(body.clone()))
                                })
                            }))
                            .when(entry.status == "active", |view| {
                                view.child(
                                    Button::new(("pin-memory", i))
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
                                                    Some(json!({"pinned":!pinned})),
                                                    cx,
                                                )
                                            })
                                        }),
                                )
                                .child(
                                    Button::new(("disable-memory", i))
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
                                                    Some(json!({"disabled":!disabled_entry})),
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
                    ))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("来源")),
                    )
                    .when(entry.sources.is_empty(), |view| {
                        view.child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("没有来源记录")))
                    })
                    .children(entry.sources.iter().map(|source| {
                        v_flex()
                            .gap_1()
                            .p_2()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .child(div().text_xs().child(format!(
                                    "{} · {} · {}{}",
                                    source_label(&source.source_type),
                                    source.actor_name_snapshot.as_deref().unwrap_or(tr("未知作者")),
                                    source.source_id.as_deref().unwrap_or(tr("无来源编号")),
                                    source
                                        .source_sequence
                                        .map(|seq| format!(" · #{seq}"))
                                        .unwrap_or_default()
                                )))
                            .when_some(source.excerpt.clone(), |view, excerpt| {
                                view.child(div().text_sm().child(excerpt))
                            })
                    }))
            })
    }
}
