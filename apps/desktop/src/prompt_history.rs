use crate::{app::HiveApp, quote_source::body_without_quote_source};
use gpui_kit::{
    component::{
        WindowExt,
        input::{BufferPoint, MoveDown, MoveUp, RopeExt, TextareaState},
    },
    *,
};
use std::borrow::Cow;

#[derive(Clone, Copy)]
enum Direction {
    Older,
    Newer,
}

struct Entry<'a> {
    id: &'a str,
    text: Cow<'a, str>,
}

struct Position {
    view: u64,
    id: String,
    text: String,
}

#[derive(Default)]
pub(crate) struct PromptHistory {
    position: Option<Position>,
}

impl PromptHistory {
    fn active(&self, view: u64, text: &str) -> Option<&Position> {
        self.position
            .as_ref()
            .filter(|entry| entry.view == view && entry.text == text)
    }

    pub(crate) fn on_change(&mut self, view: u64, text: &str) -> bool {
        let recalled = self.active(view, text).is_some();
        if !recalled {
            self.position = None;
        }
        recalled
    }

    fn step(
        &mut self,
        direction: Direction,
        view: u64,
        current: &str,
        entries: &[Entry<'_>],
    ) -> Option<String> {
        let index = self.active(view, current).and_then(|active| {
            entries
                .iter()
                .position(|entry| entry.id == active.id && entry.text == active.text)
        });
        let next = match (direction, index) {
            (Direction::Older, Some(index)) => entries.get(index.checked_sub(1)?),
            (Direction::Older, None) if current.is_empty() => entries.last(),
            (Direction::Newer, Some(index)) => {
                if index + 1 == entries.len() {
                    self.position = None;
                    return Some(String::new());
                }
                entries.get(index + 1)
            }
            _ => None,
        }?;
        let text = next.text.to_string();
        self.position = Some(Position {
            view,
            id: next.id.to_owned(),
            text: text.clone(),
        });
        Some(text)
    }
}

fn recallable_text<'a>(
    session: &str,
    explicit_user: bool,
    source: Option<&str>,
    body: &'a str,
) -> Option<Cow<'a, str>> {
    if !explicit_user || source.is_some_and(|origin| origin != session) {
        return None;
    }
    let text = body_without_quote_source(body);
    (!text.trim().is_empty()).then_some(text)
}

fn is_visual_edge(input: &TextareaState, direction: Direction) -> bool {
    if !input.selected_range().is_empty() {
        return false;
    }
    let (map, cursor) = (input.display_map(), input.cursor());
    let rope = map.text();
    let rows = map.display_row_count();
    if rows == 0 || cursor > rope.len() {
        return false;
    }
    let point = rope.offset_to_point(cursor);
    let display = map.buffer_pos_to_display_pos(BufferPoint::new(point.row, point.column));
    let mut row = display.row;
    // At a soft wrap, one byte offset can be drawn at the end of one row or
    // the start of the next. Use the painted caret to disambiguate that offset.
    if point.column > 0 && display.col == 0 && row > 0 {
        let Some(geometry) = input.prepaint_caret_geometry() else {
            return false;
        };
        let caret = geometry.cursor_bounds();
        let line = geometry.line_height();
        let offset = caret.top() - geometry.input_bounds().top() - (line - caret.size.height) / 2.;
        let line = f32::from(line);
        if line <= 0. {
            return false;
        }
        let painted = (f32::from(offset) / line).round() as usize;
        if painted != row && painted != row - 1 {
            return false;
        }
        row = painted;
    }
    match direction {
        Direction::Older => row == 0,
        Direction::Newer => row + 1 == rows,
    }
}

