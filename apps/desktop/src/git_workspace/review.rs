use crate::diffs_settings::parse_hunk_coords;
use gpui_kit::component::diff::{DiffLineRange, DiffSide};

const MAX_PATCH_BYTES: usize = 12_000;
const MAX_PATCH_LINES: usize = 120;
const CONTEXT_ROWS: usize = 2;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ReviewError {
    Unavailable,
    TooLarge,
}

struct Hunk<'a> {
    rows: Vec<Row<'a>>,
}

struct Row<'a> {
    text: &'a str,
    old: usize,
    new: usize,
    no_newline: Option<&'a str>,
}

impl Row<'_> {
    fn in_original(&self) -> bool {
        !self.text.starts_with('+')
    }

    fn in_modified(&self) -> bool {
        !self.text.starts_with('-')
    }

    fn on_side(&self, side: DiffSide) -> bool {
        match side {
            DiffSide::Original => self.in_original(),
            DiffSide::Modified => self.in_modified(),
        }
    }

    fn at(&self, side: DiffSide, line: usize) -> bool {
        match side {
            DiffSide::Original => self.in_original() && self.old == line,
            DiffSide::Modified => self.in_modified() && self.new == line,
        }
    }
}

#[derive(Clone, Copy)]
struct Anchor {
    hunk: usize,
    row: usize,
    side: DiffSide,
}

fn read_hunks(patch: &str) -> Option<Vec<Hunk<'_>>> {
    let mut hunks = Vec::new();
    // Split on LF alone so the CR in a changed CRLF source line is preserved.
    let mut lines = patch.split_terminator('\n').peekable();
    while let Some(header) = lines.next() {
        if header.starts_with("diff --git ") && !hunks.is_empty() {
            // The selected file comparison is a single-file patch.
            return None;
        }
        if !header.starts_with("@@ ") {
            continue;
        }
        let (old_start, mut old_count, new_start, mut new_count, _) = parse_hunk_coords(header)?;
        let mut old = old_start.checked_add(usize::from(old_count == 0))?;
        let mut new = new_start.checked_add(usize::from(new_count == 0))?;
        let mut rows = Vec::new();
        while old_count != 0 || new_count != 0 {
            let text = lines.next()?;
            if !matches!(text.as_bytes().first(), Some(b' ' | b'+' | b'-')) {
                return None;
            }
            let no_newline = lines.next_if(|line| line.starts_with("\\ No newline at end of file"));
            let row = Row {
                text,
                old,
                new,
                no_newline,
            };
            if row.in_original() {
                old_count = old_count.checked_sub(1)?;
                old = old.checked_add(1)?;
            }
            if row.in_modified() {
                new_count = new_count.checked_sub(1)?;
                new = new.checked_add(1)?;
            }
            rows.push(row);
        }
        hunks.push(Hunk { rows });
    }
    Some(hunks)
}

/// Extract a patch around the selected source coordinates. Hunk headings are
/// recalculated for the excerpt and source lines retain their original bytes.
pub(super) fn focused_patch(patch: &str, range: &DiffLineRange) -> Result<String, ReviewError> {
    let hunks = read_hunks(patch).ok_or(ReviewError::Unavailable)?;
    let find = |side, line| {
        hunks.iter().enumerate().find_map(|(hunk, body)| {
            body.rows
                .iter()
                .position(|row| row.at(side, line))
                .map(|row| Anchor { hunk, row, side })
        })
    };
    let start = find(range.side(), range.start()).ok_or(ReviewError::Unavailable)?;
    let end = find(range.end_side(), range.end()).ok_or(ReviewError::Unavailable)?;
    let (first, last) = if (start.hunk, start.row) <= (end.hunk, end.row) {
        (start, end)
    } else {
        (end, start)
    };
    render_excerpt(&hunks, first, last, CONTEXT_ROWS)
        .or_else(|| render_excerpt(&hunks, first, last, 0))
        .ok_or(ReviewError::TooLarge)
}

fn render_excerpt(
    hunks: &[Hunk<'_>],
    first: Anchor,
    last: Anchor,
    context: usize,
) -> Option<String> {
    let mut patch = String::new();
    let mut line_count = 0;
    for (index, hunk) in hunks
        .iter()
        .enumerate()
        .take(last.hunk + 1)
        .skip(first.hunk)
    {
        // A replacement block lists every deletion before its additions.
        // Its adjacent opposite-side row may be far from the selected code.
        let start = if index == first.hunk {
            first.row
                - hunk.rows[..first.row]
                    .iter()
                    .rev()
                    .take(context)
                    .take_while(|row| row.on_side(first.side))
                    .count()
        } else {
            0
        };
        let end = if index == last.hunk {
            let after = last.row + 1;
            after
                + hunk.rows[after..]
                    .iter()
                    .take(context)
                    .take_while(|row| row.on_side(last.side))
                    .count()
        } else {
            hunk.rows.len()
        };
        let rows = &hunk.rows[start..end];
        let Some(first_row) = rows.first() else {
            continue;
        };
        line_count += 1 + rows.len() + rows.iter().filter(|row| row.no_newline.is_some()).count();
        if line_count > MAX_PATCH_LINES {
            return None;
        }
        let old_count = rows.iter().filter(|row| row.in_original()).count();
        let new_count = rows.iter().filter(|row| row.in_modified()).count();
        let old_start = first_row.old - usize::from(old_count == 0);
        let new_start = first_row.new - usize::from(new_count == 0);
        patch.push_str(&format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
        ));
        for row in rows {
            for line in std::iter::once(row.text).chain(row.no_newline) {
                if patch.len() + line.len() + 1 > MAX_PATCH_BYTES {
                    return None;
                }
                patch.push_str(line);
                patch.push('\n');
            }
        }
    }
    Some(patch)
}

