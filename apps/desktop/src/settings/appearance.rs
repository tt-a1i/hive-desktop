use crate::language::tr;
use crate::{
    app::HiveApp,
    appearance::{self, Appearance, Mode},
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
    pub(super) fn render_appearance_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.render_language_settings(cx).into_any_element();
        let saved = cx.global::<Appearance>();
        let notice = saved.message();
        v_flex()
            .w_full()
            .min_w_0()
            .gap_4()
            .child(
                super::settings_card(cx)
                    .child(language)
                    .child(
                        super::settings_row(true, cx).child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .flex_wrap()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w(px(220.))
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .child(tr("界面主题")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Secondary.get(cx))
                                                .child(tr(
                                                    "跟随系统自动切换，或固定使用浅色、深色外观。",
                                                )),
                                        ),
                                )
                                .child(crate::focus_scroll::Reveal::new(
                                    "appearance-theme-reveal",
                                    &self.appearance_scroll,
                                    super::settings_segmented_bar(cx).children(
                                        [
                                            ("theme-system", tr("跟随系统"), Mode::System),
                                            ("theme-light", tr("浅色"), Mode::Light),
                                            ("theme-dark", tr("深色"), Mode::Dark),
                                        ]
                                        .into_iter()
                                        .map(|(id, label, mode)| {
                                            let active = saved.mode() == mode;
                                            Button::new(id)
                                                .when(active, |b| b.outline())
                                                .when(!active, |b| b.ghost())
                                                .small()
                                                .h(px(26.))
                                                .px_2p5()
                                                .rounded_md()
                                                .label(label)
                                                .accessibility_label(
                                                    if crate::language::english() {
                                                        format!("Appearance: {label}")
                                                    } else {
                                                        format!("外观：{label}")
                                                    },
                                                )
                                                .selected(active)
                                                .toggled(active)
                                                .disabled(!appearance::available(cx))
                                                .on_click(cx.listener(move |_, _, window, cx| {
                                                    appearance::choose(mode, window, cx)
                                                }))
                                        }),
                                    ),
                                )),
                        ),
                    )
                    .child(
                        super::settings_row(true, cx).child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .flex_wrap()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w(px(220.))
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .child(tr("界面字号")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Secondary.get(cx))
                                                .child(tr(
                                                    "调整菜单、设置和普通聊天文字的大小，立即生效并保存在本机。",
                                                )),
                                        ),
                                )
                                .child(crate::focus_scroll::Reveal::new(
                                    "appearance-font-reveal",
                                    &self.appearance_scroll,
                                    h_flex()
                                        .flex_shrink_0()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            super::settings_segmented_bar(cx)
                                                .child(
                                                    Button::new("decrease-interface-font")
                                                        .ghost()
                                                        .small()
                                                        .h(px(26.))
                                                        .px_2()
                                                        .rounded_md()
                                                        .label("A−")
                                                        .accessibility_label(tr("减小界面字号"))
                                                        .disabled(
                                                            !appearance::available(cx)
                                                                || saved.font_size()
                                                                    <= appearance::MIN_FONT_SIZE,
                                                        )
                                                        .on_click(cx.listener(
                                                            |_, _, window, cx| {
                                                                let size = cx
                                                                    .global::<Appearance>()
                                                                    .font_size()
                                                                    .saturating_sub(1);
                                                                appearance::choose_font_size(
                                                                    size, window, cx,
                                                                );
                                                            },
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(format!("{} px", saved.font_size())),
                                                )
                                                .child(
                                                    Button::new("increase-interface-font")
                                                        .ghost()
                                                        .small()
                                                        .h(px(26.))
                                                        .px_2()
                                                        .rounded_md()
                                                        .label("A+")
                                                        .accessibility_label(tr("增大界面字号"))
                                                        .disabled(
                                                            !appearance::available(cx)
                                                                || saved.font_size()
                                                                    >= appearance::MAX_FONT_SIZE,
                                                        )
                                                        .on_click(cx.listener(
                                                            |_, _, window, cx| {
                                                                let size = cx
                                                                    .global::<Appearance>()
                                                                    .font_size()
                                                                    .saturating_add(1);
                                                                appearance::choose_font_size(
                                                                    size, window, cx,
                                                                );
                                                            },
                                                        )),
                                                ),
                                        )
                                        .child(
                                            Button::new("reset-interface-font")
                                                .outline()
                                                .small()
                                                .h(px(28.))
                                                .px_2p5()
                                                .rounded_lg()
                                                .label(tr("恢复默认"))
                                                .accessibility_label(tr("恢复默认界面字号"))
                                                .disabled(
                                                    !appearance::available(cx)
                                                        || saved.font_size()
                                                            == appearance::DEFAULT_FONT_SIZE,
                                                )
                                                .on_click(cx.listener(|_, _, window, cx| {
                                                    appearance::choose_font_size(
                                                        appearance::DEFAULT_FONT_SIZE,
                                                        window,
                                                        cx,
                                                    )
                                                })),
                                        ),
                                )),
                        ),
                    )
                    .when_some(notice, |card, message| {
                        card.child(
                            super::settings_row(true, cx)
                                .bg(Color::Inset.get(cx))
                                .py_2p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Color::Secondary.get(cx))
                                        .child(message),
                                ),
                        )
                    }),
            )
            .child(
                super::settings_card(cx)
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .flex_wrap()
                            .px_4()
                            .py_2p5()
                            .bg(Color::Inset.get(cx))
                            .border_b_1()
                            .border_color(Color::Divider.get(cx))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr("阅读预览 · Hive Desktop")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("界面字号 12–20 px · 默认 16 px")),
                            ),
                    )
                    .child(
                        v_flex()
                            .px_4()
                            .py_3p5()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr(
                                        "在本地工作区直接连接 Agent，查看消息、代码改动与工具执行过程。",
                                    )),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr(
                                        "选择熟悉的 Agent 担任主控，在同一个地方交流、查看工具过程，与团队成员协作。",
                                    )),
                            ),
                    ),
            )
    }
}
