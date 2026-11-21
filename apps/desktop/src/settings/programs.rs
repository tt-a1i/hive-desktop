use crate::{app::HiveApp, palette::Color, settings::EffectiveCommand, language::{english,tr}};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputState},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramResult {
    status: String,
    version: Option<String>,
    verified_version: Option<String>,
    command: String,
    source: String,
    message: String,
}
impl ProgramResult {
    fn message(&self) -> String {
        if let Some(version) = self.version.as_ref().filter(|version| self.status == "available" && self.message == format!("已检测到版本 {version}。")) {
            return if english() { format!("Detected version {version}.") } else { self.message.clone() };
        }
        [
            "检查已取消。",
            "程序没有在 5 秒内返回版本，已停止检查。",
            "程序输出过长，已停止检查。",
            "程序输入通道失败，已停止检查。",
            "找不到程序，请检查路径或安装对应 CLI。",
            "无法运行程序，请检查文件权限和路径。",
            "程序未能正常返回版本，请在终端检查该 CLI。",
            "程序已运行，但没有返回可识别的版本。",
        ].into_iter().find(|key| *key == self.message).map(|key| tr(key).to_owned()).unwrap_or_else(|| self.message.clone())
    }
}
pub struct ProgramCheck {
    scope: super::CheckScope,
    loading: bool,
    result: Option<Result<ProgramResult, String>>,
}
impl ProgramCheck {
    fn input_changed(&self, current: &Result<super::CheckScope, String>) -> bool {
        current.as_ref().ok() != Some(&self.scope)
    }
}
impl HiveApp {
    pub(super) fn render_program_save_feedback(&self, window: &Window, cx: &App) -> Option<AnyElement> {
        let notice = self.program_save_message.as_ref()?;
        let is_error = matches!(notice, super::SettingsNotice::Error(..));
        Some(div().px_5().pt_3().flex_shrink_0().min_w_0().child(
            super::settings_notice_box("programs-save-feedback", notice.text(), is_error, cx)
                .max_h((window.viewport_size().height * 0.2).min(px(120.)))
                .whitespace_normal().overflow_y_scroll()
        ).into_any_element())
    }

    pub(crate) fn open_program_setup(&mut self, backend: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.program_setup_focus = Some(backend);
        self.settings_section = crate::app::SettingsSection::Programs;
        self.open_settings(window, cx);
        self.finish_program_setup(window, cx);
    }

    pub(crate) fn finish_program_setup(&mut self, window: &mut Window, cx: &mut App) {
        if self.settings_loading || self.executable_settings.is_none() { return; }
        let Some(backend) = self.program_setup_focus.take() else { return; };
        if !self.settings_panel || self.settings_section != crate::app::SettingsSection::Programs { return; }
        let input = match backend { "codex" => &self.codex_command, "claude" => &self.claude_command, _ => &self.pi_command };
        input.update(cx, |input, cx| { input.focus(window, cx); input.select_all(window, cx); });
    }

    pub(crate) fn program_setup_status(&self, backend: &'static str, cx: &Context<Self>) -> String {
        let Some(check) = self.program_checks.get(backend) else { return tr("尚未检查程序版本").into(); };
        if check.loading { return tr("检查中…").into(); }
        if check.input_changed(&self.program_check_scope(backend, cx)) { return tr("配置或输入已更改，请重新检查。").into(); }
        let message = match check.result.as_ref() {
            Some(Ok(result)) => result.message(),
            Some(Err(error)) => crate::language::localize_owned_error(error),
            None => tr("尚未检查程序版本").into(),
        };
        format!("{}: {message}", check.scope.label())
    }

    pub(crate) fn program_local_check_state(&self, backend: &'static str, cx: &Context<Self>) -> Option<bool> {
        let check = self.program_checks.get(backend)?;
        if check.loading || check.input_changed(&self.program_check_scope(backend, cx)) {
            return None;
        }
        match check.result.as_ref()? {
            Ok(result) => Some(result.status == "available"),
            Err(_) => Some(false),
        }
    }

