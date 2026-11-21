use crate::app::HiveApp;
use gpui_kit::{component::WindowExt, *};
use std::ops::Range;

const MAX_ORDERED_NUMBER: u32 = 999_999_999;

#[derive(Clone, Copy)]
enum Marker<'a> {
    Bullet(char),
    Ordered {
        digits: &'a str,
        delimiter: char,
        number: u32,
    },
}

struct ListItem<'a> {
    indent: &'a str,
    marker: Marker<'a>,
    task: bool,
    body: &'a str,
}

impl<'a> ListItem<'a> {
    fn parse(line: &'a str) -> Option<Self> {
        let text = line.trim_start_matches([' ', '\t']);
        let indent = &line[..line.len() - text.len()];
        let (marker, rest) = match text.as_bytes().first()? {
            byte @ (b'-' | b'*' | b'+') => (Marker::Bullet(char::from(*byte)), &text[1..]),
            _ => {
                let length = text.bytes().take_while(u8::is_ascii_digit).count();
                let delimiter = match text.as_bytes().get(length) {
                    Some(b'.') => '.',
                    Some(b')') => ')',
                    _ => return None,
                };
                if !(1..=9).contains(&length) {
                    return None;
                }
                let digits = &text[..length];
                (
                    Marker::Ordered {
                        digits,
                        delimiter,
                        number: digits.parse().ok()?,
                    },
                    &text[length + 1..],
                )
            }
        };
        if !rest.starts_with([' ', '\t']) {
            return None;
        }
        let body = rest.trim_start_matches([' ', '\t']);
        let task = matches!(body.as_bytes().get(..3), Some(b"[ ]" | b"[x]" | b"[X]"))
            && (body.len() == 3 || body[3..].starts_with([' ', '\t']));
        let body = if task {
            body[3..].trim_start_matches([' ', '\t'])
        } else {
            body
        };
        Some(Self {
            indent,
            marker,
            task,
            body,
        })
    }

    fn next_prefix(&self) -> Option<String> {
        let marker = match self.marker {
            Marker::Bullet(bullet) => bullet.to_string(),
            Marker::Ordered {
                digits,
                delimiter,
                number,
            } if number < MAX_ORDERED_NUMBER => {
                format!("{:0width$}{delimiter}", number + 1, width = digits.len())
            }
            Marker::Ordered { .. } => return None,
        };
        let task = if self.task { "[ ] " } else { "" };
        Some(format!("{}{marker} {task}", self.indent))
    }
}

#[derive(Clone, Copy)]
struct Fence<'a> {
    marker: u8,
    count: usize,
    rest: &'a str,
}

impl<'a> Fence<'a> {
    fn parse(line: &'a str) -> Option<Self> {
        let text = line.trim_start_matches([' ', '\t']);
        let marker = *text.as_bytes().first()?;
        if !matches!(marker, b'`' | b'~') {
            return None;
        }
        let count = text.bytes().take_while(|byte| *byte == marker).count();
        (count >= 3).then(|| Self {
            marker,
            count,
            rest: &text[count..],
        })
    }

    fn opening(line: &'a str) -> Option<Self> {
        Self::parse(line)
            .or_else(|| Self::parse(ListItem::parse(line)?.body))
            .filter(|fence| fence.marker != b'`' || !fence.rest.contains('`'))
    }

    fn closes(self, opening: Self) -> bool {
        self.marker == opening.marker && self.count >= opening.count && self.rest.trim().is_empty()
    }
}

fn in_code_block(before: &str, line: &str) -> bool {
    let mut active = None;
    for previous in before.lines() {
        if let Some(opening) = active {
            if Fence::parse(previous).is_some_and(|fence| fence.closes(opening)) {
                active = None;
            }
        } else {
            active = Fence::opening(previous);
        }
    }
    active.is_some() || Fence::opening(line).is_some()
}

#[derive(Debug, PartialEq, Eq)]
struct ListEdit {
    range: Range<usize>,
    replacement: String,
    caret: usize,
}

fn continuation(text: &str, caret: usize) -> Option<ListEdit> {
    let before = text.get(..caret)?;
    let line_start = before.rfind('\n').map_or(0, |offset| offset + 1);
    let line_break = text[caret..]
        .find('\n')
        .map_or(text.len(), |offset| caret + offset);
    let raw = &text[line_start..line_break];
    let line = raw.strip_suffix('\r').unwrap_or(raw);
    let item = ListItem::parse(line)?;
    if caret < line_start + line.len() - item.body.len()
        || caret > line_start + line.len()
        || in_code_block(&text[..line_start], line)
    {
        return None;
    }
    if item.body.trim().is_empty() {
        return Some(ListEdit {
            range: line_start..line_start + line.len(),
            replacement: String::new(),
            caret: line_start,
        });
    }

    let newline = if raw.ends_with('\r') || text[..line_start].ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let replacement = format!("{newline}{}", item.next_prefix()?);
    let mut edit = ListEdit {
        range: caret..caret,
        caret: caret + replacement.len(),
        replacement,
    };
    renumber_following(text, line_break, &item, &mut edit);
    Some(edit)
}

