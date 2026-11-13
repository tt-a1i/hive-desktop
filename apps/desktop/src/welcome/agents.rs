use crate::{app::{HiveApp, SettingsSection}, focus_scroll::Reveal, language::{english, tr}, palette::Color};
use gpui_kit::{assets::IconName as Glyph, component::{button::{Button, ButtonVariants}, *}, prelude::FluentBuilder, *};

impl HiveApp {
    pub(crate) fn render_welcome_agents(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        v_flex().flex_shrink_0().gap_3()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("选择你的 Agent")))
            .child(h_flex().w_full().flex_wrap().gap_3().items_stretch().children([
                ("builtin", tr("内置 Pi"), Glyph::Sparkles),
                ("codex", "Codex", Glyph::Cpu),
                ("claude", "Claude Code", Glyph::Bot),
                ("pi", tr("外部 Pi"), Glyph::SquareTerminal),
            ].into_iter().map(|(backend, label, glyph)| {
                let description = if backend == "builtin" { self.pi_accounts.setup_status() }
                    else { self.program_setup_status(backend, cx) };
                let check_state = if backend == "builtin" { self.pi_accounts.local_check_state() }
                    else { self.program_local_check_state(backend, cx) };
                let dot_color = match check_state {
                    Some(true) => Color::Success.get(cx),
                    Some(false) => Color::Error.get(cx),
                    None => Color::Divider.get(cx),
                };
                let account = (backend == "codex").then(|| self.codex_account_setup_status(cx));
                let action = if english() { format!("Configure {label}") } else { format!("配置 {label}") };
                div().flex_1().min_w(px(220.)).p_3().rounded_xl().bg(Color::Card.get(cx)).border_1().border_color(Color::Divider.get(cx))
                    .child(Reveal::new(SharedString::from(format!("welcome-agent-{backend}-reveal")), &self.welcome_scroll,
                    v_flex().w_full().min_w_0().gap_2()
                        .child(h_flex().w_full().min_w_0().items_center().justify_between().gap_2()
                            .child(h_flex().min_w_0().items_center().gap_2()
                                .child(div().size(px(28.)).flex_shrink_0().rounded_lg().bg(Color::Inset.get(cx)).flex().items_center().justify_center()
                                    .child(Icon::new(glyph).size(px(15.)).text_color(Color::Secondary.get(cx))))
                                .child(div().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).child(label))
                                .child(div().size(px(7.)).flex_shrink_0().rounded_full().bg(dot_color)))
                            .child(Button::new(SharedString::from(format!("welcome-agent-{backend}"))).ghost().compact().rounded_full()
                                .label(tr("配置")).accessibility_label(action.clone()).tooltip(action)
                                .disabled(!self.connected).on_click(cx.listener(move |this, _, window, cx| {
                                    if backend == "builtin" { this.settings_section = SettingsSection::PiAccounts; this.open_settings(window, cx); }
                                    else { this.open_program_setup(backend, window, cx); }
                                }))))
                        .child(div().id(SharedString::from(format!("welcome-agent-{backend}-status"))).role(Role::Label).aria_label(description.clone())
                            .text_xs().text_color(Color::Secondary.get(cx)).child(description))
                        .when_some(account, |view, description| view.child(div().id("welcome-codex-account-status").role(Role::Label).aria_label(description.clone())
                            .text_xs().text_color(Color::Secondary.get(cx)).child(description)))))
            })))
            .child(div().id("welcome-agent-check-scope").role(Role::Label)
                .aria_label(tr("本机保存状态、版本和连接检查不验证登录有效性或模型可用性；每个 Agent 使用各自的账号与模型。"))
                .text_xs().text_color(Color::Muted.get(cx)).child(tr("本机保存状态、版本和连接检查不验证登录有效性或模型可用性；每个 Agent 使用各自的账号与模型。")))
    }
}
