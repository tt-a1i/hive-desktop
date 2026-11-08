use super::types::*;
use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::json;
impl HiveApp {
    pub(crate) fn render_workflows(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.workflows;
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
        v_flex().flex_1().min_w_0().min_h_0().h_full().gap_3().p_5()
            .child(h_flex().flex_shrink_0().gap_2().flex_wrap().items_center()
                .when(state.selected.is_some(),|row|row.child(Button::new("workflow-back").ghost().icon(Glyph::ArrowLeft).label("返回运行列表")
                    .on_click(cx.listener(|this,_,_,cx|this.select_workflow(None,cx)))))
                .child(div().flex_1().min_w_0().text_xl().font_weight(FontWeight::SEMIBOLD).child("工作流"))
                .child(Button::new("workflow-refresh").ghost().icon(Glyph::RefreshCw).label(if state.loading {"更新中…"}else{"刷新"})
                    .disabled(disabled).on_click(cx.listener(|this,_,_,cx|this.refresh_workflows(cx)))))
            .when_some(state.action_error.clone(),|view,error|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)))
            .when_some(state.error.clone(),|view,error|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)))
            .child(div().id(SharedString::from(state.selected.as_ref().map(|id|format!("workflow-scroll-{id}")).unwrap_or_else(||"workflows-list-scroll".into())))
                .flex_1().min_h_0().min_w_0().overflow_y_scroll()
                .child(v_flex().w_full().flex_shrink_0().gap_4()
                    .child(self.render_workflow_settings(cx))
                    .when(state.enabled==Some(false),|view|view.child(div().text_sm().text_color(Color::Secondary.get(cx)).child("实验功能当前关闭；已有运行和计划仍可查看。")))
                    .when(state.selected.is_some(),|view|view.child(self.render_workflow_detail(cx)))
                    .when(state.selected.is_none(),|view|view
                        .child(h_flex().gap_2().flex_wrap().children([("all","全部"),("running","运行中"),("failed","需关注")].into_iter().map(|(filter,label)|Button::new(SharedString::from(format!("workflow-filter-{filter}"))).ghost().label(label).selected(state.filter==filter)
                            .on_click(cx.listener(move|this,_,_,cx|{this.workflows.filter=filter;cx.notify();})))))
                        .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("显示 {count} / {} 条运行",state.runs.len())))
                        .when(state.runs.is_empty() && !state.loading,|view|view.child(v_flex().gap_2().py_4()
                            .child(div().text_sm().child("还没有工作流运行"))
                            .child(div().text_xs().text_color(Color::Secondary.get(cx)).child("向主控描述目标，由主控编写和启动工作流。这里会显示真实运行、日志与派单。"))))
                        .when(!state.runs.is_empty() && count==0,|view|view.child(div().text_sm().child("没有符合筛选的运行。")))
                        .children(state.runs.iter().filter(|run|match state.filter {"running"=>run.status=="running","failed"=>matches!(run.status.as_str(),"failed"|"stopped"|"interrupted"),_=>true}).map(|run| {
                            let id=run.id.clone();let parent=run.parent_run_id.as_ref().map(|id|state.runs.iter().find(|run|&run.id==id).map(|run|run.name.clone()).unwrap_or_else(||id.clone()));
                            Button::new(SharedString::from(format!("workflow-run-{}",run.id))).ghost().w_full().h_auto().justify_start().p_4().rounded_lg()
                                .accessibility_label(format!("查看工作流 {}：{}",run.name,status(&run.status)))
                                .child(v_flex().w_full().min_w_0().items_start().text_left().gap_2()
                                    .child(h_flex().w_full().min_w_0().gap_2().flex_wrap()
                                        .child(div().flex_1().min_w_0().text_sm().font_weight(FontWeight::SEMIBOLD).child(run.name.clone()))
                                        .child(div().text_xs().child(status(&run.status).to_owned())))
                                    .when_some(parent,|view,parent|view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("子流程 · 父运行：{parent}"))))
                                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} · {} 位执行成员 · {}开始",basename(&run.script_path),run.agent_count,relative_time(run.started_at))))
                                    .when_some(run.phase.clone(),|view,phase|view.child(div().text_sm().child(format!("阶段：{phase}"))))
                                    .when_some(run.error.clone(),|view,error|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error))))
                                .on_click(cx.listener(move|this,_,_,cx|this.select_workflow(Some(id.clone()),cx)))
                        }))
                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("定时计划"))
                        .when(state.schedules.is_empty() && !state.loading,|view|view.child(div().text_sm().text_color(Color::Secondary.get(cx)).child("还没有定时计划，由主控创建后在此管理。")))
                        .children(state.schedules.iter().map(|schedule|self.render_workflow_schedule(schedule,disabled,cx)))
                    )))
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
        v_flex()
            .gap_2()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(basename(&schedule.script_path).to_owned()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .child(if enabled { "已启用" } else { "已暂停" }),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "workflow-schedule-toggle-{}",
                            schedule.id
                        )))
                        .ghost()
                        .label(if enabled {
                            "暂停计划"
                        } else {
                            "恢复计划"
                        })
                        .accessibility_label(format!(
                            "{}计划 {}",
                            if enabled { "暂停" } else { "恢复" },
                            basename(&schedule.script_path)
                        ))
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.workflow_action(
                                "PATCH",
                                format!("/api/workflow-schedules/{toggle_id}"),
                                Some(json!({"enabled":!enabled})),
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
                        .ghost()
                        .label("删除…")
                        .disabled(disabled)
                        .accessibility_label(format!(
                            "删除计划 {}",
                            basename(&schedule.script_path)
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.workflows.deleting = Some(delete_id.clone());
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Secondary.get(cx))
                    .child(format!(
                        "Cron：{} · {}",
                        schedule.cron,
                        if enabled {
                            format!("下次 {}", relative_time(schedule.next_run_at))
                        } else {
                            "暂停期间不触发".into()
                        }
                    )),
            )
            .when_some(schedule.last_run_at, |view, time| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(format!("上次运行 {}", relative_time(time))),
                )
            })
            .child(div().text_xs().child(schedule.script_path.clone()))
            .when(!schedule.args.is_null(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .child(format!("参数：{}", json(&schedule.args))),
                )
            })
            .when(
                self.workflows.deleting.as_ref() == Some(&schedule.id),
                |view| {
                    view.child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .items_center()
                            .child(div().text_sm().child("删除该定时计划？运行历史会保留。"))
                            .child(
                                Button::new("workflow-schedule-confirm")
                                    .label("确认删除计划")
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
                                    .ghost()
                                    .label("取消")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workflows.deleting = None;
                                        cx.notify();
                                    })),
                            ),
                    )
                },
            )
            .into_any_element()
    }
}
