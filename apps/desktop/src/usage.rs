use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
#[derive(Deserialize)]
pub struct Counter {
    pub label: String,
    pub tokens: u64,
}
#[derive(Deserialize)]
pub struct Report {
    pub label: String,
    pub counters: Vec<Counter>,
}
#[derive(Deserialize)]
pub struct ContextUsage {
    pub tokens: Option<u64>,
    pub window: u64,
    pub percent: Option<f64>,
    pub estimated: bool,
}
#[derive(Deserialize)]
pub struct Snapshot {
    pub source: String,
    pub reports: Vec<Report>,
    pub context: Option<ContextUsage>,
    pub notice: Option<String>,
}
#[derive(Default)]
pub struct Usage {
    pub visible: bool,
    pub loading: bool,
    pub snapshot: Option<Snapshot>,
    pub error: Option<String>,
    generation: u64,
    dirty: bool,
}
impl HiveApp {
    pub(crate) fn reset_usage(&mut self) {
        self.usage.generation += 1;
        self.usage.visible = false;
        self.usage.loading = false;
        self.usage.snapshot = None;
        self.usage.error = None;
        self.usage.dirty = false;
    }
    pub(crate) fn load_usage(&mut self, cx: &mut Context<Self>) {
        if !self.usage.visible
            || !self.conversation_ready
            || self.loading
            || !self.connected
            || self.closing
        {
            return;
        }
        if self.usage.loading {
            self.usage.dirty = true;
            return;
        }
        let (Some(runtime), Some(id)) = (
            self.runtime.clone(),
            self.session.as_ref().map(|s| s.id.clone()),
        ) else {
            return;
        };
        self.usage.generation += 1;
        let generation = self.usage.generation;
        let connection = self.connection_generation;
        let open = self.open_request;
        self.usage.loading = true;
        self.usage.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.usage", json!({"sessionId":id}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<Snapshot>(value)
                        .map_err(|e| format!("用量数据格式错误：{e}"))
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.open_request != open
                    || this.usage.generation != generation
                    || !this.session.as_ref().is_some_and(|s| s.id == id)
                {
                    return;
                }
                this.usage.loading = false;
                match result {
                    Ok(snapshot) => {
                        this.usage.snapshot = Some(snapshot);
                        this.usage.error = None;
                    }
                    Err(error) => this.usage.error = Some(format!("未能读取原生用量：{error}")),
                }
                let dirty = std::mem::take(&mut this.usage.dirty);
                if dirty {
                    this.load_usage(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn usage_button(&self, cx: &Context<Self>) -> impl IntoElement {
        Button::new("session-usage")
            .ghost()
            .compact()
            .label("用量")
            .accessibility_label("查看原生用量和上下文")
            .selected(self.usage.visible)
            .disabled(!self.connected || !self.conversation_ready || self.loading)
            .on_click(cx.listener(|this, _, _, cx| {
                this.usage.visible = !this.usage.visible;
                if this.usage.visible {
                    this.load_usage(cx);
                }
                cx.notify();
            }))
    }
    pub(crate) fn render_usage(&self, running: bool, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .min_w_0()
            .gap_2()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("原生用量"),
                    )
                    .child(
                        Button::new("refresh-session-usage")
                            .ghost()
                            .compact()
                            .label(if self.usage.loading {
                                "读取中…"
                            } else {
                                "刷新"
                            })
                            .accessibility_label("刷新原生用量")
                            .disabled(!self.connected || self.usage.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.load_usage(cx))),
                    )
                    .child(
                        Button::new("close-session-usage")
                            .ghost()
                            .compact()
                            .label("收起")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.usage.visible = false;
                                cx.notify();
                            })),
                    ),
            )
            .when(running, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child("运行中：以 Agent 最近已报告的数据为准。"),
                )
            })
            .when(!self.connected, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child("后台未连接，当前显示上次读取结果。"),
                )
            })
            .when_some(self.usage.error.clone(), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when_some(self.usage.snapshot.as_ref(), |view, snapshot| {
                view.child(
                    v_flex()
                        .id("native-usage-details")
                        .min_w_0()
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .gap_3()
                        .child(
                            div()
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(format!("{} · 单位：Token", snapshot.source)),
                        )
                        .when_some(snapshot.context.as_ref(), |view, context| {
                            view.child(
                                v_flex()
                                    .gap_1()
                                    .child(div().text_sm().child(format!(
                                        "当前上下文{}",
                                        if context.estimated {
                                            "（原生估算）"
                                        } else {
                                            ""
                                        }
                                    )))
                                    .child(div().text_sm().child(match context.tokens {
                                        Some(tokens) => format!(
                                                "{tokens} / {} Token{}",
                                                context.window,
                                                context
                                                    .percent
                                                    .map(|p| format!(" · {p:.1}%"))
                                                    .unwrap_or_default()
                                            ),
                                        None => {
                                            format!("占用暂不可用 · 窗口 {} Token", context.window)
                                        }
                                    })),
                            )
                        })
                        .children(snapshot.reports.iter().map(|report| {
                            v_flex()
                                .min_w_0()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(report.label.clone()),
                                )
                                .child(h_flex().min_w_0().flex_wrap().gap_3().children(
                                    report.counters.iter().map(|counter| {
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(counter.label.clone()),
                                            )
                                            .child(
                                                div().text_sm().child(counter.tokens.to_string()),
                                            )
                                    }),
                                ))
                        }))
                        .when_some(snapshot.notice.clone(), |view, notice| {
                            view.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(notice),
                            )
                        }),
                )
            })
            .into_any_element()
    }
}
