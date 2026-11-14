use crate::{app::HiveApp, language::tr, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        popover::Popover,
        Icon, *,
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

    pub(crate) fn reasoning_picker_trigger(
        &self,
        window: &mut Window,
        running: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.reasoning_picker.state.as_ref()?;
        if state.levels.len() <= 1 && state.level == "native" {
            return None;
        }
        let open_state = window.use_keyed_state("reasoning-popover-open", cx, |_, _| false);
        let open = *open_state.read(cx);
        let unavailable =
            !self.connected || !self.conversation_ready || running || self.loading || self.closing;
        let level_text = label(&state.level).to_owned();
        let trigger = Button::new("choose-reasoning")
            .custom(crate::palette::quiet_icon_variant(cx))
            .compact()
            .h(px(26.))
            .px_1p5()
            .rounded_lg()
            .selected(open)
            .disabled(unavailable)
            .accessibility_label(tr("设置会话推理强度"))
            .tooltip(tr("设置会话推理强度"))
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(Color::Muted.get(cx))
                    .child(level_text),
            );
        if unavailable {
            return Some(trigger.into_any_element());
        }
        Some(
            Popover::new("session-reasoning-popover")
                .anchor(Anchor::BottomRight)
                .p_2()
                .rounded_xl()
                .border_1()
                .border_color(Color::Divider.get(cx))
                .bg(Color::Card.get(cx))
                .shadow_lg()
                .w(px(220.))
                .open(open)
                .trigger(trigger)
                .on_open_change({
                    let open_state = open_state.clone();
                    move |next_open, _, cx| {
                        open_state.update(cx, |value, cx| {
                            *value = *next_open;
                            cx.notify();
                        });
                    }
                })
                .when(open, |popover| {
                    popover.child(self.render_reasoning_settings(running, cx))
                })
                .into_any_element(),
        )
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
            .gap_1p5()
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_1()
                    .pb_1()
                    .child(
                        h_flex()
                            .min_w_0()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(Glyph::SlidersHorizontal)
                                    .size(px(13.))
                                    .text_color(Color::Muted.get(cx))
                                    .flex_shrink_0(),
                            )
                            .child(
                                crate::text_reading::selectable_text("reasoning-heading", tr("会话推理强度"))
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Muted.get(cx)),
                            ),
                    )
                    .child(
                        crate::focus_scroll::Reveal::new(
                            "refresh-reasoning-reveal",
                            &self.reasoning_picker.scroll,
                            Button::new("refresh-reasoning")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(24.))
                                .w(px(24.))
                                .rounded_md()
                                .icon(Glyph::RefreshCw)
                                .tooltip(tr("重新读取"))
                                .accessibility_label(tr("重新读取"))
                                .disabled(self.reasoning_picker.loading || disabled)
                                .on_click(cx.listener(|this, _, _, cx| this.load_reasoning(None, cx))),
                        ),
                    ),
            )
            .when(self.reasoning_picker.loading, |view| {
                view.child(
                    crate::text_reading::selectable_text("reasoning-loading", tr("正在读取原生设置…"))
                        .px_1()
                        .text_xs()
                        .text_color(Color::Muted.get(cx)),
                )
            })
            .when_some(self.reasoning_picker.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .px_2()
                        .py_1p5()
                        .rounded_lg()
                        .bg(Color::ErrorSurface.get(cx))
                        .child(
                            crate::text_reading::selectable_text(
                                "reasoning-error",
                                crate::language::localize_owned_error(error),
                            )
                            .text_xs()
                            .text_color(Color::Error.get(cx)),
                        ),
                )
            })
            .when(
                !self.reasoning_picker.loading
                    && self.reasoning_picker.error.is_none()
                    && self.reasoning_picker.state.is_none(),
                |view| {
                    view.child(
                        crate::text_reading::selectable_text(
                            "reasoning-empty",
                            tr("当前模型没有可调推理选项。"),
                        )
                        .px_1()
                        .text_xs()
                        .text_color(Color::Muted.get(cx)),
                    )
                },
            )
            .when_some(self.reasoning_picker.state.as_ref(), |view, state| {
                view.when_some(state.description.as_ref(), |view, text| {
                    // Translate only metadata owned by Hive's native adapters.
                    // Preserve unknown/provider diagnostics verbatim.
                    let text = if let Some(inner) = text
                        .strip_prefix("原设置未能恢复：")
                        .and_then(|rest| rest.strip_suffix(" 请选择当前可用档位。"))
                    {
                        if crate::language::english() {
                            format!(
                                "Could not restore saved effort: {} Choose an available level.",
                                crate::language::localize_owned_error(inner)
                            )
                        } else {
                            text.clone()
                        }
                    } else {
                        match text.as_str() {
                            "从原生 Codex 线程读取；档位由当前模型提供。" => tr("从原生 Codex 线程读取；档位由当前模型提供。").to_owned(),
                            "读取原生实际强度；环境、组织限制或模型能力可能调整请求值。" => tr("读取原生实际强度；环境、组织限制或模型能力可能调整请求值。").to_owned(),
                            _ => text.clone(),
                        }
                    };
                    view.child(
                        crate::text_reading::selectable_text("reasoning-description", text)
                            .px_1()
                            .pb_0p5()
                            .text_size(px(11.5))
                            .text_color(Color::Muted.get(cx)),
                    )
                })
                .when(state.levels.len() < 2, |view| {
                    view.child(
                        crate::text_reading::selectable_text("reasoning-unavailable", tr("当前模型没有可调推理选项。"))
                            .px_1()
                            .text_xs()
                            .text_color(Color::Muted.get(cx)),
                    )
                })
                .child(
                    v_flex()
                        .w_full()
                        .min_w_0()
                        .gap_0p5()
                        .children(state.levels.iter().enumerate().map(|(i, level)| {
                            let active = state.level == *level;
                            let level_label = label(level).to_owned();
                            let level = level.clone();
                            crate::focus_scroll::Reveal::new(
                                ("reasoning-option-reveal", i),
                                &self.reasoning_picker.scroll,
                                Button::new(("reasoning-option", i))
                                    .ghost()
                                    .compact()
                                    .w_full()
                                    .h(px(30.))
                                    .px_2()
                                    .rounded_lg()
                                    .justify_start()
                                    .selected(active)
                                    .disabled(self.reasoning_picker.loading || disabled)
                                    .accessibility_label(format!("{}: {}", tr("推理强度"), level_label))
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_size(px(12.5))
                                                    .font_weight(if active {
                                                        FontWeight::SEMIBOLD
                                                    } else {
                                                        FontWeight::MEDIUM
                                                    })
                                                    .text_color(Color::Text.get(cx))
                                                    .child(level_label),
                                            )
                                            .when(active, |row| {
                                                row.child(
                                                    Icon::new(Glyph::Check)
                                                        .size(px(13.5))
                                                        .text_color(Color::Accent.get(cx))
                                                        .flex_shrink_0(),
                                                )
                                            }),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.load_reasoning(Some(level.clone()), cx)
                                    })),
                            )
                        })),
                )
            });
        content.into_any_element()
    }
}