fn renumber_following(text: &str, line_break: usize, item: &ListItem<'_>, edit: &mut ListEdit) {
    let Marker::Ordered {
        number, delimiter, ..
    } = item.marker
    else {
        return;
    };
    let mut expected = number + 1;
    let mut line_start = line_break + 1;
    for raw in text
        .get(line_start..)
        .unwrap_or_default()
        .split_inclusive('\n')
    {
        let start = line_start;
        line_start += raw.len();
        let line = raw.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() || Fence::opening(line).is_some() {
            break;
        }
        let unindented = line.trim_start_matches([' ', '\t']);
        let indent = &line[..line.len() - unindented.len()];
        if indent != item.indent && indent.starts_with(item.indent) {
            continue;
        }
        let Some(next) = ListItem::parse(line) else {
            break;
        };
        let Marker::Ordered {
            digits,
            delimiter: next_delimiter,
            number,
        } = next.marker
        else {
            break;
        };
        // Explicitly repeated/nonconsecutive numbering or a different list is
        // the author's choice. Shift only the existing sequential siblings.
        if indent != item.indent
            || next_delimiter != delimiter
            || number != expected
            || number >= MAX_ORDERED_NUMBER
        {
            break;
        }
        let digits_start = start + indent.len();
        edit.replacement
            .push_str(&text[edit.range.end..digits_start]);
        edit.replacement
            .push_str(&format!("{:0width$}", number + 1, width = digits.len()));
        edit.range.end = digits_start + digits.len();
        expected += 1;
    }
}

impl HiveApp {
    pub(super) fn continue_composer_list(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.command_picker.open
            || self.mention_picker.open
            || self.draft_target != self.draft_owner
            || self.conversation_is_read_only()
            || window.has_active_dialog(cx)
            || !self.input.read(cx).focus_handle(cx).is_focused(window)
        {
            return false;
        }

        self.input.update(cx, |input, cx| {
            if !input.is_editable()
                || !input.selected_range().is_empty()
                || input.marked_text_range(window, cx).is_some()
            {
                return false;
            }
            let text = input.value();
            let Some(edit) = continuation(&text, input.cursor()) else {
                return false;
            };
            let start = text[..edit.range.start].encode_utf16().count();
            let end = start + text[edit.range].encode_utf16().count();
            // Use the input engine's range edit so a single undo restores the
            // prior text and original caret, including when exiting a list.
            input.replace_text_in_range(Some(start..end), &edit.replacement, window, cx);
            if input.cursor() != edit.caret {
                input.set_selected_range(edit.caret..edit.caret, cx);
            }
            true
        })
    }
}

#[cfg(test)]
mod tests {
    use super::continuation;

    fn apply(before: &str, expected: &str) {
        let caret = before.find('|').unwrap();
        let mut text = before.replacen('|', "", 1);
        let edit = continuation(&text, caret).unwrap();
        text.replace_range(edit.range, &edit.replacement);
        text.insert(edit.caret, '|');
        assert_eq!(text, expected);
    }

    #[test]
    fn continues_bullets_and_tasks_at_the_utf8_caret() {
        apply(
            "前言\n  * 内容🐝|后缀\n末尾",
            "前言\n  * 内容🐝\n  * |后缀\n末尾",
        );
        apply("\t+ [X] 完成|", "\t+ [X] 完成\n\t+ [ ] |");
        apply("007) [x] done|", "007) [x] done\n008) [ ] |");
        apply("- 甲\r\n- 乙|", "- 甲\r\n- 乙\r\n- |");
    }

    #[test]
    fn shifts_only_sequential_siblings_after_an_ordered_insert() {
        apply(
            "1. 甲|\n   - 子项\n2. 乙\n3. 丙\n8. 我选的数字\n9. 保留",
            "1. 甲\n2. |\n   - 子项\n3. 乙\n4. 丙\n8. 我选的数字\n9. 保留",
        );
        apply("01. 保留|\n01. 另起号码", "01. 保留\n02. |\n01. 另起号码");
    }

    #[test]
    fn exits_an_empty_marker_without_removing_neighboring_text() {
        apply("- 前一项\n- [ ] |\n后面", "- 前一项\n|\n后面");
        apply("  2. \t|", "|");
    }

    #[test]
    fn leaves_prose_fenced_code_and_unrecognized_markers_to_native_newline() {
        for value in [
            "普通文字",
            "-file",
            "1.5",
            "> 文本",
            "```shell\n- foo",
            "~~~~\n~~~\n1. literal",
            "- ```ts\n  - string",
            "- ```js",
        ] {
            assert_eq!(continuation(value, value.len()), None, "{value:?}");
        }
        assert_eq!(continuation("-🐝", 2), None);
        apply(
            "```\n- literal\n```\n- list|",
            "```\n- literal\n```\n- list\n- |",
        );
    }
}
