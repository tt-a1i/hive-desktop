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
}
