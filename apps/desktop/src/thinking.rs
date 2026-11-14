use crate::app::HiveApp;
mod position;
mod view;
use gpui_kit::*;
pub(crate) use position::ThinkingPosition;
use serde_json::Value;

pub struct ThinkingBlock {
    pub index: usize,
    pub text: String,
    pub expanded: bool,
    pub running: bool,
    pub interrupted: bool,
    pub redacted: bool,
}

fn reconcile_blocks(
    mut blocks: Vec<ThinkingBlock>,
    previous: &[ThinkingBlock],
    default_expanded: bool,
) -> Vec<ThinkingBlock> {
    for block in &mut blocks {
        block.expanded = !block.redacted
            && previous
                .iter()
                .find(|old| old.index == block.index)
                .map(|old| old.expanded)
                .unwrap_or(default_expanded);
    }
    blocks
}

impl HiveApp {
    pub(crate) fn sync_thinking(
        &mut self,
        id: &str,
        value: &Value,
        running: bool,
        cx: &mut Context<Self>,
    ) {
        let default_expanded = cx
            .global::<crate::composer::ComposerPreferences>()
            .settings
            .expand_thinking;
        let blocks = value["content"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(index, block)| {
                let redacted = block["type"] == "redacted_thinking";
                let text = if block["type"] == "thinking" {
                    block["thinking"].as_str().unwrap_or("")
                } else {
                    ""
                };
                (redacted || !text.is_empty()).then(|| ThinkingBlock {
                    index,
                    text: text.into(),
                    expanded: default_expanded && !redacted,
                    running: running
                        && !redacted
                        && value["content"]
                            .as_array()
                            .is_some_and(|content| index + 1 == content.len()),
                    interrupted: value["stopReason"] == "aborted"
                        || value["rawBackendItem"]["hiveInterrupted"] == true,
                    redacted,
                })
            })
            .collect::<Vec<_>>();
        if !blocks.is_empty() && !self.messages.iter().any(|message| message.id == id) {
            self.push_message(id.into(), "assistant".into(), String::new(), cx);
        }
        if let Some(message) = self.messages.iter_mut().find(|message| message.id == id) {
            let previous = std::mem::take(&mut message.thinking);
            message.thinking = reconcile_blocks(blocks, &previous, default_expanded);
            message.layout_revision += 1;
        }
        self.refresh_conversation_find(id, cx);
    }

    pub(crate) fn append_thinking(
        &mut self,
        id: String,
        index: usize,
        delta: String,
        cx: &mut Context<Self>,
    ) {
        if delta.is_empty() {
            return;
        }
        if !self.messages.iter().any(|message| message.id == id) {
            self.push_message(id.clone(), "assistant".into(), String::new(), cx);
        }
        self.finish_thinking_before(&id, index);
        let default_expanded = cx
            .global::<crate::composer::ComposerPreferences>()
            .settings
            .expand_thinking;
        let message = self
            .messages
            .iter_mut()
            .find(|message| message.id == id)
            .unwrap();
        if let Some(block) = message
            .thinking
            .iter_mut()
            .find(|block| block.index == index && !block.redacted)
        {
            block.text.push_str(&delta);
            block.running = true;
            block.interrupted = false;
        } else {
            message.thinking.push(ThinkingBlock {
                index,
                text: delta,
                expanded: default_expanded,
                running: true,
                interrupted: false,
                redacted: false,
            });
            message.thinking.sort_by_key(|block| block.index);
        }
        message.layout_revision += 1;
        self.refresh_conversation_find(&id, cx);
    }

    pub(crate) fn finish_thinking_before(&mut self, id: &str, index: usize) {
        if let Some(message) = self.messages.iter_mut().find(|message| message.id == id) {
            for block in &mut message.thinking {
                if block.index < index {
                    block.running = false;
                }
            }
            message.layout_revision += 1;
        }
    }

    pub(crate) fn finish_thinking(&mut self, interrupted: bool) {
        for message in &mut self.messages {
            for block in &mut message.thinking {
                if block.running {
                    block.running = false;
                    block.interrupted = interrupted;
                    message.layout_revision += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ThinkingBlock, reconcile_blocks};
    fn block(index: usize, text: &str, expanded: bool, redacted: bool) -> ThinkingBlock {
        ThinkingBlock {
            index,
            text: text.into(),
            expanded,
            running: false,
            interrupted: false,
            redacted,
        }
    }
    #[test]
    fn streamed_snapshot_keeps_manual_choice_and_new_text_without_exposing_redacted_blocks() {
        let previous = [
            block(0, "first delta", false, false),
            block(1, "opened", true, false),
        ];
        let next = reconcile_blocks(
            vec![
                block(0, "first delta plus final delta", true, false),
                block(1, "opened plus final delta", false, false),
                block(2, "new block", false, false),
            ],
            &previous,
            true,
        );
        assert!(!next[0].expanded);
        assert_eq!(next[0].text, "first delta plus final delta");
        assert!(next[1].expanded);
        assert_eq!(next[1].text, "opened plus final delta");
        assert!(next[2].expanded);
        let hidden = reconcile_blocks(vec![block(1, "", true, true)], &next, true);
        assert!(!hidden[0].expanded);
        assert!(hidden[0].redacted);
    }
}
