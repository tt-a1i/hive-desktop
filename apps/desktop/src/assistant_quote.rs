use crate::app::HiveApp;
use gpui_kit::{component::text::TextViewState, *};
use std::ops::Range;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssistantQuote {
    pub(crate) workspace_id: String,
    pub(crate) session_id: String,
    pub(crate) message_id: String,
    pub(crate) excerpt: String,
    pub(crate) source_range: Option<Range<usize>>,
}

impl AssistantQuote {
    pub(crate) fn markdown(&self) -> String {
        let mut block = self
            .excerpt
            .lines()
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(source) = crate::quote_source::assistant_quote_source_line(
            &self.workspace_id,
            &self.session_id,
            &self.message_id,
        ) {
            block.push('\n');
            block.push_str(&source);
        }
        block
    }
}

impl HiveApp {
    fn assistant_quote_identity(&self) -> Option<(&str, &str)> {
        let session = self.session.as_ref()?;
        let workspace = self.workspace.as_deref()?;
        let draft_key = format!("session:{}", session.id);
        (self.conversation_ready
            && !self.loading
            && !self.closing
            && !self.current_worktree_released()
            && self.viewing_conversation()
            && session.workspace_id == workspace
            && self.draft_owner.as_deref() == Some(draft_key.as_str())
            && self.draft_target == self.draft_owner)
            .then_some((workspace, session.id.as_str()))
    }

    pub(crate) fn capture_assistant_quote(
        &self,
        message_id: &str,
        markdown: &Entity<TextViewState>,
        cx: &App,
    ) -> Option<AssistantQuote> {
        let (workspace_id, session_id) = self.assistant_quote_identity()?;
        let source = self
            .messages
            .iter()
            .find(|message| message.id == message_id && message.role == "assistant")?;
        let state = markdown.read(cx);
        if state.rendered_text().source() != source.text.as_str() {
            return None;
        }
        let excerpt = state.selected_text();
        if excerpt.trim().is_empty() {
            return None;
        }
        Some(AssistantQuote {
            workspace_id: workspace_id.into(),
            session_id: session_id.into(),
            message_id: message_id.into(),
            excerpt,
            source_range: state.selected_source_range(),
        })
    }

    pub(crate) fn insert_assistant_quote(
        &mut self,
        quote: AssistantQuote,
        open_request: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.open_request != open_request
            || self.assistant_quote_identity()
                != Some((quote.workspace_id.as_str(), quote.session_id.as_str()))
            || !self.messages.iter().any(|message| {
                message.id == quote.message_id
                    && message.role == "assistant"
                    && quote
                        .source_range
                        .as_ref()
                        .is_none_or(|range| message.text.get(range.clone()).is_some())
            })
        {
            return false;
        }
        let block = quote.markdown();
        let inserted = self.input.update(cx, |input, cx| {
            if input.marked_text_range(window, cx).is_some() {
                return false;
            }
            let caret = input.cursor();
            let text = input.value();
            let (insertion, reused_breaks) = quote_insertion(text.as_str(), caret, &block);
            input.insert(insertion, window, cx);
            if reused_breaks != 0 {
                let after_quote = input.cursor() + reused_breaks;
                input.set_selected_range(after_quote..after_quote, cx);
            }
            input.focus(window, cx);
            true
        });
        if !inserted {
            return false;
        }
        self.schedule_draft_save(cx);
        cx.notify();
        true
    }
}

fn quote_insertion(draft: &str, caret: usize, block: &str) -> (String, usize) {
    // The editor supplies a UTF-8 byte offset at a character boundary.
    let (before, after) = draft.split_at(caret);
    let preceding = if before.is_empty() || before.ends_with("\n\n") {
        ""
    } else if before.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let reused_breaks = after
        .bytes()
        .take(2)
        .take_while(|byte| *byte == b'\n')
        .count();
    let following = &"\n\n"[reused_breaks..];
    (format!("{preceding}{block}{following}"), reused_breaks)
}

#[cfg(test)]
mod tests {
    use super::quote_insertion;

    #[test]
    fn quote_insertion_preserves_utf8_text_on_both_sides_of_the_caret() {
        let mut draft = "前言 🐝后记".to_owned();
        let caret = "前言 🐝".len();
        let (quote, reused_breaks) = quote_insertion(&draft, caret, "> 选中的片段\n> 第二行");
        assert_eq!(reused_breaks, 0);
        draft.insert_str(caret, &quote);
        assert_eq!(draft, "前言 🐝\n\n> 选中的片段\n> 第二行\n\n后记");
    }

    #[test]
    fn quote_insertion_reuses_existing_blank_lines() {
        let draft = "before\n\nafter";
        assert_eq!(
            quote_insertion(draft, "before".len(), "> quote"),
            ("\n\n> quote".into(), 2)
        );
        assert_eq!(
            quote_insertion(draft, draft.len(), "> quote"),
            ("\n\n> quote\n\n".into(), 0)
        );
        assert_eq!(quote_insertion("", 0, "> quote"), ("> quote\n\n".into(), 0));
    }
}
