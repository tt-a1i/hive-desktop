use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        popover::Popover,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::json;
#[derive(serde::Deserialize)]
pub struct ReasoningState {
    pub level: String,
    pub levels: Vec<String>,
}
#[derive(Default)]
pub struct ReasoningPicker {
    pub open: bool,
    pub loading: bool,
    state: Option<ReasoningState>,
    error: Option<String>,
}
fn label(level: &str) -> &str {
    match level {
        "off" => "关闭",
        "minimal" => "最低",
        "low" => "低",
        "medium" => "中",
        "high" => "高",
        "xhigh" => "超高",
        _ => level,
    }
}
impl HiveApp {
    fn load_reasoning(&mut self, level: Option<String>, cx: &mut Context<Self>) {
        if self.reasoning_picker.loading
            || !self.conversation_ready
            || self.loading
            || !self.connected
            || self.closing
        {
            return;
        }
        let (Some(runtime), Some(session)) = (self.runtime.clone(), self.session.as_ref()) else {
            return;
        };
        let id = session.id.clone();
        let connection = self.connection_generation;
        let open = self.open_request;
        self.reasoning_picker.loading = true;
        self.reasoning_picker.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mutation = level.is_some();
            let result = if let Some(level) = level {
                runtime
                    .request("chat.setReasoning", json!({"sessionId":id,"level":level}))
                    .await
            } else {
                runtime
                    .request("chat.reasoning", json!({"sessionId":id}))
                    .await
            };
            let failure = result.as_ref().err().cloned();
            // Native setters can reject after a storage error. Read the actual
            // native state instead of leaving an optimistic selection visible.
            let result = if failure.is_some() && mutation {
                runtime
                    .request("chat.reasoning", json!({"sessionId":id}))
                    .await
            } else {
                result
            };
            let state = result.and_then(|value| {
                serde_json::from_value::<ReasoningState>(value).map_err(|error| error.to_string())
            });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.open_request != open
                    || !this.session.as_ref().is_some_and(|s| s.id == id)
                {
                    return;
                }
                this.reasoning_picker.loading = false;
                match state {
                    Ok(state) => {
                        this.reasoning_picker.state = Some(state);
                        if failure.is_none() {
                            this.reasoning_picker.error = None;
                        }
                    }
                    Err(error) => {
                        this.reasoning_picker.state = None;
                        this.reasoning_picker.error = Some(error);
                    }
                }
                if let Some(error) = failure {
                    this.reasoning_picker.error = Some(error);
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn render_reasoning_picker(
        &self,
        running: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled =
            running || self.loading || !self.conversation_ready || !self.connected || self.closing;
        let current = self
            .reasoning_picker
            .state
            .as_ref()
            .map(|state| format!("推理 · {}", label(&state.level)))
            .unwrap_or_else(|| "推理强度".into());
        let content = v_flex()
            .min_w_0()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("会话推理强度"),
            )
            .when(self.reasoning_picker.loading, |view| {
                view.child(div().text_sm().child("正在读取原生设置…"))
            })
            .when_some(self.reasoning_picker.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error.clone()),
                )
            })
            .when_some(self.reasoning_picker.state.as_ref(), |view, state| {
                view.when(state.levels.len() < 2, |view| {
                    view.child(
                        div()
                            .text_sm()
                            .text_color(Color::Muted.get(cx))
                            .child("当前模型没有可调推理选项。"),
                    )
                })
                .children(state.levels.iter().enumerate().map(|(i, level)| {
                    let level = level.clone();
                    Button::new(("reasoning-option", i))
                        .ghost()
                        .compact()
                        .w_full()
                        .justify_start()
                        .label(label(&level).to_owned())
                        .selected(state.level == level)
                        .disabled(self.reasoning_picker.loading || disabled)
                        .accessibility_label(format!("推理强度：{}", label(&level)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.load_reasoning(Some(level.clone()), cx)
                        }))
                }))
            })
            .child(
                Button::new("refresh-reasoning")
                    .ghost()
                    .compact()
                    .label("重新读取")
                    .disabled(self.reasoning_picker.loading)
                    .on_click(cx.listener(|this, _, _, cx| this.load_reasoning(None, cx))),
            );
        Popover::new("reasoning-picker")
            .anchor(Anchor::BottomRight)
            .w(px(260.))
            .open(self.reasoning_picker.open)
            .trigger(
                Button::new("choose-reasoning")
                    .ghost()
                    .compact()
                    .label(current)
                    .accessibility_label("设置会话推理强度")
                    .disabled(disabled)
                    .tooltip("只影响当前会话，不修改 Pi 全局默认"),
            )
            .on_open_change(cx.listener(|this, open, _, cx| {
                this.reasoning_picker.open = *open;
                if *open {
                    this.load_reasoning(None, cx);
                }
                cx.notify();
            }))
            .when(self.reasoning_picker.open, |popover| popover.child(content))
            .into_any_element()
    }
}
