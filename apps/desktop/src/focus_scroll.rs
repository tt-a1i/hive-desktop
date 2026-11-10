use gpui_kit::*;

// Reuse native autoscroll requests for ordinary scrollable Divs. These helpers
// own only focus/reveal geometry; they never intercept text or activation keys.
#[derive(IntoElement)]
pub(crate) struct Reveal {
    id: ElementId,
    scroll: ScrollHandle,
    child: AnyElement,
    focus: Option<FocusHandle>,
}
struct RevealState {
    focus: FocusHandle,
    last: Option<(FocusHandle, Bounds<Pixels>, Option<Size<Pixels>>)>,
}
impl Reveal {
    pub(crate) fn new(
        id: impl Into<ElementId>,
        scroll: &ScrollHandle,
        child: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            scroll: scroll.clone(),
            child: child.into_any_element(),
            focus: None,
        }
    }
    pub(crate) fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }
}
impl RenderOnce for Reveal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| RevealState {
            focus: self.focus.unwrap_or_else(|| cx.focus_handle()),
            last: None,
        });
        let focus = state.read(cx).focus.clone();
        div()
            .min_w_0()
            .track_focus(&focus)
            .tab_stop(false)
            .on_children_prepainted(move |bounds, window, cx| {
                let focused = window
                    .focused(cx)
                    .filter(|_| focus.contains_focused(window, cx))
                    // Track layout size, not the scrolled position: expanding a focused
                    // group must reveal its actions, while manual scrolling stays put.
                    .map(|focused| (focused, self.scroll.bounds(), bounds.first().map(|bounds| bounds.size)));
                let reveal = state.update(cx, |state, _| {
                    let changed = state.last != focused;
                    state.last = focused;
                    changed && state.last.is_some()
                });
                if reveal && let Some(bounds) = bounds.first() {
                    window.request_autoscroll(bounds.dilate(px(8.)));
                }
            })
            .child(self.child)
            .id(self.id)
    }
}

#[derive(IntoElement)]
pub(crate) struct Viewport {
    id: ElementId,
    scroll: ScrollHandle,
    child: AnyElement,
}
impl Viewport {
    pub(crate) fn new(
        id: impl Into<ElementId>,
        scroll: &ScrollHandle,
        child: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            scroll: scroll.clone(),
            child: child.into_any_element(),
        }
    }
}
impl RenderOnce for Viewport {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .track_focus(&focus)
            .tab_stop(false)
            .on_children_prepainted(move |_, window, cx| {
                if !focus.contains_focused(window, cx) {
                    return;
                }
                let Some(target) = window.take_autoscroll() else {
                    return;
                };
                let bounds = self.scroll.bounds();
                // Current reveal groups place their controls after their explanatory text.
                // When the group cannot fit, keep those trailing actions visible.
                let delta = if target.size.height > bounds.size.height {
                    bounds.bottom() - target.bottom()
                } else if target.top() < bounds.top() {
                    bounds.top() - target.top()
                } else if target.bottom() > bounds.bottom() {
                    bounds.bottom() - target.bottom()
                } else {
                    px(0.)
                };
                if delta != px(0.) {
                    let scroll = self.scroll.clone();
                    let offset = scroll.offset() + point(px(0.), delta);
                    window.on_next_frame(move |window, _| {
                        scroll.set_offset(offset);
                        window.refresh();
                    });
                }
            })
            .child(self.child)
            .id(self.id)
    }
}