pub(super) fn file_excerpt(patch: &str) -> (&str, bool) {
    let mut end = 0;
    for line in patch.split_inclusive('\n').take(MAX_PATCH_LINES) {
        if end + line.len() > MAX_PATCH_BYTES {
            break;
        }
        end += line.len();
    }
    (&patch[..end], end < patch.len())
}

pub(super) fn fenced_patch(patch: &str) -> String {
    let longest_run = patch.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest_run.max(2) + 1);
    let end = if patch.ends_with('\n') { "" } else { "\n" };
    format!("{fence}diff\n{patch}{end}{fence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_selected_late_hunk_with_exact_original_bytes_and_locations() {
        let patch = format!(
            "@@ -1,30 +1,30 @@\n{}@@ -60,3 +62,3 @@\n before\n-\t旧  \r\n+\t新  \r\n after\n",
            " earlier\n".repeat(30)
        );
        let deleted = DiffLineRange::new("after", DiffSide::Original, 61, 61);
        let added = DiffLineRange::new("after", DiffSide::Modified, 63, 63);
        assert_eq!(
            focused_patch(&patch, &deleted).unwrap(),
            "@@ -60,2 +62,1 @@\n before\n-\t旧  \r\n",
        );
        assert_eq!(
            focused_patch(&patch, &added).unwrap(),
            "@@ -62,1 +63,2 @@\n+\t新  \r\n after\n",
        );
    }

    #[test]
    fn added_range_does_not_quote_distant_deletions_at_a_replacement_boundary() {
        let patch = "@@ -4,3 +4,3 @@\n-Old 4\n-Old 5\n-Old 6\n+New 4\n+New 5\n+New 6\n";
        let range = DiffLineRange::new("after", DiffSide::Modified, 4, 5);
        assert_eq!(
            focused_patch(patch, &range).unwrap(),
            "@@ -6,0 +4,3 @@\n+New 4\n+New 5\n+New 6\n",
        );
    }

    #[test]
    fn supports_unified_cross_side_selection_and_eof_markers() {
        let patch = "@@ -0,0 +1 @@\n+first\n@@ -5 +6 @@\n-last\n\\ No newline at end of file\n+new last\n\\ No newline at end of file\n";
        let range =
            DiffLineRange::new("after", DiffSide::Original, 5, 6).with_end_side(DiffSide::Modified);
        assert_eq!(
            focused_patch(patch, &range).unwrap(),
            "@@ -5,1 +6,1 @@\n-last\n\\ No newline at end of file\n+new last\n\\ No newline at end of file\n"
        );
        let first = DiffLineRange::new("after", DiffSide::Modified, 1, 1);
        assert_eq!(
            focused_patch(patch, &first).unwrap(),
            "@@ -0,0 +1,1 @@\n+first\n"
        );
    }

    #[test]
    fn keeps_the_full_selection_or_reports_why_it_cannot() {
        let huge = "🦀".repeat(MAX_PATCH_BYTES / 4);
        let patch = format!("@@ -1 +1,2 @@\n-old\n+{huge}\n+selected\n");
        let second = DiffLineRange::new("after", DiffSide::Modified, 2, 2);
        assert_eq!(
            focused_patch(&patch, &second).unwrap(),
            "@@ -1,0 +2,1 @@\n+selected\n"
        );
        let both = DiffLineRange::new("after", DiffSide::Modified, 1, 2);
        assert_eq!(focused_patch(&patch, &both), Err(ReviewError::TooLarge));
        let absent = DiffLineRange::new("after", DiffSide::Modified, 3, 3);
        assert_eq!(
            focused_patch(&patch, &absent),
            Err(ReviewError::Unavailable)
        );
        assert_eq!(file_excerpt(&patch), ("@@ -1 +1,2 @@\n-old\n", true));
    }

    #[test]
    fn preserves_context_containing_markdown_fences() {
        let patch = "@@ -22,1 +22,2 @@\n ```\n+added\n";
        assert_eq!(
            fenced_patch(patch),
            "````diff\n@@ -22,1 +22,2 @@\n ```\n+added\n````",
        );
    }
}
