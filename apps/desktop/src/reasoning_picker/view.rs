use crate::{app::HiveApp, palette::Color, language::tr};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
fn label(level: &str) -> &str {
    match level {
        "native" => tr("原生配置"),
        "max" => tr("最高"),
        "off" => tr("关闭"),
        "minimal" => tr("最低"),
        "low" => tr("低"),
        "medium" => tr("中"),
        "high" => tr("高"),
        "xhigh" => tr("超高"),
        _ => level,
    }
}
impl HiveApp {
    pub(crate) fn reasoning_label(&self) -> Option<String> {
        self.reasoning_picker.state.as_ref().map(|state| label(&state.level).to_owned())
    }

    pub(crate) fn render_reasoning_settings(
        &self,
        running: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let disabled =
            running || self.loading || !self.conversation_ready || !self.connected || self.closing;
        let content = v_flex()
            .id("reasoning-picker-content")
            .role(Role::Group)
            .aria_label(tr("会话推理强度"))
            .min_w_0()
            .gap_2()
            .child(
                crate::text_reading::selectable_text("reasoning-heading", tr("会话推理强度"))
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .when(self.reasoning_picker.loading, |view| {
                view.child(crate::text_reading::selectable_text("reasoning-loading", tr("正在读取原生设置…")).text_sm())
            })
            .when_some(self.reasoning_picker.error.as_ref(), |view, error| {
                view.child(
                    crate::text_reading::selectable_text("reasoning-error", error.clone())
                        .text_sm()
                        .text_color(Color::Error.get(cx)),
                )
            })
            .when_some(self.reasoning_picker.state.as_ref(), |view, state| {
                view.when_some(state.description.as_ref(), |view, text| {
                    // Translate only metadata owned by Hive's native adapters.
                    // Preserve unknown/provider diagnostics verbatim.
                    let text = match text.as_str() {
                        "从原生 Codex 线程读取；档位由当前模型提供。" => tr("从原生 Codex 线程读取；档位由当前模型提供。"),
                        "读取原生实际强度；环境、组织限制或模型能力可能调整请求值。" => tr("读取原生实际强度；环境、组织限制或模型能力可能调整请求值。"),
                        _ => text.as_str(),
                    };
                    view.child(
                        crate::text_reading::selectable_text("reasoning-description", text.to_owned())
                            .text_xs()
                            .text_color(Color::Muted.get(cx)),
                    )
                })
                .when(state.levels.len() < 2, |view| {
                    view.child(
                        crate::text_reading::selectable_text("reasoning-unavailable", tr("当前模型没有可调推理选项。"))
                            .text_sm()
                            .text_color(Color::Muted.get(cx)),
                    )
                })
                .children(state.levels.iter().enumerate().map(|(i, level)| {
                    let level = level.clone();
                    crate::focus_scroll::Reveal::new(("reasoning-option-reveal", i), &self.reasoning_picker.scroll, Button::new(("reasoning-option", i))
                        .ghost()
                        .compact()
                        .w_full()
                        .justify_start()
                        .label(label(&level).to_owned())
                        .selected(state.level == level)
                        .disabled(self.reasoning_picker.loading || disabled)
                        .accessibility_label(format!("{}: {}",tr("推理强度"),label(&level)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.load_reasoning(Some(level.clone()), cx)
                        })))
                }))
            })
            .child(
                crate::focus_scroll::Reveal::new("refresh-reasoning-reveal", &self.reasoning_picker.scroll, Button::new("refresh-reasoning")
                    .ghost()
                    .compact()
                    .label(tr("重新读取"))
                    .disabled(self.reasoning_picker.loading || disabled)
                    .on_click(cx.listener(|this, _, _, cx| this.load_reasoning(None, cx)))),
            );
        content.into_any_element()
    }
}
