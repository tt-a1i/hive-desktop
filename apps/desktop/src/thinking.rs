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
    pub started_at: Option<std::time::Instant>,
    pub duration_ms: Option<u64>,
}

fn reconcile_blocks(
    mut blocks: Vec<ThinkingBlock>,
    previous: &[ThinkingBlock],
    default_expanded: bool,
) -> Vec<ThinkingBlock> {
    for block in &mut blocks {
        let old = previous.iter().find(|old| old.index == block.index);
        block.expanded = !block.redacted
            && old.map(|old| old.expanded).unwrap_or(default_expanded);
        if let Some(old) = old {
            block.started_at = old.started_at;
            block.duration_ms = old.duration_ms;
            if !block.running && block.duration_ms.is_none() {
                block.duration_ms = old
                    .started_at
                    .map(|started| started.elapsed().as_millis() as u64);
            }
        }
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
                    started_at: None,
                    duration_ms: None,
                })
            })
            .collect::<Vec<_>>();
        if !blocks.is_empty() && self.message_index(id).is_none() {
            self.push_message(id.into(), "assistant".into(), String::new(), cx);
        }
        if let Some(message) = self.message_mut(id) {
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
        if self.message_index(&id).is_none() {
            self.push_message(id.clone(), "assistant".into(), String::new(), cx);
        }
        self.finish_thinking_before(&id, index);
        let default_expanded = cx
            .global::<crate::composer::ComposerPreferences>()
            .settings
            .expand_thinking;
        let message = self.message_mut(&id).unwrap();
        if let Some(block) = message
            .thinking
            .iter_mut()
            .find(|block| block.index == index && !block.redacted)
        {
            block.text.push_str(&delta);
            block.duration_ms = None;
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
                started_at: Some(std::time::Instant::now()),
                duration_ms: None,
            });
            message.thinking.sort_by_key(|block| block.index);
        }
        message.layout_revision += 1;
        self.refresh_conversation_find(&id, cx);
    }

    pub(crate) fn finish_thinking_before(&mut self, id: &str, index: usize) {
        if let Some(message) = self.message_mut(id) {
            for block in &mut message.thinking {
                if block.index < index && block.running {
                    block.running = false;
                    if block.duration_ms.is_none() {
                        block.duration_ms = block
                            .started_at
                            .map(|started| started.elapsed().as_millis() as u64);
                    }
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
                    if block.duration_ms.is_none() {
                        block.duration_ms = block
                            .started_at
                            .map(|started| started.elapsed().as_millis() as u64);
                    }
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
            started_at: None,
            duration_ms: None,
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

    #[test]
    fn reconcile_blocks_freezes_duration_on_completion_and_never_fabricates_history() {
        let started = std::time::Instant::now() - std::time::Duration::from_millis(125);
        let previous = [ThinkingBlock {
            started_at: Some(started),
            running: true,
            ..block(0, "streamed", false, false)
        }];
        let finished = reconcile_blocks(vec![block(0, "streamed final", false, false)], &previous, false);
        assert_eq!(finished[0].started_at, Some(started));
        assert!(finished[0].duration_ms.is_some_and(|ms| ms >= 120));

        let historical = reconcile_blocks(vec![block(0, "from disk", false, false)], &[], false);
        assert_eq!(historical[0].started_at, None);
        assert_eq!(historical[0].duration_ms, None);
    }

    #[gpui_kit::test]
    fn append_thinking_keeps_started_at_none_for_cold_started_block(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use crate::app::{HiveApp, init_test_globals};
        let _locale = init_test_globals(cx);
        let (app, cx) = cx.add_window_view(|window, cx| HiveApp::new_for_test(window, cx));
        app.update(cx, |app, cx| {
            app.sync_thinking(
                "m1",
                &serde_json::json!({
                    "content": [{"type": "thinking", "thinking": "cold"}]
                }),
                true,
                cx,
            );
            let msg = app.message_mut("m1").unwrap();
            assert!(msg.thinking[0].running);
            assert_eq!(msg.thinking[0].started_at, None);

            app.append_thinking("m1".into(), 0, " more".into(), cx);
            let msg = app.message_mut("m1").unwrap();
            assert_eq!(msg.thinking[0].text, "cold more");
            assert!(msg.thinking[0].running);
            assert_eq!(msg.thinking[0].started_at, None);

            app.append_thinking("m1".into(), 1, "fresh block".into(), cx);
            let msg = app.message_mut("m1").unwrap();
            assert_eq!(msg.thinking.len(), 2);
            assert!(msg.thinking[1].started_at.is_some());
        });
    }
}
