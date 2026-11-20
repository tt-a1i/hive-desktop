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

pub(crate) fn numbered_source(id: &'static str, page: &TextPages, cx: &App) -> Div {
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
                .flex_1()
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
                        }),
                ),
        )
}

/// Use the same text layout as the selectable source so wrapping and font metrics
/// agree. Only paint the gutter, leaving the raw source as one selection run.
struct SourceLineNumbers {
    source: StyledText,
    numbers: std::ops::Range<usize>,
    color: Hsla,
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
    use super::{PAGE_BYTES, PAGE_LINES, TextPages, page_boundaries};

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
}
