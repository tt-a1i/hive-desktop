use crate::{
    app::{HiveApp, SettingsSection},
    focus_scroll::Reveal,
    language::{english, tr},
    palette::Color,
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
    /// Beluga `AgentsStep`-style unified glass/card readiness list.
    pub(crate) fn render_welcome_agents(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let agents = [
            ("builtin", tr("内置 Pi"), Glyph::Sparkles),
            ("codex", "Codex", Glyph::Cpu),
            ("claude", "Claude Code", Glyph::Bot),
            ("pi", tr("外部 Pi"), Glyph::SquareTerminal),
        ];
        v_flex()
            .flex_shrink_0()
            .w_full()
            .gap_2p5()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Secondary.get(cx))
                            .child(tr("本地 Agent 环境")),
                    )
                    .child(Reveal::new(
                        "welcome-programs-reveal",
                        &self.welcome_scroll,
                        Button::new("welcome-programs")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .px_2()
                            .rounded_md()
                            .label(tr("检查外部 Agent 程序"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.settings_section = SettingsSection::Programs;
                                this.open_settings(window, cx);
                            })),
                    )),
            )
            .child(
                v_flex()
                    .w_full()
                    .rounded_2xl()
                    .bg(Color::Card.get(cx))
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .shadow_xs()
                    .overflow_hidden()
                    .children(agents.into_iter().enumerate().map(|(index, (backend, label, glyph))| {
                        let description = if backend == "builtin" {
                            self.pi_accounts.setup_status()
                        } else {
                            self.program_setup_status(backend, cx)
                        };
                        let check_state = if backend == "builtin" {
                            self.pi_accounts.local_check_state()
                        } else {
                            self.program_local_check_state(backend, cx)
                        };
                        let (dot_color, badge_text) = match check_state {
                            Some(true) => (Color::Success.get(cx), tr("本机可用")),
                            Some(false) => (Color::Error.get(cx), tr("需检查")),
                            None => (Color::Muted.get(cx), tr("待配置")),
                        };
                        let account = (backend == "codex").then(|| self.codex_account_setup_status(cx));
                        let action = if english() {
                            format!("Configure {label}")
                        } else {
                            format!("配置 {label}")
                        };
                        div()
                            .w_full()
                            .px_4()
                            .py_3()
                            .when(index > 0, |row| {
                                row.border_t_1().border_color(Color::Divider.get(cx))
                            })
                            .child(Reveal::new(
                                SharedString::from(format!("welcome-agent-{backend}-reveal")),
                                &self.welcome_scroll,
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .items_center()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .size(px(34.))
                                                    .flex_shrink_0()
                                                    .rounded_xl()
                                                    .bg(Color::Inset.get(cx))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        Icon::new(glyph)
                                                            .size(px(16.))
                                                            .text_color(Color::Text.get(cx)),
                                                    ),
                                            )
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap(px(2.))
                                                    .child(
                                                        h_flex()
                                                            .min_w_0()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .truncate()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .child(label),
                                                            )
                                                            .child(
                                                                h_flex()
                                                                    .flex_shrink_0()
                                                                    .items_center()
                                                                    .gap_1()
                                                                    .px_2()
                                                                    .py(px(1.))
                                                                    .rounded_full()
                                                                    .bg(Color::Inset.get(cx))
                                                                    .child(
                                                                        div()
                                                                            .size(px(6.))
                                                                            .rounded_full()
                                                                            .bg(dot_color),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(
                                                                                Color::Secondary
                                                                                    .get(cx),
                                                                            )
                                                                            .child(badge_text),
                                                                    ),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .id(SharedString::from(format!(
                                                                "welcome-agent-{backend}-status"
                                                            )))
                                                            .role(Role::Label)
                                                            .aria_label(description.clone())
                                                            .text_xs()
                                                            .text_color(Color::Muted.get(cx))
                                                            .child(description),
                                                    )
                                                    .when_some(account, |view, description| {
                                                        view.child(
                                                            div()
                                                                .id("welcome-codex-account-status")
                                                                .role(Role::Label)
                                                                .aria_label(description.clone())
                                                                .text_xs()
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(description),
                                                        )
                                                    }),
                                            ),
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "welcome-agent-{backend}"
                                        )))
                                        .outline()
                                        .compact()
                                        .small()
                                        .h(px(28.))
                                        .rounded_lg()
                                        .px_3()
                                        .label(tr("配置"))
                                        .accessibility_label(action.clone())
                                        .tooltip(action)
                                        .disabled(!self.connected)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if backend == "builtin" {
                                                this.settings_section = SettingsSection::PiAccounts;
                                                this.open_settings(window, cx);
                                            } else {
                                                this.open_program_setup(backend, window, cx);
                                            }
                                        })),
                                    ),
                            ))
                    })),
            )
            .child(
                div()
                    .id("welcome-agent-check-scope")
                    .role(Role::Label)
                    .aria_label(tr(
                        "本机保存状态、版本和连接检查不验证登录有效性或模型可用性；每个 Agent 使用各自的账号与模型。",
                    ))
                    .px_1()
                    .text_center()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(tr(
                        "本机保存状态、版本和连接检查不验证登录有效性或模型可用性；每个 Agent 使用各自的账号与模型。",
                    )),
            )
    }
}