    pub(crate) fn render_program_settings(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let scroll = &self.program_scroll;
        let is_error = matches!(
            self.settings_message,
            Some(crate::settings::SettingsNotice::Error(..))
        );
        let changed = self.executable_paths_changed(cx);
        let entries = [
            (
                "pi",
                tr("外部 Pi"),
                &self.pi_command,
                self.executable_settings.as_ref().map(|s| &s.effective.pi),
            ),
            (
                "codex",
                "Codex",
                &self.codex_command,
                self.executable_settings.as_ref().map(|s| &s.effective.codex),
            ),
            (
                "claude",
                "Claude Code",
                &self.claude_command,
                self.executable_settings.as_ref().map(|s| &s.effective.claude),
            ),
        ];
        crate::focus_scroll::Viewport::new(
            "programs-viewport",
            scroll,
            v_flex()
                .id("settings-programs-content")
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(scroll)
                .vertical_scrollbar(scroll)
                .pr_1()
                .gap_4()
                .child(self.render_readiness_and_mcp_card(cx))
                .child(self.render_skills_and_rules_card(cx))
                .child(self.render_local_usage_summary_card(cx))
                .child(
                    v_flex()
                        .gap_1()
                        .px_1()
                        .child(
                            crate::text_reading::selectable_text(
                                "programs-description",
                                tr("路径留空时使用 PATH 查找，环境变量优先。可先检查当前输入，再保存；版本检查只读取程序版本。保存影响后续新建会话，已有会话保留固定配置。"),
                            )
                            .text_xs()
                            .text_color(Color::Secondary.get(cx)),
                        )
                        .child(
                            crate::text_reading::selectable_text(
                                "programs-account-guidance",
                                tr("Claude Code、Codex 和外部 Pi 使用各自 CLI 的登录与配置。检测到版本不代表已登录或模型可用；请先在对应 CLI 中完成设置。"),
                            )
                            .text_xs()
                            .text_color(Color::Muted.get(cx)),
                        ),
                )
                .when_some(
                    self.settings_message.as_ref().filter(|notice| {
                        // The modal already exposes this save result above the
                        // scroller. Keep other errors, and the standalone view,
                        // on their original inline feedback path.
                        !(self.preferences_modal_open && self.settings_panel && !self.roles.visible
                            && self.settings_section == crate::app::SettingsSection::Programs
                            && self.program_save_message.as_ref().is_some_and(|saved| saved.text() == notice.text()))
                    }).map(|notice| notice.text()),
                    |view, message| {
                        view.child(super::settings_notice_box(
                            "programs-save-notice",
                            message,
                            is_error,
                            cx,
                        ))
                    },
                )
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .items_center()
                        .gap_1p5()
                        .px_3()
                        .py_1p5()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .child(
                            Icon::new(gpui_kit::assets::IconName::Info)
                                .size(px(12.5))
                                .flex_shrink_0()
                                .text_color(Color::Secondary.get(cx)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(tr("解析优先级：环境变量 > 应用设置 > Agent 配置 > 系统 PATH")),
                        ),
                )
                .child(super::settings_card(cx).flex_shrink_0().children(entries.into_iter().enumerate().map(
                    |(index, (backend, label, input, effective))| {
                        div()
                            .w_full()
                            .min_w_0()
                            .when(index > 0, |row| {
                                row.border_t_1().border_color(Color::Divider.get(cx))
                            })
                            .child(self.executable_field(backend, label, input, effective, cx))
                    },
                )))
                .child(crate::focus_scroll::Reveal::new(
                    "programs-actions-reveal",
                    scroll,
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .flex_wrap()
                        .gap_3()
                        .child(
                            div().when(!self.settings_loading && changed, |slot| {
                                slot.child(
                                    h_flex()
                                        .items_center()
                                        .gap_1p5()
                                        .px_2p5()
                                        .py_1()
                                        .rounded_full()
                                        .bg(Color::Inset.get(cx))
                                        .child(
                                            div()
                                                .size(px(6.))
                                                .rounded_full()
                                                .bg(super::amber_dot(cx)),
                                        )
                                        .child(
                                            crate::text_reading::selectable_text(
                                                "programs-unsaved",
                                                tr("有未保存的修改"),
                                            )
                                            .text_xs()
                                            .text_color(Color::Secondary.get(cx)),
                                        ),
                                )
                            }),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("reload-executable-settings")
                                        .outline()
                                        .small()
                                        .h(px(30.))
                                        .px_3()
                                        .rounded_lg()
                                        .label(tr("重新读取"))
                                        .disabled(!self.connected || self.settings_loading)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.settings_request(false, window.window_handle(), cx)
                                        })),
                                )
                                .child(
                                    Button::new("save-executable-settings")
                                        .primary()
                                        .small()
                                        .h(px(30.))
                                        .px_3p5()
                                        .rounded_lg()
                                        .label(if self.settings_loading {
                                            tr("处理中…")
                                        } else {
                                            tr("保存设置")
                                        })
                                        .disabled(
                                            !self.connected || self.settings_loading || !changed,
                                        )
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.settings_request(true, window.window_handle(), cx)
                                        })),
                                ),
                        ),
                )),
        )
    }

    fn check_program(
        &mut self,
        backend: &'static str,
        _input: Entity<InputState>,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self
                .program_checks
                .get(backend)
                .is_some_and(|check| check.loading)
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let scope = match self.program_check_scope(backend, cx) {
            Ok(scope) => scope,
            Err(error) => { self.settings_message = Some(super::SettingsNotice::Error("读取配置失败", error)); cx.notify(); return; }
        };
        let mut params = scope.params();
        params["backend"] = json!(backend);
        self.program_checks.insert(
            backend,
            ProgramCheck {
                scope: scope.clone(),
                loading: true,
                result: None,
            },
        );
        let connection = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "desktop.settings.probe",
                    params,
                )
                .await
                .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                let current = this.program_check_scope(backend, cx);
                if let Some(check) = this.program_checks.get_mut(backend) {
                    if check.scope == scope {
                        check.loading = false;
                        if current.as_ref().ok() == Some(&scope) && this.connected { check.result = Some(result); }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn executable_field(
        &self,
        backend: &'static str,
        label: &'static str,
        input: &Entity<InputState>,
        effective: Option<&EffectiveCommand>,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let disabled =
            !self.connected || self.settings_loading || self.executable_settings.is_none();
        let target = input.clone();
        let probe_input = input.clone();
        let check = self.program_checks.get(backend);
        let loading = check.is_some_and(|check| check.loading);
        let changed = check.is_some_and(|check| check.input_changed(&self.program_check_scope(backend, cx)));
        let (pill_dot, pill_label): (Rgba, SharedString) = if loading {
            (super::amber_dot(cx), tr("检查中…").into())
        } else if changed {
            (super::amber_dot(cx), tr("需检查").into())
        } else {
            match check.and_then(|c| c.result.as_ref()) {
                Some(Ok(result)) if result.status == "available" => (
                    Color::Success.get(cx),
                    result
                        .version
                        .clone()
                        .unwrap_or_else(|| tr("本机可用").to_owned())
                        .into(),
                ),
                Some(Ok(_)) | Some(Err(_)) => (Color::Error.get(cx), tr("需检查").into()),
                None => (Color::Muted.get(cx), tr("待配置").into()),
            }
        };
        let checked_scope = check.map(|check| check.scope.label());
        let checked_result = check.filter(|_| !changed).and_then(|check| check.result.as_ref());
        let has_meta = effective.is_some() || changed || checked_result.is_some();
        let readout = |suffix: &str, text: String| {
            crate::text_reading::selectable_text(
                SharedString::from(format!("program-{backend}-{suffix}")),
                text,
            )
        };
        let field = v_flex()
            .min_w_0()
            .gap_2p5()
            .px_4()
            .py_3p5()
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id(SharedString::from(format!("program-{backend}-name")))
                                    .role(Role::Label)
                                    .aria_value(label)
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(label),
                            )
                            .child(super::status_pill(pill_dot, pill_label, cx))
                            .when(effective.is_some_and(|e| e.source == "env"), |row| {
                                row.child(
                                    div()
                                        .px_2()
                                        .py(px(1.5))
                                        .rounded_full()
                                        .bg(Color::Attention.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Color::Text.get(cx))
                                        .child(tr("环境变量覆盖")),
                                )
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("probe-{backend}")))
                            .outline()
                            .small()
                            .h(px(28.))
                            .px_2p5()
                            .rounded_lg()
                            .label(if loading {
                                tr("检查中…")
                            } else {
                                tr("检查程序")
                            })
                            .accessibility_label(if english() {
                                format!("Check {label} version")
                            } else {
                                format!("检查 {label} 程序版本")
                            })
                            .disabled(disabled || loading)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.check_program(backend, probe_input.clone(), cx)
                            })),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(input)
                                .aria_label(if english() {
                                    format!("{label} executable path")
                                } else {
                                    format!("{label} 可执行文件路径")
                                })
                                .disabled(disabled),
                        ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("browse-{backend}")))
                            .outline()
                            .small()
                            .h(px(30.))
                            .px_3()
                            .rounded_lg()
                            .label(tr("浏览…"))
                            .accessibility_label(if english() {
                                format!("Select {label} executable")
                            } else {
                                format!("选择 {label} 可执行文件")
                            })
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.browse_executable(label, target.clone(), window, cx)
                            })),
                    ),
            )
            .when(has_meta, |block| {
                block.child(
                    v_flex()
                        .min_w_0()
                        .gap_1()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .when_some(effective, |row, value| {
                            row.child(
                                readout("effective", value.label(tr("当前使用")))
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx)),
                            )
                        })
                        .when(changed, |row| {
                            row.child(
                                readout("changed", tr("配置或输入已更改，请重新检查。").into())
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx)),
                            )
                        })
                        .when(checked_result.is_some(), |row| row.child(readout("scope", checked_scope.clone().unwrap_or_default()).text_xs().text_color(Color::Muted.get(cx))))
                        .when_some(checked_result, |row, result| match result {
                            Err(error) => row.child(
                                readout("error", crate::language::localize_owned_error(error))
                                    .text_xs()
                                    .text_color(Color::Error.get(cx)),
                            ),
                            Ok(result) => row.child(
                                v_flex()
                                    .min_w_0()
                                    .gap_1()
                                    .child(
                                        readout("result", result.message())
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if result.status == "available" {
                                                Color::Text.get(cx)
                                            } else {
                                                Color::Error.get(cx)
                                            }),
                                    )
                                    .child(
                                        readout(
                                            "checked",
                                            EffectiveCommand {
                                                command: result.command.clone(),
                                                source: result.source.clone(),
                                            }
                                            .label(tr("本次检查")),
                                        )
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx)),
                                    )
                                    .when(result.verified_version.is_none(), |row| row.child(readout("compatibility-unknown", if english() { "No client-verified version is available; compatibility is unverified." } else { "客户端没有已验证版本，兼容性尚未验证。" }.into()).text_xs().text_color(Color::Secondary.get(cx))))
                                    .when(
                                        result
                                            .version
                                            .as_ref()
                                            .zip(result.verified_version.as_ref())
                                            .is_some_and(|(version, verified)| version != verified),
                                        |row| {
                                            row.child(
                                                readout(
                                                    "compatibility",
                                                    if english() {
                                                        format!(
                                                            "Client-verified version: {}. The detected version differs; compatibility has not been verified.",
                                                            result.verified_version.as_deref().unwrap_or_default()
                                                        )
                                                    } else {
                                                        format!(
                                                            "客户端已验证版本：{}。检测版本不同，尚未验证兼容性。",
                                                            result.verified_version.as_deref().unwrap_or_default()
                                                        )
                                                    },
                                                )
                                                .text_xs()
                                                .text_color(Color::Secondary.get(cx)),
                                            )
                                        },
                                    ),
                            ),
                        }),
                )
            });
        v_flex()
            .child(crate::focus_scroll::Reveal::new(
                SharedString::from(format!("program-{backend}-reveal")),
                &self.program_scroll,
                field,
            ))
            .when(backend == "codex", |field| {
                field.child(div().min_w_0().px_4().pb_3p5().child(self.render_codex_account_check(cx)))
            })
    }

    fn probe_mcp_resource(
        &mut self,
        name: String,
        scope: String,
        config_path: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let key = format!("{scope}:{name}");
        if !self.connected || self.mcp_probing.contains(&key) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.mcp_probing.insert(key.clone());
        self.mcp_action_error = None;
        cx.notify();
        let workspace_id = self.workspace.clone();
        cx.spawn(async move |this, cx| {
            let mut params = serde_json::Map::new();
            params.insert("name".into(), serde_json::json!(name));
            if scope == "global" || scope == "project" {
                params.insert("scope".into(), serde_json::json!(scope));
            }
            if let Some(ws) = workspace_id {
                params.insert("workspaceId".into(), serde_json::json!(ws));
            }
            if let Some(cfg) = config_path {
                params.insert("configPath".into(), serde_json::json!(cfg));
            }
            let result = runtime
                .request("mcp.probe", serde_json::Value::Object(params))
                .await
                .and_then(|v| {
                    serde_json::from_value::<crate::protocol::McpProbeResult>(v)
                        .map_err(|e| e.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                this.mcp_probing.remove(&key);
                match result {
                    Ok(probe) => {
                        this.mcp_probe_results.insert(key, probe);
                    }
                    Err(err) => {
                        this.mcp_probe_results.insert(
                            key,
                            crate::protocol::McpProbeResult {
                                ok: false,
                                status: "error".into(),
                                latency_ms: 0,
                                tools: vec![],
                                error: Some(err),
                            },
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_project_mcp(&mut self, name: String, disabled: bool, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.workspace.clone() else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        if !self.connected {
            return;
        }
        self.mcp_action_error = None;
        cx.spawn(async move |this, cx| {
            let res = runtime
                .request(
                    "mcp.toggleProjectServer",
                    serde_json::json!({
                        "workspaceId": workspace_id,
                        "name": name,
                        "disabled": disabled,
                    }),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(_) => this.refresh_readiness(cx),
                    Err(err) => {
                        this.mcp_action_error = Some(err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn remove_project_mcp(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.workspace.clone() else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        if !self.connected {
            return;
        }
        self.mcp_action_error = None;
        cx.spawn(async move |this, cx| {
            let res = runtime
                .request(
                    "mcp.removeProjectServer",
                    serde_json::json!({
                        "workspaceId": workspace_id,
                        "name": name,
                    }),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(_) => {
                        this.mcp_probe_results.remove(&format!("project:{name}"));
                        this.refresh_readiness(cx);
                    }
                    Err(err) => {
                        this.mcp_action_error = Some(err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_project_mcp_form(
        &mut self,
        editing_name: Option<String>,
        initial_transport: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name_val = editing_name.clone().unwrap_or_default();
        let name_state = cx.new(|cx| {
            let mut s = gpui_kit::component::input::InputState::new(window, cx)
                .placeholder(tr("服务名称，如 context7"));
            if !name_val.is_empty() {
                s.set_value(name_val, window, cx);
            }
            s
        });
        let command_state = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .placeholder(tr("启动命令，如 npx"))
        });
        let args_state = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .placeholder(tr("参数（空格分隔），如 -y @upstash/context7-mcp"))
        });
        let url_state = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .placeholder("https://example.com/mcp")
        });
        let env_state = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .masked(true)
                .placeholder(tr("可选环境变量（脱敏输入），如 API_KEY=xxx, ENV=prod"))
        });
        self.mcp_action_error = None;
        self.mcp_form = Some(crate::app::ProjectMcpForm {
            editing_name,
            transport: if initial_transport == "http" { "http" } else { "stdio" },
            name: name_state,
            command: command_state,
            args: args_state,
            url: url_state,
            env: env_state,
            saving: false,
        });
        cx.notify();
    }

    fn save_project_mcp_form(&mut self, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.workspace.clone() else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        if !self.connected {
            return;
        }
        let Some(form) = self.mcp_form.as_mut() else {
            return;
        };
        if form.saving {
            return;
        }
        let name = form.name.read(cx).value().trim().to_owned();
        let transport = form.transport;
        let command = form.command.read(cx).value().trim().to_owned();
        let args_raw = form.args.read(cx).value().trim().to_owned();
        let url = form.url.read(cx).value().trim().to_owned();
        let env_raw = form.env.read(cx).value().trim().to_owned();

        if name.is_empty() {
            self.mcp_action_error = Some(tr("请填写 MCP 服务名称").to_owned());
            cx.notify();
            return;
        }
        if transport == "stdio" && command.is_empty() {
            self.mcp_action_error = Some(tr("stdio 类型 MCP 必须填写启动命令").to_owned());
            cx.notify();
            return;
        }
        if transport == "http" && !url.starts_with("http://") && !url.starts_with("https://") {
            self.mcp_action_error = Some(tr("HTTP 类型 MCP 必须填写 http:// 或 https:// 地址").to_owned());
            cx.notify();
            return;
        }

        let args: Vec<String> = args_raw
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let mut env_map = serde_json::Map::new();
        for pair in env_raw.split([',', ';', '\n']) {
            let trimmed = pair.trim();
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim();
                if !key.is_empty() {
                    env_map.insert(key.to_owned(), serde_json::Value::String(v.trim().to_owned()));
                }
            }
        }

        form.saving = true;
        self.mcp_action_error = None;
        cx.notify();

        let mut payload = serde_json::Map::new();
        payload.insert("workspaceId".into(), serde_json::json!(workspace_id));
        payload.insert("name".into(), serde_json::json!(name));
        payload.insert("transport".into(), serde_json::json!(transport));
        if transport == "stdio" {
            payload.insert("command".into(), serde_json::json!(command));
            if !args.is_empty() {
                payload.insert("args".into(), serde_json::json!(args));
            }
        } else {
            payload.insert("url".into(), serde_json::json!(url));
        }
        if !env_map.is_empty() {
            payload.insert("env".into(), serde_json::Value::Object(env_map));
        }

        cx.spawn(async move |this, cx| {
            let res = runtime
                .request("mcp.upsertProjectServer", serde_json::Value::Object(payload))
                .await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(_) => {
                        this.mcp_form = None;
                        this.mcp_action_error = None;
                        this.refresh_readiness(cx);
                    }
                    Err(err) => {
                        if let Some(form) = this.mcp_form.as_mut() {
                            form.saving = false;
                        }
                        this.mcp_action_error = Some(err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_readiness_and_mcp_card(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let report = self.readiness.as_ref();
        v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .shadow(crate::palette::card_shadow(cx))
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("后端环境就绪度与 MCP / 扩展资源")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("只读检查各后端路径、凭据存在性与已配置 MCP 服务器，不发起模型推理或修改全局配置。")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(match self.workspace.as_ref().and_then(|id| {
                                        self.workspaces.iter().find(|w| &w.id == id)
                                    }) {
                                        Some(ws) => format!("{}（{}）", tr("当前作用域：全局 + 项目"), ws.name),
                                        None => tr("未选择项目 · 仅扫描全局目录").to_owned(),
                                    }),
                            ),
                    )
                    .child({
                        let workspace_mcp_path = self
                            .workspace
                            .as_ref()
                            .and_then(|id| self.workspaces.iter().find(|w| &w.id == id))
                            .map(|w| std::path::Path::new(&w.path).join(".mcp.json"));
                        h_flex()
                            .flex_shrink_0()
                            .flex_wrap()
                            .items_center()
                            .gap_1p5()
                            .when_some(workspace_mcp_path, |row, mcp_path| {
                                let reveal_path = mcp_path.clone();
                                row.child(
                                    Button::new("programs-add-project-mcp")
                                        .outline()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .px_2p5()
                                        .rounded_md()
                                        .icon(gpui_kit::assets::IconName::Plus)
                                        .label(tr("添加项目 MCP"))
                                        .accessibility_label(tr("添加项目 MCP"))
                                        .disabled(!self.connected)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_project_mcp_form(None, "stdio", window, cx);
                                        })),
                                )
                                .when(mcp_path.exists(), |r| {
                                    r.child(
                                        Button::new("programs-reveal-project-mcp")
                                            .custom(crate::palette::subtle_icon_variant(cx))
                                            .compact()
                                            .xsmall()
                                            .h(px(26.))
                                            .px_2()
                                            .rounded_md()
                                            .label(tr("打开 .mcp.json"))
                                            .accessibility_label(tr("打开 .mcp.json"))
                                            .on_click(move |_, _, cx| {
                                                cx.reveal_path(&reveal_path);
                                            }),
                                    )
                                })
                            })
                            .child(
                                Button::new("programs-refresh-readiness")
                                    .outline()
                                    .compact()
                                    .xsmall()
                                    .h(px(26.))
                                    .px_2p5()
                                    .rounded_md()
                                    .icon(gpui_kit::assets::IconName::RefreshCw)
                                    .label(if self.readiness_loading {
                                        tr("诊断中…")
                                    } else {
                                        tr("重新诊断")
                                    })
                                    .accessibility_label(tr("重新诊断"))
                                    .disabled(!self.connected || self.readiness_loading)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.refresh_readiness(cx);
                                    })),
                            )
                    }),
            )
            .when_some(self.mcp_form.as_ref(), |view, form| {
                let is_stdio = form.transport == "stdio";
                let editing = form.editing_name.is_some();
                let labeled_field = |label_key: &'static str, input: gpui_kit::component::input::Input, cx: &App| {
                    v_flex()
                        .w_full()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Color::Secondary.get(cx))
                                .child(tr(label_key)),
                        )
                        .child(input.small())
                };
                view.child(
                    v_flex()
                        .w_full()
                        .gap_2p5()
                        .p_3()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Color::Text.get(cx))
                                        .child(if editing {
                                            tr("编辑项目级 MCP 服务器（.mcp.json）")
                                        } else {
                                            tr("添加项目级 MCP 服务器（.mcp.json）")
                                        }),
                                )
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_0p5()
                                        .p_0p5()
                                        .rounded_lg()
                                        .bg(Color::Surface.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .child(
                                            Button::new("mcp-form-transport-stdio")
                                                .custom(crate::palette::segment_variant(is_stdio, cx))
                                                .compact()
                                                .xsmall()
                                                .h(px(22.))
                                                .px_2p5()
                                                .rounded_md()
                                                .label("stdio")
                                                .selected(is_stdio)
                                                .disabled(form.saving)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    if let Some(f) = this.mcp_form.as_mut()
                                                        && f.transport != "stdio"
                                                    {
                                                        f.transport = "stdio";
                                                        cx.notify();
                                                    }
                                                })),
                                        )
                                        .child(
                                            Button::new("mcp-form-transport-http")
                                                .custom(crate::palette::segment_variant(!is_stdio, cx))
                                                .compact()
                                                .xsmall()
                                                .h(px(22.))
                                                .px_2p5()
                                                .rounded_md()
                                                .label("HTTP / SSE")
                                                .selected(!is_stdio)
                                                .disabled(form.saving)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    if let Some(f) = this.mcp_form.as_mut()
                                                        && f.transport != "http"
                                                    {
                                                        f.transport = "http";
                                                        cx.notify();
                                                    }
                                                })),
                                        ),
                                ),
                        )
                        .child(labeled_field(
                            "服务名称",
                            gpui_kit::component::input::Input::new(&form.name),
                            cx,
                        ))
                        .when(is_stdio, |col| {
                            col.child(labeled_field(
                                "启动命令",
                                gpui_kit::component::input::Input::new(&form.command),
                                cx,
                            ))
                            .child(labeled_field(
                                "命令行参数",
                                gpui_kit::component::input::Input::new(&form.args),
                                cx,
                            ))
                        })
                        .when(!is_stdio, |col| {
                            col.child(labeled_field(
                                "服务地址 (URL)",
                                gpui_kit::component::input::Input::new(&form.url),
                                cx,
                            ))
                        })
                        .child(labeled_field(
                            "环境变量（可选，脱敏）",
                            gpui_kit::component::input::Input::new(&form.env),
                            cx,
                        ))
                        .child(
                            h_flex()
                                .w_full()
                                .justify_end()
                                .gap_1p5()
                                .pt_0p5()
                                .child(
                                    Button::new("mcp-form-cancel")
                                        .custom(crate::palette::subtle_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .px_2p5()
                                        .rounded_md()
                                        .label(tr("取消"))
                                        .disabled(form.saving)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.mcp_form = None;
                                            this.mcp_action_error = None;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("mcp-form-save")
                                        .primary()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .px_3()
                                        .rounded_md()
                                        .label(if form.saving {
                                            tr("保存中…")
                                        } else {
                                            tr("保存到 .mcp.json")
                                        })
                                        .disabled(form.saving || !self.connected)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.save_project_mcp_form(cx);
                                        })),
                                ),
                        ),
                )
            })
            .when_some(self.readiness_error.as_ref(), |view, err| {
                view.child(
                    div()
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .bg(Color::ErrorSurface.get(cx))
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(format!("{}: {err}", tr("环境诊断读取失败"))),
                )
            })
            .when_some(self.mcp_action_error.as_ref(), |view, err| {
                view.child(
                    div()
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .bg(Color::ErrorSurface.get(cx))
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(err.clone()),
                )
            })
            .when_some(report, |view, report| {
                view.child(
                    v_flex()
                        .w_full()
                        .gap_1p5()
                        .children(report.backends.iter().enumerate().map(|(i, b)| {
                            let (badge_text, badge_color) =
                                crate::welcome::agents::readiness_badge(&b.level, cx);
                            let name = match b.backend.as_str() {
                                "pi" => tr("内置 Pi"),
                                "codex" => "Codex",
                                "claude" => "Claude Code",
                                "pi-rpc" => tr("外部 Pi"),
                                "acp" => "ACP",
                                other => other,
                            };
                            let detail_line = b.config_error.clone().or_else(|| {
                                let localized = crate::language::localize_owned_error(&b.remediation_key);
                                let trimmed = localized.trim();
                                (!trimmed.is_empty()
                                    && trimmed != tr("已就绪")
                                    && trimmed != "已就绪"
                                    && trimmed != "Ready")
                                    .then_some(localized)
                            });
                            h_flex()
                                .id(("readiness-backend", i))
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_0p5()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_1p5()
                                                .flex_wrap()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(name.to_owned()),
                                                )
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .gap_1()
                                                        .px_2()
                                                        .py(px(1.))
                                                        .rounded_full()
                                                        .bg(Color::Card.get(cx))
                                                        .text_size(px(10.5))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(badge_color)
                                                        .child(
                                                            div()
                                                                .size(px(5.5))
                                                                .rounded_full()
                                                                .bg(badge_color),
                                                        )
                                                        .child(badge_text.to_owned()),
                                                )
                                                .when_some(b.executable.as_ref(), |row, exec_path| {
                                                    row.child(
                                                        div()
                                                            .text_size(px(10.5))
                                                            .text_color(Color::Muted.get(cx))
                                                            .truncate()
                                                            .child(exec_path.clone()),
                                                    )
                                                }),
                                        )
                                        .when_some(detail_line, |col, line| {
                                            col.child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(if b.config_error.is_some() {
                                                        Color::Error.get(cx)
                                                    } else {
                                                        Color::Muted.get(cx)
                                                    })
                                                    .truncate()
                                                    .child(line),
                                            )
                                        }),
                                )
                        })),
                )
                .child(
                    v_flex()
                        .w_full()
                        .gap_1p5()
                        .pt_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Color::Secondary.get(cx))
                                .child(if english() {
                                    format!(
                                        "Configured MCP servers & extensions ({})",
                                        report.mcp_resources.len()
                                    )
                                } else {
                                    format!(
                                        "已配置 MCP 服务器与扩展资源（{}）",
                                        report.mcp_resources.len()
                                    )
                                }),
                        )
                        .when(report.mcp_resources.is_empty(), |col| {
                            col.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("暂未在已知本地配置文件中发现 MCP 服务器或扩展。")),
                            )
                        })
                        .children(report.mcp_resources.iter().enumerate().map(|(i, res)| {
                            let (status_label, status_color) = match res.status.as_str() {
                                "configured" => (tr("已配置"), Color::Success.get(cx)),
                                "disabled" => (tr("已停用"), Color::Muted.get(cx)),
                                "error" => (tr("配置错误"), Color::Error.get(cx)),
                                other => (other, Color::Muted.get(cx)),
                            };
                            h_flex()
                                .id(("readiness-mcp", i))
                                .w_full()
                                .min_w_0()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px_3()
                                .py_1p5()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .when(res.status == "error", |row| {
                                    row.border_1().border_color(Color::Error.get(cx).opacity(0.4))
                                })
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_0p5()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_1p5()
                                                .flex_wrap()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(res.name.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(format!("{} · {}", res.source, res.transport)),
                                                )
                                                .when(!res.scope.is_empty(), |row| {
                                                    let is_project = res.scope == "project";
                                                    row.child(
                                                        div()
                                                            .px_1()
                                                            .py(px(0.5))
                                                            .rounded_sm()
                                                            .bg(if is_project {
                                                                Color::AccentSoft.get(cx)
                                                            } else {
                                                                Color::Card.get(cx)
                                                            })
                                                            .text_size(px(10.))
                                                            .text_color(if is_project {
                                                                Color::Accent.get(cx)
                                                            } else {
                                                                Color::Muted.get(cx)
                                                            })
                                                            .child(if is_project {
                                                                tr("项目级")
                                                            } else {
                                                                tr("全局")
                                                            }),
                                                    )
                                                })
                                                .when_some(res.detail.as_ref(), |row, detail| {
                                                    row.child(
                                                        div()
                                                            .text_size(px(10.5))
                                                            .text_color(Color::Muted.get(cx))
                                                            .truncate()
                                                            .child(detail.clone()),
                                                    )
                                                }),
                                        )
                                        .when_some(res.error.as_ref(), |col, err| {
                                            col.child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(Color::Error.get(cx))
                                                    .truncate()
                                                    .child(err.clone()),
                                            )
                                        })
                                        .when_some(
                                            self.mcp_probe_results.get(&format!("{}:{}", res.scope, res.name)),
                                            |col, probe| {
                                                let summary = if probe.ok {
                                                    let tool_names: Vec<&str> = probe
                                                        .tools
                                                        .iter()
                                                        .take(5)
                                                        .map(|t| t.name.as_str())
                                                        .collect();
                                                    let more = probe.tools.len().saturating_sub(tool_names.len());
                                                    let suffix = if tool_names.is_empty() {
                                                        String::new()
                                                    } else if more > 0 {
                                                        format!(" · {} +{}", tool_names.join(", "), more)
                                                    } else {
                                                        format!(" · {}", tool_names.join(", "))
                                                    };
                                                    if english() {
                                                        format!(
                                                            "Handshake OK · {}ms · {} tools{}",
                                                            probe.latency_ms,
                                                            probe.tools.len(),
                                                            suffix
                                                        )
                                                    } else {
                                                        format!(
                                                            "握手正常 · {}ms · {} 个工具{}",
                                                            probe.latency_ms,
                                                            probe.tools.len(),
                                                            suffix
                                                        )
                                                    }
                                                } else {
                                                    format!(
                                                        "{}: {}",
                                                        tr("探活失败"),
                                                        probe.error.as_deref().unwrap_or(&probe.status)
                                                    )
                                                };
                                                col.child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(if probe.ok {
                                                            Color::Success.get(cx)
                                                        } else {
                                                            Color::Error.get(cx)
                                                        })
                                                        .truncate()
                                                        .child(summary),
                                                )
                                            },
                                        ),
                                )
                                .child({
                                    let probe_key = format!("{}:{}", res.scope, res.name);
                                    let is_probing = self.mcp_probing.contains(&probe_key);
                                    let can_probe = self.connected
                                        && res.status != "error"
                                        && res.transport != "extension";
                                    let is_project_mcp_json = res.scope == "project"
                                        && res
                                            .config_path
                                            .as_deref()
                                            .is_some_and(|p| p.ends_with(".mcp.json"));
                                    let is_disabled = res.status == "disabled";
                                    let p_name = res.name.clone();
                                    let p_scope = res.scope.clone();
                                    let p_cfg = res.config_path.clone();
                                    let t_name = res.name.clone();
                                    let r_name = res.name.clone();
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1()
                                        .when(can_probe, |row| {
                                            row.child(
                                                Button::new(("readiness-mcp-probe", i))
                                                    .custom(crate::palette::subtle_icon_variant(cx))
                                                    .compact()
                                                    .xsmall()
                                                    .h(px(22.))
                                                    .px_1p5()
                                                    .rounded_sm()
                                                    .disabled(is_probing)
                                                    .label(if is_probing {
                                                        tr("探活中…")
                                                    } else {
                                                        tr("探活")
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.probe_mcp_resource(
                                                            p_name.clone(),
                                                            p_scope.clone(),
                                                            p_cfg.clone(),
                                                            cx,
                                                        );
                                                    })),
                                            )
                                        })
                                        .when(is_project_mcp_json && self.workspace.is_some(), |row| {
                                            let edit_name = res.name.clone();
                                            let edit_transport: &'static str =
                                                if res.transport == "http" || res.transport == "sse" {
                                                    "http"
                                                } else {
                                                    "stdio"
                                                };
                                            row.child(
                                                Button::new(("readiness-mcp-edit", i))
                                                    .custom(crate::palette::subtle_icon_variant(cx))
                                                    .compact()
                                                    .xsmall()
                                                    .h(px(22.))
                                                    .px_1p5()
                                                    .rounded_sm()
                                                    .label(tr("编辑"))
                                                    .accessibility_label(tr("编辑"))
                                                    .on_click(cx.listener(move |this, _, window, cx| {
                                                        this.open_project_mcp_form(
                                                            Some(edit_name.clone()),
                                                            edit_transport,
                                                            window,
                                                            cx,
                                                        );
                                                    })),
                                            )
                                            .child(
                                                Button::new(("readiness-mcp-toggle", i))
                                                    .custom(crate::palette::subtle_icon_variant(cx))
                                                    .compact()
                                                    .xsmall()
                                                    .h(px(22.))
                                                    .px_1p5()
                                                    .rounded_sm()
                                                    .label(if is_disabled {
                                                        tr("启用")
                                                    } else {
                                                        tr("禁用")
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.toggle_project_mcp(
                                                            t_name.clone(),
                                                            !is_disabled,
                                                            cx,
                                                        );
                                                    })),
                                            )
                                            .child(
                                                Button::new(("readiness-mcp-remove", i))
                                                    .custom(crate::palette::subtle_icon_variant(cx))
                                                    .compact()
                                                    .xsmall()
                                                    .h(px(22.))
                                                    .px_1p5()
                                                    .rounded_sm()
                                                    .label(tr("删除"))
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.remove_project_mcp(
                                                            r_name.clone(),
                                                            cx,
                                                        );
                                                    })),
                                            )
                                        })
                                        .when_some(res.config_path.clone(), |row, cfg_path| {
                                            row.child(
                                                Button::new(("readiness-mcp-reveal", i))
                                                    .custom(crate::palette::subtle_icon_variant(cx))
                                                    .compact()
                                                    .xsmall()
                                                    .h(px(22.))
                                                    .px_1p5()
                                                    .rounded_sm()
                                                    .label(tr("打开配置"))
                                                    .tooltip(cfg_path.clone())
                                                    .on_click(move |_, _, cx| {
                                                        cx.reveal_path(std::path::Path::new(&cfg_path));
                                                    }),
                                            )
                                        })
                                        .child(
                                            div()
                                                .px_1p5()
                                                .py(px(1.))
                                                .rounded_sm()
                                                .bg(Color::Card.get(cx))
                                                .text_size(px(10.5))
                                                .text_color(status_color)
                                                .child(status_label.to_owned()),
                                        )
                                })
                        })),
                )
            })
    }

    fn insert_reference_to_composer(
        &mut self,
        snippet: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_settings();
        let existing = self.input.read(cx).value().to_string();
        let next = if existing.trim().is_empty() {
            snippet
        } else if existing.ends_with('\n') {
            format!("{existing}{snippet}")
        } else {
            format!("{existing}\n{snippet}")
        };
        let input = self.input.clone();
        input.update(cx, |state, cx| {
            state.set_value(next, window, cx);
        });
        let focus = self.input.read(cx).focus_handle(cx);
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
        cx.notify();
    }

    fn render_skills_and_rules_card(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let report = self.skills_rules.as_ref();
        let rule_count = report.map(|r| r.rules.len()).unwrap_or(0);
        let skill_count = report.map(|r| r.skills.len()).unwrap_or(0);
        v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .shadow(crate::palette::card_shadow(cx))
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("Skills 与项目规则（AGENTS.md / SKILL.md）")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(if english() {
                                        format!("Discovered {rule_count} rules · {skill_count} skills (read-only scan; click to open or cite in chat)")
                                    } else {
                                        format!("已发现 {rule_count} 份规则文件 · {skill_count} 个 Skill（只读扫描，支持定位文件或一键引用到对话）")
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(match self.workspace.as_ref().and_then(|id| {
                                        self.workspaces.iter().find(|w| &w.id == id)
                                    }) {
                                        Some(ws) => format!("{}（{}）", tr("当前作用域：全局 + 项目"), ws.name),
                                        None => tr("未选择项目 · 仅扫描全局目录").to_owned(),
                                    }),
                            ),
                    ),
            )
            .when_some(report, |view, report| {
                let mut combined: Vec<(&str, &crate::protocol::SkillOrRuleEntry)> = Vec::new();
                for r in &report.rules {
                    combined.push(("规则", r));
                }
                for s in report.skills.iter().take(24) {
                    combined.push(("Skill", s));
                }
                if combined.is_empty() {
                    return view.child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("暂未在当前项目或常见全局目录发现 AGENTS.md / CLAUDE.md / SKILL.md。")),
                    );
                }
                view.child(
                    v_flex()
                        .w_full()
                        .gap_1p5()
                        .children(combined.into_iter().enumerate().map(|(i, (kind_label, item))| {
                            let is_project = item.scope == "project";
                            let reveal_path = item.path.clone();
                            let cite_snippet = if english() {
                                let en_kind = if kind_label == "规则" { "rule" } else { kind_label };
                                format!("Please follow {} `{}` ({}).", en_kind, item.name, item.path)
                            } else {
                                format!("请参考并遵循{} `{}`（{}）。", kind_label, item.name, item.path)
                            };
                            let desc = item
                                .description
                                .as_deref()
                                .filter(|d| !d.trim().is_empty())
                                .unwrap_or(&item.excerpt)
                                .to_owned();
                            h_flex()
                                .id(("skills-rules-item", i))
                                .w_full()
                                .min_w_0()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px_3()
                                .py_1p5()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_0p5()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_1p5()
                                                .flex_wrap()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(item.name.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .px_1()
                                                        .py(px(0.5))
                                                        .rounded_sm()
                                                        .bg(if is_project {
                                                            Color::AccentSoft.get(cx)
                                                        } else {
                                                            Color::Card.get(cx)
                                                        })
                                                        .text_size(px(10.))
                                                        .text_color(if is_project {
                                                            Color::Accent.get(cx)
                                                        } else {
                                                            Color::Muted.get(cx)
                                                        })
                                                        .child(format!(
                                                            "{} · {}",
                                                            tr(kind_label),
                                                            if is_project { tr("项目级") } else { tr("全局") }
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(item.source.clone()),
                                                ),
                                        )
                                        .when(!desc.is_empty(), |col| {
                                            col.child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(Color::Secondary.get(cx))
                                                    .truncate()
                                                    .child(desc),
                                            )
                                        }),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            Button::new(("skill-cite", i))
                                                .custom(crate::palette::subtle_icon_variant(cx))
                                                .compact()
                                                .xsmall()
                                                .h(px(22.))
                                                .px_2()
                                                .rounded_sm()
                                                .label(tr("引用到对话"))
                                                .accessibility_label(tr("引用到对话"))
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    this.insert_reference_to_composer(
                                                        cite_snippet.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                })),
                                        )
                                        .child(
                                            Button::new(("skill-reveal", i))
                                                .custom(crate::palette::subtle_icon_variant(cx))
                                                .compact()
                                                .xsmall()
                                                .h(px(22.))
                                                .px_2()
                                                .rounded_sm()
                                                .label(tr("打开文件"))
                                                .tooltip(reveal_path.clone())
                                                .accessibility_label(tr("打开文件"))
                                                .on_click(move |_, _, cx| {
                                                    cx.reveal_path(std::path::Path::new(&reveal_path));
                                                }),
                                        ),
                                )
                        })),
                )
            })
    }

    fn render_local_usage_summary_card(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let fmt_tok = |n: u64| -> String {
            if n >= 1_000_000 {
                format!("{:.2}M", n as f64 / 1_000_000.0)
            } else if n >= 1_000 {
                format!("{:.1}K", n as f64 / 1_000.0)
            } else {
                n.to_string()
            }
        };
        v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .shadow(crate::palette::card_shadow(cx))
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(tr("多后端本地活跃与 Token 统计")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("纯本地只读汇总全部项目的跨后端会话分布与已落盘 Token 统计，不发起任何模型或网络请求。")),
                    ),
            )
            .when_some(self.local_usage_summary.as_ref(), |view, summary| {
                let metrics = [
                    (tr("总会话数"), summary.total_sessions.to_string()),
                    (tr("近 7 天活跃"), summary.active_last7_days.to_string()),
                    (tr("近 30 天活跃"), summary.active_last30_days.to_string()),
                    (tr("Worktree 会话"), summary.worktree_sessions.to_string()),
                    (tr("已归档"), summary.archived_sessions.to_string()),
                    (
                        tr("采样 Token（入/出/缓存）"),
                        format!(
                            "{} / {} / {}",
                            fmt_tok(summary.total_input_tokens),
                            fmt_tok(summary.total_output_tokens),
                            fmt_tok(summary.total_cache_read_tokens),
                        ),
                    ),
                ];
                view.child(
                    h_flex()
                        .w_full()
                        .flex_wrap()
                        .gap_2()
                        .children(metrics.into_iter().enumerate().map(|(i, (label, val))| {
                            v_flex()
                                .id(("usage-metric", i))
                                .px_2p5()
                                .py_1p5()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .text_color(Color::Muted.get(cx))
                                        .child(label.to_owned()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Color::Text.get(cx))
                                        .child(val),
                                )
                        })),
                )
                .child(
                    v_flex()
                        .w_full()
                        .gap_1()
                        .children(summary.backends.iter().enumerate().map(|(i, b)| {
                            let name = match b.backend.as_str() {
                                "pi" => tr("内置 Pi"),
                                "codex" => "Codex",
                                "claude" => "Claude Code",
                                "pi-rpc" => tr("外部 Pi"),
                                "acp" => "ACP",
                                other => other,
                            };
                            let token_text = if b.input_tokens > 0 || b.output_tokens > 0 {
                                format!(
                                    " · Token {}↓/{}↑/{}⚡",
                                    fmt_tok(b.input_tokens),
                                    fmt_tok(b.output_tokens),
                                    fmt_tok(b.cache_read_tokens)
                                )
                            } else {
                                String::new()
                            };
                            h_flex()
                                .id(("usage-backend-row", i))
                                .w_full()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .bg(Color::Inset.get(cx))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Color::Text.get(cx))
                                        .child(name.to_owned()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(Color::Secondary.get(cx))
                                        .child(if english() {
                                            format!(
                                                "{} total · {} (7d) · {} worktree · {} archived{}",
                                                b.total_sessions,
                                                b.active_last7_days,
                                                b.worktree_sessions,
                                                b.archived_sessions,
                                                token_text
                                            )
                                        } else {
                                            format!(
                                                "共 {} 个 · 近7天 {} · 工作树 {} · 归档 {}{}",
                                                b.total_sessions,
                                                b.active_last7_days,
                                                b.worktree_sessions,
                                                b.archived_sessions,
                                                token_text
                                            )
                                        }),
                                )
                        })),
                )
                .when(summary.sampled_transcripts > 0, |col| {
                    col.child(
                        div()
                            .text_size(px(10.5))
                            .text_color(Color::Muted.get(cx))
                            .child(if english() {
                                format!("Token totals sampled from {} recent local transcripts.", summary.sampled_transcripts)
                            } else {
                                format!("Token 数值采样自最近 {} 份本地持久化会话记录。", summary.sampled_transcripts)
                            }),
                    )
                })
            })
    }
}
