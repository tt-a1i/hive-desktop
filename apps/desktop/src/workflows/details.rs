use super::types::*;
use crate::{app::HiveApp, palette::Color, focus_scroll::Reveal, language::{english,tr}};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
impl HiveApp {
    pub(super) fn render_workflow_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(run) = &self.workflows.detail else {
            return div()
                .text_sm()
                .child(if self.workflows.loading {
                    tr("正在读取运行详情…")
                } else {
                    tr("运行详情尚未读取，请刷新重试。")
                })
                .into_any_element();
        };
        let disabled = !self.connected || self.workflows.acting || !self.workspace_writes_allowed();
        let id = run.id.clone();
        let interrupted = matches!(run.status.as_str(), "stopped" | "interrupted");
        let result_heading = if run.error.is_some() {
            match run.status.as_str() {
                "stopped" => tr("停止原因"),
                "interrupted" => tr("中断原因"),
                _ => tr("运行错误"),
            }
        } else { tr("运行结果") };
        let result = run.error.clone().unwrap_or_else(|| json(&run.result));
        let mut metadata = format!("{}: {}\n{}: {}", tr("来源"), run.script_path,
            tr("源校验"), run.script_hash.as_deref().unwrap_or(tr("旧记录未提供")));
        if !run.args.is_null() {
            metadata.push_str(&format!("\n{}: {}", tr("参数"), json(&run.args)));
        }
        let log_text = self
            .workflows
            .logs
            .iter()
            .map(|log| log.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        v_flex()
            .gap_4()
            .child(
                Reveal::new("workflow-detail-header-reveal", &self.workflows.detail_scroll, h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(run.name.clone()),
                    )
                    .child(div().text_sm().child(status(&run.status).to_owned()))
                    .when(run.status == "running", |row| {
                        row.child(
                            Button::new("workflow-stop")
                                .label(tr("停止运行"))
                                .disabled(disabled)
                                .accessibility_label(format!("{}: {}", tr("停止工作流"), run.name))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.workflow_action(
                                        "POST",
                                        format!("/api/workflows/runs/{id}/stop"),
                                        None,
                                        false,
                                        cx,
                                    )
                                })),
                        )
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Secondary.get(cx))
                    .child(if english() { format!("Started {} · {} agents", relative_time(run.started_at), run.agent_count) } else { format!("{} 开始 · {} 位执行成员", relative_time(run.started_at), run.agent_count) }),
            )
            .when_some(run.finished_at, |view, time| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(format!("{}: {}",tr("结束于"),relative_time(time))),
                )
            })
            .when_some(run.phase.clone(), |view, phase| {
                view.child(div().text_sm().child(format!("{}: {phase}",tr("当前阶段"))))
            })
            .when(run.error.is_some() || !run.result.is_null(), |view| {
                view.child(
                    v_flex()
                        .gap_2()
                        .p_3()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .child(
                            Reveal::new("workflow-result-actions-reveal", &self.workflows.detail_scroll, h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(result_heading),
                                )
                                .child(
                                    Button::new("workflow-copy-result")
                                        .ghost()
                                        .compact()
                                        .label(tr("复制"))
                                        .accessibility_label(tr("复制工作流结果或错误"))
                                        .on_click(cx.listener({
                                            let result = result.clone();
                                            move |_, _, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    result.clone(),
                                                ))
                                            }
                                        })),
                                )),
                        )
                        .child(
                            div()
                                .id("workflow-result-text")
                                .role(Role::Label)
                                .aria_label(format!("{result_heading}: {result}"))
                                .text_sm()
                                .when(run.error.is_some() && !interrupted, |view| {
                                    view.text_color(Color::Error.get(cx))
                                })
                                .child(result),
                        ),
                )
            })
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{} · {}",tr("关联派单"),self.workflows.dispatches.len())),
            )
            .when(self.workflows.dispatches.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Secondary.get(cx))
                        .child(tr("此运行还没有成员派单。")),
                )
            })
            .children(
                self.workflows
                    .dispatches
                    .iter()
                    .map(|dispatch| self.render_workflow_dispatch(dispatch, cx)),
            )
            .child(
                Reveal::new("workflow-log-actions-reveal", &self.workflows.detail_scroll, h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{} · {}",tr("运行日志"),self.workflows.logs.len())),
                    )
                    .child(
                        Button::new("workflow-copy-logs")
                            .ghost()
                            .compact()
                            .label(tr("复制日志"))
                            .disabled(log_text.is_empty())
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(log_text.clone()))
                            })),
                    )),
            )
            .when(self.workflows.logs.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Secondary.get(cx))
                        .child(tr("还没有运行日志。")),
                )
            })
            .children(self.workflows.logs.iter().map(|log| {
                v_flex()
                    .id(("workflow-log", log.id as usize))
                    .gap_1()
                    .p_3()
                    .rounded_lg()
                    .bg(Color::Inset.get(cx))
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .child(relative_time(log.ts)),
                    )
                    .child(div().text_sm().child(log.message.clone()))
            }))
            .child(v_flex().min_w_0().gap_2().pt_3().border_t_1()
                .border_color(Color::Divider.get(cx))
                .child(Reveal::new("workflow-metadata-actions-reveal", &self.workflows.detail_scroll, h_flex().min_w_0().gap_2().items_center()
                    .child(div().flex_1().min_w_0().text_sm().font_weight(FontWeight::SEMIBOLD)
                        .child(tr("来源与参数")))
                    .child(Button::new("workflow-copy-metadata").ghost().compact()
                        .label(tr("复制")).accessibility_label(tr("复制来源与参数"))
                        .on_click(cx.listener({let metadata = metadata.clone(); move |_, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(metadata.clone()))
                        }})))))
                .child(div().id("workflow-source-metadata").role(Role::Label)
                    .aria_label(metadata.clone()).min_w_0().whitespace_normal().text_xs()
                    .text_color(Color::Secondary.get(cx))
                    .child(gpui_kit::base::SelectableText::new("workflow-source-text", metadata))))
            .into_any_element()
    }
    fn render_workflow_dispatch(&self, dispatch: &Dispatch, cx: &mut Context<Self>) -> AnyElement {
        let item = crate::collaboration::Dispatch {
            id: dispatch.id.clone(),
            state: dispatch.status.clone(),
            text: dispatch.text.clone(),
            to_agent_id: dispatch.to_agent_id.clone(),
            from_agent_id: dispatch.from_agent_id.clone(),
            report_text: dispatch.report_text.clone(),
            artifacts: dispatch.artifacts.clone(),
        };
        let session = self
            .sessions
            .iter()
            .find(|session| session.agent_id.as_ref() == Some(&dispatch.to_agent_id))
            .map(|session| session.id.clone());
        v_flex()
            .gap_2()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(dispatch.label.clone().unwrap_or_else(|| {
                                format!("{} {}",tr("步骤"),dispatch.step_index.unwrap_or_default())
                            })),
                    )
                    .child(
                        div().text_xs().child(
                            crate::collaboration::dispatch_state(&dispatch.status).to_owned(),
                        ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Secondary.get(cx))
                    .child(self.agent_name(&dispatch.to_agent_id)),
            )
            .when_some(dispatch.phase.clone(), |view, phase| {
                view.child(div().text_xs().child(format!("{}: {phase}",tr("阶段"))))
            })
            .child(div().text_sm().child(dispatch.text.clone()))
            .when_some(dispatch.report_text.clone(), |view, text| {
                let label = if dispatch.status == "cancelled" { tr("取消原因") } else { tr("汇报") };
                view.child(div().id(SharedString::from(format!("workflow-dispatch-outcome-{}", dispatch.id)))
                    .role(Role::Label).aria_label(format!("{label}: {text}"))
                    .text_sm().child(format!("{label}: {text}")))
            })
            .when_some(dispatch.last_pty_line.clone(), |view, text| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(format!("{}: {text}",tr("终端"))),
                )
            })
            .children(
                dispatch
                    .artifacts
                    .iter()
                    .enumerate()
                    .map(|(index, path)| Reveal::new(SharedString::from(format!("workflow-artifact-reveal-{}-{index}",dispatch.id)), &self.workflows.detail_scroll, self.artifact_link(&dispatch.id, index, path, cx))),
            )
            .child(
                Reveal::new(SharedString::from(format!("workflow-dispatch-actions-reveal-{}",dispatch.id)), &self.workflows.detail_scroll, h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new(SharedString::from(format!(
                            "workflow-dispatch-{}",
                            dispatch.id
                        )))
                        .ghost()
                        .compact()
                        .label(tr("派单详情"))
                        .accessibility_label(format!(
                            "{}: {}", tr("查看工作流派单"),
                            dispatch
                                .label
                                .clone()
                                .unwrap_or_else(|| dispatch.id.clone())
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_collaboration(cx);
                            this.select_dispatch(item.clone(), cx);
                            this.collaboration.workflow_origin = true;
                        })),
                    )
                    .when_some(session, |row, session| {
                        row.child(
                            Button::new(SharedString::from(format!(
                                "workflow-chat-{}",
                                dispatch.id
                            )))
                            .ghost()
                            .compact()
                            .label(tr("成员会话"))
                            .on_click(cx.listener(
                                move |this, _, window, cx| this.open_member_session(session.clone(), window, cx),
                            )),
                        )
                    })),
            )
            .into_any_element()
    }
}
