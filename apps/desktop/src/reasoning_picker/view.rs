use crate::{app::HiveApp, language::tr, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
pub(crate) fn reasoning_level_label(level: &str) -> &str {
    match level {
        "native" => tr("原生配置"),
        "default" => tr("默认"),
        "max" => tr("最高"),
        "ultra" => tr("极致"),
        "off" | "none" => tr("关闭"),
        "minimal" => tr("最低"),
        "low" => tr("低"),
        "medium" => tr("中"),
        "high" => tr("高"),
        "xhigh" => tr("超高"),
        _ => level,
    }
}
pub(crate) fn should_render_reasoning_panel(
    levels_len: usize,
    loading: bool,
    has_error: bool,
    has_recovery: bool,
    keep_while_loading: bool,
) -> bool {
    levels_len >= 2 || (loading && keep_while_loading) || has_error || has_recovery
}

impl HiveApp {
    pub(crate) fn reasoning_label(&self) -> Option<String> {
        self.reasoning_picker.state.as_ref()
            .filter(|state| !state.level.is_empty() && (state.levels.len() > 1 || state.level != "native"))
            .map(|state| state.level.as_str())
            .or_else(|| self.session.as_ref().and_then(|session| session.inherited_draft_reasoning.as_deref()))
            .map(|level| reasoning_level_label(level).to_owned())
    }

    pub(crate) fn render_reasoning_settings(
        &self,
        running: bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let disabled =
            running || self.loading || !self.conversation_ready || !self.connected || self.closing;
        let state = self.reasoning_picker.state.as_ref();
        let has_options = state.is_some_and(|s| s.levels.len() >= 2);
        let recovery_notice = state
            .and_then(|s| s.description.as_ref())
            .filter(|text| text.starts_with("原设置未能恢复："))
            .map(|text| {
                if let Some(inner) = text
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
                    text.clone()
                }
            });
        if !should_render_reasoning_panel(
            state.map_or(0, |s| s.levels.len()),
            self.reasoning_picker.loading,
            self.reasoning_picker.error.is_some(),
            recovery_notice.is_some(),
            self.model_picker.had_right_column,
        ) {
            return None;
        }
        let focused_level = self.model_picker_focused_reasoning();
        let content = v_flex()
            .id("reasoning-picker-content")
            .role(Role::Group)
            .aria_label(tr("推理强度"))
            .w_full()
            .min_w_0()
            .px_1p5()
            .py_1p5()
            .gap_1()
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_1()
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Muted.get(cx))
                            .child(tr("推理强度")),
                    )
                    .when(self.reasoning_picker.error.is_some(), |row| {
                        row.child(
                            Button::new("refresh-reasoning")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(20.))
                                .w(px(20.))
                                .rounded_md()
                                .icon(Glyph::RefreshCw)
                                .tooltip(tr("重新读取"))
                                .accessibility_label(tr("重新读取"))
                                .disabled(self.reasoning_picker.loading || disabled)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.load_reasoning(None, cx)),
                                ),
                        )
                    }),
            )
            .when(self.reasoning_picker.loading && !has_options, |view| {
                view.child(
                    div()
                        .px_1()
                        .text_size(px(11.5))
                        .text_color(Color::Muted.get(cx))
                        .child(tr("正在读取…")),
                )
            })
            .when_some(self.reasoning_picker.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(Color::ErrorSurface.get(cx))
                        .text_size(px(11.5))
                        .text_color(Color::Error.get(cx))
                        .child(crate::language::localize_owned_error(error)),
                )
            })
            .when_some(recovery_notice, |view, text| {
                view.child(
                    div()
                        .px_1()
                        .text_size(px(11.))
                        .text_color(Color::Muted.get(cx))
                        .child(text),
                )
            })
            .when_some(state.filter(|s| s.levels.len() >= 2), |view, state| {
                view.child(
                    v_flex()
                        .w_full()
                        .min_w_0()
                        .gap_0p5()
                        .children(state.levels.iter().enumerate().map(|(i, level)| {
                            let active = state.level == *level;
                            let keyboard_focused = focused_level == Some(level.as_str());
                            let level_label = reasoning_level_label(level).to_owned();
                            let level_key = level.clone();
                            let is_default_tag =
                                !active && matches!(level.as_str(), "default" | "native");
                            Button::new(("reasoning-option", i))
                                .custom(crate::palette::sidebar_item_variant(
                                    active || keyboard_focused,
                                    cx,
                                ))
                                .compact()
                                .w_full()
                                .min_w_0()
                                .h(px(28.))
                                .px_2()
                                .rounded_md()
                                .justify_start()
                                .selected(active || keyboard_focused)
                                .disabled(self.reasoning_picker.loading || disabled)
                                .accessibility_label(format!("{}: {}", tr("推理强度"), level_label))
                                .child(
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .items_center()
                                        .justify_between()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_size(px(12.5))
                                                .font_weight(if active {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::NORMAL
                                                })
                                                .text_color(if active {
                                                    Color::Text.get(cx)
                                                } else {
                                                    Color::Secondary.get(cx)
                                                })
                                                .child(level_label),
                                        )
                                        .when(active, |row| {
                                            row.child(
                                                Icon::new(Glyph::Check)
                                                    .size(px(13.))
                                                    .text_color(Color::Accent.get(cx))
                                                    .flex_shrink_0(),
                                            )
                                        })
                                        .when(is_default_tag, |row| {
                                            row.child(
                                                div()
                                                    .flex_shrink_0()
                                                    .text_size(px(10.5))
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(tr("默认")),
                                            )
                                        }),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.load_reasoning(Some(level_key.clone()), cx)
                                }))
                        })),
                )
            });
        Some(content.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::{reasoning_level_label, should_render_reasoning_panel};

    #[test]
    fn reasoning_level_labels_are_localized() {
        let _locale = crate::language::test_locale_guard();
        for level in [
            "native", "default", "max", "ultra", "off", "none", "minimal", "low", "medium",
            "high", "xhigh",
        ] {
            assert_ne!(
                reasoning_level_label(level),
                level,
                "expected localized label for effort `{level}`"
            );
        }
    }

    #[test]
    fn reasoning_settings_return_none_when_single_level_and_idle() {
        assert!(!should_render_reasoning_panel(0, false, false, false, false));
        assert!(!should_render_reasoning_panel(1, false, false, false, false));
        assert!(should_render_reasoning_panel(2, false, false, false, false));
        // Initial/empty loading must not expand a transient right column.
        assert!(!should_render_reasoning_panel(0, true, false, false, false));
        assert!(!should_render_reasoning_panel(1, true, false, false, false));
        // Switching from a model that already had a right column keeps placeholder while loading.
        assert!(should_render_reasoning_panel(0, true, false, false, true));
        assert!(should_render_reasoning_panel(0, false, true, false, false));
        assert!(should_render_reasoning_panel(1, false, false, true, false));
    }
}
