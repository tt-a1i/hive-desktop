use crate::app::HiveApp;
mod view;
use gpui_kit::*;
use serde_json::Value;

pub struct ThinkingBlock {
    pub index: usize,
    pub text: String,
    pub expanded: bool,
    pub running: bool,
    pub interrupted: bool,
    pub redacted: bool,
}

impl HiveApp {
    pub(crate) fn sync_thinking(
        &mut self,
        id: &str,
        value: &Value,
        running: bool,
        cx: &mut Context<Self>,
    ) {
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
                    expanded: false,
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
            message.thinking = blocks
                .into_iter()
                .map(|mut block| {
                    block.expanded = !block.redacted
                        && previous
                            .iter()
                            .any(|old| old.index == block.index && old.expanded);
                    block
                })
                .collect();
            message.layout_revision += 1;
        }
        self.refresh_conversation_find(id);
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
                expanded: false,
                running: true,
                interrupted: false,
                redacted: false,
            });
            message.thinking.sort_by_key(|block| block.index);
        }
        message.layout_revision += 1;
        self.refresh_conversation_find(&id);
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
