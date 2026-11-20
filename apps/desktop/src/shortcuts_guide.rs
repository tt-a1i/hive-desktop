use crate::{
    app::{HiveApp, SettingsSection},
    keybindings::{self, Target, TargetGroup},
    language::{english, tr},
    palette::{self, Color},
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        scroll::ScrollableElement,
        *,
    },
    *,
};

fn keycap(label: String, cx: &App) -> impl IntoElement + use<> {
    div()
        .flex_shrink_0()
        .px_1p5()
        .py(px(1.5))
        .rounded_sm()
        .bg(Color::Inset.get(cx))
        .border_1()
        .border_color(Color::Divider.get(cx))
        .font_family(Theme::global(cx).mono_font_family.clone())
        .text_size(px(10.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(Color::Text.get(cx))
        .child(label)
}

fn shortcut_row(
    id: (&'static str, usize),
    title: String,
    desc: String,
    chord: String,
    cx: &App,
) -> impl IntoElement + use<> {
    h_flex()
        .id(id)
        .w_full()
        .items_center()
        .justify_between()
        .gap_3()
        .px_2p5()
        .py_1p5()
        .rounded_md()
        .bg(Color::Inset.get(cx).opacity(0.65))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Color::Text.get(cx))
                        .truncate()
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(Color::Muted.get(cx))
                        .truncate()
                        .child(desc),
                ),
        )
        .child(keycap(chord, cx))
}

impl HiveApp {
    pub(crate) fn open_shortcuts_guide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing || window.has_active_dialog(cx) {
            return;
        }
        let app = cx.weak_entity();
        let width = (window.viewport_size().width - px(48.)).min(px(640.));
        window.open_dialog(cx, move |dialog, _, cx| {
            let mut groups: Vec<(&'static str, Vec<(String, String, String)>)> = Vec::new();
            for group in TargetGroup::ALL {
                let mut rows = Vec::new();
                for target in [
                    Target::NewSession,
                    Target::Search,
                    Target::CommandPalette,
                    Target::Find,
                    Target::PreviousSession,
                    Target::NextSession,
                    Target::ToggleSidebar,
                    Target::ToggleTerminal,
                    Target::OpenGitWorkspace,
                    Target::SearchProjectFiles,
                    Target::ChooseModel,
                    Target::ToggleMode,
                    Target::Preferences,
                ] {
                    if target.group() != group {
                        continue;
                    }
                    let chord = keybindings::chord_label(target, cx)
                        .unwrap_or_else(|| tr("已停用").to_owned());
                    rows.push((
                        target.label().to_owned(),
                        target.description().to_owned(),
                        chord,
                    ));
                }
                if !rows.is_empty() {
                    groups.push((group.label(), rows));
                }
            }

            let composer_tips: Vec<(String, String, String)> = vec![
                (
                    tr("引用项目文件").to_owned(),
                    tr("在输入框输入 @ 快速筛选并附加当前项目文件").to_owned(),
                    "@".to_owned(),
                ),
                (
                    tr("斜杠命令与快捷操作").to_owned(),
                    tr("在输入框输入 / 调用会话与 Agent 快捷命令").to_owned(),
                    "/".to_owned(),
                ),
                (
                    tr("换行与列表续行").to_owned(),
                    tr("输入时插入换行，在 Markdown 列表项后自动延续编号").to_owned(),
                    "Shift+Enter".to_owned(),
                ),
                (
                    tr("召回历史提示").to_owned(),
                    tr("空输入框按上下方向键快速回填最近发送过的提示词").to_owned(),
                    "↑ / ↓".to_owned(),
                ),
            ];

            let app_settings = app.clone();
            let app_welcome = app.clone();
            dialog
                .title(tr("快捷键与效率速查"))
                .w(width)
                .on_ok(|_, _, _| false)
                .child(
                    v_flex()
                        .w_full()
                        .gap_3()
                        .max_h(px(520.))
                        .overflow_y_scrollbar()
                        .children(groups.into_iter().enumerate().map(|(gi, (g_label, rows))| {
                            v_flex()
                                .id(("shortcuts-group", gi))
                                .w_full()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Color::Secondary.get(cx))
                                        .child(g_label),
                                )
                                .children(rows.into_iter().enumerate().map(|(ri, (t, d, c))| {
                                    shortcut_row(("shortcut-item", gi * 32 + ri), t, d, c, cx)
                                }))
                        }))
                        .child(
                            v_flex()
                                .w_full()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Color::Secondary.get(cx))
                                        .child(tr("输入框高效技巧")),
                                )
                                .children(composer_tips.into_iter().enumerate().map(
                                    |(i, (t, d, c))| {
                                        shortcut_row(("composer-tip", i), t, d, c, cx)
                                    },
                                )),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .pt_2()
                                .border_t_1()
                                .border_color(Color::Divider.get(cx))
                                .child(
                                    Button::new("shortcuts-guide-reopen-welcome")
                                        .custom(palette::subtle_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .px_2p5()
                                        .rounded_md()
                                        .icon(Glyph::Sparkles)
                                        .label(tr("重新打开首次引导"))
                                        .accessibility_label(tr("重新打开首次引导"))
                                        .on_click(move |_, window, cx| {
                                            window.close_dialog(cx);
                                            let _ = app_welcome.update(cx, |this, cx| {
                                                this.open_welcome_setup(cx);
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("shortcuts-guide-customize")
                                        .primary()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .px_3()
                                        .rounded_md()
                                        .icon(Glyph::Settings)
                                        .label(if english() {
                                            "Customize shortcuts"
                                        } else {
                                            "自定义快捷键"
                                        })
                                        .accessibility_label(tr("自定义快捷键"))
                                        .on_click(move |_, window, cx| {
                                            window.close_dialog(cx);
                                            let _ = app_settings.update(cx, |this, cx| {
                                                this.select_settings_section(
                                                    SettingsSection::Keybindings,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }),
                                ),
                        ),
                )
        });
    }
}
