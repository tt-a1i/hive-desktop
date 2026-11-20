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
                            .child(super::status_pill(pill_dot, pill_label, cx)),
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
        let field = field.when(backend == "codex", |field| {
            field.child(self.render_codex_account_check(cx))
        });
        crate::focus_scroll::Reveal::new(
            SharedString::from(format!("program-{backend}-reveal")),
            &self.program_scroll,
            field,
        )
    }

    fn render_readiness_and_mcp_card(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let report = self.readiness.as_ref();
        v_flex()
            .w_full()
            .gap_2p5()
            .p_3p5()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .w_full()
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
                            ),
                    )
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
                            .disabled(!self.connected || self.readiness_loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_readiness(cx);
                            })),
                    ),
            )
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
                            h_flex()
                                .id(("readiness-backend", i))
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px_2p5()
                                .py_1p5()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
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
                                                    div()
                                                        .px_1p5()
                                                        .py(px(1.))
                                                        .rounded_sm()
                                                        .bg(Color::Card.get(cx))
                                                        .text_size(px(10.5))
                                                        .text_color(badge_color)
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
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(if b.config_error.is_some() {
                                                    Color::Error.get(cx)
                                                } else {
                                                    Color::Muted.get(cx)
                                                })
                                                .truncate()
                                                .child(
                                                    b.config_error
                                                        .clone()
                                                        .unwrap_or_else(|| crate::language::localize_owned_error(&b.remediation_key)),
                                                ),
                                        ),
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
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(if res.status == "error" {
                                    Color::Error.get(cx).opacity(0.4)
                                } else {
                                    Color::Divider.get(cx)
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
                                        }),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .px_1p5()
                                        .py(px(1.))
                                        .rounded_sm()
                                        .bg(Color::Card.get(cx))
                                        .text_size(px(10.5))
                                        .text_color(status_color)
                                        .child(status_label.to_owned()),
                                )
                        })),
                )
            })
    }
}
