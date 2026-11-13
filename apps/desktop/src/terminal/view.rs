use super::*;
use crate::palette::Color;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        tooltip::Tooltip,
        Icon, *,
    },
    prelude::FluentBuilder,
};

struct TerminalRunItem {
    run_id: String,
    name: String,
    status: String,
    current: bool,
}

#[derive(IntoElement)]
struct TerminalDrawer {
    app: Entity<HiveApp>,
    sidebar_open: bool,
    connected: bool,
    has_workspace: bool,
    workspace_name: Option<String>,
    workspace_path: Option<String>,
    workspace_writes_allowed: bool,
    loading: bool,
    ready: bool,
    has_run: bool,
    status_text: String,
    terminal_title: String,
    terminal_error: Option<String>,
    app_error_banner: Option<AnyElement>,
    runs: Vec<TerminalRunItem>,
    terminal_view: Entity<TerminalView>,
}

impl RenderOnce for TerminalDrawer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled =
            !self.connected || !self.has_workspace || !self.workspace_writes_allowed || self.loading;
        let topbar_inset = if self.sidebar_open {
            px(16.)
        } else {
            let inset = crate::chrome::traffic_light_inset(window);
            if inset > px(0.) {
                inset + px(8.)
            } else {
                px(12.)
            }
        };
        let status_color = if self.terminal_error.is_some() || !self.connected {
            Color::Error.get(cx)
        } else if self.ready {
            Color::Success.get(cx)
        } else if self.loading {
            rgb(if Theme::global(cx).is_dark() {
                0xfbbf24
            } else {
                0xd97706
            })
        } else {
            Color::Muted.get(cx)
        };
        let status_tooltip = format!(
            "{}\n{}",
            self.status_text,
            tr("隐藏后 Shell 继续运行；“停止 Shell”才结束进程。")
        );
        let subtitle = self
            .workspace_path
            .clone()
            .filter(|path| !path.is_empty())
            .or_else(|| {
                if self.terminal_title.is_empty() {
                    None
                } else {
                    Some(self.terminal_title.clone())
                }
            });
        let title = self
            .workspace_name
            .clone()
            .unwrap_or_else(|| tr("工作区终端").to_owned());

        let app_nav = self.app.clone();
        let app_refresh = self.app.clone();
        let app_start = self.app.clone();
        let app_stop = self.app.clone();
        let app_hide = self.app.clone();
        let term_clear = self.terminal_view.clone();

        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .bg(Color::Surface.get(cx))
            .child(
                crate::chrome::drag_bar("terminal-topbar", window, cx)
                    .w_full()
                    .pl(topbar_inset)
                    .pr_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(Color::Divider.get(cx))
                    .when(!self.sidebar_open, |bar| {
                        bar.child(
                            Button::new("toggle-navigation")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(28.))
                                .w(px(28.))
                                .rounded_lg()
                                .icon(Glyph::PanelLeft)
                                .tooltip(tr("打开或收起导航"))
                                .accessibility_label(tr("打开或收起工作区与会话导航"))
                                .on_click(move |_, _, cx| {
                                    app_nav.update(cx, |this, cx| {
                                        this.sidebar_open = !this.sidebar_open;
                                        cx.notify();
                                    });
                                }),
                        )
                    })
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(Glyph::SquareTerminal)
                                    .size(px(15.))
                                    .text_color(Color::Secondary.get(cx))
                                    .flex_shrink_0(),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(title),
                            )
                            .when_some(subtitle, |row, dir| {
                                row.child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .font_family(Theme::global(cx).mono_font_family.clone())
                                        .text_size(px(11.5))
                                        .text_color(Color::Muted.get(cx))
                                        .child(dir),
                                )
                            })
                            .child(
                                div()
                                    .id("terminal-status-dot")
                                    .flex_shrink_0()
                                    .w(px(7.))
                                    .h(px(7.))
                                    .rounded_full()
                                    .bg(status_color)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(status_tooltip.clone()).build(window, cx)
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("terminal-refresh")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(28.))
                                    .w(px(28.))
                                    .rounded_lg()
                                    .icon(Glyph::RefreshCw)
                                    .tooltip(tr("刷新 Shell"))
                                    .accessibility_label(tr("刷新 Shell"))
                                    .disabled(disabled)
                                    .on_click(move |_, _, cx| {
                                        app_refresh.update(cx, |this, cx| {
                                            this.load_terminals(false, cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new("terminal-clear")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(28.))
                                    .w(px(28.))
                                    .rounded_lg()
                                    .icon(Glyph::Eraser)
                                    .tooltip(tr("清除"))
                                    .accessibility_label(tr("清除"))
                                    .on_click(move |_, _, cx| {
                                        term_clear.update(cx, |view, cx| view.reset(cx));
                                    }),
                            )
                            .child({
                                let btn = Button::new("terminal-start")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(28.))
                                    .rounded_lg()
                                    .icon(Glyph::Play)
                                    .tooltip(tr("启动 Shell"))
                                    .accessibility_label(tr("启动 Shell"))
                                    .disabled(disabled)
                                    .on_click(move |_, window, cx| {
                                        app_start.update(cx, |this, cx| {
                                            this.remember_terminal_focus(window, cx);
                                            this.start_terminal(cx);
                                        });
                                    });
                                if self.has_run {
                                    btn.w(px(28.))
                                } else {
                                    btn.px_2().label(tr("启动 Shell"))
                                }
                            })
                            .child(
                                Button::new("terminal-stop")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(28.))
                                    .w(px(28.))
                                    .rounded_lg()
                                    .icon(Glyph::Square)
                                    .tooltip(tr("停止 Shell"))
                                    .accessibility_label(tr("停止 Shell"))
                                    .disabled(disabled || !self.has_run)
                                    .on_click(move |_, _, cx| {
                                        app_stop.update(cx, |this, cx| this.stop_terminal(cx));
                                    }),
                            )
                            .child(
                                Button::new("terminal-hide")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(28.))
                                    .px_2()
                                    .rounded_lg()
                                    .icon(Glyph::CornerDownLeft)
                                    .label(tr("返回会话"))
                                    .tooltip(tr("隐藏终端"))
                                    .accessibility_label(tr("隐藏终端"))
                                    .on_click(move |_, _, cx| {
                                        app_hide.update(cx, |this, cx| this.hide_terminal(cx));
                                    }),
                            ),
                    ),
            )
            .when(!self.runs.is_empty() || !self.terminal_title.is_empty(), |col| {
                col.child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .flex_shrink_0()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_4()
                        .py_1p5()
                        .border_b_1()
                        .border_color(Color::Divider.get(cx))
                        .bg(Color::Inset.get(cx).opacity(0.45))
                        .child(
                            h_flex()
                                .min_w_0()
                                .flex_wrap()
                                .gap_1p5()
                                .children(self.runs.into_iter().enumerate().map(|(i, run)| {
                                    let id = run.run_id;
                                    let app = self.app.clone();
                                    let label = if run.current && self.ready {
                                        if english() {
                                            format!("Current {} · Connected", run.name)
                                        } else {
                                            format!("当前 {} · 已连接", run.name)
                                        }
                                    } else {
                                        format!(
                                            "{} {} · {}",
                                            tr("连接"),
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
                                        .xsmall()
                                        .h(px(24.))
                                        .px_2()
                                        .rounded_md()
                                        .label(label)
                                        .selected(run.current)
                                        .disabled(disabled)
                                        .on_click(move |_, window, cx| {
                                            let id = id.clone();
                                            app.update(cx, |this, cx| {
                                                this.remember_terminal_focus(window, cx);
                                                this.attach_terminal(id, cx);
                                            });
                                        })
                                })),
                        )
                        .when(!self.terminal_title.is_empty(), |row| {
                            row.child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                    .text_size(px(11.5))
                                    .text_color(Color::Muted.get(cx))
                                    .child(self.terminal_title.clone()),
                            )
                        }),
                )
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .p_3()
                    .gap_2p5()
                    .when_some(self.app_error_banner, |col, banner| col.child(banner))
                    .when(!self.has_workspace || !self.workspace_writes_allowed, |col| {
                        col.child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .bg(Color::Card.get(cx))
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(if !self.has_workspace {
                                    tr("先选择工作区。")
                                } else {
                                    tr("正在确认或移除工作区；终端输入已禁用")
                                }),
                        )
                    })
                    .when_some(self.terminal_error, |col, error| {
                        col.child(
                            div()
                                .id("terminal-error")
                                .w_full()
                                .min_w_0()
                                .max_h(px(110.))
                                .overflow_y_scroll()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .border_1()
                                .border_color(Color::Error.get(cx).opacity(0.28))
                                .bg(Color::ErrorSurface.get(cx))
                                .child(
                                    crate::text_reading::selectable_text(
                                        "terminal-error-text",
                                        error,
                                    )
                                    .text_xs()
                                    .text_color(Color::Error.get(cx)),
                                ),
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
                            .bg(rgb(0x141417))
                            .child(self.terminal_view),
                    ),
            )
    }
}

