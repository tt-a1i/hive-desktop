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
                            .selected(saved.mode() == mode)
                            .toggled(saved.mode() == mode)
                            .disabled(!appearance::available(cx))
                            .on_click(cx.listener(move |_, _, window, cx| {
                                appearance::choose(mode, window, cx)
                            }))
                    }),
                ),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("界面字号"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(Color::Secondary.get(cx))
                    .child("调整菜单、设置和普通聊天文字的大小，立即生效并保存在本机。"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("decrease-interface-font")
                            .label("A−")
                            .accessibility_label("减小界面字号")
                            .disabled(
                                !appearance::available(cx)
                                    || saved.font_size() <= appearance::MIN_FONT_SIZE,
                            )
                            .on_click(cx.listener(|_, _, window, cx| {
                                let size = cx.global::<Appearance>().font_size().saturating_sub(1);
                                appearance::choose_font_size(size, window, cx);
                            })),
                    )
                    .child(div().text_sm().child(format!("{} px", saved.font_size())))
                    .child(
                        Button::new("increase-interface-font")
                            .label("A+")
                            .accessibility_label("增大界面字号")
                            .disabled(
                                !appearance::available(cx)
                                    || saved.font_size() >= appearance::MAX_FONT_SIZE,
                            )
                            .on_click(cx.listener(|_, _, window, cx| {
                                let size = cx.global::<Appearance>().font_size().saturating_add(1);
                                appearance::choose_font_size(size, window, cx);
                            })),
                    )
                    .child(
                        Button::new("reset-interface-font")
                            .label("恢复默认")
                            .accessibility_label("恢复默认界面字号")
                            .disabled(
                                !appearance::available(cx)
                                    || saved.font_size() == appearance::DEFAULT_FONT_SIZE,
                            )
                            .on_click(cx.listener(|_, _, window, cx| {
                                appearance::choose_font_size(
                                    appearance::DEFAULT_FONT_SIZE,
                                    window,
                                    cx,
                                )
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .child(div().text_base().child("阅读预览 · Hive Desktop"))
                    .child(
                        div()
                            .text_sm()
                            .child("在主会话与成员之间协作，查看消息、工具过程和配置。"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .child("界面字号 12–20 px · 默认 16 px"),
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
