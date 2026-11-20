use super::ExpandedFilePreview;
use crate::{
    language::tr,
    palette::{self, Color},
    text_reading::SourceMatch,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        *,
    },
    *,
};
use std::{cell::Cell, ops::Range, rc::Rc};

const FIND_LABEL: &str = "在此文件中查找";
pub(super) const KEY_CONTEXT: &str = "HiveExpandedFileFind";

pub(super) struct ReaderFind {
    input: Entity<InputState>,
    source: FoldedSource,
    query: String,
    matches: Vec<Range<usize>>,
    selected: usize,
    reveal: Rc<Cell<bool>>,
    _subscription: Subscription,
}

impl ReaderFind {
    fn request_reveal(&mut self) {
        self.reveal.set(false);
        self.reveal = Rc::new(Cell::new(true));
    }

    fn source_match(&self, scroll: &ScrollHandle, wrap: bool) -> Option<SourceMatch> {
        Some(SourceMatch {
            range: self.matches.get(self.selected)?.clone(),
            scroll: scroll.clone(),
            reveal: self.reveal.clone(),
            horizontal: !wrap,
        })
    }
}

impl Drop for ReaderFind {
    fn drop(&mut self) {
        self.reveal.set(false);
    }
}

/// Lowercase the snapshot once and retain character boundaries when folding changes
/// byte offsets, so selection and scrolling always use the original UTF-8 bytes.
struct FoldedSource {
    text: String,
    boundaries: Vec<(usize, usize)>,
}

impl FoldedSource {
    fn new(source: &str) -> Self {
        if source.is_ascii() {
            return Self {
                text: source.to_ascii_lowercase(),
                boundaries: Vec::new(),
            };
        }
        let mut text = String::with_capacity(source.len());
        let mut boundaries = Vec::new();
        for (offset, character) in source.char_indices() {
            boundaries.push((text.len(), offset));
            text.extend(character.to_lowercase());
        }
        boundaries.push((text.len(), source.len()));
        Self { text, boundaries }
    }

    fn find(&self, query: &str) -> Vec<Range<usize>> {
        if query.is_empty() {
            return Vec::new();
        }
        let query: String = query.chars().flat_map(char::to_lowercase).collect();
        let hits = self.text.match_indices(&query);
        if self.boundaries.is_empty() {
            return hits.map(|(start, hit)| start..start + hit.len()).collect();
        }
        let mut starts = self.boundaries.iter().peekable();
        let mut ends = self.boundaries.iter().peekable();
        hits.filter_map(|(start, hit)| {
            let end = start + hit.len();
            // A single character can lowercase to several characters. Retain
            // complete source characters so highlights never split their bytes.
            while starts.peek().is_some_and(|&&(folded, _)| folded < start) {
                starts.next();
            }
            while ends.peek().is_some_and(|&&(folded, _)| folded < end) {
                ends.next();
            }
            let &&(folded_start, source_start) = starts.peek()?;
            let &&(folded_end, source_end) = ends.peek()?;
            (folded_start == start && folded_end == end).then_some(source_start..source_end)
        })
        .collect()
    }
}

