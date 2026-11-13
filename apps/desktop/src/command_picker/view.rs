use crate::language::tr;
use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::Value;
impl HiveApp {
    pub(crate) fn render_commands(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let commands = self.matching_commands(cx);
        v_flex()
            .w((window.viewport_size().width - px(48.)).min(px(480.)))
            .id("native-command-menu")
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.command_picker.open = false;
                cx.notify();
            }))
            .occlude()
            .shadow_lg()
            .gap_2()
            .p_2()
            .rounded_lg()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Inset.get(cx))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("原生命令")),
                    )
                    .child(
                        Button::new("browse-session-resources").ghost().compact().label(tr("浏览资源"))
                            .accessibility_label(tr("浏览当前会话的技能、模板与扩展来源"))
                            .on_click(cx.listener(|this,_,window,cx|this.open_session_resources(window,cx))),
                    )
                    .child(
                        Button::new("refresh-native-commands")
                            .ghost()
                            .compact()
                            .label(tr("刷新"))
                            .disabled(self.command_picker.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.load_commands(cx))),
                    )
                    .child(
                        Button::new("close-native-commands")
                            .ghost()
                            .compact()
                            .icon(Glyph::X)
                            .accessibility_label(tr("关闭命令选择"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.command_picker.open = false;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.command_picker.loading, |view| {
                view.child(div().text_sm().child(tr("正在读取会话命令…")))
            })
            .when_some(self.command_picker.error.as_ref().map(|error|error.text()), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when(
                !self.command_picker.loading
                    && self.command_picker.error.is_none()
                    && commands.is_empty(),
                |view| {
                    view.child(
                        div()
                            .text_sm()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("没有匹配的命令；可继续输入或按 Esc 返回。")),
                    )
                },
            )
            .child(crate::focus_scroll::Viewport::new(
                "native-command-results-viewport",
                &self.command_picker.scroll,
                v_flex()
                    .id("native-command-results")
                    .max_h(px(176.))
                    .overflow_y_scroll()
                    .track_scroll(&self.command_picker.scroll)
                    .vertical_scrollbar(&self.command_picker.scroll)
                    .children(commands.iter().enumerate().map(|(i, command)| {
                        let name = command.name.clone();
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
                                .w_full()
                                .h_auto()
                                .py_2()
                                .justify_start()
                                .selected(i == self.command_picker.selected)
                                .disabled(self.command_picker.loading)
                                .accessibility_label(format!("{} /{}",tr("选择命令"),command.name))
                                .tooltip(format!(
                                    "{}\n{}",
                                    command.description.as_deref().unwrap_or(""),
                                    path
                                ))
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .flex_1()
                                        .gap_1()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .truncate()
                                                        .child(format!("/{}", command.name)),
                                                )
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .px(px(6.))
                                                        .py(px(1.))
                                                        .rounded_full()
                                                        .bg(Color::Card.get(cx))
                                                        .border_1()
                                                        .border_color(Color::Divider.get(cx))
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(kind),
                                                ),
                                        )
                                        .when_some(command.description.clone(), |view, text| {
                                            view.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Color::Secondary.get(cx))
                                                    .truncate()
                                                    .child(text),
                                            )
                                        }),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.accept_command(name.clone(), window, cx)
                                })),
                        )
                    })),
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(tr("输入 / 搜索 · ↑↓ 选择 · Enter 补入草稿 · Esc 关闭")),
            )
            .into_any_element()
    }
}
