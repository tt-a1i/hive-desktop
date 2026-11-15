use crate::{
    app::HiveApp,
    diffs_settings::{self, DiffSettings, PreferredDiffMode},
    focus_scroll::Reveal,
    language::tr,
    palette::Color,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        switch::Switch,
        *,
    },
    prelude::FluentBuilder,
    *,
};

impl HiveApp {
    pub(crate) fn save_diffs_pr_base(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.diffs_pr_base_input.read(cx).value().to_string();
        let mut next = diffs_settings::current(cx);
        next.pr_default_base = raw.trim().to_owned();
        if diffs_settings::set(next, cx) {
            let saved = diffs_settings::current(cx).pr_default_base;
            self.diffs_pr_base_input
                .update(cx, |input, cx| input.set_value(saved, window, cx));
        }
        cx.notify();
    }

    pub(crate) fn render_diffs_settings(&self, cx: &mut Context<Self>) -> Div {
        let saved = diffs_settings::current(cx);
        let defaults = DiffSettings::default();
        let notice = diffs_settings::notice(cx);
        let is_default = saved == defaults;

        let highlight_gen = |anchor: &'static str| {
            self.settings_highlight_anchor
                .filter(|(active, _)| *active == anchor)
                .map(|(_, generation)| generation)
        };

        let reset_bool_or_mode = |id: &'static str,
                                  show: bool,
                                  apply: fn(&mut DiffSettings, &DiffSettings)| {
            show.then(|| {
                Reveal::new(
                    SharedString::from(format!("{id}-reveal")),
                    &self.diffs_scroll,
                    super::settings_reset_button(id, "恢复默认值", cx, move |_, _, cx| {
                        let mut next = diffs_settings::current(cx);
                        apply(&mut next, &DiffSettings::default());
                        diffs_settings::set(next, cx);
                    }),
                )
            })
        };

        let draft_base = self.diffs_pr_base_input.read(cx).value().to_string();
        let draft_base_trimmed = draft_base.trim().to_owned();
        let base_dirty = draft_base_trimmed != saved.pr_default_base;

        let hl_view = highlight_gen("diffs-view-mode");
        let hl_pr = highlight_gen("diffs-pr-defaults");

