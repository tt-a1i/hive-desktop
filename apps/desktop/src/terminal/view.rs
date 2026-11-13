use super::*;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
};

impl HiveApp {
    pub fn render_terminal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = !self.connected
            || self.workspace.is_none()
            || !self.workspace_writes_allowed()
            || self.terminal.loading;
        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .gap_3()
            .p_3()
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("工作区终端")),
                    )
                    .child(
                        Button::new("terminal-refresh")
                            .label(tr("刷新 Shell"))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| this.load_terminals(false, cx))),
                    )
                    .child(
                        Button::new("terminal-start")
                            .primary()
                            .label(tr("启动 Shell"))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.remember_terminal_focus(window, cx);
                                this.start_terminal(cx);
                            })),
                    )
                    .child(
                        Button::new("terminal-stop")
                            .label(tr("停止 Shell"))
                            .disabled(disabled || self.terminal.run_id.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.stop_terminal(cx))),
                    )
                    .child(
                        Button::new("terminal-hide")
                            .label(tr("隐藏终端"))
                            .on_click(cx.listener(|this, _, _, cx| this.hide_terminal(cx))),
                    ),
            )
            .when(self.error.is_some(), |view| {
                view.child(self.render_error_banner(cx))
            })
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .text_sm()
                    .whitespace_normal()
                    .child(if self.workspace.is_none() {
                        tr("先选择工作区。").to_string()
                    } else if !self.workspace_writes_allowed() {
                        tr("正在确认或移除工作区；终端输入已禁用").to_string()
                    } else {
                        self.terminal.status.text()
                    }),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .whitespace_normal()
                    .child(tr("隐藏后 Shell 继续运行；“停止 Shell”才结束进程。")),
            )
            .when_some(self.terminal.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .id("terminal-error")
                        .w_full()
                        .min_w_0()
                        .max_h(px(120.))
                        .overflow_y_scroll()
                        .p_2()
                        .bg(Color::ErrorSurface.get(cx))
                        .text_sm()
                        .whitespace_normal()
                        .child(terminal_error_text(error)),
                )
            })
            .child(
                h_flex()
                    .min_w_0()
                    .flex_shrink_0()
                    .flex_wrap()
                    .gap_2()
                    .children(self.terminal.runs.iter().enumerate().map(|(i, run)| {
                        let id = run.run_id.clone();
                        let current = self.terminal.run_id.as_ref() == Some(&run.run_id)
                            && self.terminal.attachment.is_some();
                        let label = if current && self.terminal.ready {
                            if english() {format!("Current {} · Connected",run.name)} else {format!("当前 {} · 已连接",run.name)}
                        } else {
                            format!(
                                "{} {} · {}",tr("连接"),
                                run.name,
                                match run.status.as_str() {
                                    "running" => tr("运行中"),
                                    "starting" => tr("启动中"),
                                    _ => run.status.as_str(),
                                }
                            )
                        };
                        Button::new(("terminal-run", i))
                            .ghost()
                            .compact()
                            .rounded_full()
                            .label(label)
                            .selected(current)
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.remember_terminal_focus(window, cx);
                                this.attach_terminal(id.clone(), cx);
                            }))
                    })),
            )
            .when(!self.terminal.title.is_empty(), |view| {
                view.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(Theme::global(cx).mono_font_family.clone())
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(self.terminal.title.clone()),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .rounded_xl()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(rgb(0x181a1b))
                    .child(self.terminal.view.clone()),
            )
    }
}
