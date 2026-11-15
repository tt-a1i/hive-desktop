use crate::{
    app::HiveApp,
    focus_scroll::Reveal,
    general::{
        self, BusySendMode, GeneralSettings, OpenTarget, ProjectSortOrder, QuitConfirmation,
        SessionTimeFormat,
    },
    language::tr,
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

fn project_sort_label(order: ProjectSortOrder) -> &'static str {
    match order {
        ProjectSortOrder::AddedOrder => tr("添加顺序"),
        ProjectSortOrder::RecentActivity => tr("最近活跃"),
        ProjectSortOrder::Alphabetical => tr("名称 A-Z"),
    }
}

fn time_format_label(format: SessionTimeFormat) -> &'static str {
    match format {
        SessionTimeFormat::Relative => tr("相对时间"),
        SessionTimeFormat::Clock24h => tr("24 小时制"),
        SessionTimeFormat::Clock12h => tr("12 小时制"),
    }
}

fn busy_send_label(mode: BusySendMode) -> &'static str {
    match mode {
        BusySendMode::Queue => tr("加入排队"),
        BusySendMode::SteerWhenSupported => tr("支持时立即插话"),
        BusySendMode::BlockUntilIdle => tr("运行中禁用发送"),
    }
}

fn quit_confirm_label(mode: QuitConfirmation) -> &'static str {
    match mode {
        QuitConfirmation::Never => tr("直接关闭"),
        QuitConfirmation::WhenBusy => tr("有任务运行时确认"),
        QuitConfirmation::Always => tr("总是确认"),
    }
}

fn confirmation_differs(saved: &GeneralSettings, defaults: &GeneralSettings) -> bool {
    saved.confirm_archive_session != defaults.confirm_archive_session
        || saved.confirm_unpin_session != defaults.confirm_unpin_session
        || saved.confirm_cancel_queued_input != defaults.confirm_cancel_queued_input
        || saved.quit_confirmation != defaults.quit_confirmation
}

impl HiveApp {
    pub(crate) fn render_general_settings(&self, cx: &mut Context<Self>) -> Div {
        let saved = general::current(cx);
        let defaults = GeneralSettings::default();
        let notice = general::notice(cx);
        let is_default = saved == defaults;

        let highlight_gen = |anchor: &'static str| {
            self.settings_highlight_anchor
                .filter(|(active, _)| *active == anchor)
                .map(|(_, generation)| generation)
        };

