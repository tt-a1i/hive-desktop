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
fn label(level: &str) -> &str {
    match level {
        "native" => "原生配置",
        "max" => "最高",
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
                view.when_some(state.description.as_ref(), |view, text| {
                    view.child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(text.clone()),
                    )
                })
                .when(state.levels.len() < 2, |view| {
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
                    .tooltip("只影响当前会话，不修改 Agent 全局默认"),
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
