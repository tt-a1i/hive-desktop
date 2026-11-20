use crate::palette::Color;
use gpui_kit::{
    component::{
        scroll::{ScrollableElement, ScrollbarAxis},
        *,
    },
    prelude::FluentBuilder,
    *,
};

use std::{cell::Cell, ops::Range, rc::Rc};

pub(crate) fn region(
    id: String,
    label: String,
    handle: &ScrollHandle,
    height: Pixels,
    cx: &App,
) -> Stateful<Div> {
    reading_region(id, label, handle, height, false, cx)
}

pub(crate) fn source_region(
    id: String,
    label: String,
    handle: &ScrollHandle,
    height: Pixels,
    wrap: bool,
    cx: &App,
) -> Stateful<Div> {
    reading_region(id, label, handle, height, !wrap, cx)
}

fn reading_region(
    id: String,
    label: String,
    handle: &ScrollHandle,
    height: Pixels,
    horizontal: bool,
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
        .when(horizontal, |view| view.overflow_x_scroll().w_full())
        .scrollbar(
            handle,
            if horizontal {
                ScrollbarAxis::Both
            } else {
                ScrollbarAxis::Vertical
            },
        )
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
                "left" | "right" if horizontal && scroll.max_offset().x > px(0.) => {
                    let x =
                        horizontal_offset(&event.keystroke.key, offset.x, scroll.max_offset().x);
                    scroll.set_offset(point(x, offset.y));
                }
                _ => return,
            }
            window.refresh();
            cx.stop_propagation();
        })
}

