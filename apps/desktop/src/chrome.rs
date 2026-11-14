//! Unified desktop window chrome helpers for seamless macOS traffic-light
//! integration while preserving native caption bars on Windows and Linux.
use crate::app::HiveApp;
use crate::language::tr;
use crate::palette::{self, Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub const TOPBAR_HEIGHT: f32 = 52.;

#[inline]
pub(crate) fn attention_dot(cx: &App) -> Rgba {
    rgb(if Theme::global(cx).is_dark() {
        0xfbbf24
    } else {
        0xd97706
    })
}

#[inline]
pub fn traffic_light_inset(window: &Window) -> Pixels {
    if cfg!(target_os = "macos") && !window.is_fullscreen() {
        px(78.)
    } else {
        px(0.)
    }
}

pub fn drag_bar(id: &'static str, window: &mut Window, cx: &mut App) -> Stateful<Div> {
    let drag_state = window.use_keyed_state(
        SharedString::from(format!("{id}-window-drag")),
        cx,
        |_, _| false,
    );
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .flex_shrink_0()
        .min_h(px(TOPBAR_HEIGHT))
        .when(cfg!(target_os = "macos"), |bar| {
            let down = drag_state.clone();
            let out = drag_state.clone();
            let up = drag_state.clone();
            let mv = drag_state.clone();
            bar.window_control_area(WindowControlArea::Drag)
                .on_click(|event, window, _| {
                    if event.click_count() == 2 {
                        window.titlebar_double_click();
                    }
                })
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    down.update(cx, |dragging, _| *dragging = true);
                })
                .on_mouse_down_out(move |_, _, cx| {
                    out.update(cx, |dragging, _| *dragging = false);
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    up.update(cx, |dragging, _| *dragging = false);
                })
                .on_mouse_move(move |_, window, cx| {
                    let should_move = mv.update(cx, |dragging, _| std::mem::replace(dragging, false));
                    if should_move {
                        window.start_window_move();
                    }
                })
        })
}

/// Shared 46px immersive topbar for secondary workspace panels (members, collaboration,
/// memory, workflows). Renders inside `RenderOnce` so panels without a `&mut Window`
/// parameter still receive live macOS traffic-light insets and drag-bar behavior.
#[derive(IntoElement)]
pub(crate) struct SubviewTopbar {
    id: &'static str,
    title: SharedString,
    badge: Option<SharedString>,
    sidebar_open: bool,
    app: WeakEntity<HiveApp>,
    back: Option<AnyElement>,
    actions: Vec<AnyElement>,
}

impl SubviewTopbar {
    pub(crate) fn new(
        id: &'static str,
        title: impl Into<SharedString>,
        sidebar_open: bool,
        app: WeakEntity<HiveApp>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            badge: None,
            sidebar_open,
            app,
            back: None,
            actions: Vec::new(),
        }
    }

    pub(crate) fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub(crate) fn back(mut self, back: impl IntoElement) -> Self {
        self.back = Some(back.into_any_element());
        self
    }

    pub(crate) fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
}

impl RenderOnce for SubviewTopbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let inset = traffic_light_inset(window);
        let needs_inset = !self.sidebar_open && inset > px(0.);
        let app_for_toggle = self.app;

        drag_bar(self.id, window, cx)
            .w_full()
            .h(px(TOPBAR_HEIGHT))
            .px_3()
            .when(needs_inset, |bar| bar.pl(inset + px(8.)))
            .justify_between()
            .gap_2()
            .bg(Color::Surface.get(cx))
            .border_b_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .flex_1()
                    .when(!self.sidebar_open, |left| {
                        left.child(
                            Button::new("toggle-navigation")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(26.))
                                .w(px(26.))
                                .rounded_md()
                                .icon(Glyph::PanelLeft)
                                .tooltip(tr("打开或收起导航"))
                                .accessibility_label(tr("打开或收起工作区与会话导航"))
                                .on_click(move |_, _, cx| {
                                    let _ = app_for_toggle.update(cx, |this, cx| {
                                        this.sidebar_open = !this.sidebar_open;
                                        cx.notify();
                                    });
                                }),
                        )
                    })
                    .when_some(self.back, |left, back| {
                        left.child(back).child(
                            div()
                                .w(px(1.))
                                .h(px(14.))
                                .flex_shrink_0()
                                .bg(Color::Divider.get(cx)),
                        )
                    })
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .truncate()
                                    .child(self.title),
                            )
                            .when_some(self.badge, |row, badge| {
                                row.child(
                                    div()
                                        .px_1p5()
                                        .py(px(1.))
                                        .rounded_md()
                                        .flex_shrink_0()
                                        .bg(Color::Inset.get(cx))
                                        .border_1()
                                        .border_color(Color::Divider.get(cx))
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Color::Muted.get(cx))
                                        .child(badge),
                                )
                            }),
                    ),
            )
            .when(!self.actions.is_empty(), |bar| {
                bar.child(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .flex_shrink_0()
                        .children(self.actions),
                )
            })
    }
}

/// Responsive 2-column split layout that switches to single-column / stacked
/// presentation below `breakpoint` without requiring `&mut Window` in callers.
#[derive(IntoElement)]
pub(crate) struct ResponsivePane {
    breakpoint: Pixels,
    left_width: Pixels,
    detail_active: bool,
    stack_right_on_narrow: bool,
    left: AnyElement,
    right: AnyElement,
}

impl ResponsivePane {
    pub(crate) fn new(left: impl IntoElement, right: impl IntoElement) -> Self {
        Self {
            breakpoint: px(880.),
            left_width: px(360.),
            detail_active: false,
            stack_right_on_narrow: false,
            left: left.into_any_element(),
            right: right.into_any_element(),
        }
    }

    pub(crate) fn left_width(mut self, width: Pixels) -> Self {
        self.left_width = width;
        self
    }

    pub(crate) fn detail_active(mut self, active: bool) -> Self {
        self.detail_active = active;
        self
    }

    pub(crate) fn stack_right_on_narrow(mut self) -> Self {
        self.stack_right_on_narrow = true;
        self
    }
}

impl RenderOnce for ResponsivePane {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if window.viewport_size().width >= self.breakpoint {
            h_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_hidden()
                .child(
                    div()
                        .w(self.left_width)
                        .h_full()
                        .min_h_0()
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .border_r_1()
                        .border_color(Color::Divider.get(cx))
                        .child(self.left),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .child(self.right),
                )
                .into_any_element()
        } else if self.detail_active {
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .flex_col()
                .child(self.right)
                .into_any_element()
        } else if self.stack_right_on_narrow {
            v_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_hidden()
                .child(
                    div()
                        .id("responsive-right-stacked")
                        .flex()
                        .flex_col()
                        .flex_shrink_0()
                        .max_h(px(340.))
                        .overflow_hidden()
                        .border_b_1()
                        .border_color(Color::Divider.get(cx))
                        .child(self.right),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .flex()
                        .flex_col()
                        .child(self.left),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .flex_col()
                .child(self.left)
                .into_any_element()
        }
    }
}
