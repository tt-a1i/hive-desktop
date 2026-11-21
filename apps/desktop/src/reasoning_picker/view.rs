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
        if !has_options
            && !self.reasoning_picker.loading
            && self.reasoning_picker.error.is_none()
            && recovery_notice.is_none()
        {
            return None;
        }
        let content = v_flex()
            .id("reasoning-picker-content")
            .role(Role::Group)
            .aria_label(tr("推理强度"))
            .w_full()
            .min_w_0()
            .px_2p5()
            .py_2()
            .gap_1p5()
            .border_t_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_2()
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
                        .text_size(px(11.))
                        .text_color(Color::Muted.get(cx))
                        .child(text),
                )
            })
            .when_some(state.filter(|s| s.levels.len() >= 2), |view, state| {
                view.child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .items_center()
                        .gap(px(2.))
                        .p(px(2.))
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .children(state.levels.iter().enumerate().map(|(i, level)| {
                            let active = state.level == *level;
                            let level_label = reasoning_level_label(level).to_owned();
                            let level = level.clone();
                            Button::new(("reasoning-option", i))
                                .custom(crate::palette::segment_variant(active, cx))
                                .compact()
                                .xsmall()
                                .flex_1()
                                .min_w_0()
                                .h(px(24.))
                                .px_1()
                                .rounded_md()
                                .justify_center()
                                .selected(active)
                                .disabled(self.reasoning_picker.loading || disabled)
                                .accessibility_label(format!("{}: {}", tr("推理强度"), level_label))
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(11.5))
                                        .font_weight(if active {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(if active {
                                            Color::Text.get(cx)
                                        } else {
                                            Color::Muted.get(cx)
                                        })
                                        .child(level_label),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.load_reasoning(Some(level.clone()), cx)
                                }))
                        })),
                )
            });
        Some(content.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::reasoning_level_label;

    #[test]
    fn reasoning_level_labels_are_localized() {
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
}