fn horizontal_offset(key: &str, offset: Pixels, maximum: Pixels) -> Pixels {
    let step = if key == "left" { px(48.) } else { px(-48.) };
    (offset + step).clamp(-maximum, px(0.))
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

pub(crate) struct SourceMatch {
    pub range: Range<usize>,
    pub scroll: ScrollHandle,
    pub reveal: Rc<Cell<bool>>,
    pub horizontal: bool,
}

pub(crate) fn numbered_source(
    id: &'static str,
    page: &TextPages,
    wrap: bool,
    active_match: Option<SourceMatch>,
    cx: &App,
) -> Div {
    let active_match = active_match.and_then(|mut active| {
        active.range = page.local_range(&active.range)?;
        Some(active)
    });
    let numbers = page.line_numbers();
    let digits = numbers.end.saturating_sub(1).to_string().len().max(2) as f32;
    let gutter = rems(0.6 * digits + 0.625);
    let text = page.visible.clone();
    h_flex()
        .min_w_0()
        .items_start()
        .child(div().w(gutter).flex_shrink_0())
        .child(
            v_flex()
                .when(wrap, |view| view.flex_1())
                .when(!wrap, |view| view.flex_none())
                .min_w_0()
                .relative()
                .child(selectable_text(id, text.clone()))
                .child(
                    v_flex()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .child(SourceLineNumbers {
                            source: StyledText::new(text),
                            numbers,
                            color: Color::Muted.get(cx).into(),
                            active_match,
                            highlight: Color::Accent.get(cx).opacity(0.30).into(),
                        }),
                ),
        )
}

/// Use the same text layout as the selectable source so wrapping and font metrics
/// agree. Paint the gutter and the current Find match independently of selection.
struct SourceLineNumbers {
    source: StyledText,
    numbers: std::ops::Range<usize>,
    color: Hsla,
    active_match: Option<SourceMatch>,
    highlight: Hsla,
}

impl IntoElement for SourceLineNumbers {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for SourceLineNumbers {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.source.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.source
            .prepaint(id, inspector, bounds, &mut (), window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.source.layout();
        let line_height = layout.line_height();
        let mut y = bounds.top();
        let viewport = window.content_mask().bounds;
        if let Some(active) = &self.active_match
            && let Some(start) = layout.position_for_index(active.range.start)
        {
            let end = layout.position_for_index(active.range.end).unwrap_or(start);
            let source = layout.bounds();
            let rectangles = match_rectangles(start, end, source, line_height);
            for rectangle in rectangles.iter().filter(|rect| rect.intersects(&viewport)) {
                window.paint_quad(fill(*rectangle, self.highlight));
            }
            if active.reveal.get()
                && let Some(target) = rectangles.first()
            {
                reveal_source_match(active, *target, viewport, window);
            }
        }
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        for (number, line) in self.numbers.clone().zip(layout.line_layouts()) {
            if y >= viewport.bottom() {
                break;
            }
            if y + line_height > viewport.top() {
                let label = number.to_string();
                let mut run = style.to_run(label.len());
                run.color = self.color;
                let label = window
                    .text_system()
                    .shape_line(label.into(), font_size, &[run], None);
                let origin = point(bounds.left() - label.width() - px(8.), y);
                let _ = label.paint(origin, line_height, TextAlign::Left, None, window, cx);
            }
            y += line.size(line_height).height;
        }
    }
}

fn match_rectangles(
    start: Point<Pixels>,
    end: Point<Pixels>,
    source: Bounds<Pixels>,
    line_height: Pixels,
) -> Vec<Bounds<Pixels>> {
    let rect =
        |left, top, right, bottom| Bounds::from_corners(point(left, top), point(right, bottom));
    if start.y == end.y {
        return vec![rect(
            start.x.min(end.x),
            start.y,
            start.x.max(end.x) + px(1.),
            start.y + line_height,
        )];
    }
    let mut rectangles = vec![rect(
        start.x,
        start.y,
        source.right(),
        start.y + line_height,
    )];
    if end.y > start.y + line_height {
        rectangles.push(rect(
            source.left(),
            start.y + line_height,
            source.right(),
            end.y,
        ));
    }
    if end.x > source.left() {
        rectangles.push(rect(source.left(), end.y, end.x, end.y + line_height));
    }
    rectangles
}

fn reveal_axis(
    start: Pixels,
    end: Pixels,
    top: Pixels,
    bottom: Pixels,
    offset: Pixels,
    max_offset: Pixels,
) -> Pixels {
    let inset = px(8.).min((bottom - top) / 4.);
    let viewport_start = top + inset;
    let viewport_end = bottom - inset;
    let delta = if start < viewport_start || end - start > viewport_end - viewport_start {
        viewport_start - start
    } else if end > viewport_end {
        viewport_end - end
    } else {
        px(0.)
    };
    (offset + delta).clamp(-max_offset, px(0.))
}

fn reveal_source_match(
    active: &SourceMatch,
    target: Bounds<Pixels>,
    viewport: Bounds<Pixels>,
    window: &mut Window,
) {
    let scroll = active.scroll.clone();
    let offset = scroll.offset();
    let maximum = scroll.max_offset();
    let next = point(
        if active.horizontal {
            reveal_axis(
                target.left(),
                target.right(),
                viewport.left(),
                viewport.right(),
                offset.x,
                maximum.x,
            )
        } else {
            offset.x
        },
        reveal_axis(
            target.top(),
            target.bottom(),
            viewport.top(),
            viewport.bottom(),
            offset.y,
            maximum.y,
        ),
    );
    let reveal = active.reveal.clone();
    window.on_next_frame(move |window, _| {
        if reveal.replace(false) && scroll.offset() != next {
            scroll.set_offset(next);
            window.refresh();
        }
    });
}

const PAGE_BYTES: usize = 32 * 1024;
const PAGE_LINES: usize = 256;

#[derive(Clone)]
pub(crate) struct TextPages {
    pub full: SharedString,
    pub visible: SharedString,
    pub page: usize,
    pub first_line: usize,
    boundaries: Vec<(usize, usize)>,
}

impl TextPages {
    pub fn new(full: SharedString) -> Self {
        let boundaries = page_boundaries(&full);
        let visible = if boundaries.len() == 2 {
            full.clone()
        } else {
            SharedString::from(&full[..boundaries[1].0])
        };
        Self {
            full,
            visible,
            page: 0,
            first_line: 1,
            boundaries,
        }
    }

    pub fn count(&self) -> usize {
        self.boundaries.len() - 1
    }

    fn line_numbers(&self) -> std::ops::Range<usize> {
        let lines = self.visible.split_terminator('\n').count();
        self.first_line..self.first_line + lines
    }

    pub fn show(&mut self, page: usize) -> bool {
        if page >= self.count() || page == self.page {
            return false;
        }
        let (start, first_line) = self.boundaries[page];
        self.first_line = first_line;
        self.page = page;
        self.visible = SharedString::from(&self.full[start..self.boundaries[page + 1].0]);
        true
    }

    /// Open the canonical page containing the start of a find hit. Where a
    /// hit straddles a page break, show its remaining bytes without changing
    /// the page boundaries used for manual paging or the underlying preview.
    pub(crate) fn show_match(&mut self, range: &Range<usize>) -> bool {
        debug_assert!(
            self.full
                .get(range.clone())
                .is_some_and(|text| !text.is_empty())
        );
        let page = self.boundaries[..self.count()]
            .partition_point(|&(offset, _)| offset <= range.start)
            .saturating_sub(1);
        let (start, first_line) = self.boundaries[page];
        let end = self.boundaries[page + 1].0.max(range.end);
        if self.page == page && self.visible.len() == end - start {
            return false;
        }
        self.page = page;
        self.first_line = first_line;
        self.visible = SharedString::from(&self.full[start..end]);
        true
    }

    fn local_range(&self, range: &Range<usize>) -> Option<Range<usize>> {
        let start = self.boundaries[self.page].0;
        let end = start + self.visible.len();
        let clipped = range.start.max(start)..range.end.min(end);
        (!clipped.is_empty()).then(|| clipped.start - start..clipped.end - start)
    }

    pub fn show_line(&mut self, line: usize) {
        // The final boundary is the end of the text, not the start of a page.
        let page =
            self.boundaries[..self.count()].partition_point(|&(_, first_line)| first_line < line);
        if self
            .boundaries
            .get(page)
            .is_some_and(|&(_, first_line)| first_line == line)
            && page < self.count()
        {
            self.show(page);
            return;
        }
        let previous = page.saturating_sub(1);
        let (start, first_line) = self.boundaries[previous];
        let end = self.boundaries[previous + 1].0;
        let Some(before_match) = line
            .checked_sub(first_line)
            .and_then(|distance| distance.checked_sub(1))
        else {
            return;
        };
        if let Some((index, _)) = self.full.as_bytes()[start..end]
            .iter()
            .enumerate()
            .filter(|&(_, byte)| *byte == b'\n')
            .nth(before_match)
            .filter(|&(index, _)| start + index + 1 < end)
        {
            let offset = start + index + 1;
            self.boundaries.insert(page, (offset, line));
            self.page += usize::from(self.page >= page);
            self.show(page);
        } else {
            self.show(previous);
        }
    }
}

fn page_boundaries(text: &str) -> Vec<(usize, usize)> {
    let mut boundaries = vec![(0, 1)];
    let mut start = 0;
    let mut first_line = 1;
    while start < text.len() {
        let mut limit = text.floor_char_boundary((start + PAGE_BYTES).min(text.len()));
        if text.as_bytes().get(limit) == Some(&b'\n') && text.as_bytes()[limit - 1] == b'\r' {
            limit -= 1;
        }
        let newlines = text.as_bytes()[start..limit]
            .iter()
            .enumerate()
            .filter_map(|(index, &byte)| (byte == b'\n').then_some(start + index + 1));
        // Prefer whole lines, with a UTF-8 boundary for an oversized line.
        let mut last = None;
        let mut line_count = 0;
        for end in newlines.take(PAGE_LINES) {
            last = Some(end);
            line_count += 1;
        }
        let end = if line_count == PAGE_LINES || limit < text.len() {
            last.unwrap_or(limit)
        } else {
            limit
        };
        first_line += line_count;
        boundaries.push((end, first_line));
        start = end;
    }
    if boundaries.len() == 1 {
        boundaries.push((0, 1));
    }
    boundaries
}

#[cfg(test)]
mod tests {
    use super::{PAGE_BYTES, PAGE_LINES, TextPages, horizontal_offset, page_boundaries};
    use gpui_kit::px;

    #[test]
    fn find_matches_select_exact_byte_pages_including_long_line_continuations() {
        let source = format!(
            "{}marker{}marker",
            "x".repeat(PAGE_BYTES - 3),
            "x".repeat(PAGE_BYTES)
        );
        let matches: Vec<_> = source
            .match_indices("marker")
            .map(|(start, hit)| start..start + hit.len())
            .collect();
        let mut pages = TextPages::new(source.into());
        let count = pages.count();
        assert!(pages.show_match(&matches[0]));
        assert_eq!(pages.page, 0);
        assert_eq!(
            &pages.visible[pages.local_range(&matches[0]).unwrap()],
            "marker"
        );
        assert!(pages.show_match(&matches[1]));
        assert_eq!(pages.page, 2);
        assert_eq!(pages.first_line, 1);
        assert_eq!(
            &pages.visible[pages.local_range(&matches[1]).unwrap()],
            "marker"
        );
        assert!(pages.local_range(&matches[0]).is_none());
        assert_eq!(pages.count(), count);
        assert!(pages.show(0));
        assert_eq!(pages.visible.len(), PAGE_BYTES);
    }

    #[test]
    fn match_reveal_keeps_visible_axes_and_clamps_offscreen_matches() {
        let axis = |start, end, offset, maximum| {
            super::reveal_axis(
                gpui_kit::px(start),
                gpui_kit::px(end),
                gpui_kit::px(0.),
                gpui_kit::px(100.),
                gpui_kit::px(offset),
                gpui_kit::px(maximum),
            )
        };
        assert_eq!(axis(30., 60., -150., 500.), gpui_kit::px(-150.));
        assert_eq!(axis(170., 190., 0., 500.), gpui_kit::px(-98.));
        assert_eq!(axis(-30., -20., -20., 500.), gpui_kit::px(0.));
        assert_eq!(axis(800., 900., 0., 500.), gpui_kit::px(-500.));
    }

    #[test]
    fn text_pages_preserve_every_byte_and_bound_each_layout() {
        let text = format!(
            "{}+{}\r\n-fin\n+tail",
            " context\r\n".repeat(PAGE_LINES + 13),
            "汉🦀".repeat(PAGE_BYTES),
        );
        let boundaries = page_boundaries(&text);
        assert_eq!(
            boundaries[1],
            (" context\r\n".len() * PAGE_LINES, PAGE_LINES + 1)
        );
        let mut rebuilt = String::new();
        for bounds in boundaries.windows(2) {
            let page = &text[bounds[0].0..bounds[1].0];
            assert!(!page.is_empty() && page.len() <= PAGE_BYTES);
            assert!(page.lines().count() <= PAGE_LINES);
            rebuilt.push_str(page);
        }
        assert_eq!(rebuilt.as_bytes(), text.as_bytes());
        assert_eq!(page_boundaries(""), [(0, 1), (0, 1)]);
        assert_eq!(page_boundaries("one\ntwo"), [(0, 1), (7, 2)]);
    }

    #[test]
    fn text_pages_preserve_crlf_and_original_line_numbers() {
        let long_line = "s".repeat(PAGE_BYTES - 1);
        let text = format!("{long_line}\r\nlast");
        let mut pages = TextPages::new(text.clone().into());
        assert_eq!(pages.visible.as_ref(), long_line);
        assert!(pages.show(1));
        assert_eq!(pages.first_line, 1);
        assert_eq!(pages.visible.as_ref(), "\r\nlast");
        assert!(!pages.show(2));
        assert_eq!(pages.full.as_ref(), text);

        let mut pages = TextPages::new(format!("row\r\n{}", "row\n".repeat(PAGE_LINES)).into());
        assert!(pages.show(1));
        assert_eq!(pages.first_line, PAGE_LINES + 1);
        assert_eq!(pages.visible.as_ref(), "row\n");
        assert!(pages.show(0));
        assert_eq!(pages.first_line, 1);
    }

    #[test]
    fn showing_a_match_starts_at_its_line_and_preserves_earlier_content() {
        let original = "one\n二\nthree\n";
        let mut pages = TextPages::new(original.into());
        pages.show_line(2);
        assert_eq!(pages.first_line, 2);
        assert_eq!(pages.visible.as_ref(), "二\nthree\n");
        assert!(pages.show(0));
        assert_eq!(pages.visible.as_ref(), "one\n");
        assert_eq!(pages.full.as_ref(), original);
    }

    #[test]
    fn gutter_counts_logical_source_lines_after_jump_and_on_later_pages() {
        let mut pages = TextPages::new(format!("one\r\n\n{}", "    文🦀\r\n".repeat(698)).into());
        assert_eq!(pages.line_numbers(), 1..257);
        pages.show_line(350);
        assert_eq!(pages.line_numbers(), 350..513);
        assert!(pages.show(pages.page + 1));
        assert_eq!(pages.line_numbers(), 513..701);

        let pages = TextPages::new("\n\n".into());
        assert_eq!(pages.line_numbers(), 1..3);
        assert!(TextPages::new("".into()).line_numbers().is_empty());
    }

    #[test]
    fn gutter_keeps_the_same_number_for_a_line_split_across_pages() {
        let line = "文🦀".repeat(PAGE_BYTES);
        let full = format!("header\n{line}\r\nlast");
        let mut pages = TextPages::new(full.clone().into());
        assert_eq!(pages.line_numbers(), 1..2);
        let mut source = pages.visible.to_string();
        for page in 1..pages.count() {
            assert!(pages.show(page));
            let end = if page == pages.count() - 1 { 4 } else { 3 };
            assert_eq!(pages.line_numbers(), 2..end);
            source.push_str(&pages.visible);
        }
        assert_eq!(source, full);
    }

    #[test]
    fn horizontal_source_scroll_follows_arrow_direction_and_clamps_to_text() {
        assert_eq!(horizontal_offset("right", px(0.), px(80.)), px(-48.));
        assert_eq!(horizontal_offset("right", px(-48.), px(80.)), px(-80.));
        assert_eq!(horizontal_offset("left", px(-80.), px(80.)), px(-32.));
        assert_eq!(horizontal_offset("left", px(-32.), px(80.)), px(0.));
    }
}