        v_flex()
            .flex_shrink_0()
            .w_full()
            .min_w_0()
            .gap_4()
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Secondary.get(cx))
                            .child(tr("代码差异（Diff）")),
                    )
                    .child(
                        super::settings_card(cx)
                            .child(
                                super::settings_anchor_row(
                                    "diffs-view-mode",
                                    false,
                                    hl_view.is_some(),
                                    hl_view.unwrap_or(0),
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
                                                .items_start()
                                                .justify_between()
                                                .gap_3()
                                                .flex_wrap()
                                                .child(
                                                    v_flex()
                                                        .flex_1()
                                                        .min_w(px(200.))
                                                        .gap_0p5()
                                                        .child(
                                                            h_flex()
                                                                .gap_1p5()
                                                                .items_center()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .text_color(Color::Text.get(cx))
                                                                        .child(tr("默认视图模式")),
                                                                )
                                                                .when_some(
                                                                    reset_bool_or_mode(
                                                                        "diffs-reset-mode",
                                                                        saved.default_diff_mode != defaults.default_diff_mode,
                                                                        |s, d| s.default_diff_mode = d.default_diff_mode,
                                                                    ),
                                                                    |row, btn| row.child(btn),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(tr("选择工具 Diff 与 Git 工作区对比时默认采用的排版方式。")),
                                                        ),
                                                )
                                                .child(
                                                    super::settings_segmented_bar(cx).children(
                                                        [
                                                            (
                                                                PreferredDiffMode::Unified,
                                                                "diffs-mode-unified",
                                                                Glyph::Rows2,
                                                                "合并视图",
                                                            ),
                                                            (
                                                                PreferredDiffMode::Split,
                                                                "diffs-mode-split",
                                                                Glyph::Columns2,
                                                                "并排视图",
                                                            ),
                                                        ]
                                                        .into_iter()
                                                        .map(|(mode, id, icon, label)| {
                                                            let active =
                                                                saved.default_diff_mode == mode;
                                                            Reveal::new(
                                                                SharedString::from(format!(
                                                                    "{id}-reveal"
                                                                )),
                                                                &self.diffs_scroll,
                                                                Button::new(id)
                                                                    .when(active, |b| b.outline())
                                                                    .when(!active, |b| b.ghost())
                                                                    .small()
                                                                    .h(px(26.))
                                                                    .px_2p5()
                                                                    .rounded_md()
                                                                    .icon(icon)
                                                                    .label(tr(label))
                                                                    .accessibility_label(tr(label))
                                                                    .selected(active)
                                                                    .toggled(active)
                                                                    .on_click(move |_, _, cx| {
                                                                        let mut next =
                                                                            diffs_settings::current(
                                                                                cx,
                                                                            );
                                                                        next.default_diff_mode =
                                                                            mode;
                                                                        diffs_settings::set(
                                                                            next, cx,
                                                                        );
                                                                    }),
                                                            )
                                                        }),
                                                    ),
                                                ),
                                        ),
                                ),
                            )
                            .child(
                                super::settings_row(true, cx).child(
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w(px(180.))
                                                .gap_0p5()
                                                .child(
                                                    h_flex()
                                                        .gap_1p5()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(Color::Text.get(cx))
                                                                .child(tr("多文件 Diff 默认折叠")),
                                                        )
                                                        .when_some(
                                                            reset_bool_or_mode(
                                                                "diffs-reset-collapse",
                                                                saved.collapse_multi_file_diffs != defaults.collapse_multi_file_diffs,
                                                                |s, d| s.collapse_multi_file_diffs = d.collapse_multi_file_diffs,
                                                            ),
                                                            |row, btn| row.child(btn),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(tr("当一次改动涉及多个文件时，打开 Diff 默认折叠每个文件的内容块。")),
                                                ),
                                        )
                                        .child(Reveal::new(
                                            "diffs-collapse-multi-reveal",
                                            &self.diffs_scroll,
                                            Switch::new("diffs-collapse-multi-switch")
                                                .accessibility_label(tr("多文件 Diff 默认折叠"))
                                                .checked(saved.collapse_multi_file_diffs)
                                                .on_change(|checked, _, cx| {
                                                    let mut next = diffs_settings::current(cx);
                                                    next.collapse_multi_file_diffs = *checked;
                                                    diffs_settings::set(next, cx);
                                                }),
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
                                        .gap_3()
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w(px(180.))
                                                .gap_0p5()
                                                .child(
                                                    h_flex()
                                                        .gap_1p5()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(Color::Text.get(cx))
                                                                .child(tr("默认忽略空白字符变更")),
                                                        )
                                                        .when_some(
                                                            reset_bool_or_mode(
                                                                "diffs-reset-ignore-ws",
                                                                saved.ignore_whitespace != defaults.ignore_whitespace,
                                                                |s, d| s.ignore_whitespace = d.ignore_whitespace,
                                                            ),
                                                            |row, btn| row.child(btn),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(tr("打开工具或 Git Diff 时默认隐藏纯空格与缩进变化（可在查看器工具栏临时切换）。")),
                                                ),
                                        )
                                        .child(Reveal::new(
                                            "diffs-ignore-ws-reveal",
                                            &self.diffs_scroll,
                                            Switch::new("diffs-ignore-ws-switch")
                                                .accessibility_label(tr("默认忽略空白字符变更"))
                                                .checked(saved.ignore_whitespace)
                                                .on_change(|checked, _, cx| {
                                                    let mut next = diffs_settings::current(cx);
                                                    next.ignore_whitespace = *checked;
                                                    diffs_settings::set(next, cx);
                                                }),
                                        )),
                                ),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Secondary.get(cx))
                            .child(tr("Pull Request 默认值")),
                    )
                    .child(
                        super::settings_card(cx).child(
                            super::settings_anchor_row(
                                "diffs-pr-defaults",
                                false,
                                hl_pr.is_some(),
                                hl_pr.unwrap_or(0),
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
                                                    .min_w(px(180.))
                                                    .gap_0p5()
                                                    .child(
                                                        h_flex()
                                                            .gap_1p5()
                                                            .items_center()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(Color::Text.get(cx))
                                                                    .child(tr("新建 PR 默认设为草稿 (Draft)")),
                                                            )
                                                            .when_some(
                                                                reset_bool_or_mode(
                                                                    "diffs-reset-pr-draft",
                                                                    saved.pr_default_draft != defaults.pr_default_draft,
                                                                    |s, d| s.pr_default_draft = d.pr_default_draft,
                                                                ),
                                                                |row, btn| row.child(btn),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Color::Muted.get(cx))
                                                            .child(tr("在远端面板创建 Pull Request 时，默认勾选「新建为草稿 PR」。")),
                                                    ),
                                            )
                                            .child(Reveal::new(
                                                "diffs-pr-draft-reveal",
                                                &self.diffs_scroll,
                                                Switch::new("diffs-pr-draft-switch")
                                                    .accessibility_label(tr("新建 PR 默认设为草稿 (Draft)"))
                                                    .checked(saved.pr_default_draft)
                                                    .on_change(|checked, _, cx| {
                                                        let mut next = diffs_settings::current(cx);
                                                        next.pr_default_draft = *checked;
                                                        diffs_settings::set(next, cx);
                                                    }),
                                            )),
                                    )
                                    .child(
                                        v_flex()
                                            .pt_2p5()
                                            .border_t_1()
                                            .border_color(Color::Divider.get(cx))
                                            .gap_2()
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .min_w_0()
                                                    .items_center()
                                                    .justify_between()
                                                    .gap_3()
                                                    .flex_wrap()
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .min_w(px(180.))
                                                            .gap_0p5()
                                                            .child(
                                                                h_flex()
                                                                    .gap_1p5()
                                                                    .items_center()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_weight(FontWeight::MEDIUM)
                                                                            .text_color(Color::Text.get(cx))
                                                                            .child(tr("默认目标分支 (Base Branch)")),
                                                                    )
                                                                    .when(saved.pr_default_base != defaults.pr_default_base, |row| {
                                                                        row.child(Reveal::new(
                                                                            "diffs-reset-pr-base-reveal",
                                                                            &self.diffs_scroll,
                                                                            super::settings_reset_button(
                                                                                "diffs-reset-pr-base",
                                                                                "恢复默认值",
                                                                                cx,
                                                                                cx.listener(|this, _, window, cx| {
                                                                                    let mut next = diffs_settings::current(cx);
                                                                                    next.pr_default_base = DiffSettings::default().pr_default_base;
                                                                                    if diffs_settings::set(next, cx) {
                                                                                        let saved = diffs_settings::current(cx).pr_default_base;
                                                                                        this.diffs_pr_base_input.update(cx, |input, cx| {
                                                                                            input.set_value(saved, window, cx);
                                                                                        });
                                                                                    }
                                                                                }),
                                                                            ),
                                                                        ))
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(Color::Muted.get(cx))
                                                                    .child(tr("创建 PR 时默认填入的目标分支名，例如 main 或 master。")),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(
                                                                div().w(px(180.)).child(
                                                                    Input::new(&self.diffs_pr_base_input)
                                                                        .small()
                                                                        .aria_label(tr("默认目标分支 (Base Branch)")),
                                                                ),
                                                            )
                                                            .child(Reveal::new(
                                                                "diffs-save-pr-base-reveal",
                                                                &self.diffs_scroll,
                                                                Button::new("diffs-save-pr-base")
                                                                    .outline()
                                                                    .small()
                                                                    .h(px(28.))
                                                                    .px_2p5()
                                                                    .rounded_lg()
                                                                    .label(tr("保存"))
                                                                    .disabled(!base_dirty || draft_base_trimmed.is_empty())
                                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                                        this.save_diffs_pr_base(window, cx);
                                                                    })),
                                                            )),
                                                    ),
                                            ),
                                    ),
                            ),
                        ),
                    ),
            )
            .child(
                super::settings_card(cx).child(
                    super::settings_row(false, cx).child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .flex_wrap()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(180.))
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("所有 Diff 与 PR 偏好均自动保存至本地配置并生效。")),
                            )
                            .child(Reveal::new(
                                "diffs-reset-all-reveal",
                                &self.diffs_scroll,
                                Button::new("diffs-reset-all")
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .px_3()
                                    .rounded_lg()
                                    .label(tr("全部恢复默认"))
                                    .disabled(is_default)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if diffs_settings::reset(cx) {
                                            let saved = diffs_settings::current(cx).pr_default_base;
                                            this.diffs_pr_base_input.update(cx, |input, cx| {
                                                input.set_value(saved, window, cx);
                                            });
                                        }
                                    })),
                            )),
                    ),
                ),
            )
            .when_some(notice, |panel, (is_error, text)| {
                panel.child(super::settings_notice_box(
                    "diffs-settings-notice",
                    text,
                    is_error,
                    cx,
                ))
            })
    }
}
