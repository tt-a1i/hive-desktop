use crate::{
    app::{HiveApp, SettingsSection},
    language::{english, tr},
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
        self.readiness_loading = true;
        self.readiness_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request("desktop.readiness", serde_json::json!({})).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != generation {
                    return;
                }
                this.readiness_loading = false;
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
        let _ = (
            crate::settings::pi_accounts::PiAccounts::setup_status,
            Self::program_setup_status,
            Self::codex_account_setup_status,
        );
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
        let pi_ready = self.pi_accounts.local_check_state() == Some(true)
            || self.backend_readiness("pi").is_some_and(|b| b.level == "ready");
        let codex_ready = self.program_local_check_state("codex", cx) == Some(true)
            || self.backend_readiness("codex").is_some_and(|b| b.level == "ready");
        let claude_ready = self.program_local_check_state("claude", cx) == Some(true)
            || self.backend_readiness("claude").is_some_and(|b| b.level == "ready");
        let ext_pi_ready = self.program_local_check_state("pi", cx) == Some(true)
            || self.backend_readiness("pi-rpc").is_some_and(|b| b.level == "ready");
        let acp_ready = self.backend_readiness("acp").is_some_and(|b| b.level == "ready");

        let rows: [(&str, &str, String, bool, String, SettingsSection); 5] = [
            (
                "guide-agent-pi",
                "pi",
                tr("内置 Pi").to_string(),
                pi_ready,
                self.backend_readiness("pi")
                    .map(|b| crate::language::localize_owned_error(&b.remediation_key))
                    .unwrap_or_else(|| self.pi_accounts.setup_status()),
                SettingsSection::PiAccounts,
            ),
            (
                "guide-agent-codex",
                "codex",
                "Codex".to_owned(),
                codex_ready,
                self.backend_readiness("codex")
                    .map(|b| crate::language::localize_owned_error(&b.remediation_key))
                    .unwrap_or_else(|| self.codex_account_setup_status(cx)),
                SettingsSection::Programs,
            ),
            (
                "guide-agent-claude",
                "claude",
                "Claude Code".to_owned(),
                claude_ready,
                self.backend_readiness("claude")
                    .map(|b| crate::language::localize_owned_error(&b.remediation_key))
                    .unwrap_or_else(|| self.program_setup_status("claude", cx)),
                SettingsSection::Programs,
            ),
            (
                "guide-agent-ext-pi",
                "pi-rpc",
                tr("外部 Pi").to_string(),
                ext_pi_ready,
                self.backend_readiness("pi-rpc")
                    .map(|b| crate::language::localize_owned_error(&b.remediation_key))
                    .unwrap_or_else(|| self.program_setup_status("pi", cx)),
                SettingsSection::Programs,
            ),
            (
                "guide-agent-acp",
                "acp",
                "ACP".to_owned(),
                acp_ready,
                self.backend_readiness("acp")
                    .map(|b| crate::language::localize_owned_error(&b.remediation_key))
                    .unwrap_or_else(|| tr("可在“外部程序”中添加 ACP Agent 配置").to_string()),
                SettingsSection::Programs,
            ),
        ];

        v_flex()
            .id("welcome-guide-panel")
            .w_full()
            .mt_4()
            .gap_3()
            .child(
                v_flex()
                    .w_full()
                    .rounded_xl()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Card.get(cx))
                    .child(
                        h_flex()
                            .px_3p5()
                            .py_2p5()
                            .border_b_1()
                            .border_color(Color::Divider.get(cx))
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Color::Text.get(cx))
                                            .child(tr("本地 Agent 环境")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(tr("完成任一 Agent 配置后即可在工作区发起对话")),
                                    ),
                            )
                            .child(
                                Button::new("welcome-refresh-readiness")
                                    .ghost()
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .icon(Glyph::RefreshCw)
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
                    .children(rows.into_iter().enumerate().map(
                        |(ix, (id, backend_key, name, ready, status, target_section))| {
                            let report_entry = self.backend_readiness(backend_key);
                            let (badge_text, badge_color) = report_entry
                                .map(|b| readiness_badge(&b.level, cx))
                                .unwrap_or_else(|| {
                                    if ready {
                                        (tr("本机可用"), Color::Success.get(cx))
                                    } else {
                                        (tr("待配置"), Color::Muted.get(cx))
                                    }
                                });
                            let mcp_count = report_entry.map(|b| b.mcp_count).unwrap_or(0);
                            h_flex()
                                .when(ix > 0, |row| {
                                    row.border_t_1().border_color(Color::Divider.get(cx))
                                })
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
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(name),
                                                )
                                                .child(
                                                    div()
                                                        .px_1p5()
                                                        .py_0p5()
                                                        .rounded_sm()
                                                        .bg(Color::Surface.get(cx))
                                                        .text_xs()
                                                        .text_color(badge_color)
                                                        .child(badge_text),
                                                )
                                                .when(mcp_count > 0, |row| {
                                                    row.child(
                                                        div()
                                                            .px_1p5()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(Color::Inset.get(cx))
                                                            .text_xs()
                                                            .text_color(Color::Muted.get(cx))
                                                            .child(if english() {
                                                                format!("{mcp_count} MCP/ext")
                                                            } else {
                                                                format!("{mcp_count} 项 MCP/扩展")
                                                            }),
                                                    )
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Muted.get(cx))
                                                .truncate()
                                                .child(status),
                                        ),
                                )
                                .child(
                                    Button::new(id)
                                        .outline()
                                        .small()
                                        .h(px(26.))
                                        .px_2p5()
                                        .rounded_md()
                                        .label(tr("配置"))
                                        .disabled(!self.connected)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.select_settings_section(
                                                target_section,
                                                window,
                                                cx,
                                            );
                                        })),
                                )
                        },
                    )),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_center()
                    .flex_wrap()
                    .gap_3()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(tr("快捷键提示：搜索会话 · 偏好设置 · 在输入框输入 @ 引用文件")),
            )
    }
}
