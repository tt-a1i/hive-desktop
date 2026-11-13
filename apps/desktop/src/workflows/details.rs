use super::types::*;
use crate::{
    app::HiveApp,
    focus_scroll::Reveal,
    language::{english, tr},
    palette::{self, Color},
};
use gpui_kit::{
    assets::IconName as Glyph,
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
            return v_flex()
                .items_center()
                .justify_center()
                .py_10()
                .px_6()
                .rounded_xl()
                .bg(Color::Card.get(cx))
                .border_1()
                .border_color(Color::Divider.get(cx))
                .child(
                    crate::text_reading::selectable_text(
                        "workflow-detail-empty",
                        if self.workflows.loading {
                            tr("正在读取运行详情…")
                        } else {
                            tr("运行详情尚未读取，请刷新重试。")
                        },
                    )
                    .text_xs()
                    .text_color(Color::Muted.get(cx)),
                )
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
        } else {
            tr("运行结果")
        };
        let result = run.error.clone().unwrap_or_else(|| json(&run.result));
        let mut metadata = format!(
            "{}: {}\n{}: {}",
            tr("来源"),
            run.script_path,
            tr("源校验"),
            run.script_hash.as_deref().unwrap_or(tr("旧记录未提供"))
        );
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
        let (_run_badge_bg, run_badge_fg, run_badge_dot) = status_badge_colors(&run.status, cx);
        let run_dot_color = run_badge_dot.unwrap_or_else(|| match run.status.as_str() {
            "running" => Color::Accent.get(cx),
            "failed" => Color::Error.get(cx),
            _ => Color::Muted.get(cx),
        });
        let mono_font = Theme::global(cx).mono_font_family.clone();

        v_flex()
            .gap_4()
            .child(
                v_flex()
                    .gap_2()
                    .p_4()
                    .rounded_xl()
                    .bg(Color::Card.get(cx))
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .child(Reveal::new(
                        "workflow-detail-header-reveal",
                        &self.workflows.detail_scroll,
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .items_center()
                            .child(
                                div()
                                    .id("workflow-detail-name")
                                    .role(Role::Label)
                                    .aria_label(run.name.clone())
                                    .flex_1()
                                    .min_w_0()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(run.name.clone()),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .px_2()
                                    .py(px(2.))
                                    .rounded_md()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(
                                        div()
                                            .size(px(6.))
                                            .rounded_full()
                                            .bg(run_dot_color),
                                    )
                                    .child(
                                        crate::text_reading::selectable_text(
                                            "workflow-detail-status",
                                            status(&run.status),
                                        )
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(run_badge_fg),
                                    ),
                            )
                            .when(run.status == "running", |row| {
                                row.child(
                                    Button::new("workflow-stop")
                                        .danger()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .rounded_md()
                                        .label(tr("停止运行"))
                                        .disabled(disabled)
                                        .accessibility_label(format!(
                                            "{}: {}",
                                            tr("停止工作流"),
                                            run.name
                                        ))
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
                            }),
                    ))
                    .child(
                        crate::text_reading::selectable_text(
                            "workflow-detail-started",
                            if english() {
                                format!(
                                    "Started {} · {} agents",
                                    relative_time(run.started_at),
                                    run.agent_count
                                )
                            } else {
                                format!(
                                    "{} 开始 · {} 位执行成员",
                                    relative_time(run.started_at),
                                    run.agent_count
                                )
                            },
                        )
                        .text_xs()
                        .text_color(Color::Muted.get(cx)),
                    )
                    .when_some(run.finished_at, |view, time| {
                        view.child(
                            crate::text_reading::selectable_text(
                                "workflow-detail-finished",
                                format!("{}: {}", tr("结束于"), relative_time(time)),
                            )
                            .text_xs()
                            .text_color(Color::Muted.get(cx)),
                        )
                    })
                    .when_some(run.phase.clone(), |view, phase| {
                        view.child(
                            crate::text_reading::selectable_text(
                                "workflow-detail-phase",
                                format!("{}: {phase}", tr("当前阶段")),
                            )
                            .text_xs()
                            .text_color(Color::Secondary.get(cx)),
                        )
                    }),
            )
            .when(run.error.is_some() || !run.result.is_null(), |view| {
                view.child(
                    v_flex()
                        .gap_2()
                        .p_4()
                        .rounded_xl()
                        .bg(if run.error.is_some() && !interrupted {
                            Color::ErrorSurface.get(cx)
                        } else {
                            Color::Card.get(cx)
                        })
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .child(Reveal::new(
                            "workflow-result-actions-reveal",
                            &self.workflows.detail_scroll,
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Color::Text.get(cx))
                                        .child(result_heading),
                                )
                                .child(
                                    Button::new("workflow-copy-result")
                                        .custom(palette::subtle_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(24.))
                                        .w(px(24.))
                                        .rounded_md()
                                        .icon(Glyph::Copy)
                                        .tooltip(tr("复制"))
                                        .accessibility_label(tr("复制工作流结果或错误"))
                                        .on_click(cx.listener({
                                            let result = result.clone();
                                            move |_, _, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    result.clone(),
                                                ))
                                            }
                                        })),
                                ),
                        ))
                        .child(
                            div()
                                .id("workflow-result-text")
                                .role(Role::Label)
                                .aria_label(format!("{result_heading}: {result}"))
                                .p_3()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .text_xs()
                                .when(run.error.is_some() && !interrupted, |view| {
                                    view.text_color(Color::Error.get(cx))
                                })
                                .child(gpui_kit::base::SelectableText::new(
                                    "workflow-result-selection",
                                    result,
                                )),
                        ),
                )
            })
            .child(
                div()
                    .id("workflow-dispatch-count")
                    .role(Role::Label)
                    .aria_label(format!(
                        "{} · {}",
                        tr("关联派单"),
                        self.workflows.dispatches.len()
                    ))
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Color::Text.get(cx))
                    .child(format!(
                        "{} · {}",
                        tr("关联派单"),
                        self.workflows.dispatches.len()
                    )),
            )
            .when(self.workflows.dispatches.is_empty(), |view| {
                view.child(
                    v_flex()
                        .items_center()
                        .justify_center()
                        .py_6()
                        .px_4()
                        .rounded_xl()
                        .bg(Color::Card.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .child(
                            crate::text_reading::selectable_text(
                                "workflow-dispatch-empty",
                                tr("此运行还没有成员派单。"),
                            )
                            .text_xs()
                            .text_color(Color::Muted.get(cx)),
                        ),
                )
            })
            .when(!self.workflows.dispatches.is_empty(), |view| {
                view.child(
                    v_flex()
                        .rounded_xl()
                        .bg(Color::Card.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .overflow_hidden()
                        .children(self.workflows.dispatches.iter().enumerate().map(
                            |(idx, dispatch)| {
                                v_flex()
                                    .when(idx > 0, |row| {
                                        row.border_t_1().border_color(Color::Divider.get(cx))
                                    })
                                    .child(self.render_workflow_dispatch(dispatch, cx))
                            },
                        )),
                )
            })
            .child(Reveal::new(
                "workflow-log-actions-reveal",
                &self.workflows.detail_scroll,
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .id("workflow-log-count")
                            .role(Role::Label)
                            .aria_label(format!(
                                "{} · {}",
                                tr("运行日志"),
                                self.workflows.logs.len()
                            ))
                            .flex_1()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(format!(
                                "{} · {}",
                                tr("运行日志"),
                                self.workflows.logs.len()
                            )),
                    )
                    .child(
                        Button::new("workflow-copy-logs")
                            .outline()
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .rounded_md()
                            .icon(Glyph::Copy)
                            .label(tr("复制日志"))
                            .disabled(log_text.is_empty())
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(log_text.clone()))
                            })),
                    ),
            ))
            .when(self.workflows.logs.is_empty(), |view| {
                view.child(
                    v_flex()
                        .items_center()
                        .justify_center()
                        .py_6()
                        .px_4()
                        .rounded_xl()
                        .bg(Color::Card.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .child(
                            crate::text_reading::selectable_text(
                                "workflow-log-empty",
                                tr("还没有运行日志。"),
                            )
                            .text_xs()
                            .text_color(Color::Muted.get(cx)),
                        ),
                )
            })
            .when(!self.workflows.logs.is_empty(), |view| {
                view.child(
                    v_flex()
                        .p_3()
                        .gap_1()
                        .rounded_xl()
                        .bg(rgb(0x16171b))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .font_family(mono_font.clone())
                        .children(self.workflows.logs.iter().map(|log| {
                            h_flex()
                                .id(("workflow-log", log.id as usize))
                                .items_start()
                                .gap_2p5()
                                .py(px(1.))
                                .child(
                                    div().flex_shrink_0().child(
                                        crate::text_reading::selectable_text(
                                            SharedString::from(format!(
                                                "workflow-log-time-{}",
                                                log.id
                                            )),
                                            relative_time(log.ts),
                                        )
                                        .text_size(px(11.))
                                        .text_color(rgb(0x8b92a0)),
                                    ),
                                )
                                .child(
                                    div().flex_1().min_w_0().child(
                                        crate::text_reading::selectable_text(
                                            SharedString::from(format!(
                                                "workflow-log-message-{}",
                                                log.id
                                            )),
                                            log.message.clone(),
                                        )
                                        .text_size(px(12.))
                                        .text_color(rgb(0xe4e7ec)),
                                    ),
                                )
                        })),
                )
            })
            .child(
                v_flex()
                    .min_w_0()
                    .gap_2()
                    .p_4()
                    .rounded_xl()
                    .bg(Color::Card.get(cx))
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .child(Reveal::new(
                        "workflow-metadata-actions-reveal",
                        &self.workflows.detail_scroll,
                        h_flex()
                            .min_w_0()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("来源与参数")),
                            )
                            .child(
                                Button::new("workflow-copy-metadata")
                                    .custom(palette::subtle_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .w(px(24.))
                                    .rounded_md()
                                    .icon(Glyph::Copy)
                                    .tooltip(tr("复制"))
                                    .accessibility_label(tr("复制来源与参数"))
                                    .on_click(cx.listener({
                                        let metadata = metadata.clone();
                                        move |_, _, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                metadata.clone(),
                                            ))
                                        }
                                    })),
                            ),
                    ))
                    .child(
                        div()
                            .id("workflow-source-metadata")
                            .role(Role::Label)
                            .aria_label(metadata.clone())
                            .min_w_0()
                            .whitespace_normal()
                            .p_3()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .font_family(mono_font)
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .child(gpui_kit::base::SelectableText::new(
                                "workflow-source-text",
                                metadata,
                            )),
                    ),
            )
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
        let (_badge_bg, badge_fg, badge_dot) =
            crate::collaboration::dispatch_badge_colors(&dispatch.status, cx);
        let dot_color = badge_dot.unwrap_or_else(|| match dispatch.status.as_str() {
            "queued" | "submitted" => crate::chrome::attention_dot(cx),
            _ => Color::Muted.get(cx),
        });
        v_flex()
            .gap_2()
            .p_4()
            .child(
                h_flex()
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
                            .child(dispatch.label.clone().unwrap_or_else(|| {
                                format!("{} {}", tr("步骤"), dispatch.step_index.unwrap_or_default())
                            })),
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
                            .child(div().size(px(6.)).rounded_full().bg(dot_color))
                            .child(
                                crate::collaboration::dispatch_state(&dispatch.status).to_owned(),
                            ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(self.agent_name(&dispatch.to_agent_id)),
            )
            .when_some(dispatch.phase.clone(), |view, phase| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(format!("{}: {phase}", tr("阶段"))),
                )
            })
            .child(
                div()
                    .text_sm()
                    .text_color(Color::Text.get(cx))
                    .child(dispatch.text.clone()),
            )
            .when_some(dispatch.report_text.clone(), |view, text| {
                let label = if dispatch.status == "cancelled" {
                    tr("取消原因")
                } else {
                    tr("汇报")
                };
                view.child(
                    div()
                        .id(SharedString::from(format!(
                            "workflow-dispatch-outcome-{}",
                            dispatch.id
                        )))
                        .role(Role::Label)
                        .aria_label(format!("{label}: {text}"))
                        .p_2p5()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(format!("{label}: {text}")),
                )
            })
            .when_some(dispatch.last_pty_line.clone(), |view, text| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(format!("{}: {text}", tr("终端"))),
                )
            })
            .children(dispatch.artifacts.iter().enumerate().map(|(index, path)| {
                Reveal::new(
                    SharedString::from(format!(
                        "workflow-artifact-reveal-{}-{index}",
                        dispatch.id
                    )),
                    &self.workflows.detail_scroll,
                    self.artifact_link(&dispatch.id, index, path, cx),
                )
            }))
            .child(Reveal::new(
                SharedString::from(format!("workflow-dispatch-actions-reveal-{}", dispatch.id)),
                &self.workflows.detail_scroll,
                h_flex()
                    .gap_1p5()
                    .flex_wrap()
                    .pt_1()
                    .child(
                        Button::new(SharedString::from(format!(
                            "workflow-dispatch-{}",
                            dispatch.id
                        )))
                        .outline()
                        .compact()
                        .xsmall()
                        .h(px(26.))
                        .rounded_md()
                        .label(tr("派单详情"))
                        .accessibility_label(format!(
                            "{}: {}",
                            tr("查看工作流派单"),
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
                            .outline()
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .rounded_md()
                            .label(tr("成员会话"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_member_session(session.clone(), window, cx)
                            })),
                        )
                    }),
            ))
            .into_any_element()
    }
}
