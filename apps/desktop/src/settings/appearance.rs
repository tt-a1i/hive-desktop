use crate::{
    app::HiveApp,
    appearance::{self, Appearance, Mode},
    palette::Color,
};
use gpui_kit::{
    component::{button::Button, *},
    *,
};

impl HiveApp {
    pub(super) fn render_appearance_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = cx.global::<Appearance>();
        v_flex()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("界面主题"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(Color::Secondary.get(cx))
                    .child("跟随系统自动切换，或固定使用浅色、深色外观。"),
            )
            .child(
                h_flex().gap_2().flex_wrap().children(
                    [
                        ("theme-system", "跟随系统", Mode::System),
                        ("theme-light", "浅色", Mode::Light),
                        ("theme-dark", "深色", Mode::Dark),
                    ]
                    .into_iter()
                    .map(|(id, label, mode)| {
                        Button::new(id)
                            .rounded_full()
                            .label(label)
                            .accessibility_label(format!("外观：{label}"))
                            .selected(saved.mode == mode)
                            .disabled(!appearance::available(cx))
                            .on_click(cx.listener(move |_, _, window, cx| {
                                appearance::choose(mode, window, cx)
                            }))
                    }),
                ),
            )
            .children(
                saved
                    .message
                    .clone()
                    .map(|message| div().text_sm().child(message)),
            )
    }
}
