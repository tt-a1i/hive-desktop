use serde::Deserialize;
use std::ops::Range;

const MAX_SNIPPET_BYTES: usize = 192;
const MAX_RANGES: usize = 8;

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
        if self.text.is_empty()
            || self.text.len() > MAX_SNIPPET_BYTES
            || self
                .text
                .chars()
                .any(|ch| (ch.is_control() && ch != '\t') || matches!(ch, '\u{2028}' | '\u{2029}'))
        {
            return None;
        }
        let before = if self.leading_ellipsis { "…" } else { "" };
        let after = if self.trailing_ellipsis { "…" } else { "" };
        let mut previous_end = 0;
        let is_valid = self.ranges.len() <= MAX_RANGES
            && self.ranges.iter().all(|&[start, end]| {
                let valid = start >= previous_end
                    && start < end
                    && end <= self.text.len()
                    && self.text.is_char_boundary(start)
                    && self.text.is_char_boundary(end);
                previous_end = end;
                valid
            });
        let ranges = if is_valid {
            self.ranges
                .iter()
                .map(|&[start, end]| start + before.len()..end + before.len())
                .collect()
        } else {
            Vec::new()
        };
        Some((format!("{before}{}{after}", self.text), ranges))
    }
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