impl HiveApp {
    pub(crate) fn composer_up(
        &mut self,
        action: &MoveUp,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_picker.open {
            self.command_up(action, window, cx);
        } else if self.recall_prompt(Direction::Older, window, cx) {
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }

    pub(crate) fn composer_down(
        &mut self,
        action: &MoveDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_picker.open {
            self.command_down(action, window, cx);
        } else if self.recall_prompt(Direction::Newer, window, cx) {
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }

    fn recall_prompt(
        &mut self,
        direction: Direction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let (Some(session), Some(key)) = (self.session.as_ref(), self.draft_target.as_ref()) else {
            return false;
        };
        if self.workspace.as_ref() != Some(&session.workspace_id)
            || key.strip_prefix("session:") != Some(session.id.as_str())
            || self.draft_owner.as_ref() != Some(key)
            || !self.conversation_ready
            || !self.viewing_conversation()
            || self.conversation_is_read_only()
            || !self.workspace_writes_allowed()
            || self.loading
            || self.closing
            || window.has_active_dialog(cx)
            || !self.input.read(cx).focus_handle(cx).is_focused(window)
            || self
                .attachments
                .drafts
                .get(key)
                .is_some_and(|refs| !refs.is_empty())
            || !self.attachments.ready(Some(key))
        {
            return false;
        }

        let current = self.input.read(cx).value();
        if (self
            .prompt_history
            .active(self.open_request, &current)
            .is_none()
            && (!current.is_empty() || matches!(direction, Direction::Newer)))
            || self.input.update(cx, |state, cx| {
                state.marked_text_range(window, cx).is_some()
            })
            || (!current.is_empty() && !is_visual_edge(self.input.read(cx), direction))
        {
            return false;
        }

        let mut entries: Vec<Entry<'_>> = Vec::new();
        for message in &self.messages {
            let Some(text) = recallable_text(
                &session.id,
                message.explicit_user && message.role == "user",
                message.source_session_id.as_deref(),
                &message.text,
            ) else {
                continue;
            };
            if entries.last().is_some_and(|previous| previous.text == text) {
                entries.pop();
            }
            entries.push(Entry {
                id: &message.id,
                text,
            });
        }
        let Some(text) = self
            .prompt_history
            .step(direction, self.open_request, &current, &entries)
        else {
            return false;
        };
        self.input.update(cx, |state, cx| {
            let end = text.len();
            state.replace_all(text, window, cx);
            state.set_selected_range(end..end, cx);
        });
        cx.notify();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, Entry, PromptHistory, recallable_text};
    use std::borrow::Cow;

    #[test]
    fn recalls_loaded_prompts_by_id_and_returns_to_empty() {
        let entries = [
            Entry {
                id: "old",
                text: Cow::Borrowed("第一条 🐝"),
            },
            Entry {
                id: "new",
                text: Cow::Borrowed("second"),
            },
        ];
        let mut history = PromptHistory::default();
        assert_eq!(
            history.step(Direction::Older, 1, "", &entries).as_deref(),
            Some("second")
        );
        assert_eq!(
            history
                .step(Direction::Older, 1, "second", &entries)
                .as_deref(),
            Some("第一条 🐝")
        );
        assert_eq!(
            history.step(Direction::Older, 1, "第一条 🐝", &entries),
            None
        );
        assert_eq!(
            history
                .step(Direction::Newer, 1, "第一条 🐝", &entries)
                .as_deref(),
            Some("second")
        );
        assert_eq!(
            history
                .step(Direction::Newer, 1, "second", &entries)
                .as_deref(),
            Some("")
        );
        assert_eq!(history.step(Direction::Newer, 1, "", &entries), None);
    }

    #[test]
    fn text_edits_and_view_changes_end_recall() {
        let mut history = PromptHistory::default();
        let entries = [Entry {
            id: "new",
            text: Cow::Borrowed("exact"),
        }];
        assert_eq!(history.step(Direction::Older, 1, "typed", &entries), None);
        history.step(Direction::Older, 1, "", &entries).unwrap();
        assert!(history.on_change(1, "exact"));
        assert!(!history.on_change(1, "edited"));
        assert_eq!(history.step(Direction::Newer, 1, "exact", &entries), None);
        history.step(Direction::Older, 1, "", &entries).unwrap();
        assert_eq!(history.step(Direction::Newer, 2, "exact", &entries), None);
    }

    #[test]
    fn recall_requires_explicit_current_conversation_text() {
        assert_eq!(
            recallable_text("this", true, Some("this"), "中文\n").as_deref(),
            Some("中文\n")
        );
        assert!(recallable_text("this", true, Some("other"), "foreign").is_none());
        assert!(recallable_text("this", false, None, "native or automated").is_none());
        assert!(recallable_text("this", true, None, "\n \t").is_none());
        let marker =
            crate::quote_source::assistant_quote_source_line("workspace", "this", "message")
                .unwrap();
        assert_eq!(
            recallable_text("this", true, None, &format!("text\n{marker}\n")).as_deref(),
            Some("text\n")
        );
    }
}
