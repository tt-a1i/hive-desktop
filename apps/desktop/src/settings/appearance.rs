use crate::language::tr;
use crate::{
    app::HiveApp,
    appearance::{self, Appearance, ChatWidth, Mode, MonoFontPreset},
    palette::Color,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        switch::Switch,
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
        let highlight_gen = |anchor: &'static str| {
            self.settings_highlight_anchor
                .filter(|(active, _)| *active == anchor)
                .map(|(_, generation)| generation)
        };
        let code_custom = saved.code_font_family() != MonoFontPreset::default()
            || saved.code_font_size() != appearance::DEFAULT_CODE_FONT_SIZE
            || saved.code_word_wrap();
        let terminal_custom = saved.terminal_font_family() != MonoFontPreset::default()
            || saved.terminal_font_size() != appearance::DEFAULT_TERMINAL_FONT_SIZE;
        let is_default = saved.is_default();
        let preview_code_font = saved
            .code_font_family()
            .font_family_name()
            .map(SharedString::from)
            .unwrap_or_else(|| Theme::global(cx).mono_font_family.clone());
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
                    .child({
                        let hl = highlight_gen("appearance-chat-width");
                        super::settings_anchor_row(
                            "appearance-chat-width",
                            true,
                            hl.is_some(),
                            hl.unwrap_or(0),
                            cx,
                        )
                        .child(
                            v_flex()
                                .w_full()
                                .min_w_0()
                                .gap_2p5()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(tr("对话内容宽度")),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Secondary.get(cx))
                                                        .child(tr("调节会话消息栏与输入框的最大排版宽度。")),
                                                ),
                                        )
                                        .when(saved.chat_width() != ChatWidth::default(), |row| {
                                            row.child(crate::focus_scroll::Reveal::new(
                                                "reset-appearance-chat-width-reveal",
                                                &self.appearance_scroll,
                                                super::settings_reset_button(
                                                    "reset-appearance-chat-width",
                                                    "恢复默认值",
                                                    cx,
                                                    |_, window, cx| {
                                                        appearance::choose_chat_width(
                                                            ChatWidth::default(),
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                ),
                                            ))
                                        }),
                                )
                                .child(crate::focus_scroll::Reveal::new(
                                    "appearance-chat-width-reveal",
                                    &self.appearance_scroll,
                                    super::settings_segmented_bar(cx).children(
                                        [
                                            (ChatWidth::Standard, "standard"),
                                            (ChatWidth::Comfortable, "comfortable"),
                                            (ChatWidth::Wide, "wide"),
                                        ]
                                        .into_iter()
                                        .map(|(width, key)| {
                                            let active = saved.chat_width() == width;
                                            Button::new(SharedString::from(format!(
                                                "appearance-chat-width-{key}"
                                            )))
                                            .when(active, |b| b.outline())
                                            .when(!active, |b| b.ghost())
                                            .small()
                                            .h(px(26.))
                                            .px_3()
                                            .rounded_md()
                                            .label(width.label())
                                            .disabled(!appearance::available(cx))
                                            .on_click(move |_, window, cx| {
                                                appearance::choose_chat_width(width, window, cx);
                                            })
                                        }),
                                    ),
                                )),
                        )
                    })
                    .child({
                        let hl = highlight_gen("appearance-code-typography");
                        super::settings_anchor_row(
                            "appearance-code-typography",
                            true,
                            hl.is_some(),
                            hl.unwrap_or(0),
                            cx,
                        )
                        .child(
                            v_flex()
                                .w_full()
                                .min_w_0()
                                .gap_3()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(tr("代码与 Diff 排版")),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Secondary.get(cx))
                                                        .child(tr("控制文件预览、工具 Diff 与原始补丁视图的等宽字体、字号和换行方式。")),
                                                ),
                                        )
                                        .when(code_custom, |row| {
                                            row.child(crate::focus_scroll::Reveal::new(
                                                "reset-appearance-code-reveal",
                                                &self.appearance_scroll,
                                                super::settings_reset_button(
                                                    "reset-appearance-code",
                                                    "恢复默认值",
                                                    cx,
                                                    |_, window, cx| {
                                                        appearance::choose_code_font_family(
                                                            MonoFontPreset::default(),
                                                            window,
                                                            cx,
                                                        );
                                                        appearance::choose_code_font_size(
                                                            appearance::DEFAULT_CODE_FONT_SIZE,
                                                            window,
                                                            cx,
                                                        );
                                                        appearance::choose_code_word_wrap(
                                                            false, window, cx,
                                                        );
                                                    },
                                                ),
                                            ))
                                        }),
                                )
                                .child(crate::focus_scroll::Reveal::new(
                                    "appearance-code-font-family-reveal",
                                    &self.appearance_scroll,
                                    super::settings_segmented_bar(cx).flex_wrap().children(
                                        MonoFontPreset::for_platform().iter().copied().map(
                                            |preset| {
                                                let active = saved.code_font_family() == preset;
                                                Button::new(SharedString::from(format!(
                                                    "appearance-code-font-{}",
                                                    preset.label()
                                                )))
                                                .when(active, |b| b.outline())
                                                .when(!active, |b| b.ghost())
                                                .small()
                                                .h(px(26.))
                                                .px_2p5()
                                                .rounded_md()
                                                .label(preset.label())
                                                .disabled(!appearance::available(cx))
                                                .on_click(move |_, window, cx| {
                                                    appearance::choose_code_font_family(
                                                        preset, window, cx,
                                                    );
                                                })
                                            },
                                        ),
                                    ),
                                ))
                                .child(
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .flex_wrap()
                                        .child(crate::focus_scroll::Reveal::new(
                                            "appearance-code-font-size-reveal",
                                            &self.appearance_scroll,
                                            super::settings_segmented_bar(cx)
                                                .child(
                                                    Button::new("decrease-code-font")
                                                        .ghost()
                                                        .small()
                                                        .h(px(26.))
                                                        .px_2()
                                                        .rounded_md()
                                                        .label("A−")
                                                        .accessibility_label(tr("减小代码字号"))
                                                        .disabled(
                                                            !appearance::available(cx)
                                                                || saved.code_font_size()
                                                                    <= appearance::MIN_CODE_FONT_SIZE,
                                                        )
                                                        .on_click(|_, window, cx| {
                                                            let size = cx
                                                                .global::<Appearance>()
                                                                .code_font_size()
                                                                .saturating_sub(1);
                                                            appearance::choose_code_font_size(
                                                                size, window, cx,
                                                            );
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(format!(
                                                            "{} px",
                                                            saved.code_font_size()
                                                        )),
                                                )
                                                .child(
                                                    Button::new("increase-code-font")
                                                        .ghost()
                                                        .small()
                                                        .h(px(26.))
                                                        .px_2()
                                                        .rounded_md()
                                                        .label("A+")
                                                        .accessibility_label(tr("增大代码字号"))
                                                        .disabled(
                                                            !appearance::available(cx)
                                                                || saved.code_font_size()
                                                                    >= appearance::MAX_CODE_FONT_SIZE,
                                                        )
                                                        .on_click(|_, window, cx| {
                                                            let size = cx
                                                                .global::<Appearance>()
                                                                .code_font_size()
                                                                .saturating_add(1);
                                                            appearance::choose_code_font_size(
                                                                size, window, cx,
                                                            );
                                                        }),
                                                ),
                                        ))
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Secondary.get(cx))
                                                        .child(tr("自动换行")),
                                                )
                                                .child(crate::focus_scroll::Reveal::new(
                                                    "appearance-code-wrap-reveal",
                                                    &self.appearance_scroll,
                                                    Switch::new("appearance-code-word-wrap")
                                                        .accessibility_label(tr("代码与 Diff 自动换行"))
                                                        .checked(saved.code_word_wrap())
                                                        .disabled(!appearance::available(cx))
                                                        .on_change(|checked, window, cx| {
                                                            appearance::choose_code_word_wrap(
                                                                *checked, window, cx,
                                                            );
                                                        }),
                                                )),
                                        ),
                                ),
                        )
                    })
                    .child({
                        let hl = highlight_gen("appearance-terminal-typography");
                        super::settings_anchor_row(
                            "appearance-terminal-typography",
                            true,
                            hl.is_some(),
                            hl.unwrap_or(0),
                            cx,
                        )
                        .child(
                            v_flex()
                                .w_full()
                                .min_w_0()
                                .gap_2p5()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(tr("工作区终端字体")),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Secondary.get(cx))
                                                        .child(tr("设置内置工作区终端使用的等宽字体与字号，修改后实时生效。")),
                                                ),
                                        )
                                        .when(terminal_custom, |row| {
                                            row.child(crate::focus_scroll::Reveal::new(
                                                "reset-appearance-terminal-reveal",
                                                &self.appearance_scroll,
                                                super::settings_reset_button(
                                                    "reset-appearance-terminal",
                                                    "恢复默认值",
                                                    cx,
                                                    cx.listener(|this, _, window, cx| {
                                                        appearance::choose_terminal_font_family(
                                                            MonoFontPreset::default(),
                                                            window,
                                                            cx,
                                                        );
                                                        appearance::choose_terminal_font_size(
                                                            appearance::DEFAULT_TERMINAL_FONT_SIZE,
                                                            window,
                                                            cx,
                                                        );
                                                        this.sync_terminal_typography(cx);
                                                    }),
                                                ),
                                            ))
                                        }),
                                )
                                .child(crate::focus_scroll::Reveal::new(
                                    "appearance-terminal-font-family-reveal",
                                    &self.appearance_scroll,
                                    super::settings_segmented_bar(cx).flex_wrap().children(
                                        MonoFontPreset::for_platform().iter().copied().map(
                                            |preset| {
                                                let active =
                                                    saved.terminal_font_family() == preset;
                                                Button::new(SharedString::from(format!(
                                                    "appearance-terminal-font-{}",
                                                    preset.label()
                                                )))
                                                .when(active, |b| b.outline())
                                                .when(!active, |b| b.ghost())
                                                .small()
                                                .h(px(26.))
                                                .px_2p5()
                                                .rounded_md()
                                                .label(preset.label())
                                                .disabled(!appearance::available(cx))
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    appearance::choose_terminal_font_family(
                                                        preset, window, cx,
                                                    );
                                                    this.sync_terminal_typography(cx);
                                                }))
                                            },
                                        ),
                                    ),
                                ))
                                .child(crate::focus_scroll::Reveal::new(
                                    "appearance-terminal-font-size-reveal",
                                    &self.appearance_scroll,
                                    super::settings_segmented_bar(cx)
                                        .child(
                                            Button::new("decrease-terminal-font")
                                                .ghost()
                                                .small()
                                                .h(px(26.))
                                                .px_2()
                                                .rounded_md()
                                                .label("A−")
                                                .accessibility_label(tr("减小终端字号"))
                                                .disabled(
                                                    !appearance::available(cx)
                                                        || saved.terminal_font_size()
                                                            <= appearance::MIN_TERMINAL_FONT_SIZE,
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    let size = cx
                                                        .global::<Appearance>()
                                                        .terminal_font_size()
                                                        .saturating_sub(1);
                                                    appearance::choose_terminal_font_size(
                                                        size, window, cx,
                                                    );
                                                    this.sync_terminal_typography(cx);
                                                })),
                                        )
                                        .child(
                                            div()
                                                .px_2()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .child(format!(
                                                    "{} px",
                                                    saved.terminal_font_size()
                                                )),
                                        )
                                        .child(
                                            Button::new("increase-terminal-font")
                                                .ghost()
                                                .small()
                                                .h(px(26.))
                                                .px_2()
                                                .rounded_md()
                                                .label("A+")
                                                .accessibility_label(tr("增大终端字号"))
                                                .disabled(
                                                    !appearance::available(cx)
                                                        || saved.terminal_font_size()
                                                            >= appearance::MAX_TERMINAL_FONT_SIZE,
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    let size = cx
                                                        .global::<Appearance>()
                                                        .terminal_font_size()
                                                        .saturating_add(1);
                                                    appearance::choose_terminal_font_size(
                                                        size, window, cx,
                                                    );
                                                    this.sync_terminal_typography(cx);
                                                })),
                                        ),
                                )),
                        )
                    })
                    .child(
                        super::settings_row(true, cx)
                            .bg(Color::Inset.get(cx).opacity(0.6))
                            .py_2p5()
                            .child(
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(tr("主题、宽度与等宽排版均自动保存至本地配置。")),
                                    )
                                    .child(crate::focus_scroll::Reveal::new(
                                        "appearance-reset-all-reveal",
                                        &self.appearance_scroll,
                                        Button::new("appearance-reset-all")
                                            .outline()
                                            .small()
                                            .h(px(28.))
                                            .px_3()
                                            .rounded_lg()
                                            .label(tr("全部恢复默认"))
                                            .disabled(!appearance::available(cx) || is_default)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                appearance::reset_all(window, cx);
                                                this.sync_terminal_typography(cx);
                                            })),
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
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(Color::Inset.get(cx))
                                    .border_1()
                                    .border_color(Color::Divider.get(cx))
                                    .font_family(preview_code_font)
                                    .text_size(px(f32::from(saved.code_font_size())))
                                    .child("fn preview(width: ChatWidth) -> usize { 42 }"),
                            ),
                    ),
            )
    }
}
