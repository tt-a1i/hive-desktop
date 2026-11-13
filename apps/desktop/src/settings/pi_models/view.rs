use super::*;
use crate::focus_scroll::Reveal;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        scroll::ScrollableElement,
        switch::Switch,
        *,
    },
    prelude::FluentBuilder,
};

impl HiveApp {
    pub fn render_pi_models(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.pi_models;
        let disabled = !self.connected || state.loading;
        let scroll = state.scroll.clone();
        let query = state.search.read(cx).value().trim().to_lowercase();
        let matching = state.snapshot.iter().flat_map(|snapshot| &snapshot.providers).filter_map(|provider| {
            let provider_match = [&provider.id, provider.name.as_deref().unwrap_or_default(), provider.base_url.as_deref().unwrap_or_default()]
                .iter().any(|text| text.to_lowercase().contains(&query));
            let models = provider.models.iter().filter(|model| provider_match || format!("{}/{} {}", provider.id, model.id, model.name.as_deref().unwrap_or_default()).to_lowercase().contains(&query)).collect::<Vec<_>>();
            (provider_match || !models.is_empty()).then_some((provider, models))
        }).collect::<Vec<_>>();
        let model_count: usize = matching.iter().map(|(_, models)| models.len()).sum();
        let count = if english() {
            format!("{} {} · {model_count} {}", matching.len(), if matching.len() == 1 { "provider" } else { "providers" }, if model_count == 1 { "model" } else { "models" })
        } else { format!("{} 个提供商 · {model_count} 个模型", matching.len()) };
        let search = v_flex()
            .flex_shrink_0()
            .min_w_0()
            .gap_2()
            .child(
                Input::new(&state.search)
                    .aria_label(tr("搜索模型配置"))
                    .disabled(state.snapshot.is_none()),
            )
            .when(state.snapshot.is_some(), |view| {
                view.child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            crate::text_reading::selectable_text("pi-model-search-count", count)
                                .text_xs()
                                .text_color(Color::Secondary.get(cx)),
                        )
                        .when(!query.is_empty(), |row| {
                            row.child(
                                Button::new("clear-pi-model-search")
                                    .ghost()
                                    .small()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_lg()
                                    .label(tr("清除搜索"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.pi_models.search.update(cx, |input, cx| {
                                            input.set_value("", window, cx);
                                            input.focus_handle(cx).focus(window, cx);
                                        });
                                    })),
                            )
                        }),
                )
            });
        let body = v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .id("pi-model-config-scroll")
            .overflow_y_scroll()
            .track_scroll(&state.scroll)
            .vertical_scrollbar(&state.scroll)
            .pr_1()
            .gap_4()
            .when(query.is_empty() || state.target.is_some(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Secondary.get(cx))
                        .child(tr("管理自定义接口、模型和内置配置覆盖。保存后在“内置 Pi”设置 API Key；免认证服务可使用占位值。")),
                )
            })
            .child(Reveal::new(
                "pi-model-list-actions-reveal",
                &scroll,
                h_flex()
                    .flex_shrink_0()
                    .gap_2()
                    .items_center()
                    .flex_wrap()
                    .child(
                        Button::new("add-pi-provider")
                            .primary()
                            .h(px(30.))
                            .px_3p5()
                            .rounded_lg()
                            .label(tr("添加提供商"))
                            .disabled(disabled || state.snapshot.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit_pi_model_config(
                                    Some(Target {
                                        provider: String::new(),
                                        model: None,
                                        is_model: false,
                                        creating: true,
                                    }),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("reload-pi-model-config")
                            .outline()
                            .small()
                            .h(px(30.))
                            .px_3()
                            .rounded_lg()
                            .label(if state.loading {
                                tr("处理中…")
                            } else {
                                tr("重新读取列表")
                            })
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| this.pi_models_request(None, cx))),
                    )
                    .when_some(
                        state
                            .snapshot
                            .as_ref()
                            .map(|s| std::path::PathBuf::from(&s.path))
                            .or_else(|| self.pi_agent_dir.clone()),
                        |row, path| {
                            row.child(
                                Button::new("locate-pi-model-config")
                                    .ghost()
                                    .small()
                                    .h(px(30.))
                                    .px_3()
                                    .rounded_lg()
                                    .label(tr("定位配置文件"))
                                    .on_click(move |_, _, cx| cx.reveal_path(&path)),
                            )
                        },
                    ),
            ))
            .when_some(
                state.message.as_ref().filter(|_| state.target.is_none()),
                |view, notice| {
                    view.child(super::super::settings_notice_box(
                        "pi-model-list-notice",
                        notice.text(),
                        notice.is_error(),
                        cx,
                    ))
                },
            )
            .when_some(state.target.clone(), |view, target| {
                view.child(self.render_pi_model_form(target, cx))
            })
            .when(
                state
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.providers.is_empty())
                    && state.target.is_none(),
                |view| {
                    view.child(
                        super::super::settings_card(cx).p_6().child(
                            div()
                                .text_sm()
                                .text_color(Color::Secondary.get(cx))
                                .child(tr("尚未添加自定义配置。内置模型仍可在会话的模型选择器中使用。")),
                        ),
                    )
                },
            )
            .when(
                state.target.is_none()
                    && state.snapshot.is_some()
                    && !query.is_empty()
                    && matching.is_empty(),
                |view| {
                    view.child(
                        super::super::settings_card(cx).p_6().child(
                            crate::text_reading::selectable_text(
                                "pi-model-search-empty",
                                tr("没有匹配的模型配置。试试提供商、模型名称、ID 或接口地址。"),
                            )
                            .text_sm()
                            .text_color(Color::Secondary.get(cx)),
                        ),
                    )
                },
            )
            .children(
                matching
                    .into_iter()
                    .filter(|_| state.target.is_none())
                    .map(|(provider, models)| {
                        let id = provider.id.clone();
                        let remove = id.clone();
                        let add = id.clone();
                        super::super::settings_card(cx)
                            .child(
                                super::super::settings_row(false, cx)
                                    .bg(Color::Inset.get(cx))
                                    .child(Reveal::new(
                                        format!("pi-provider-{}-actions-reveal", provider.id),
                                        &scroll,
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .items_center()
                                            .justify_between()
                                            .gap_3()
                                            .flex_wrap()
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(Color::Text.get(cx))
                                                            .child(format!(
                                                                "{} · {}",
                                                                provider
                                                                    .name
                                                                    .as_deref()
                                                                    .unwrap_or(&provider.id),
                                                                provider.id
                                                            )),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Color::Secondary.get(cx))
                                                            .child(
                                                                provider.base_url.clone().unwrap_or_else(
                                                                    || tr("使用内置接口地址").into(),
                                                                ),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .flex_shrink_0()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child(
                                                        Button::new(SharedString::from(format!(
                                                            "edit-pi-provider-{id}"
                                                        )))
                                                        .outline()
                                                        .small()
                                                        .h(px(28.))
                                                        .px_2p5()
                                                        .rounded_lg()
                                                        .label(tr("编辑接口"))
                                                        .accessibility_label(format!(
                                                            "{}: {}",
                                                            tr("编辑接口"),
                                                            provider.id
                                                        ))
                                                        .disabled(disabled)
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.edit_pi_model_config(
                                                                    Some(Target {
                                                                        provider: id.clone(),
                                                                        model: None,
                                                                        is_model: false,
                                                                        creating: false,
                                                                    }),
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                    )
                                                    .child(
                                                        Button::new(SharedString::from(format!(
                                                            "remove-pi-provider-{remove}"
                                                        )))
                                                        .ghost()
                                                        .small()
                                                        .h(px(28.))
                                                        .px_2p5()
                                                        .rounded_lg()
                                                        .label(tr("移除"))
                                                        .accessibility_label(format!(
                                                            "{}: {}",
                                                            tr("移除接口配置"),
                                                            provider.id
                                                        ))
                                                        .disabled(disabled || state.target.is_some())
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.remove_pi_model_config(
                                                                    remove.clone(),
                                                                    None,
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                    ),
                                            ),
                                    )),
                            )
                            .children(models.into_iter().map(|model| {
                                let provider_id = provider.id.clone();
                                let model_id = model.id.clone();
                                let remove_provider = provider.id.clone();
                                let remove_model = model.id.clone();
                                super::super::settings_row(true, cx).child(Reveal::new(
                                    format!(
                                        "pi-provider-{}-model-{}-reveal",
                                        provider.id, model.id
                                    ),
                                    &scroll,
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .gap_3()
                                        .items_center()
                                        .flex_wrap()
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .gap_1()
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .flex_wrap()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(Color::Text.get(cx))
                                                                .child(
                                                                    model
                                                                        .name
                                                                        .clone()
                                                                        .unwrap_or(model.id.clone()),
                                                                ),
                                                        )
                                                        .when(model.override_only, |row| {
                                                            row.child(
                                                                div()
                                                                    .px_2()
                                                                    .py(px(1.))
                                                                    .rounded_full()
                                                                    .bg(Color::AccentSoft.get(cx))
                                                                    .text_xs()
                                                                    .text_color(Color::Accent.get(cx))
                                                                    .child(tr("模型覆盖")),
                                                            )
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(model.id.clone()),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .flex_shrink_0()
                                                .items_center()
                                                .gap_1p5()
                                                .child(
                                                    Button::new(SharedString::from(format!(
                                                        "edit-pi-model-{provider_id}-{model_id}"
                                                    )))
                                                    .outline()
                                                    .small()
                                                    .h(px(28.))
                                                    .px_2p5()
                                                    .rounded_lg()
                                                    .label(tr("模型设置"))
                                                    .accessibility_label(format!(
                                                        "{}: {provider_id}/{model_id}",
                                                        tr("编辑模型")
                                                    ))
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.edit_pi_model_config(
                                                                Some(Target {
                                                                    provider: provider_id.clone(),
                                                                    model: Some(model_id.clone()),
                                                                    is_model: true,
                                                                    creating: false,
                                                                }),
                                                                window,
                                                                cx,
                                                            )
                                                        },
                                                    )),
                                                )
                                                .child(
                                                    Button::new(SharedString::from(format!(
                                                        "remove-pi-model-{remove_provider}-{remove_model}"
                                                    )))
                                                    .ghost()
                                                    .small()
                                                    .h(px(28.))
                                                    .px_2p5()
                                                    .rounded_lg()
                                                    .label(if model.override_only {
                                                        tr("移除覆盖")
                                                    } else {
                                                        tr("移除模型")
                                                    })
                                                    .accessibility_label(format!(
                                                        "{}: {remove_provider}/{remove_model}",
                                                        tr("移除模型配置")
                                                    ))
                                                    .disabled(disabled || state.target.is_some())
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.remove_pi_model_config(
                                                                remove_provider.clone(),
                                                                Some(remove_model.clone()),
                                                                window,
                                                                cx,
                                                            )
                                                        },
                                                    )),
                                                ),
                                        ),
                                ))
                            }))
                            .child(
                                super::super::settings_row(true, cx)
                                    .py_2p5()
                                    .child(Reveal::new(
                                        format!("pi-provider-{}-add-model-reveal", provider.id),
                                        &scroll,
                                        h_flex().child(
                                            Button::new(SharedString::from(format!(
                                                "add-pi-model-{add}"
                                            )))
                                            .outline()
                                            .small()
                                            .h(px(28.))
                                            .px_2p5()
                                            .rounded_lg()
                                            .label(tr("添加模型"))
                                            .accessibility_label(format!(
                                                "{}: {add}",
                                                tr("添加模型")
                                            ))
                                            .disabled(disabled)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.edit_pi_model_config(
                                                    Some(Target {
                                                        provider: add.clone(),
                                                        model: None,
                                                        is_model: true,
                                                        creating: true,
                                                    }),
                                                    window,
                                                    cx,
                                                )
                                            })),
                                        ),
                                    )),
                            )
                    }),
            );
        v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .gap_3()
            .when(state.target.is_none(), |view| view.child(search))
            .child(crate::focus_scroll::Viewport::new(
                "pi-model-viewport",
                &scroll,
                body,
            ))
            .when(state.target.is_some(), |view| {
                view.child(self.render_pi_model_actions(cx))
            })
    }
    fn render_pi_model_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.pi_models;
        let disabled = !self.connected || state.loading;
        v_flex()
            .flex_shrink_0()
            .gap_2p5()
            .pt_3()
            .border_t_1()
            .border_color(Color::Divider.get(cx))
            .when_some(state.message.as_ref(), |view, notice| {
                view.child(super::super::settings_notice_box(
                    "pi-model-form-notice",
                    notice.text(),
                    notice.is_error(),
                    cx,
                ))
            })
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .flex_wrap()
                    .child(
                        Button::new("save-pi-model-form")
                            .primary()
                            .h(px(30.))
                            .px_3p5()
                            .rounded_lg()
                            .label(tr("保存配置"))
                            .disabled(disabled || !state.dirty(cx))
                            .on_click(cx.listener(|this, _, _, cx| this.save_pi_model_form(cx))),
                    )
                    .child(
                        Button::new("cancel-pi-model-form")
                            .outline()
                            .h(px(30.))
                            .px_3p5()
                            .rounded_lg()
                            .label(tr("关闭表单"))
                            .disabled(state.loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit_pi_model_config(None, window, cx)
                            })),
                    )
                    .when(state.dirty(cx), |row| {
                        row.child(super::super::status_pill(
                            super::super::amber_dot(cx),
                            tr("有未保存的修改"),
                            cx,
                        ))
                    }),
            )
    }
    fn render_pi_model_form(&self, target: Target, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.pi_models;
        let disabled = !self.connected || state.loading;
        let label_color = Color::Secondary.get(cx);
        let field = |index, label: &'static str, input: &Entity<InputState>, locked: bool| {
            self.reveal_model_control(
                index,
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(label_color)
                            .child(label),
                    )
                    .child(Input::new(input).aria_label(label).disabled(disabled || locked)),
            )
        };
        super::super::settings_card(cx)
            .child(
                super::super::settings_row(false, cx)
                    .bg(Color::Inset.get(cx))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(if target.is_model {
                                format!(
                                    "{} · {}",
                                    target.provider,
                                    if target.creating {
                                        tr("添加模型")
                                    } else {
                                        tr("编辑模型")
                                    }
                                )
                            } else if target.creating {
                                tr("添加提供商").into()
                            } else {
                                format!("{} · {}", tr("编辑接口"), target.provider)
                            }),
                    ),
            )
            .child(
                v_flex()
                    .p_4()
                    .gap_3p5()
                    .child(field(
                        3,
                        if target.is_model {
                            tr("模型 ID")
                        } else {
                            tr("提供商 ID")
                        },
                        &state.id,
                        !target.creating,
                    ))
                    .child(field(4, tr("显示名称（可留空）"), &state.name, false))
                    .when(!target.is_model, |form| {
                        form.child(field(5, tr("接口地址（Base URL）"), &state.url, false))
                            .child(
                                v_flex()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Color::Secondary.get(cx))
                                            .child(tr("接口类型")),
                                    )
                                    .child(self.reveal_model_control(
                                        0,
                                        super::super::settings_segmented_bar(cx)
                                            .flex_wrap()
                                            .children(
                                                [
                                                    ("", tr("继承内置")),
                                                    ("openai-completions", "Chat Completions"),
                                                    ("openai-responses", "Responses"),
                                                    ("anthropic-messages", "Anthropic"),
                                                    ("google-generative-ai", "Gemini"),
                                                ]
                                                .into_iter()
                                                .map(|(api, label)| {
                                                    let active = state.api == api;
                                                    Button::new(SharedString::from(format!(
                                                        "pi-config-api-{api}"
                                                    )))
                                                    .when(active, |b| b.outline())
                                                    .when(!active, |b| b.ghost())
                                                    .small()
                                                    .h(px(26.))
                                                    .px_2p5()
                                                    .rounded_md()
                                                    .label(label)
                                                    .accessibility_label(format!(
                                                        "{}: {label}",
                                                        tr("模型接口类型")
                                                    ))
                                                    .selected(active)
                                                    .toggled(active)
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.pi_models.api = api.into();
                                                        this.schedule_draft_save(cx);
                                                        cx.notify();
                                                    }))
                                                }),
                                            ),
                                    )),
                            )
                            .when(!state.api.is_empty(), |form| {
                                form.child(
                                    div()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(state.api.clone()),
                                )
                            })
                            .when(target.creating, |form| {
                                form.child(field(
                                    6,
                                    tr("首个模型 ID（新接口必填，内置覆盖可留空）"),
                                    &state.first,
                                    false,
                                ))
                            })
                    })
                    .when(target.is_model, |form| {
                        form.when(state.override_only, |form| {
                            form.child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr("仅修改模型覆盖；继承内置表示不固定此能力。模型是否可用以会话原生目录为准。")),
                            )
                        })
                        .child(field(7, tr("上下文 Token 数（可留空）"), &state.context, false))
                        .child(field(8, tr("最大输出 Token 数（可留空）"), &state.output, false))
                        .when(!state.override_only, |form| {
                            form.child(
                                v_flex()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(
                                        div().px_3p5().py_2p5().child(self.reveal_model_control(
                                            1,
                                            h_flex()
                                                .gap_3()
                                                .items_center()
                                                .child(div().flex_1().text_sm().child(tr("支持思考")))
                                                .child(
                                                    Switch::new("pi-model-reasoning")
                                                        .accessibility_label(tr("模型支持思考"))
                                                        .checked(state.reasoning.unwrap_or(false))
                                                        .disabled(disabled)
                                                        .on_change(cx.listener(
                                                            |this, value, _, cx| {
                                                                this.pi_models.reasoning =
                                                                    Some(*value);
                                                                this.schedule_draft_save(cx);
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
                                        )),
                                    )
                                    .child(
                                        div()
                                            .px_3p5()
                                            .py_2p5()
                                            .border_t_1()
                                            .border_color(Color::Divider.get(cx))
                                            .child(self.reveal_model_control(
                                                2,
                                                h_flex()
                                                    .gap_3()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .text_sm()
                                                            .child(tr("支持图片输入")),
                                                    )
                                                    .child(
                                                        Switch::new("pi-model-vision")
                                                            .accessibility_label(tr(
                                                                "模型支持图片输入",
                                                            ))
                                                            .checked(state.vision.unwrap_or(false))
                                                            .disabled(disabled)
                                                            .on_change(cx.listener(
                                                                |this, value, _, cx| {
                                                                    this.pi_models.vision =
                                                                        Some(*value);
                                                                    this.schedule_draft_save(cx);
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    ),
                                            )),
                                    ),
                            )
                        })
                        .when(state.override_only, |form| {
                            form.child(
                                v_flex()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(
                                        div().px_3p5().py_2p5().child(self.reveal_model_control(
                                            1,
                                            self.override_capability(
                                                tr("思考能力"),
                                                true,
                                                state.reasoning,
                                                disabled,
                                                cx,
                                            ),
                                        )),
                                    )
                                    .child(
                                        div()
                                            .px_3p5()
                                            .py_2p5()
                                            .border_t_1()
                                            .border_color(Color::Divider.get(cx))
                                            .child(self.reveal_model_control(
                                                2,
                                                self.override_capability(
                                                    tr("图片输入"),
                                                    false,
                                                    state.vision,
                                                    disabled,
                                                    cx,
                                                ),
                                            )),
                                    ),
                            )
                        })
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("保存到 Pi 的 models.json，高级字段和其他提供商保持原样。仅保存配置，不验证服务端认证。")),
                    ),
            )
    }
    fn reveal_model_control(&self, index: usize, control: impl IntoElement) -> impl IntoElement {
        crate::focus_scroll::Reveal::new(
            format!("pi-model-control-{index}"),
            &self.pi_models.scroll,
            control,
        )
        .track_focus(&self.pi_models.control_focus[index])
    }

    fn override_capability(
        &self,
        label: &'static str,
        reasoning: bool,
        selected: Option<bool>,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .gap_3()
            .items_center()
            .justify_between()
            .flex_wrap()
            .child(div().flex_1().text_sm().child(label))
            .child(
                super::super::settings_segmented_bar(cx).children(
                    [
                        (None, tr("继承内置")),
                        (Some(true), tr("支持")),
                        (Some(false), tr("不支持")),
                    ]
                    .into_iter()
                    .map(|(value, text)| {
                        let active = selected == value;
                        Button::new(SharedString::from(format!(
                            "pi-override-{reasoning}-{}",
                            match value {
                                None => "inherit",
                                Some(true) => "enabled",
                                Some(false) => "disabled",
                            }
                        )))
                        .when(active, |b| b.outline())
                        .when(!active, |b| b.ghost())
                        .small()
                        .h(px(26.))
                        .px_2p5()
                        .rounded_md()
                        .label(text)
                        .accessibility_label(format!("{label}: {text}"))
                        .selected(active)
                        .toggled(active)
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if reasoning {
                                this.pi_models.reasoning = value;
                            } else {
                                this.pi_models.vision = value;
                            }
                            this.schedule_draft_save(cx);
                            cx.notify();
                        }))
                    }),
                ),
            )
    }
}
