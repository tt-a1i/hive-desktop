use crate::language::tr;
use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        scroll::ScrollableElement,
        Icon, *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::Value;
impl HiveApp {
    pub(crate) fn render_commands(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let commands = self.matching_commands(cx);
        v_flex()
            .w((window.viewport_size().width - px(48.)).min(px(500.)))
            .id("native-command-menu")
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.command_picker.open = false;
                cx.notify();
            }))
            .occlude()
            .shadow_lg()
            .p_0()
            .rounded_xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Card.get(cx))
            .overflow_hidden()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(Color::Divider.get(cx))
                    .child(
                        h_flex()
                            .min_w_0()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(Glyph::Slash)
                                    .size(px(13.))
                                    .text_color(Color::Accent.get(cx))
                                    .flex_shrink_0(),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr("原生命令")),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("browse-session-resources")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(26.))
                                    .px_2()
                                    .rounded_lg()
                                    .icon(Glyph::FolderOpen)
                                    .label(tr("浏览资源"))
                                    .accessibility_label(tr("浏览当前会话的技能、模板与扩展来源"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_session_resources(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("refresh-native-commands")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(26.))
                                    .w(px(26.))
                                    .rounded_lg()
                                    .icon(Glyph::RefreshCw)
                                    .tooltip(tr("刷新"))
                                    .accessibility_label(tr("刷新"))
                                    .disabled(self.command_picker.loading)
                                    .on_click(cx.listener(|this, _, _, cx| this.load_commands(cx))),
                            )
                            .child(
                                Button::new("close-native-commands")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(26.))
                                    .w(px(26.))
                                    .rounded_lg()
                                    .icon(Glyph::X)
                                    .tooltip(tr("关闭命令选择"))
                                    .accessibility_label(tr("关闭命令选择"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.command_picker.open = false;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .when(self.command_picker.loading, |view| {
                view.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("正在读取会话命令…")),
                )
            })
            .when_some(
                self.command_picker.error.as_ref().map(|error| error.text()),
                |view, error| {
                    view.child(
                        div()
                            .mx_3()
                            .my_2()
                            .p_2p5()
                            .rounded_lg()
                            .bg(Color::ErrorSurface.get(cx))
                            .text_xs()
                            .text_color(Color::Error.get(cx))
                            .child(error),
                    )
                },
            )
            .when(
                !self.command_picker.loading
                    && self.command_picker.error.is_none()
                    && commands.is_empty(),
                |view| {
                    view.child(
                        div()
                            .px_4()
                            .py_5()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("没有匹配的命令；可继续输入或按 Esc 返回。")),
                    )
                },
            )
            .when(!commands.is_empty(), |view| {
                view.child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .child(crate::focus_scroll::Viewport::new(
                            "native-command-results-viewport",
                            &self.command_picker.scroll,
                            v_flex()
                                .id("native-command-results")
                                .w_full()
                                .flex_1()
                                .min_w_0()
                                .max_h(px(220.))
                                .p_1p5()
                                .gap_0p5()
                                .overflow_y_scroll()
                                .track_scroll(&self.command_picker.scroll)
                                .vertical_scrollbar(&self.command_picker.scroll)
                        .children(commands.iter().enumerate().map(|(i, command)| {
                            let name = command.name.clone();
                            let active = i == self.command_picker.selected;
                            let kind = match command.source.as_str() {
                                "skill" => tr("技能"),
                                "prompt" => tr("模板"),
                                _ => tr("扩展"),
                            };
                            let path = command
                                .source_info
                                .get("path")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            crate::focus_scroll::Reveal::new(
                                ("native-command-reveal", i),
                                &self.command_picker.scroll,
                                Button::new(("native-command", i))
                                    .ghost()
                                    .compact()
                                    .w_full()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .h_auto()
                                    .px_2p5()
                                    .py_1p5()
                                    .rounded_lg()
                                    .justify_start()
                                    .selected(active)
                                    .disabled(self.command_picker.loading)
                                    .accessibility_label(format!("{} /{}", tr("选择命令"), command.name))
                                    .tooltip(format!(
                                        "{}\n{}",
                                        command.description.as_deref().unwrap_or(""),
                                        path
                                    ))
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .items_center()
                                            .justify_between()
                                            .gap_2p5()
                                            .child(
                                                v_flex()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .items_start()
                                                    .gap_0p5()
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .min_w_0()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .min_w_0()
                                                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                                                    .text_size(px(12.5))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(Color::Text.get(cx))
                                                                    .truncate()
                                                                    .child(format!("/{}", command.name)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .flex_shrink_0()
                                                                    .px_1p5()
                                                                    .py(px(1.))
                                                                    .rounded_md()
                                                                    .bg(Color::Inset.get(cx))
                                                                    .text_size(px(10.5))
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(Color::Muted.get(cx))
                                                                    .child(kind),
                                                            ),
                                                    )
                                                    .when_some(command.description.clone(), |view, text| {
                                                        view.child(
                                                            div()
                                                                .w_full()
                                                                .min_w_0()
                                                                .text_xs()
                                                                .text_color(Color::Muted.get(cx))
                                                                .truncate()
                                                                .child(text),
                                                        )
                                                    }),
                                            )
                                            .when(active, |row| {
                                                row.child(
                                                    Icon::new(Glyph::CornerDownLeft)
                                                        .size(px(13.))
                                                        .text_color(Color::Accent.get(cx))
                                                        .flex_shrink_0(),
                                                )
                                            }),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.accept_command(name.clone(), window, cx)
                                    })),
                            )
                        })),
                        ),
                    ),
                )
            })
            .child(
                div()
                    .px_3()
                    .py_1p5()
                    .border_t_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Inset.get(cx).opacity(0.45))
                    .text_size(px(11.))
                    .text_color(Color::Muted.get(cx))
                    .child(tr("输入 / 搜索 · ↑↓ 选择 · Enter 补入草稿 · Esc 关闭")),
            )
            .into_any_element()
    }
}
