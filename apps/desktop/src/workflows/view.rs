use super::types::*;
use crate::{
    app::HiveApp,
    chrome::SubviewTopbar,
    focus_scroll::{Reveal, Viewport},
    language::{english, tr},
    palette::{self, Color},
};
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
use serde_json::json;

impl HiveApp {
    pub(crate) fn render_workflows(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.workflows.loading && std::mem::take(&mut self.workflows.focus_navigation) {
            let focus = self.workflow_navigation_focus.clone();
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        let state = &self.workflows;
        let scroll = if state.selected.is_some() {
            &state.detail_scroll
        } else {
            &state.list_scroll
        };
        let disabled =
            !self.connected || state.loading || state.acting || !self.workspace_writes_allowed();
        let count = state
            .runs
            .iter()
            .filter(|run| match state.filter {
                "running" => run.status == "running",
                "failed" => matches!(run.status.as_str(), "failed" | "stopped" | "interrupted"),
                _ => true,
            })
            .count();

        let topbar_back = if state.selected.is_some() {
            Button::new("workflow-back")
                .ghost()
                .compact()
                .rounded_lg()
                .icon(Glyph::ArrowLeft)
                .label(tr("返回运行列表"))
                .on_click(cx.listener(|this, _, _, cx| this.select_workflow(None, cx)))
        } else {
            self.return_to_chat_button(cx).icon(Glyph::ArrowLeft)
        };

        let mut topbar = SubviewTopbar::new(
            "workflows-topbar",
            tr("工作流"),
            self.sidebar_open,
            cx.weak_entity(),
        )
        .back(topbar_back)
        .action(
            Button::new("workflow-refresh")
                .track_focus(&self.workflow_navigation_focus)
                .custom(palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(28.))
                .w(px(28.))
                .rounded_lg()
                .icon(Glyph::RefreshCw)
                .tooltip(if state.loading {
                    tr("更新中…")
                } else {
                    tr("刷新")
                })
                .accessibility_label(tr("刷新"))
                .disabled(disabled)
                .on_click(cx.listener(|this, _, _, cx| this.refresh_workflows(cx))),
        );

        if state.selected.is_none() {
            topbar = topbar.badge(format!("{}", state.runs.len()));
        }

        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_hidden()
            .bg(Color::Surface.get(cx))
            .child(topbar)
            .when_some(state.action_error.as_ref().map(|error| error.text()), |view, error| {
                view.child(
                    div()
                        .w_full()
                        .max_w(px(880.))
                        .mx_auto()
                        .px_5()
                        .pt_3()
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .child(
                                    crate::text_reading::selectable_text(
                                        "workflow-action-error",
                                        error,
                                    )
                                    .text_xs()
                                    .text_color(Color::Error.get(cx)),
                                ),
                        ),
                )
            })
            .when_some(state.error.as_ref().map(|error| error.text()), |view, error| {
                view.child(
                    div()
                        .w_full()
                        .max_w(px(880.))
                        .mx_auto()
                        .px_5()
                        .pt_3()
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .child(
                                    crate::text_reading::selectable_text(
                                        "workflow-read-error",
                                        error,
                                    )
                                    .text_xs()
                                    .text_color(Color::Error.get(cx)),
                                ),
                        ),
                )
            })
            .child(Viewport::new(
                "workflow-viewport",
                scroll,
                crate::text_reading::region(
                    state
                        .selected
                        .as_ref()
                        .map(|id| format!("workflow-scroll-{id}"))
                        .unwrap_or_else(|| "workflows-list-scroll".into()),
                    tr("工作流内容，可用方向键、Home/End 或翻页键滚动").into(),
                    scroll,
                    window.viewport_size().height,
                    cx,
                )
                .flex_1()
                .w_full()
                .min_h_0()
                .border_color(rgba(0))
                .p_0()
                .vertical_scrollbar(scroll)
                .px_5()
                .py_5()
                .child(
                    v_flex()
                        .w_full()
                        .max_w(px(880.))
                        .mx_auto()
                        .flex_shrink_0()
                        .gap_4()
                        .when(state.selected.is_none(), |view| {
                            view.child(self.render_workflow_settings(cx))
                        })
                        .when(state.enabled == Some(false), |view| {
                            view.child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr("实验功能当前关闭；已有运行和计划仍可查看。")),
                            )
                        })
                        .when(state.selected.is_some(), |view| {
                            view.child(self.render_workflow_detail(cx))
                        })
                        .when(state.selected.is_none(), |view| {
                            view.child(Reveal::new(
                                "workflow-filters-reveal",
                                scroll,
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .p_1()
                                            .rounded_lg()
                                            .bg(Color::Inset.get(cx))
                                            .border_1()
                                            .border_color(Color::Divider.get(cx))
                                            .children(
                                                [
                                                    ("all", tr("全部")),
                                                    ("running", tr("运行中")),
                                                    ("failed", tr("需关注")),
                                                ]
                                                .into_iter()
                                                .map(|(filter, label)| {
                                                    Button::new(SharedString::from(format!(
                                                        "workflow-filter-{filter}"
                                                    )))
                                                    .compact()
                                                    .xsmall()
                                                    .h(px(24.))
                                                    .rounded_md()
                                                    .label(label)
                                                    .selected(state.filter == filter)
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.workflows.filter = filter;
                                                        cx.notify();
                                                    }))
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(if english() {
                                                format!(
                                                    "Showing {count} of {} runs",
                                                    state.runs.len()
                                                )
                                            } else {
                                                format!(
                                                    "显示 {count} / {} 条运行",
                                                    state.runs.len()
                                                )
                                            }),
                                    ),
                            ))
                            .when(state.runs.is_empty() && state.loaded, |view| {
                                view.child(
                                    v_flex()
                                        .items_center()
                                        .justify_center()
                                        .gap_2()
                                        .py_10()
                                        .px_6()
                                        .rounded_xl()
                                        .bg(Color::Card.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .child(
                                            div()
                                                .size(px(36.))
                                                .rounded_xl()
                                                .bg(Color::Inset.get(cx))
                                                .border_1()
                                                .border_color(Color::Divider.get(cx))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    Icon::new(Glyph::Workflow)
                                                        .size(px(18.))
                                                        .text_color(Color::Muted.get(cx)),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(Color::Text.get(cx))
                                                .child(tr("还没有工作流运行")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_center()
                                                .max_w(px(360.))
                                                .text_color(Color::Muted.get(cx))
                                                .child(tr(
                                                    "向主控描述目标，由主控编写和启动工作流。这里会显示真实运行、日志与派单。",
                                                )),
                                        ),
                                )
                            })
                            .when(!state.runs.is_empty() && count == 0, |view| {
                                view.child(
                                    v_flex()
                                        .items_center()
                                        .justify_center()
                                        .py_8()
                                        .px_6()
                                        .rounded_xl()
                                        .bg(Color::Card.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("没有符合筛选的运行。")),
                                )
                            })
                            .when(count > 0, |view| {
                                view.child(
                                    v_flex()
                                        .rounded_xl()
                                        .bg(Color::Card.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .overflow_hidden()
                                        .children(
                                            state
                                                .runs
                                                .iter()
                                                .filter(|run| match state.filter {
                                                    "running" => run.status == "running",
                                                    "failed" => matches!(
                                                        run.status.as_str(),
                                                        "failed" | "stopped" | "interrupted"
                                                    ),
                                                    _ => true,
                                                })
                                                .enumerate()
                                                .map(|(idx, run)| {
                                                    let id = run.id.clone();
                                                    let parent = run.parent_run_id.as_ref().map(|id| {
                                                        state
                                                            .runs
                                                            .iter()
                                                            .find(|run| &run.id == id)
                                                            .map(|run| run.name.clone())
                                                            .unwrap_or_else(|| id.clone())
                                                    });
                                                    let (_badge_bg, badge_fg, badge_dot) =
                                                        status_badge_colors(&run.status, cx);
                                                    let dot_color = badge_dot.unwrap_or_else(|| {
                                                        match run.status.as_str() {
                                                            "running" => Color::Accent.get(cx),
                                                            "failed" => Color::Error.get(cx),
                                                            _ => Color::Muted.get(cx),
                                                        }
                                                    });
                                                    Reveal::new(
                                                        SharedString::from(format!(
                                                            "workflow-run-reveal-{}",
                                                            run.id
                                                        )),
                                                        scroll,
                                                        div()
                                                            .when(idx > 0, |row| {
                                                                row.border_t_1().border_color(
                                                                    Color::Divider.get(cx),
                                                                )
                                                            })
                                                            .child(
                                                                Button::new(SharedString::from(
                                                                    format!("workflow-run-{}", run.id),
                                                                ))
                                                                .ghost()
                                                                .w_full()
                                                                .h_auto()
                                                                .justify_start()
                                                                .p_3p5()
                                                                .rounded_none()
                                                                .accessibility_label(format!(
                                                                    "{}: {} · {}",
                                                                    tr("查看工作流"),
                                                                    run.name,
                                                                    status(&run.status)
                                                                ))
                                                                .child(
                                                                    v_flex()
                                                                        .w_full()
                                                                        .min_w_0()
                                                                        .items_start()
                                                                        .text_left()
                                                                        .gap_1p5()
                                                                        .child(
                                                                            h_flex()
                                                                                .w_full()
                                                                                .min_w_0()
                                                                                .gap_2()
                                                                                .flex_wrap()
                                                                                .items_center()
                                                                                .child(
                                                                                    div()
                                                                                        .flex_1()
                                                                                        .min_w_0()
                                                                                        .text_size(px(13.))
                                                                                        .font_weight(FontWeight::SEMIBOLD)
                                                                                        .text_color(Color::Text.get(cx))
                                                                                        .child(run.name.clone()),
                                                                                )
                                                                                .child(
                                                                                    h_flex()
                                                                                        .items_center()
                                                                                        .gap_1p5()
                                                                                        .px_2()
                                                                                        .py(px(1.))
                                                                                        .rounded_md()
                                                                                        .bg(Color::Inset.get(cx))
                                                                                        .border_1()
                                                                                        .border_color(Color::Divider.get(cx))
                                                                                        .text_size(px(11.))
                                                                                        .font_weight(FontWeight::MEDIUM)
                                                                                        .text_color(badge_fg)
                                                                                        .child(
                                                                                            div()
                                                                                                .size(px(6.))
                                                                                                .rounded_full()
                                                                                                .bg(dot_color),
                                                                                        )
                                                                                        .child(
                                                                                            status(&run.status)
                                                                                                .to_owned(),
                                                                                        ),
                                                                                ),
                                                                        )
                                                                        .when_some(parent, |view, parent| {
                                                                            view.child(
                                                                                div()
                                                                                    .text_xs()
                                                                                    .text_color(
                                                                                        Color::Secondary.get(cx),
                                                                                    )
                                                                                    .child(format!(
                                                                                        "{}: {parent}",
                                                                                        tr("子流程 · 父运行")
                                                                                    )),
                                                                            )
                                                                        })
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .text_color(
                                                                                    Color::Muted.get(cx),
                                                                                )
                                                                                .child(if english() {
                                                                                    format!(
                                                                                        "{} · {} agents · Started {}",
                                                                                        basename(&run.script_path),
                                                                                        run.agent_count,
                                                                                        relative_time(run.started_at)
                                                                                    )
                                                                                } else {
                                                                                    format!(
                                                                                        "{} · {} 位执行成员 · {}开始",
                                                                                        basename(&run.script_path),
                                                                                        run.agent_count,
                                                                                        relative_time(run.started_at)
                                                                                    )
                                                                                }),
                                                                        )
                                                                        .when_some(
                                                                            run.phase.clone(),
                                                                            |view, phase| {
                                                                                view.child(
                                                                                    div()
                                                                                        .text_xs()
                                                                                        .text_color(
                                                                                            Color::Secondary.get(cx),
                                                                                        )
                                                                                        .child(format!(
                                                                                            "{}: {phase}",
                                                                                            tr("阶段")
                                                                                        )),
                                                                                )
                                                                            },
                                                                        )
                                                                        .when_some(
                                                                            run.error.clone(),
                                                                            |view, error| {
                                                                                view.child(
                                                                                    div()
                                                                                        .text_xs()
                                                                                        .text_color(
                                                                                            if matches!(
                                                                                                run.status.as_str(),
                                                                                                "stopped" | "interrupted"
                                                                                            ) {
                                                                                                Color::Secondary.get(cx)
                                                                                            } else {
                                                                                                Color::Error.get(cx)
                                                                                            },
                                                                                        )
                                                                                        .child(error),
                                                                                )
                                                                            },
                                                                        ),
                                                                )
                                                                .on_click(cx.listener(
                                                                    move |this, _, _, cx| {
                                                                        this.select_workflow(
                                                                            Some(id.clone()),
                                                                            cx,
                                                                        )
                                                                    },
                                                                )),
                                                            ),
                                                    )
                                                }),
                                        ),
                                )
                            })
                            .child(
                                div()
                                    .pt_2()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("定时计划")),
                            )
                            .when(state.schedules.is_empty() && state.loaded, |view| {
                                view.child(
                                    v_flex()
                                        .items_center()
                                        .justify_center()
                                        .py_8()
                                        .px_6()
                                        .rounded_xl()
                                        .bg(Color::Card.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("还没有定时计划，由主控创建后在此管理。")),
                                )
                            })
                            .when(!state.schedules.is_empty(), |view| {
                                view.child(
                                    v_flex()
                                        .rounded_xl()
                                        .bg(Color::Card.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .overflow_hidden()
                                        .children(state.schedules.iter().enumerate().map(
                                            |(idx, schedule)| {
                                                v_flex()
                                                    .when(idx > 0, |row| {
                                                        row.border_t_1().border_color(
                                                            Color::Divider.get(cx),
                                                        )
                                                    })
                                                    .child(self.render_workflow_schedule(
                                                        schedule,
                                                        disabled,
                                                        cx,
                                                    ))
                                            },
                                        )),
                                )
                            })
                        }),
                ),
            ))
            .into_any_element()
    }

    fn render_workflow_schedule(
        &self,
        schedule: &Schedule,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let toggle_id = schedule.id.clone();
        let delete_id = schedule.id.clone();
        let confirm_id = schedule.id.clone();
        let enabled = schedule.enabled;
        let mono_font = Theme::global(cx).mono_font_family.clone();
        v_flex()
            .gap_2()
            .p_4()
            .child(Reveal::new(
                SharedString::from(format!("schedule-header-{}", schedule.id)),
                &self.workflows.list_scroll,
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .id(SharedString::from(format!("schedule-name-{}", schedule.id)))
                            .role(Role::Label)
                            .aria_label(basename(&schedule.script_path).to_owned())
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(basename(&schedule.script_path).to_owned()),
                    )
                    .child(
                        h_flex()
                            .id(SharedString::from(format!(
                                "schedule-status-{}",
                                schedule.id
                            )))
                            .role(Role::Label)
                            .aria_label(if enabled {
                                tr("已启用")
                            } else {
                                tr("已暂停")
                            })
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py(px(1.))
                            .rounded_md()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if enabled {
                                Color::Text.get(cx)
                            } else {
                                Color::Secondary.get(cx)
                            })
                            .child(div().size(px(6.)).rounded_full().bg(if enabled {
                                Color::Success.get(cx)
                            } else {
                                Color::Muted.get(cx)
                            }))
                            .child(if enabled {
                                tr("已启用")
                            } else {
                                tr("已暂停")
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "workflow-schedule-toggle-{}",
                            schedule.id
                        )))
                        .outline()
                        .compact()
                        .xsmall()
                        .h(px(26.))
                        .rounded_md()
                        .label(if enabled {
                            tr("暂停计划")
                        } else {
                            tr("恢复计划")
                        })
                        .accessibility_label(format!(
                            "{}: {}",
                            if enabled {
                                tr("暂停计划")
                            } else {
                                tr("恢复计划")
                            },
                            basename(&schedule.script_path)
                        ))
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.workflow_action(
                                "PATCH",
                                format!("/api/workflow-schedules/{toggle_id}"),
                                Some(json!({"enabled": !enabled})),
                                false,
                                cx,
                            )
                        })),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "workflow-schedule-delete-{}",
                            schedule.id
                        )))
                        .outline()
                        .compact()
                        .xsmall()
                        .h(px(26.))
                        .rounded_md()
                        .label(tr("删除…"))
                        .disabled(disabled)
                        .accessibility_label(format!(
                            "{}: {}",
                            tr("删除计划"),
                            basename(&schedule.script_path)
                        ))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.workflows.deleting = Some(delete_id.clone());
                            let focus = this
                                .workflows
                                .delete_cancel_focus
                                .get_or_insert_with(|| cx.focus_handle())
                                .clone();
                            window.on_next_frame(move |window, cx| focus.focus(window, cx));
                            cx.notify();
                        })),
                    ),
            ))
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .px_2()
                            .py(px(1.))
                            .rounded_md()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .font_family(mono_font.clone())
                            .child(
                                crate::text_reading::selectable_text(
                                    SharedString::from(format!("schedule-cron-{}", schedule.id)),
                                    format!(
                                        "Cron: {} · {}",
                                        schedule.cron,
                                        if enabled {
                                            format!(
                                                "{}: {}",
                                                tr("下次运行"),
                                                relative_time(schedule.next_run_at)
                                            )
                                        } else {
                                            tr("暂停期间不触发").into()
                                        }
                                    ),
                                )
                                .text_size(px(11.))
                                .text_color(Color::Secondary.get(cx)),
                            ),
                    ),
            )
            .when_some(schedule.last_run_at, |view, time| {
                view.child(
                    crate::text_reading::selectable_text(
                        SharedString::from(format!("schedule-last-run-{}", schedule.id)),
                        format!("{}: {}", tr("上次运行"), relative_time(time)),
                    )
                    .text_xs()
                    .text_color(Color::Muted.get(cx)),
                )
            })
            .child(
                crate::text_reading::selectable_text(
                    SharedString::from(format!("schedule-path-{}", schedule.id)),
                    schedule.script_path.clone(),
                )
                .font_family(mono_font)
                .text_xs()
                .text_color(Color::Muted.get(cx)),
            )
            .when(!schedule.args.is_null(), |view| {
                view.child(
                    crate::text_reading::selectable_text(
                        SharedString::from(format!("schedule-args-{}", schedule.id)),
                        format!("{}: {}", tr("参数"), json(&schedule.args)),
                    )
                    .text_xs()
                    .text_color(Color::Secondary.get(cx)),
                )
            })
            .when(
                self.workflows.deleting.as_ref() == Some(&schedule.id),
                |view| {
                    view.child(Reveal::new(
                        SharedString::from(format!("schedule-confirm-{}", schedule.id)),
                        &self.workflows.list_scroll,
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .items_center()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .bg(Color::ErrorSurface.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(
                                div()
                                    .id(SharedString::from(format!(
                                        "schedule-confirm-description-{}",
                                        schedule.id
                                    )))
                                    .role(Role::Label)
                                    .aria_label(format!(
                                        "{}: {}",
                                        basename(&schedule.script_path),
                                        tr("删除该定时计划？运行历史会保留。")
                                    ))
                                    .flex_1()
                                    .text_xs()
                                    .text_color(Color::Error.get(cx))
                                    .child(tr("删除该定时计划？运行历史会保留。")),
                            )
                            .child(
                                Button::new("workflow-schedule-confirm")
                                    .danger()
                                    .compact()
                                    .xsmall()
                                    .rounded_md()
                                    .label(tr("确认删除计划"))
                                    .accessibility_label(format!(
                                        "{}: {}",
                                        tr("确认删除计划"),
                                        basename(&schedule.script_path)
                                    ))
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.workflow_action(
                                            "DELETE",
                                            format!("/api/workflow-schedules/{confirm_id}"),
                                            None,
                                            false,
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new("workflow-schedule-cancel")
                                    .when_some(
                                        self.workflows.delete_cancel_focus.as_ref(),
                                        |button, focus| button.track_focus(focus),
                                    )
                                    .ghost()
                                    .compact()
                                    .xsmall()
                                    .rounded_md()
                                    .label(tr("取消"))
                                    .accessibility_label(format!(
                                        "{}: {}",
                                        tr("取消删除计划"),
                                        basename(&schedule.script_path)
                                    ))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workflows.deleting = None;
                                        this.workflows.focus_navigation = true;
                                        cx.notify();
                                    })),
                            ),
                    ))
                },
            )
            .into_any_element()
    }
}
