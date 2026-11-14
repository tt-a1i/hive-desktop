use crate::palette::Color;
use gpui_kit::{
    component::{scroll::ScrollableElement, *},
    *,
};

pub(crate) fn region(
    id: String,
    label: String,
    handle: &ScrollHandle,
    height: Pixels,
    cx: &App,
) -> Stateful<Div> {
    let scroll = handle.clone();
    v_flex()
        .id(SharedString::from(id))
        .min_w_0()
        .max_h(height)
        .overflow_y_scroll()
        .track_scroll(handle)
        .relative()
        .vertical_scrollbar(handle)
        .tab_index(0)
        .border_1()
        .border_color(Color::Divider.get(cx))
        .rounded_md()
        .p_2()
        .focus(|style| style.border_color(Color::Secondary.get(cx)))
        .role(Role::Region)
        .aria_label(label)
        .on_key_down(move |event, window, cx| {
            let modifiers = event.keystroke.modifiers;
            if modifiers.control || modifiers.platform || modifiers.alt || modifiers.shift {
                return;
            }
            let offset = scroll.offset();
            let page = (scroll.bounds().size.height - px(24.)).max(px(24.));
            match event.keystroke.key.as_str() {
                "up" => scroll.set_offset(point(offset.x, (offset.y + px(24.)).min(px(0.)))),
                "down" => scroll.set_offset(point(offset.x, offset.y - px(24.))),
                "pageup" => scroll.set_offset(point(offset.x, (offset.y + page).min(px(0.)))),
                "pagedown" => scroll.set_offset(point(offset.x, offset.y - page)),
                "home" => scroll.set_offset(point(offset.x, px(0.))),
                "end" => scroll.scroll_to_bottom(),
                _ => return,
            }
            window.refresh();
            cx.stop_propagation();
        })
}

/// Preserve native selection. AccessKit Label text belongs in its value, which
/// Windows exposes as the static text name and macOS exposes as AXValue.
pub(crate) fn selectable_text(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
) -> Stateful<Div> {
    let id = id.into();
    let text = text.into();
    div()
        .id(SharedString::from(format!("{id}-accessible")))
        .role(Role::Label)
        .aria_value(text.clone())
        .min_w_0()
        .child(gpui_kit::base::SelectableText::new(id, text))
}