        let reset_control = |id: &'static str, show: bool, apply: fn(&mut GeneralSettings, &GeneralSettings)| {
            show.then(|| {
                Reveal::new(
                    SharedString::from(format!("{id}-reveal")),
                    &self.general_scroll,
                    super::settings_reset_button(id, "恢复默认值", cx, move |_, _, cx| {
                        let mut next = general::current(cx);
                        apply(&mut next, &GeneralSettings::default());
                        general::set(next, cx);
                    }),
                )
            })
        };

        let render_sub_toggle = |id: &'static str,
                                 title: &'static str,
                                 desc: &'static str,
                                 checked: bool,
                                 update: fn(&mut GeneralSettings, bool)| {
            h_flex()
                .w_full()
                .min_w_0()
                .items_center()
                .justify_between()
                .gap_4()
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
                                .child(tr(title)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(tr(desc)),
                        ),
                )
                .child(Reveal::new(
                    SharedString::from(format!("{id}-reveal")),
                    &self.general_scroll,
                    Switch::new(id)
                        .accessibility_label(tr(title))
                        .checked(checked)
                        .on_change(move |next_checked, _, cx| {
                            let mut next = general::current(cx);
                            update(&mut next, *next_checked);
                            general::set(next, cx);
                        }),
                ))
        };

        v_flex()
            .gap_3()
            .child(
                super::settings_card(cx)
                    .child({
                        let hl = highlight_gen("general-open-target");
                        super::settings_anchor_row(
                            "general-open-target",
                            false,
                            hl.is_some(),
                            hl.unwrap_or(0),
                            cx,
                        )
                        .child(
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
                                        .min_w(px(200.))
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .child(tr("默认打开工具")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Muted.get(cx))
                                                .child(tr("点击顶栏“打开”按钮时默认调用的系统或编辑器工具。")),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1p5()
                                        .when_some(
                                            reset_control(
                                                "reset-general-open-target",
                                                saved.default_open_target
                                                    != defaults.default_open_target,
                                                |s, d| s.default_open_target = d.default_open_target,
                                            ),
                                            |row, btn| row.child(btn),
                                        )
                                        .child(Reveal::new(
                                            "general-open-target-bar-reveal",
                                            &self.general_scroll,
                                            super::settings_segmented_bar(cx)
                                                .flex_wrap()
                                                .children(
                                                    OpenTarget::for_platform().iter().copied().map(
                                                        |target| {
                                                            let active =
                                                                saved.default_open_target == target;
                                                            Button::new(SharedString::from(format!(
                                                                "general-open-target-{}",
                                                                target.wire()
                                                            )))
                                                            .when(active, |b| b.outline())
                                                            .when(!active, |b| b.ghost())
                                                            .small()
                                                            .h(px(26.))
                                                            .px_2p5()
                                                            .rounded_md()
                                                            .label(target.label())
                                                            .on_click(move |_, _, cx| {
                                                                let mut next =
                                                                    general::current(cx);
                                                                next.default_open_target = target;
                                                                general::set(next, cx);
                                                            })
                                                        },
                                                    ),
                                                ),
                                        )),
                                ),
                        )
                    })
                    .child({
                        let hl = highlight_gen("general-project-sort");
                        super::settings_anchor_row(
                            "general-project-sort",
                            true,
                            hl.is_some(),
                            hl.unwrap_or(0),
                            cx,
                        )
                        .child(
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
                                        .min_w(px(200.))
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .child(tr("侧边栏项目排序")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Muted.get(cx))
                                                .child(tr("控制左侧边栏中各工作区项目分组的排列顺序。")),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1p5()
                                        .when_some(
                                            reset_control(
                                                "reset-general-project-sort",
                                                saved.project_sort_order
                                                    != defaults.project_sort_order,
                                                |s, d| s.project_sort_order = d.project_sort_order,
                                            ),
                                            |row, btn| row.child(btn),
                                        )
                                        .child(Reveal::new(
                                            "general-project-sort-bar-reveal",
                                            &self.general_scroll,
                                            super::settings_segmented_bar(cx).children(
                                                [
                                                    (ProjectSortOrder::AddedOrder, "added"),
                                                    (ProjectSortOrder::RecentActivity, "recent"),
                                                    (ProjectSortOrder::Alphabetical, "alpha"),
                                                ]
                                                .into_iter()
                                                .map(|(order, key)| {
                                                    let active = saved.project_sort_order == order;
                                                    Button::new(SharedString::from(format!(
                                                        "general-project-sort-{key}"
                                                    )))
                                                    .when(active, |b| b.outline())
                                                    .when(!active, |b| b.ghost())
                                                    .small()
                                                    .h(px(26.))
                                                    .px_2p5()
                                                    .rounded_md()
                                                    .label(project_sort_label(order))
                                                    .on_click(move |_, _, cx| {
                                                        let mut next = general::current(cx);
                                                        next.project_sort_order = order;
                                                        general::set(next, cx);
                                                    })
                                                }),
                                            ),
                                        )),
                                ),
                        )
                    })
                    .child({
                        let hl = highlight_gen("general-pin-active");
                        super::settings_anchor_row(
                            "general-pin-active",
                            true,
                            hl.is_some(),
                            hl.unwrap_or(0),
                            cx,
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .justify_between()
                                .gap_4()
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
                                                .child(tr("活跃会话排在前面")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Muted.get(cx))
                                                .child(tr("在同一工作区和置顶分组内，将正在运行、等待输入或有排队的会话优先靠前显示。")),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_2()
                                        .when_some(
                                            reset_control(
                                                "reset-general-pin-active",
                                                saved.pin_active_sessions_top
                                                    != defaults.pin_active_sessions_top,
                                                |s, d| {
                                                    s.pin_active_sessions_top =
                                                        d.pin_active_sessions_top
                                                },
                                            ),
                                            |row, btn| row.child(btn),
                                        )
                                        .child(Reveal::new(
                                            "general-pin-active-switch-reveal",
                                            &self.general_scroll,
                                            Switch::new("general-pin-active-switch")
                                                .accessibility_label(tr("活跃会话排在前面"))
                                                .checked(saved.pin_active_sessions_top)
                                                .on_change(|checked, _, cx| {
                                                    let mut next = general::current(cx);
                                                    next.pin_active_sessions_top = *checked;
                                                    general::set(next, cx);
                                                }),
                                        )),
                                ),
                        )
                    })
                    .child({
                        let hl = highlight_gen("general-time-format");
                        super::settings_anchor_row(
                            "general-time-format",
                            true,
                            hl.is_some(),
                            hl.unwrap_or(0),
                            cx,
                        )
                        .child(
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
                                        .min_w(px(200.))
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .child(tr("会话时间格式")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Muted.get(cx))
                                                .child(tr("控制侧边栏会话列表右侧展示更新时间的方式。")),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1p5()
                                        .when_some(
                                            reset_control(
                                                "reset-general-time-format",
                                                saved.session_time_format
                                                    != defaults.session_time_format,
                                                |s, d| {
                                                    s.session_time_format = d.session_time_format
                                                },
                                            ),
                                            |row, btn| row.child(btn),
                                        )
                                        .child(Reveal::new(
                                            "general-time-format-bar-reveal",
                                            &self.general_scroll,
                                            super::settings_segmented_bar(cx).children(
                                                [
                                                    (SessionTimeFormat::Relative, "relative"),
                                                    (SessionTimeFormat::Clock24h, "24h"),
                                                    (SessionTimeFormat::Clock12h, "12h"),
                                                ]
                                                .into_iter()
                                                .map(|(format, key)| {
                                                    let active =
                                                        saved.session_time_format == format;
                                                    Button::new(SharedString::from(format!(
                                                        "general-time-format-{key}"
                                                    )))
                                                    .when(active, |b| b.outline())
                                                    .when(!active, |b| b.ghost())
                                                    .small()
                                                    .h(px(26.))
                                                    .px_2p5()
                                                    .rounded_md()
                                                    .label(time_format_label(format))
                                                    .on_click(move |_, _, cx| {
                                                        let mut next = general::current(cx);
                                                        next.session_time_format = format;
                                                        general::set(next, cx);
                                                    })
                                                }),
                                            ),
                                        )),
                                ),
                        )
                    }),
            )
            .child(
                super::settings_card(cx)
                    .child({
                        let hl = highlight_gen("general-busy-send");
                        super::settings_anchor_row(
                            "general-busy-send",
                            false,
                            hl.is_some(),
                            hl.unwrap_or(0),
                            cx,
                        )
                        .child(
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
                                        .min_w(px(200.))
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .child(tr("忙碌时发送行为")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Muted.get(cx))
                                                .child(tr("选择 Agent 仍在处理上一轮输出时发送新消息的处理策略；不支持实时插话的运行时会自动按顺序排队。")),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1p5()
                                        .when_some(
                                            reset_control(
                                                "reset-general-busy-send",
                                                saved.busy_send_mode != defaults.busy_send_mode,
                                                |s, d| s.busy_send_mode = d.busy_send_mode,
                                            ),
                                            |row, btn| row.child(btn),
                                        )
                                        .child(Reveal::new(
                                            "general-busy-send-bar-reveal",
                                            &self.general_scroll,
                                            super::settings_segmented_bar(cx).flex_wrap().children(
                                                [
                                                    (BusySendMode::Queue, "queue"),
                                                    (BusySendMode::SteerWhenSupported, "steer"),
                                                    (BusySendMode::BlockUntilIdle, "block"),
                                                ]
                                                .into_iter()
                                                .map(|(mode, key)| {
                                                    let active = saved.busy_send_mode == mode;
                                                    Button::new(SharedString::from(format!(
                                                        "general-busy-send-{key}"
                                                    )))
                                                    .when(active, |b| b.outline())
                                                    .when(!active, |b| b.ghost())
                                                    .small()
                                                    .h(px(26.))
                                                    .px_2p5()
                                                    .rounded_md()
                                                    .label(busy_send_label(mode))
                                                    .on_click(move |_, _, cx| {
                                                        let mut next = general::current(cx);
                                                        next.busy_send_mode = mode;
                                                        general::set(next, cx);
                                                    })
                                                }),
                                            ),
                                        )),
                                ),
                        )
                    })
                    .child({
                        let hl = highlight_gen("general-confirmations");
                        super::settings_anchor_row(
                            "general-confirmations",
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
                                                        .child(tr("操作与退出确认")),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(tr("为会话操作和窗口关闭设置二次确认。")),
                                                ),
                                        )
                                        .when_some(
                                            reset_control(
                                                "reset-general-confirmations",
                                                confirmation_differs(&saved, &defaults),
                                                |s, d| {
                                                    s.confirm_archive_session =
                                                        d.confirm_archive_session;
                                                    s.confirm_unpin_session =
                                                        d.confirm_unpin_session;
                                                    s.confirm_cancel_queued_input =
                                                        d.confirm_cancel_queued_input;
                                                    s.quit_confirmation = d.quit_confirmation;
                                                },
                                            ),
                                            |row, btn| row.child(btn),
                                        ),
                                )
                                .child(render_sub_toggle(
                                    "general-confirm-archive",
                                    "归档会话前确认",
                                    "将活跃或历史会话移入归档前弹出确认对话框。",
                                    saved.confirm_archive_session,
                                    |s, v| s.confirm_archive_session = v,
                                ))
                                .child(render_sub_toggle(
                                    "general-confirm-unpin",
                                    "取消置顶前确认",
                                    "取消已置顶会话前先弹出一次确认提示。",
                                    saved.confirm_unpin_session,
                                    |s, v| s.confirm_unpin_session = v,
                                ))
                                .child(render_sub_toggle(
                                    "general-confirm-cancel-queue",
                                    "取消排队消息前确认",
                                    "撤销排队中的待发送消息时先预览并确认。",
                                    saved.confirm_cancel_queued_input,
                                    |s, v| s.confirm_cancel_queued_input = v,
                                ))
                                .child(
                                    v_flex()
                                        .w_full()
                                        .min_w_0()
                                        .gap_2()
                                        .pt_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .child(tr("关闭窗口或退出应用确认")),
                                        )
                                        .child(
                                            Reveal::new(
                                                "general-quit-confirm-bar-reveal",
                                                &self.general_scroll,
                                                super::settings_segmented_bar(cx)
                                                    .flex_wrap()
                                                    .children(
                                                        [
                                                            (QuitConfirmation::Never, "never"),
                                                            (
                                                                QuitConfirmation::WhenBusy,
                                                                "when-busy",
                                                            ),
                                                            (QuitConfirmation::Always, "always"),
                                                        ]
                                                        .into_iter()
                                                        .map(|(mode, key)| {
                                                            let active =
                                                                saved.quit_confirmation == mode;
                                                            Button::new(SharedString::from(
                                                                format!(
                                                                    "general-quit-confirm-{key}"
                                                                ),
                                                            ))
                                                            .when(active, |b| b.outline())
                                                            .when(!active, |b| b.ghost())
                                                            .small()
                                                            .h(px(26.))
                                                            .px_3()
                                                            .rounded_md()
                                                            .label(quit_confirm_label(mode))
                                                            .on_click(move |_, _, cx| {
                                                                let mut next = general::current(cx);
                                                                next.quit_confirmation = mode;
                                                                general::set(next, cx);
                                                            })
                                                        }),
                                                    ),
                                            ),
                                        ),
                                ),
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
                                            .child(tr("所有通用偏好均自动保存至本地配置并立即生效。")),
                                    )
                                    .child(Reveal::new(
                                        "general-reset-all-reveal",
                                        &self.general_scroll,
                                        Button::new("general-reset-all")
                                            .outline()
                                            .small()
                                            .h(px(28.))
                                            .px_3()
                                            .rounded_lg()
                                            .label(tr("全部恢复默认"))
                                            .disabled(is_default)
                                            .on_click(|_, _, cx| {
                                                general::reset(cx);
                                            }),
                                    )),
                            ),
                    ),
            )
            .when_some(notice, |panel, (is_error, text)| {
                panel.child(super::settings_notice_box(
                    "general-settings-notice",
                    text,
                    is_error,
                    cx,
                ))
            })
    }
}
