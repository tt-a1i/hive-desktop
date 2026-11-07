use std::ops::Range;

/// Only the uncommitted composition belongs to the UI; the shell owns committed text.
#[derive(Default)]
pub(crate) struct Composition {
    pub text: String,
    pub marked: Option<Range<usize>>,
    pub selected: Range<usize>,
}
impl Composition {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn text_for(&self, range: Range<usize>) -> (String, Range<usize>) {
        let bytes = self.byte_range(range);
        let start = self.text[..bytes.start].encode_utf16().count();
        let end = start + self.text[bytes.clone()].encode_utf16().count();
        (self.text[bytes].into(), start..end)
    }
    pub fn mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selected.clone());
        let bytes = self.byte_range(range);
        let start = self.text[..bytes.start].encode_utf16().count();
        self.text.replace_range(bytes, text);
        let len = text.encode_utf16().count();
        self.marked = (!text.is_empty()).then_some(start..start + len);
        let selection = selected.unwrap_or(len..len);
        self.selected = start + selection.start.min(len)..start + selection.end.min(len);
    }
    fn byte_range(&self, range: Range<usize>) -> Range<usize> {
        let offset = |target| {
            let mut units = 0;
            let mut bytes = 0;
            for ch in self.text.chars() {
                if units >= target {
                    break;
                }
                units += ch.len_utf16();
                bytes += ch.len_utf8();
            }
            bytes
        };
        let start = offset(range.start);
        start..offset(range.end).max(start)
    }
}
