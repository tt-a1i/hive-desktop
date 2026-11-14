use crate::{app::HiveApp, language::{english, tr}, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        popover::Popover,
        progress::{Progress, ProgressCircle},
        Icon, Sizable, *,
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
        format!("{}: {}",tr(key),crate::language::localize_owned_error(error))
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
}

fn context_percent_color(percent: f64, cx: &App) -> Rgba {
    if percent >= 90. {
        Color::Error.get(cx)
    } else if percent >= 75. {
        rgb(if Theme::global(cx).is_dark() {
            0xfbbf24
        } else {
            0xd97706
        })
    } else {
        Color::Accent.get(cx)
    }
}

impl HiveApp {
    pub(crate) fn usage_button(&self, window: &Window, running: bool, cx: &Context<Self>) -> impl IntoElement + use<> {
        let context_percent = self
            .usage
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.context.as_ref())
            .and_then(|context| context.percent);
        let ring_color = context_percent.map(|percent| context_percent_color(percent, cx));
        let trigger = Button::new("session-usage")
            .custom(crate::palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(22.))
            .px_1p5()
            .rounded_md()
            .tooltip(tr("查看原生用量和上下文"))
            .accessibility_label(tr("查看原生用量和上下文"))
            .selected(self.usage.visible)
            .disabled(!self.connected || !self.conversation_ready || self.loading);
        let fallback_label = self
            .session
            .as_ref()
            .map(|session| tr(session.backend.label()))
            .unwrap_or_else(|| tr("用量"));
        let trigger = if let (Some(percent), Some(color)) = (context_percent, ring_color) {
            trigger.child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        ProgressCircle::new("session-usage-ring")
                            .xsmall()
                            .size(px(12.))
                            .value(percent.clamp(0., 100.) as f32)
                            .color(color),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(Color::Muted.get(cx))
                            .child(format!("{percent:.0}%")),
                    ),
            )
        } else {
            trigger.child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Icon::new(Glyph::Gauge)
                            .size(px(12.))
                            .text_color(Color::Muted.get(cx))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(Color::Muted.get(cx))
                            .child(fallback_label),
                    ),
            )
        };
        Popover::new("session-usage-popover")
            .anchor(Anchor::BottomRight)
            .p_0()
            .rounded_xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Card.get(cx))
            .shadow_lg()
            .overflow_hidden()
            .w(px(430.).min(window.viewport_size().width - px(32.)))
            .open(self.usage.visible)
            .trigger(trigger)
            .on_open_change(cx.listener(|this, open, _, cx| {
                this.usage.visible = *open;
                if *open { this.load_usage(cx); }
                cx.notify();
            }))
            .when(self.usage.visible, |popover| {
                popover.child(
                    crate::text_reading::region(
                        "usage-popover-reading".into(),
                        tr("原生用量").into(),
                        &self.usage.scroll,
                        (window.viewport_size().height - px(96.)).min(px(380.)).max(px(120.)),
                        cx,
                    )
                    .border_0()
                    .rounded_none()
                    .p_0()
                    .child(self.render_usage(running, cx)),
                )
            })
    }
    pub(crate) fn render_usage(&self, running: bool, cx: &Context<Self>) -> AnyElement {
        let header = h_flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3p5()
            .py_2p5()
            .border_b_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Icon::new(Glyph::Gauge)
                            .size(px(14.))
                            .text_color(Color::Accent.get(cx))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(tr("原生用量")),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("refresh-session-usage")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .px_2()
                            .rounded_lg()
                            .icon(Glyph::RefreshCw)
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
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_lg()
                            .icon(Glyph::X)
                            .tooltip(tr("收起"))
                            .accessibility_label(tr("收起"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.usage.visible = false;
                                cx.notify();
                            })),
                    ),
            );
        v_flex()
            .min_w_0()
            .child(header)
            .child(
                v_flex()
                    .min_w_0()
                    .p_3p5()
                    .gap_3()
                    .when(running, |view| {
                        view.child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(tr("运行中：以 Agent 最近已报告的数据为准。")),
                        )
                    })
                    .when(!self.connected, |view| {
                        view.child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .text_xs()
                                .text_color(Color::Error.get(cx))
                                .child(tr("后台未连接，当前显示上次读取结果。")),
                        )
                    })
                    .when_some(self.usage.error.as_ref().map(|error| error.text()), |view, error| {
                        view.child(
                            div()
                                .px_2p5()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .text_xs()
                                .text_color(Color::Error.get(cx))
                                .child(error),
                        )
                    })
                    .when_some(self.usage.snapshot.as_ref(), |view, snapshot| {
                        view.child(
                            v_flex()
                                .id("native-usage-details")
                                .min_w_0()
                                .gap_3()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_md()
                                                .bg(Color::Inset.get(cx))
                                                .child(
                                                    crate::text_reading::selectable_text(
                                                        "usage-source",
                                                        format!(
                                                            "{} · {}: Token",
                                                            native_label(&snapshot.source),
                                                            tr("单位")
                                                        ),
                                                    )
                                                    .text_size(px(11.5))
                                                    .text_color(Color::Secondary.get(cx)),
                                                ),
                                        ),
                                )
                                .when_some(snapshot.context.as_ref(), |view, context| {
                                    let title = format!(
                                        "{}{}",
                                        tr("当前上下文"),
                                        if context.estimated { tr("（原生估算）") } else { "" }
                                    );
                                    let amount = match context.tokens {
                                        Some(tokens) => format!(
                                            "{tokens} / {} Token{}",
                                            context.window,
                                            context
                                                .percent
                                                .map(|p| format!(" · {p:.1}%"))
                                                .unwrap_or_default()
                                        ),
                                        None => {
                                            if english() {
                                                format!(
                                                    "Usage unavailable · Context window {} tokens",
                                                    context.window
                                                )
                                            } else {
                                                format!("占用暂不可用 · 窗口 {} Token", context.window)
                                            }
                                        }
                                    };
                                    let bar_color = context.percent.map(|p| context_percent_color(p, cx));
                                    view.child(
                                        v_flex()
                                            .id("usage-context")
                                            .role(Role::Label)
                                            .aria_value(format!("{title}: {amount}"))
                                            .p_3()
                                            .rounded_xl()
                                            .border_1()
                                            .border_color(Color::Divider.get(cx))
                                            .bg(Color::Inset.get(cx).opacity(0.45))
                                            .gap_2()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(Color::Secondary.get(cx))
                                                            .child(title),
                                                    )
                                                    .when_some(context.percent, |row, pct| {
                                                        row.child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(
                                                                    bar_color.unwrap_or_else(|| Color::Accent.get(cx)),
                                                                )
                                                                .child(format!("{pct:.1}%")),
                                                        )
                                                    }),
                                            )
                                            .when_some(context.percent, |card, pct| {
                                                card.child(
                                                    Progress::new("usage-context-progress")
                                                        .xsmall()
                                                        .value(pct.clamp(0., 100.) as f32)
                                                        .color(
                                                            bar_color.unwrap_or_else(|| Color::Accent.get(cx)),
                                                        ),
                                                )
                                            })
                                            .child(
                                                div()
                                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                                    .text_xs()
                                                    .text_color(Color::Text.get(cx))
                                                    .child(amount),
                                            ),
                                    )
                                })
                                .children(snapshot.reports.iter().enumerate().map(|(i, report)| {
                                    let title = native_label(&report.label);
                                    v_flex()
                                        .id(SharedString::from(format!("usage-report-{i}")))
                                        .role(Role::Group)
                                        .aria_label(title.clone())
                                        .min_w_0()
                                        .p_3()
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .bg(Color::Inset.get(cx).opacity(0.35))
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(Color::Secondary.get(cx))
                                                .child(title),
                                        )
                                        .child(
                                            h_flex().min_w_0().flex_wrap().gap_2().children(
                                                report.counters.iter().enumerate().map(|(j, counter)| {
                                                    let label = native_label(&counter.label);
                                                    v_flex()
                                                        .id(SharedString::from(format!(
                                                            "usage-counter-{i}-{j}"
                                                        )))
                                                        .role(Role::Label)
                                                        .aria_value(format!("{label}: {}", counter.tokens))
                                                        .flex_1()
                                                        .min_w(px(108.))
                                                        .px_2p5()
                                                        .py_2()
                                                        .rounded_lg()
                                                        .border_1()
                                                        .border_color(Color::Divider.get(cx))
                                                        .bg(Color::Card.get(cx))
                                                        .gap_0p5()
                                                        .child(
                                                            div()
                                                                .text_size(px(11.))
                                                                .text_color(Color::Muted.get(cx))
                                                                .truncate()
                                                                .child(label),
                                                        )
                                                        .child(
                                                            div()
                                                                .font_family(
                                                                    Theme::global(cx)
                                                                        .mono_font_family
                                                                        .clone(),
                                                                )
                                                                .text_sm()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(Color::Text.get(cx))
                                                                .child(counter.tokens.to_string()),
                                                        )
                                                }),
                                            ),
                                        )
                                }))
                                .when_some(snapshot.notice.as_ref(), |view, notice| {
                                    view.child(
                                        crate::text_reading::selectable_text(
                                            "usage-notice",
                                            native_label(notice),
                                        )
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx)),
                                    )
                                }),
                        )
                    }),
            )
            .into_any_element()
    }
}
