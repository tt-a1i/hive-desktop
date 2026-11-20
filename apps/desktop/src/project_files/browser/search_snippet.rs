use serde::Deserialize;
use serde_json::Value;
use std::ops::Range;

const MAX_SNIPPET_BYTES: usize = 192;
const MAX_RANGES: usize = 8;
const MAX_PATH_HIGHLIGHT_BYTES: usize = 16 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SearchSnippet {
    text: String,
    #[serde(default)]
    ranges: Vec<[usize; 2]>,
    #[serde(default)]
    leading_ellipsis: bool,
    #[serde(default)]
    trailing_ellipsis: bool,
}

impl SearchSnippet {
    pub(super) fn display(&self) -> Option<(String, Vec<Range<usize>>)> {
        if self.text.is_empty() || self.text.len() > MAX_SNIPPET_BYTES || !safe_label(&self.text) {
            return None;
        }
        let before = if self.leading_ellipsis { "…" } else { "" };
        let after = if self.trailing_ellipsis { "…" } else { "" };
        let ranges = validated_ranges(&self.text, &self.ranges).map_or_else(Vec::new, |ranges| {
            ranges
                .map(|range| range.start + before.len()..range.end + before.len())
                .collect()
        });
        Some((format!("{before}{}{after}", self.text), ranges))
    }
}

fn safe_label(text: &str) -> bool {
    !text
        .chars()
        .any(|ch| (ch.is_control() && ch != '\t') || matches!(ch, '\u{2028}' | '\u{2029}'))
}

fn validated_ranges<'a>(
    text: &str,
    ranges: &'a [[usize; 2]],
) -> Option<impl Iterator<Item = Range<usize>> + 'a> {
    let mut previous_end = 0;
    let valid = ranges.len() <= MAX_RANGES
        && ranges.iter().all(|&[start, end]| {
            let valid = start >= previous_end
                && start < end
                && end <= text.len()
                && text.is_char_boundary(start)
                && text.is_char_boundary(end);
            previous_end = end;
            valid
        });
    valid.then(|| ranges.iter().map(|&[start, end]| start..end))
}

pub(super) fn path_highlights(path: &str, ranges: Value) -> Vec<Range<usize>> {
    if !matches!(&ranges, Value::Array(entries) if entries.len() <= MAX_RANGES)
        || path.len() > MAX_PATH_HIGHLIGHT_BYTES
        || !safe_label(path)
    {
        return Vec::new();
    }
    let Ok(ranges) = serde_json::from_value::<Vec<[usize; 2]>>(ranges) else {
        return Vec::new();
    };
    validated_ranges(path, &ranges).map_or_else(Vec::new, Iterator::collect)
}

#[cfg(test)]
mod tests {
    use super::SearchSnippet;
    use serde_json::json;

    #[test]
    fn utf8_ranges_offset_into_the_displayed_excerpt_and_malformed_ranges_are_ignored() {
        let make = |ranges| {
            serde_json::from_value::<SearchSnippet>(json!({
                "text": "🙂 Cafe\u{301}", "ranges": ranges, "leadingEllipsis": true,
            }))
            .unwrap()
            .display()
            .unwrap()
        };
        let (text, ranges) = make(json!([[5, 11]]));
        assert_eq!(&text[ranges[0].clone()], "Cafe\u{301}");
        for ranges in [json!([[1, 4]]), json!([[5, 11], [5, 11]]), json!([[5, 12]])] {
            assert!(make(ranges).1.is_empty());
        }
        let blocked: SearchSnippet = serde_json::from_value(json!({"text": "a\u{1b}b"})).unwrap();
        assert!(blocked.display().is_none());
    }
}
