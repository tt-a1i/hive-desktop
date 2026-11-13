use crate::{app::HiveApp, focus_scroll::Reveal, language::tr, palette::Color};
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
    pub(super) fn render_workflow_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.workflows;
        let disabled = !self.connected || state.acting || !self.workspace_writes_allowed();
        v_flex()
            .gap_3()
            .p_4()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(Reveal::new(
                "workflow-feature-reveal",
                &state.list_scroll,
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(tr("工作流 · 实验功能")),
                    )
                    .when_some(state.enabled, |row, enabled| {
                        row.child(
                            Button::new("workflow-feature-toggle")
                                .when(enabled, |b| b.outline())
                                .when(!enabled, |b| b.primary())
                                .compact()
                                .xsmall()
                                .h(px(26.))
                                .rounded_md()
                                .label(if enabled {
                                    tr("关闭工作流")
                                } else {
                                    tr("开启工作流")
                                })
                                .toggled(enabled)
                                .disabled(disabled)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.workflow_action(
                                        "PUT",
                                        "/api/settings/workflow-feature".into(),
                                        Some(json!({"enabled": !enabled})),
                                        false,
                                        cx,
                                    )
                                })),
                        )
                    }),
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Secondary.get(cx))
                    .child(tr(
                        "由主控编写和启动，沿用 Hive 的实验开关；运行中的任务请使用“停止运行”。",
                    )),
            )
            .child(Reveal::new(
                "workflow-settings-toggle-reveal",
                &state.list_scroll,
                h_flex().child(
                    Button::new("workflow-settings-toggle")
                        .outline()
                        .compact()
                        .xsmall()
                        .h(px(26.))
                        .rounded_md()
                        .icon(if state.settings_open {
                            Glyph::ChevronDown
                        } else {
                            Glyph::ChevronRight
                        })
                        .label(if state.settings_open {
                            tr("收起执行器设置")
                        } else {
                            tr("执行器设置…")
                        })
                        .toggled(state.settings_open)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.workflows.settings_open = !this.workflows.settings_open;
                            cx.notify();
                        })),
                ),
            ))
            .when(state.settings_open, |view| {
                view.when_some(state.policy.as_ref(), |view, policy| {
                    view.child(
                        v_flex()
                            .gap_2p5()
                            .p_3()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr(
                                        "工作流未指定 CLI 时使用默认项；允许列表沿用 Hive，实际接入能力以各 Agent 为准。",
                                    )),
                            )
                            .child(
                                v_flex()
                                    .rounded_lg()
                                    .bg(Color::Card.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .overflow_hidden()
                                    .children(policy.supported.iter().enumerate().map(
                                        |(index, cli)| {
                                            let allowed = policy.allowed.contains(cli);
                                            let selected = policy.default == *cli;
                                            let toggle = cli.clone();
                                            let pick = cli.clone();
                                            Reveal::new(
                                                ("workflow-policy-row-reveal", index),
                                                &state.list_scroll,
                                                h_flex()
                                                    .when(index > 0, |row| {
                                                        row.border_t_1()
                                                            .border_color(Color::Divider.get(cx))
                                                    })
                                                    .px_3()
                                                    .py_2()
                                                    .gap_2()
                                                    .items_center()
                                                    .justify_between()
                                                    .flex_wrap()
                                                    .child(
                                                        Button::new(("workflow-cli-allowed", index))
                                                            .ghost()
                                                            .compact()
                                                            .xsmall()
                                                            .h(px(26.))
                                                            .rounded_md()
                                                            .label(cli.clone())
                                                            .toggled(allowed)
                                                            .selected(allowed)
                                                            .accessibility_label(format!(
                                                                "{}: {cli}",
                                                                tr("允许工作流使用")
                                                            ))
                                                            .disabled(disabled)
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    if let Some(policy) =
                                                                        this.workflows.policy.as_mut()
                                                                    {
                                                                        if policy
                                                                            .allowed
                                                                            .contains(&toggle)
                                                                        {
                                                                            policy.allowed.retain(
                                                                                |item| item != &toggle,
                                                                            );
                                                                            if policy.default
                                                                                == toggle
                                                                            {
                                                                                policy.default =
                                                                                    policy
                                                                                        .allowed
                                                                                        .first()
                                                                                        .cloned()
                                                                                        .unwrap_or_default();
                                                                            }
                                                                        } else {
                                                                            policy.allowed.push(
                                                                                toggle.clone(),
                                                                            );
                                                                            if policy
                                                                                .default
                                                                                .is_empty()
                                                                            {
                                                                                policy.default =
                                                                                    toggle.clone();
                                                                            }
                                                                        }
                                                                        this.workflows.policy_dirty =
                                                                            true;
                                                                    }
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        Button::new(("workflow-cli-default", index))
                                                            .when(selected, |b| b.primary())
                                                            .when(!selected, |b| b.outline())
                                                            .compact()
                                                            .xsmall()
                                                            .h(px(26.))
                                                            .rounded_md()
                                                            .label(if selected {
                                                                tr("默认")
                                                            } else {
                                                                tr("设为默认")
                                                            })
                                                            .selected(selected)
                                                            .accessibility_label(format!(
                                                                "{}: {cli}",
                                                                tr("工作流默认执行器")
                                                            ))
                                                            .disabled(disabled || selected)
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    if let Some(policy) =
                                                                        this.workflows.policy.as_mut()
                                                                    {
                                                                        policy.default = pick.clone();
                                                                        if !policy
                                                                            .allowed
                                                                            .contains(&pick)
                                                                        {
                                                                            policy
                                                                                .allowed
                                                                                .push(pick.clone());
                                                                        }
                                                                        this.workflows.policy_dirty =
                                                                            true;
                                                                    }
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    ),
                                            )
                                        },
                                    )),
                            )
                            .child(Reveal::new(
                                "workflow-policy-save-reveal",
                                &state.list_scroll,
                                h_flex()
                                    .gap_2()
                                    .flex_wrap()
                                    .pt_0p5()
                                    .child(
                                        Button::new("workflow-policy-save")
                                            .primary()
                                            .small()
                                            .rounded_lg()
                                            .label(tr("保存执行器设置"))
                                            .disabled(
                                                disabled
                                                    || !state.policy_dirty
                                                    || policy.allowed.is_empty()
                                                    || !policy.allowed.contains(&policy.default),
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Some(policy) = &this.workflows.policy {
                                                    let body = json!({
                                                        "default": policy.default,
                                                        "allowed": policy.allowed
                                                    });
                                                    this.workflow_action(
                                                        "PUT",
                                                        "/api/settings/workflow-cli-policy".into(),
                                                        Some(body),
                                                        true,
                                                        cx,
                                                    );
                                                }
                                            })),
                                    )
                                    .child(
                                        Button::new("workflow-policy-reload")
                                            .ghost()
                                            .small()
                                            .rounded_lg()
                                            .label(tr("放弃修改并重新读取"))
                                            .disabled(
                                                disabled || state.loading || !state.policy_dirty,
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.workflows.policy_dirty = false;
                                                this.workflows.policy = None;
                                                this.refresh_workflows(cx);
                                                cx.notify();
                                            })),
                                    ),
                            ))
                            .when(state.policy_dirty, |view| {
                                view.child(
                                    div()
                                        .text_xs()
                                        .text_color(Color::Secondary.get(cx))
                                        .child(tr("执行器设置尚未保存")),
                                )
                            }),
                    )
                })
            })
            .into_any_element()
    }
}
