use crate::{app::HiveApp, palette::Color, language::{english,tr}};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        popover::Popover,
        *,
    },
    prelude::FluentBuilder,
    *,
};
mod labels;
use labels::native_label;
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
    scroll: ScrollHandle,
    pub visible: bool,
    pub loading: bool,
    pub snapshot: Option<Snapshot>,
    pub error: Option<UsageError>,
    generation: u64,
    dirty: bool,
}
pub enum UsageError { Decode(String), Request(String) }
impl UsageError {
    fn text(&self) -> String {
        let (key,error)=match self { Self::Decode(error)=>("用量数据格式错误",error),Self::Request(error)=>("未能读取原生用量",error) };
        format!("{}: {error}",tr(key))
    }
}
impl HiveApp {
    pub(crate) fn reset_usage(&mut self) {
        self.usage.generation += 1;
        self.usage.visible = false;
        self.usage.scroll.set_offset(point(px(0.), px(0.)));
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
                .await.map_err(UsageError::Request)
                .and_then(|value|serde_json::from_value::<Snapshot>(value).map_err(|error|UsageError::Decode(error.to_string())));
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
                    Err(error) => this.usage.error = Some(error),
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
    pub(crate) fn usage_button(&self, window: &Window, running: bool, cx: &Context<Self>) -> impl IntoElement + use<> {
        Popover::new("session-usage-popover")
            .anchor(Anchor::BottomRight)
            .w(px(420.).min(window.viewport_size().width - px(32.)))
            .open(self.usage.visible)
            .trigger(Button::new("session-usage").ghost().compact().label(tr("用量"))
                .accessibility_label(tr("查看原生用量和上下文"))
                .selected(self.usage.visible)
                .disabled(!self.connected || !self.conversation_ready || self.loading))
            .on_open_change(cx.listener(|this, open, _, cx| {
                this.usage.visible = *open;
                if *open { this.load_usage(cx); }
                cx.notify();
            }))
            .when(self.usage.visible, |popover| popover.child(crate::text_reading::region(
                "usage-popover-reading".into(), tr("原生用量").into(), &self.usage.scroll,
                (window.viewport_size().height - px(96.)).min(px(360.)).max(px(100.)), cx
            ).border_color(rgba(0)).p_0().child(self.render_usage(running, cx))))
    }
    pub(crate) fn render_usage(&self, running: bool, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .min_w_0()
            .gap_2()
            .p_1()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("原生用量")),
                    )
                    .child(
                        Button::new("refresh-session-usage")
                            .ghost()
                            .compact()
                            .label(if self.usage.loading {
                                tr("读取中…")
                            } else {
                                tr("刷新")
                            })
                            .accessibility_label(tr("刷新原生用量"))
                            .disabled(!self.connected || self.usage.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.load_usage(cx))),
                    )
                    .child(
                        Button::new("close-session-usage")
                            .ghost()
                            .compact()
                            .label(tr("收起"))
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
                        .child(tr("运行中：以 Agent 最近已报告的数据为准。")),
                )
            })
            .when(!self.connected, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(tr("后台未连接，当前显示上次读取结果。")),
                )
            })
            .when_some(self.usage.error.as_ref().map(|error|error.text()), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when_some(self.usage.snapshot.as_ref(), |view, snapshot| {
                view.child(v_flex().id("native-usage-details").min_w_0().gap_3()
                    .child(crate::text_reading::selectable_text("usage-source", format!("{} · {}: Token",native_label(&snapshot.source),tr("单位"))).text_xs().text_color(Color::Secondary.get(cx)))
                    .when_some(snapshot.context.as_ref(), |view, context| {
                        let title = format!("{}{}",tr("当前上下文"),if context.estimated {tr("（原生估算）")} else {""});
                        let amount = match context.tokens {
                            Some(tokens) => format!("{tokens} / {} Token{}",context.window,context.percent.map(|p| format!(" · {p:.1}%")).unwrap_or_default()),
                            None => if english() {format!("Usage unavailable · Context window {} tokens",context.window)} else {format!("占用暂不可用 · 窗口 {} Token",context.window)},
                        };
                        view.child(v_flex().id("usage-context").role(Role::Label).aria_label(format!("{title}: {amount}")).gap_1()
                            .child(div().text_sm().child(title)).child(div().text_sm().child(amount)))
                    })
                    .children(snapshot.reports.iter().enumerate().map(|(i, report)| {
                        let title = native_label(&report.label);
                        v_flex().id(SharedString::from(format!("usage-report-{i}"))).role(Role::Group).aria_label(title.clone()).min_w_0().gap_2()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(title))
                            .child(h_flex().min_w_0().flex_wrap().gap_3().children(report.counters.iter().enumerate().map(|(j, counter)| {
                                let label = native_label(&counter.label);
                                v_flex().id(SharedString::from(format!("usage-counter-{i}-{j}"))).role(Role::Label).aria_label(format!("{label}: {}",counter.tokens)).gap_1()
                                    .child(div().text_xs().text_color(Color::Muted.get(cx)).child(label))
                                    .child(div().text_sm().child(counter.tokens.to_string()))
                            })))
                    }))
                    .when_some(snapshot.notice.as_ref(), |view, notice| view.child(crate::text_reading::selectable_text("usage-notice", native_label(notice)).text_xs().text_color(Color::Secondary.get(cx)))))
            })
            .into_any_element()
    }
}
