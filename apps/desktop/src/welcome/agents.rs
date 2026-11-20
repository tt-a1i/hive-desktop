use crate::{
    app::{HiveApp, SettingsSection},
    language::{localize_owned_error, tr},
    palette::Color,
    protocol::BackendReadiness,
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

pub(crate) fn readiness_badge(level: &str, cx: &App) -> (&'static str, Rgba) {
    match level {
        "ready" => (tr("本机可用"), Color::Success.get(cx)),
        "missing_executable" => (tr("程序缺失"), Color::Error.get(cx)),
        "invalid_path" => (tr("路径无效"), Color::Error.get(cx)),
        "missing_credentials" => (tr("缺少凭据"), crate::chrome::attention_dot(cx)),
        "config_error" => (tr("配置异常"), Color::Error.get(cx)),
        "unconfigured" => (tr("未配置"), Color::Muted.get(cx)),
        _ => (tr("待配置"), Color::Muted.get(cx)),
    }
}

impl HiveApp {
    pub(crate) fn refresh_readiness(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.readiness_loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let generation = self.connection_generation;
        let workspace_id = self.workspace.clone();
        self.readiness_loading = true;
        self.readiness_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let params = match workspace_id.clone() {
                Some(ws) => serde_json::json!({ "workspaceId": ws }),
                None => serde_json::json!({}),
            };
            let result = runtime.request("desktop.readiness", params.clone()).await;
            let skills_res = runtime
                .request("desktop.skillsAndRules", params)
                .await
                .ok()
                .and_then(|v| {
                    serde_json::from_value::<crate::protocol::SkillsAndRulesReport>(v).ok()
                });
            let usage_res = runtime
                .request("desktop.usageSummary", serde_json::json!({}))
                .await
                .ok()
                .and_then(|v| serde_json::from_value::<crate::protocol::LocalUsageSummary>(v).ok());
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != generation {
                    return;
                }
                this.readiness_loading = false;
                if let Some(sr) = skills_res {
                    this.skills_rules = Some(sr);
                }
                if let Some(us) = usage_res {
                    this.local_usage_summary = Some(us);
                }
                match result.and_then(|v| {
                    serde_json::from_value::<crate::protocol::ReadinessReport>(v)
                        .map_err(|e| e.to_string())
                }) {
                    Ok(report) => {
                        this.readiness = Some(report);
                        this.readiness_error = None;
                    }
                    Err(error) => {
                        this.readiness_error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn backend_readiness(&self, backend: &str) -> Option<&BackendReadiness> {
        self.readiness
            .as_ref()
            .and_then(|r| r.backends.iter().find(|b| b.backend == backend))
    }
    /// Returns true if built-in Pi has saved credentials or any external Agent
    /// executable has been verified available on this machine.
    pub(crate) fn has_any_ready_agent(&self, cx: &Context<Self>) -> bool {
        self.pi_accounts.local_check_state() == Some(true)
            || ["codex", "claude", "pi"]
                .into_iter()
                .any(|backend| self.program_local_check_state(backend, cx) == Some(true))
    }

    /// Single-line muted entry shown only when no local Agent is ready yet,
    /// replacing the old four-row onboarding card.
    pub(crate) fn render_agent_setup_link(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if self.has_any_ready_agent(cx) {
            return None;
        }
        Some(
            h_flex()
                .w_full()
                .justify_center()
                .pt_2()
                .child(
                    Button::new("welcome-configure-agents")
                        .custom(crate::palette::quiet_icon_variant(cx))
                        .compact()
                        .xsmall()
                        .h(px(24.))
                        .px_2()
                        .rounded_md()
                        .tooltip(tr("配置本地 Agent 与模型账号"))
                        .accessibility_label(tr("配置本地 Agent 与模型账号"))
                        .disabled(!self.connected)
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(Color::Muted.get(cx))
                                .child(tr("配置本地 Agent →")),
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.settings_section = SettingsSection::PiAccounts;
                            this.open_settings(window, cx);
                        })),
                )
                .into_any_element(),
        )
    }

    pub(crate) fn render_welcome_guide_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut agents = [
            ("pi", tr("内置 Pi")),
            ("codex", "Codex"),
            ("claude", "Claude Code"),
            ("pi-rpc", tr("外部 Pi")),
            ("acp", "ACP"),
        ];
        agents.sort_by_key(|(backend, _)| {
            let report = self.backend_readiness(backend);
            if report
                .is_some_and(|entry| entry.level == "ready" && entry.credential_state == "present")
            {
                0
            } else if report.is_some_and(|entry| entry.level == "ready") {
                1
            } else {
                2
            }
        });

        v_flex()
            .id("welcome-guide-panel")
            .w_full()
            .rounded_xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Card.get(cx))
            .child(
                h_flex()
                    .px_3p5()
                    .py_2p5()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(tr("本地 Agent 环境")),
                    )
                    .child(
                        Button::new("welcome-refresh-readiness")
                            .ghost()
                            .small()
                            .icon(Glyph::RefreshCw)
                            .label(if self.readiness_loading {
                                tr("诊断中…")
                            } else {
                                tr("重新诊断")
                            })
                            .disabled(
                                !self.connected
                                    || self.readiness_loading
                                    || self.welcome_setup.starting,
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_readiness(cx))),
                    ),
            )
            .when_some(self.readiness_error.as_ref(), |panel, error| {
                panel.child(
                    div()
                        .px_3p5()
                        .pb_2()
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(format!(
                            "{} · {}",
                            tr("无法检查本机 Agent"),
                            localize_owned_error(error)
                        )),
                )
            })
            .children(agents.into_iter().map(|(backend, name)| {
                let report = self.backend_readiness(backend);
                let codex_sign_in = backend == "codex"
                    && !self.readiness_loading
                    && self.readiness_error.is_none()
                    && report.is_some_and(|entry| entry.level == "missing_credentials")
                    && self.can_open_codex_sign_in_setup(cx);
                let action = tr(if codex_sign_in { "继续设置 Codex 登录" } else { "配置" });
                let (label, color) = match report {
                    Some(entry)
                        if entry.level == "ready" && entry.credential_state != "present" =>
                    {
                        (tr("检测到配置"), Color::Muted.get(cx))
                    }
                    Some(entry) => readiness_badge(&entry.level, cx),
                    None if self.readiness_loading => (tr("检查中…"), Color::Muted.get(cx)),
                    None => (tr("尚未检查"), Color::Muted.get(cx)),
                };
                let detail = report
                    .map(|entry| {
                        if codex_sign_in {
                            tr("已检测到 Codex CLI。请先检查连接与账号。").to_owned()
                        } else {
                            localize_owned_error(&entry.remediation_key)
                        }
                    })
                    .or_else(|| match backend {
                        "pi" => Some(self.pi_accounts.setup_status()),
                        "codex" => Some(self.codex_account_setup_status(cx)),
                        "claude" => Some(self.program_setup_status("claude", cx)),
                        "pi-rpc" => Some(self.program_setup_status("pi", cx)),
                        _ => None,
                    });
                let role_tagline = match backend {
                    "pi" => tr("开箱即用，免装外部 CLI"),
                    "codex" => tr("深度代码修改、原生回退与分叉"),
                    "claude" => tr("长上下文推理与原生 MCP"),
                    "pi-rpc" => tr("连接本机外部 Pi 服务"),
                    "acp" => tr("异构多 Agent 团队协作"),
                    _ => "",
                };
                h_flex()
                    .border_t_1()
                    .border_color(Color::Divider.get(cx))
                    .px_3p5()
                    .py_2p5()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .flex_wrap()
                                    .child(
                                        div()
                                            .w(px(7.))
                                            .h(px(7.))
                                            .rounded_full()
                                            .flex_shrink_0()
                                            .bg(color),
                                    )
                                    .child(
                                        div().text_sm().font_weight(FontWeight::MEDIUM).child(name),
                                    )
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py(px(0.5))
                                            .rounded_sm()
                                            .bg(Color::Inset.get(cx))
                                            .text_size(px(10.5))
                                            .text_color(color)
                                            .child(label),
                                    )
                                    .when(!role_tagline.is_empty(), |row| {
                                        row.child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(Color::Muted.get(cx))
                                                .child(role_tagline),
                                        )
                                    }),
                            )
                            .when_some(detail, |row, detail| {
                                row.child(
                                    div()
                                        .text_xs()
                                        .text_color(Color::Muted.get(cx))
                                        .child(detail),
                                )
                            }),
                    )
                    .child(
                        Button::new(format!("guide-agent-{backend}"))
                            .outline()
                            .small()
                            .label(action)
                            .accessibility_label(if codex_sign_in {
                                action.to_owned()
                            } else {
                                format!("{action} · {name}")
                            })
                            .disabled(
                                !self.connected || self.closing || self.welcome_setup.starting,
                            )
                            .on_click(cx.listener(move |this, _, window, cx| match backend {
                                "pi" => this.select_settings_section(
                                    SettingsSection::PiAccounts,
                                    window,
                                    cx,
                                ),
                                "acp" => this.select_settings_section(
                                    SettingsSection::Profiles,
                                    window,
                                    cx,
                                ),
                                "codex" if codex_sign_in => {
                                    this.open_codex_sign_in_setup(window, cx)
                                }
                                "pi-rpc" => this.open_program_setup("pi", window, cx),
                                _ => this.open_program_setup(backend, window, cx),
                            })),
                    )
            }))
    }
}
