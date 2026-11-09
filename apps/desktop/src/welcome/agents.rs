use crate::{app::{HiveApp, SettingsSection}, focus_scroll::Reveal, language::{english, tr}, palette::Color};
use gpui_kit::{component::{button::{Button, ButtonVariants}, *}, *};

impl HiveApp {
    pub(crate) fn render_welcome_agents(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        v_flex().flex_shrink_0().gap_3()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("选择你的 Agent")))
            .child(h_flex().w_full().flex_wrap().gap_3().items_stretch().children([
                ("builtin", tr("内置 Pi")), ("codex", "Codex"), ("claude", "Claude Code"), ("pi", tr("外部 Pi")),
            ].into_iter().map(|(backend, label)| {
                let description = if backend == "builtin" { tr("连接 Pi 账号或配置模型提供商。").into() }
                    else { self.program_setup_status(backend, cx) };
                let action = if english() { format!("Configure {label}") } else { format!("配置 {label}") };
                div().flex_1().min_w(px(220.)).p_3().rounded_lg().border_1().border_color(Color::Divider.get(cx))
                    .child(Reveal::new(SharedString::from(format!("welcome-agent-{backend}-reveal")), &self.welcome_scroll,
                    v_flex().w_full().min_w_0().gap_2()
                        .child(h_flex().w_full().min_w_0().items_center().justify_between().gap_2()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(label))
                            .child(Button::new(SharedString::from(format!("welcome-agent-{backend}"))).ghost().compact()
                                .label(tr("配置")).accessibility_label(action.clone()).tooltip(action)
                                .disabled(!self.connected).on_click(cx.listener(move |this, _, window, cx| {
                                    if backend == "builtin" { this.settings_section = SettingsSection::PiAccounts; this.open_settings(window, cx); }
                                    else { this.open_program_setup(backend, window, cx); }
                                }))))
                        .child(div().id(SharedString::from(format!("welcome-agent-{backend}-status"))).role(Role::Label).aria_label(description.clone())
                            .text_xs().text_color(Color::Secondary.get(cx)).child(description))))
            })))
            .child(div().id("welcome-agent-check-scope").role(Role::Label)
                .aria_label(tr("版本检查不验证登录或模型可用性；选择 Agent 后使用其原有账号与模型。"))
                .text_xs().text_color(Color::Muted.get(cx)).child(tr("版本检查不验证登录或模型可用性；选择 Agent 后使用其原有账号与模型。")))
    }
}
