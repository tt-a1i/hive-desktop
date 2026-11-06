use super::*;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        *,
    },
    prelude::FluentBuilder,
};

impl HiveApp {
    pub fn render_memory(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let m = &self.memory;
        let disabled = m.loading || m.acting;
        v_flex().flex_1().min_w_0().h_full().p_5().gap_3()
            .child(h_flex().justify_between().gap_2().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("工作区记忆"))
                .child(Button::new("refresh-memory").label("刷新").disabled(disabled).on_click(cx.listener(|this, _, _, cx| this.load_memory(cx)))))
            .child(v_flex().id("memory-scroll").flex_1().min_h_0().overflow_y_scroll().child(v_flex().w_full().min_w_0().flex_shrink_0().gap_3()
                .child(h_flex().flex_wrap().gap_2().items_center()
                    .child(div().text_sm().child(match m.enabled { Some(true) => "工作区记忆注入：已启用", Some(false) => "工作区记忆注入：已禁用", None if m.loading => "正在读取注入设置…", None => "注入设置暂不可用" }))
                    .when_some(m.enabled, |view, enabled| view.child(Button::new("toggle-memory-injection").label(if enabled { "禁用注入" } else { "启用注入" }).disabled(disabled).on_click(cx.listener(move |this, _, _, cx| this.memory_action("PUT", "/settings".into(), Some(json!({"enabled":!enabled})), cx))))))
                .child(div().text_xs().text_color(rgb(0x777c75)).child("设置影响后续构建的会话与派单上下文；已发送给 Agent 的记忆不会从已有对话中移除。"))
                .when_some(m.settings_error.clone(), |view, error| view.child(div().text_sm().text_color(rgb(0xb54131)).child(error)))
                .child(h_flex().gap_2().child(div().flex_1().min_w_0().child(Input::new(&m.query).aria_label("搜索工作区记忆").disabled(disabled))).child(Button::new("search-memory").label("搜索").disabled(disabled).on_click(cx.listener(|this, _, _, cx| { this.memory.selected = None; this.load_memory(cx); }))))
                .child(h_flex().flex_wrap().gap_1().children([("active", "有效"), ("candidate", "待审核"), ("archived", "已归档"), ("rejected", "已拒绝"), ("all", "全部状态")].into_iter().enumerate().map(|(i, (status, label))| Button::new(("memory-status", i)).label(label).selected(m.status == status).disabled(disabled).on_click(cx.listener(move |this, _, _, cx| { this.memory.status = status; this.memory.selected = None; this.load_memory(cx); })))))
                .child(h_flex().flex_wrap().gap_1().children([("workspace", "工作区范围"), ("user", "用户范围"), ("all", "全部范围")].into_iter().enumerate().map(|(i, (scope, label))| Button::new(("memory-scope", i)).label(label).selected(m.scope == scope).disabled(disabled).on_click(cx.listener(move |this, _, _, cx| { this.memory.scope = scope; this.memory.selected = None; this.load_memory(cx); })))))
                .when(m.loading, |view| view.child(div().text_sm().child("正在读取记忆…")))
                .when_some(m.error.clone(), |view, error| view.child(div().text_sm().text_color(rgb(0xb54131)).child(error)))
                .when_some(m.notice.clone(), |view, notice| view.child(div().text_sm().child(notice)))
                .when(!m.loading && m.error.is_none() && m.entries.is_empty(), |view| view.child(div().py_4().text_sm().child("没有匹配的记忆")))
                .child(div().text_xs().text_color(rgb(0x777c75)).child(format!("显示 {} 条，最多 50 条。可搜索或筛选缩小范围。", m.entries.len())))
                .children(m.entries.iter().enumerate().map(|(i, entry)| self.render_memory_entry(i, entry, cx)))))
    }

    fn render_memory_entry(
        &self,
        i: usize,
        entry: &MemoryEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let selected = self.memory.selected.as_ref() == Some(&entry.id);
        let id = entry.id.clone();
        let disabled = self.memory.acting || self.memory.loading;
        v_flex()
            .min_w_0()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(rgb(0xffffff))
            .child(div().text_xs().text_color(rgb(0x63715c)).child(format!(
                "{} · {} · {}{}{}",
                kind_label(&entry.kind),
                status_label(&entry.status),
                if entry.scope == "user" {
                    "用户"
                } else {
                    "工作区"
                },
                if entry.pinned { " · 已固定" } else { "" },
                if entry.disabled { " · 已禁用" } else { "" }
            )))
            .child(
                Button::new(("memory-detail", i))
                    .w_full()
                    .min_w_0()
                    .accessibility_label(format!(
                        "{}记忆详情：{}",
                        if selected { "收起" } else { "打开" },
                        entry.body.chars().take(80).collect::<String>()
                    ))
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
            )
            .when(selected, |view| {
                view.child(div().text_sm().child(entry.body.clone()))
                    .when_some(entry.procedure_ref.as_ref(), |view, reference| {
                        view.child(div().text_sm().child(format!(
                            "引用：{} · {} · {}",
                            reference.title.as_deref().unwrap_or("未命名"),
                            reference.ref_type,
                            reference.id
                        )))
                    })
                    .child(div().text_xs().child(format!(
                        "编号：{} · 标签：{} · 更新：{}",
                        entry.id,
                        entry.tags.join("、"),
                        updated_label(entry.updated_at)
                    )))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(Button::new(("copy-memory", i)).label("复制内容").on_click({
                                let body = entry.body.clone();
                                cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(body.clone()))
                                })
                            }))
                            .when(entry.status == "active", |view| {
                                view.child(
                                    Button::new(("pin-memory", i))
                                        .label(if entry.pinned {
                                            "取消固定"
                                        } else {
                                            "固定"
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
                                            "启用此条"
                                        } else {
                                            "禁用此条"
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
                                        .label("批准")
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
                                        .label("拒绝")
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
                                        .label("归档")
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
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("来源"),
                    )
                    .when(entry.sources.is_empty(), |view| {
                        view.child(div().text_xs().child("没有来源记录"))
                    })
                    .children(entry.sources.iter().map(|source| {
                        v_flex()
                            .gap_1()
                            .p_2()
                            .bg(rgb(0xf0f2ed))
                            .child(div().text_xs().child(format!(
                                    "{} · {} · {}{}",
                                    source_label(&source.source_type),
                                    source.actor_name_snapshot.as_deref().unwrap_or("未知作者"),
                                    source.source_id.as_deref().unwrap_or("无来源编号"),
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