impl HiveApp {
    pub fn render_terminal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self
            .workspaces
            .iter()
            .find(|ws| Some(&ws.id) == self.workspace.as_ref());
        let workspace_name = workspace.map(|ws| ws.name.clone());
        let workspace_path = workspace.map(|ws| crate::palette::display_path(&ws.path));
        let runs = self
            .terminal
            .runs
            .iter()
            .map(|run| TerminalRunItem {
                run_id: run.run_id.clone(),
                name: run.name.clone(),
                status: run.status.clone(),
                current: self.terminal.run_id.as_ref() == Some(&run.run_id)
                    && self.terminal.attachment.is_some(),
            })
            .collect();
        TerminalDrawer {
            app: cx.entity(),
            sidebar_open: self.sidebar_open,
            connected: self.connected,
            has_workspace: self.workspace.is_some(),
            workspace_name,
            workspace_path,
            workspace_writes_allowed: self.workspace_writes_allowed(),
            loading: self.terminal.loading,
            ready: self.terminal.ready,
            has_run: self.terminal.run_id.is_some(),
            status_text: self.terminal.status.text(),
            terminal_title: self.terminal.title.clone(),
            terminal_error: self.terminal.error.as_deref().map(terminal_error_text),
            app_error_banner: self
                .error
                .is_some()
                .then(|| self.render_error_banner(cx).into_any_element()),
            runs,
            terminal_view: self.terminal.view.clone(),
        }
    }
}
