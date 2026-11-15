use crate::{
    app::{HiveApp, SettingsSection},
    language::tr,
    palette::Color,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

impl HiveApp {
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
        let pi_ready = self.pi_accounts.local_check_state() == Some(true);
        let codex_ready = self.program_local_check_state("codex", cx) == Some(true);
        let claude_ready = self.program_local_check_state("claude", cx) == Some(true);
        let ext_pi_ready = self.program_local_check_state("pi", cx) == Some(true);
        let rows = [
            (
                "guide-agent-pi",
                tr("内置 Pi").to_string(),
                pi_ready,
                self.pi_accounts.setup_status(),
                SettingsSection::PiAccounts,
            ),
            (
                "guide-agent-codex",
                "Codex".to_owned(),
                codex_ready,
                self.codex_account_setup_status(cx),
                SettingsSection::Programs,
            ),
            (
                "guide-agent-claude",
                "Claude Code".to_owned(),
                claude_ready,
                self.program_setup_status("claude", cx),
                SettingsSection::Programs,
            ),
            (
                "guide-agent-ext-pi",
                tr("外部 Pi").to_string(),
                ext_pi_ready,
                self.program_setup_status("pi", cx),
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
                    .children(rows.into_iter().enumerate().map(
                        |(ix, (id, name, ready, status, target_section))| {
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
                                                        .text_color(if ready {
                                                            Color::Success.get(cx)
                                                        } else {
                                                            Color::Muted.get(cx)
                                                        })
                                                        .child(if ready {
                                                            tr("本机可用")
                                                        } else {
                                                            tr("待配置")
                                                        }),
                                                ),
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