impl ExpandedFilePreview {
    pub(super) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.current(cx)
            || self
                .markdown
                .as_ref()
                .is_some_and(super::MarkdownPreview::active)
        {
            cx.propagate();
            return;
        }
        let Some(text) = self.text.as_ref().filter(|text| !text.full.is_empty()) else {
            cx.propagate();
            return;
        };
        if self.find.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(tr(FIND_LABEL)));
            let subscription =
                cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.update_find(cx);
                    }
                });
            self.find = Some(ReaderFind {
                input,
                source: FoldedSource::new(&text.full),
                query: String::new(),
                matches: Vec::new(),
                selected: 0,
                reveal: Rc::new(Cell::new(false)),
                _subscription: subscription,
            });
        }
        let input = self.find.as_ref().expect("find opened").input.clone();
        let reader = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let active = reader.upgrade().is_some_and(|reader| {
                let reader = reader.read(cx);
                reader.current(cx)
                    && reader.find.as_ref().is_some_and(|find| find.input == input)
                    && reader
                        .dialog_focus
                        .as_ref()
                        .is_some_and(|dialog| dialog.contains_focused(window, cx))
            });
            if active {
                input.update(cx, |input, cx| {
                    input.focus_handle(cx).focus(window, cx);
                    input.select_all(window, cx);
                });
            }
        });
        cx.notify();
    }

    fn update_find(&mut self, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        let Some(find) = &mut self.find else {
            return;
        };
        find.query = find.input.read(cx).value().trim().to_owned();
        find.matches = find.source.find(&find.query);
        find.selected = 0;
        self.show_find_match(cx);
    }

    fn show_find_match(&mut self, cx: &mut Context<Self>) {
        let (Some(find), Some(text)) = (&mut self.find, &mut self.text) else {
            return;
        };
        if let Some(range) = find.matches.get(find.selected) {
            if text.show_match(range) {
                self.scroll = ScrollHandle::new();
            }
            find.request_reveal();
        } else {
            find.reveal.set(false);
        }
        cx.notify();
    }

    fn find_composing(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.find.as_ref().is_some_and(|find| {
            find.input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            })
        })
    }

    fn step_find(&mut self, previous: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.current(cx) || self.find_composing(window, cx) {
            cx.propagate();
            return;
        }
        let Some(find) = &mut self.find else {
            return;
        };
        let count = find.matches.len();
        if count > 0 {
            find.selected = if previous {
                (find.selected + count - 1) % count
            } else {
                (find.selected + 1) % count
            };
            self.show_find_match(cx);
        }
    }

    pub(super) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            cx.propagate();
        } else if !self.find_composing(window, cx) {
            self.find = None;
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    pub(super) fn clear_find_reveal(&self) {
        if let Some(find) = &self.find {
            find.reveal.set(false);
        }
    }

    pub(super) fn refresh_find_reveal(&mut self) {
        if let Some(find) = &mut self.find {
            find.request_reveal();
        }
    }

    pub(super) fn find_source_match(&self, wrap: bool) -> Option<SourceMatch> {
        self.find.as_ref()?.source_match(&self.scroll, wrap)
    }

    pub(super) fn find_bar(&self, cx: &Context<Self>) -> Option<Stateful<Div>> {
        let find = self.find.as_ref()?;
        let status = if find.query.is_empty() {
            String::new()
        } else if find.matches.is_empty() {
            tr("未找到匹配项").to_owned()
        } else {
            tr("第 {current} 项，共 {total} 项")
                .replace("{current}", &(find.selected + 1).to_string())
                .replace("{total}", &find.matches.len().to_string())
        };
        Some(
            h_flex()
                .id("file-reader-find")
                .gap_2()
                .items_center()
                .on_action(
                    cx.listener(|this, _: &crate::NextConversationMatch, window, cx| {
                        this.step_find(false, window, cx);
                    }),
                )
                .on_action(
                    cx.listener(|this, _: &crate::PreviousConversationMatch, window, cx| {
                        this.step_find(true, window, cx);
                    }),
                )
                .child(
                    div().min_w_0().flex_1().key_context(KEY_CONTEXT).child(
                        Input::new(&find.input)
                            .small()
                            .cleanable(true)
                            .aria_label(tr(FIND_LABEL)),
                    ),
                )
                .child(
                    div()
                        .id("file-reader-find-count")
                        .role(Role::Status)
                        .aria_label(status.clone())
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(status),
                )
                .child(self.find_step_button(true, find.matches.is_empty(), cx))
                .child(self.find_step_button(false, find.matches.is_empty(), cx))
                .child(
                    Button::new("file-reader-find-close")
                        .custom(palette::quiet_icon_variant(cx))
                        .compact()
                        .xsmall()
                        .size(px(24.))
                        .icon(Glyph::X)
                        .accessibility_label(tr("关闭文件查找"))
                        .tooltip(tr("关闭文件查找"))
                        .on_click(cx.listener(|this, _, window, cx| this.close_find(window, cx))),
                ),
        )
    }

    fn find_step_button(&self, previous: bool, disabled: bool, cx: &Context<Self>) -> Button {
        let (id, icon, label) = if previous {
            (
                "file-reader-find-previous",
                Glyph::ChevronUp,
                tr("上一处匹配"),
            )
        } else {
            (
                "file-reader-find-next",
                Glyph::ChevronDown,
                tr("下一处匹配"),
            )
        };
        Button::new(id)
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .size(px(24.))
            .icon(icon)
            .accessibility_label(label)
            .tooltip(label)
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| this.step_find(previous, window, cx)))
    }
}

#[cfg(test)]
mod tests {
    use super::FoldedSource;

    #[test]
    fn file_find_preserves_original_ranges_across_case_and_multibyte_characters() {
        let source = "🦀 Éclair ÉCLAIR\r\n🧪 Tekİn KELVIN";
        let folded = FoldedSource::new(source);
        let words = |query: &str| {
            folded
                .find(query)
                .into_iter()
                .map(|range| &source[range])
                .collect::<Vec<_>>()
        };
        assert_eq!(words("éclAIr"), ["Éclair", "ÉCLAIR"]);
        assert_eq!(words("kelvin"), ["KELVIN"]);
        assert_eq!(words("İ"), ["İ"]);
        assert!(FoldedSource::new("İ").find("i").is_empty());
        assert_eq!(FoldedSource::new("İi").find("i"), [2..3]);
        assert!(words("").is_empty());
        assert_eq!(FoldedSource::new("a A a").find("A"), [0..1, 2..3, 4..5]);
    }
}
