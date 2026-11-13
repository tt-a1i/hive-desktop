use crate::language::tr;
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
        let language = self.render_language_settings(cx).into_any_element();
        let saved = cx.global::<Appearance>();
        v_flex()
            .gap_4()
            .child(language)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("界面主题")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(Color::Secondary.get(cx))
                    .child(tr("跟随系统自动切换，或固定使用浅色、深色外观。")),
            )
            .child(crate::focus_scroll::Reveal::new(
                "appearance-theme-reveal",
                &self.appearance_scroll,
                h_flex().gap_2().flex_wrap().children(
                    [
                        ("theme-system", tr("跟随系统"), Mode::System),
                        ("theme-light", tr("浅色"), Mode::Light),
                        ("theme-dark", tr("深色"), Mode::Dark),
                    ]
                    .into_iter()
                    .map(|(id, label, mode)| {
                        Button::new(id)
                            .rounded_full()
                            .label(label)
                            .accessibility_label(if crate::language::english() { format!("Appearance: {label}") } else { format!("外观：{label}") })
                            .selected(saved.mode() == mode)
                            .toggled(saved.mode() == mode)
                            .disabled(!appearance::available(cx))
                            .on_click(cx.listener(move |_, _, window, cx| {
                                appearance::choose(mode, window, cx)
                            }))
                    }),
                ),
            ))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("界面字号")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(Color::Secondary.get(cx))
                    .child(tr("调整菜单、设置和普通聊天文字的大小，立即生效并保存在本机。")),
            )
            .child(crate::focus_scroll::Reveal::new(
                "appearance-font-reveal",
                &self.appearance_scroll,
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .child(
                        Button::new("decrease-interface-font")
                            .label("A−")
                            .accessibility_label(tr("减小界面字号"))
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
                            .accessibility_label(tr("增大界面字号"))
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
                            .label(tr("恢复默认"))
                            .accessibility_label(tr("恢复默认界面字号"))
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
            ))
            .child(
                v_flex()
                    .gap_2()
                    .p_3p5()
                    .rounded_xl()
                    .bg(Color::Card.get(cx))
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::MEDIUM)
                            .child(tr("阅读预览 · Hive Desktop")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .child(tr("在本地工作区直接连接 Agent，查看消息、代码改动与工具执行过程。")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .child(tr("界面字号 12–20 px · 默认 16 px")),
                    ),
            )
            .children(
                saved
                    .message()
                    .map(|message| div().text_sm().child(message)),
            )
    }
}
