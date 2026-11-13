//! Unified desktop window chrome helpers for seamless macOS traffic-light
//! integration while preserving native caption bars on Windows and Linux.
use gpui_kit::{prelude::FluentBuilder, *};

pub const TOPBAR_HEIGHT: f32 = 46.;

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
