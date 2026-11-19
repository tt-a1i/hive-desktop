use super::GitWorkspace;
use crate::{language::tr, palette::Color, text_reading};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

const PAGE_BYTES: usize = 32 * 1024;
const PAGE_LINES: usize = 256;

pub(super) struct PatchReader {
    patch: SharedString,
    boundaries: Vec<usize>,
    page: usize,
    visible: SharedString,
    pub scroll: ScrollHandle,
}

impl PatchReader {
    pub fn new(patch: SharedString) -> Self {
        let boundaries = page_boundaries(&patch);
        let visible = if boundaries.len() == 2 {
            patch.clone()
        } else {
            SharedString::from(&patch[..boundaries[1]])
        };
        Self {
            patch,
            boundaries,
            page: 0,
            visible,
            scroll: ScrollHandle::new(),
        }
    }

    fn show(&mut self, page: usize) {
        self.page = page;
        self.visible =
            SharedString::from(&self.patch[self.boundaries[page]..self.boundaries[page + 1]]);
        self.scroll = ScrollHandle::new();
    }

    pub fn view(
        &self,
        id: &'static str,
        region: Stateful<Div>,
        reader: fn(&mut GitWorkspace) -> Option<&mut Self>,
        cx: &Context<GitWorkspace>,
    ) -> impl IntoElement {
        let page = self.page;
        let count = self.boundaries.len() - 1;
        let group_id = ElementId::from(id);
        v_flex()
            .min_w_0()
            .gap_1()
            .child(region.child(text_reading::selectable_text(
                format!("{id}-{page}"),
                self.visible.clone(),
            )))
            .when(count > 1, |view| {
                let label = format!("{} {} / {count}", tr("补丁页"), page + 1);
                view.child(
                    h_flex()
                        .flex_wrap()
                        .items_center()
                        .gap_1p5()
                        .child(
                            Button::new((group_id.clone(), "previous"))
                                .ghost()
                                .xsmall()
                                .label(tr("上一页补丁"))
                                .disabled(page == 0)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(reader) = reader(this).filter(|r| r.page > 0) {
                                        reader.show(reader.page - 1);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            div()
                                .id((group_id.clone(), "position"))
                                .role(Role::Status)
                                .aria_label(label.clone())
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(label),
                        )
                        .child(
                            Button::new((group_id.clone(), "next"))
                                .ghost()
                                .xsmall()
                                .label(tr("下一页补丁"))
                                .disabled(page + 1 == count)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(reader) =
                                        reader(this).filter(|r| r.page + 2 < r.boundaries.len())
                                    {
                                        reader.show(reader.page + 1);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            Button::new((group_id, "copy"))
                                .ghost()
                                .xsmall()
                                .label(tr("复制完整补丁"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(reader) = reader(this) {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            reader.patch.to_string(),
                                        ));
                                    }
                                })),
                        ),
                )
            })
    }
}

fn page_boundaries(text: &str) -> Vec<usize> {
    let mut boundaries = vec![0];
    let mut start = 0;
    while start < text.len() {
        let limit = text.floor_char_boundary((start + PAGE_BYTES).min(text.len()));
        let newlines = text.as_bytes()[start..limit]
            .iter()
            .enumerate()
            .filter_map(|(index, &byte)| (byte == b'\n').then_some(start + index + 1));
        // Keep whole lines where possible, but split one oversized line at
        // a UTF-8 boundary so that even minified files have a bounded layout.
        let end = {
            let mut last = None;
            let mut max_lines = false;
            for (line, end) in newlines.enumerate() {
                last = Some(end);
                if line + 1 == PAGE_LINES {
                    max_lines = true;
                    break;
                }
            }
            if max_lines || limit < text.len() {
                last.unwrap_or(limit)
            } else {
                limit
            }
        };
        boundaries.push(end);
        start = end;
    }
    if boundaries.len() == 1 {
        boundaries.push(0);
    }
    boundaries
}

#[cfg(test)]
mod tests {
    use super::{PAGE_BYTES, PAGE_LINES, page_boundaries};

    #[test]
    fn patch_pages_preserve_every_byte_and_bound_each_layout() {
        let patch = format!(
            "{}+{}\r\n-fin\n+tail",
            " context\r\n".repeat(PAGE_LINES + 13),
            "汉🦀".repeat(PAGE_BYTES),
        );
        let boundaries = page_boundaries(&patch);
        assert_eq!(boundaries[1], " context\r\n".len() * PAGE_LINES);
        let mut rebuilt = String::new();
        for bounds in boundaries.windows(2) {
            let page = &patch[bounds[0]..bounds[1]];
            assert!(!page.is_empty() && page.len() <= PAGE_BYTES);
            assert!(page.lines().count() <= PAGE_LINES);
            rebuilt.push_str(page);
        }
        assert_eq!(rebuilt.as_bytes(), patch.as_bytes());
        assert_eq!(page_boundaries(""), [0, 0]);
        assert_eq!(page_boundaries("one\ntwo"), [0, 7]);
    }
}
