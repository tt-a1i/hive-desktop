use crate::language::{english, tr};
use crate::{
    app::{HiveApp, Message},
    palette::Color,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        Icon,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use icu_normalizer::ComposingNormalizer;
use serde::Deserialize;
use serde_json::json;
use std::{borrow::Cow, collections::HashSet, time::Duration};

const FIND_PLACEHOLDER: &str = "查找会话消息、思考和工具…";

#[derive(Default)]
enum HistorySearch {
    #[default]
    Loaded,
    Searching,
    Found(HistoryMatches),
    Failed,
}

#[derive(Deserialize)]
struct HistoryMatches {
    matches: Vec<MessageMatch>,
    complete: bool,
    #[serde(default)]
    limited: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageMatch {
    message_id: String,
}

pub struct ConversationFind {
    input: Entity<InputState>,
    window: AnyWindowHandle,
    query: String,
    loaded_hits: Vec<String>,
    hits: Vec<String>,
    selected: Option<String>,
    history: HistorySearch,
    search_task: Option<Task<()>>,
    return_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

impl ConversationFind {
    fn merge_hits(&mut self) {
        let saved = match &self.history {
            HistorySearch::Found(result) => result
                .matches
                .iter()
                .map(|hit| hit.message_id.as_str())
                .collect(),
            _ => Vec::new(),
        };
        let mut seen = HashSet::with_capacity(saved.len() + self.loaded_hits.len());
        self.hits = saved
            .into_iter()
            .chain(self.loaded_hits.iter().map(String::as_str))
            .filter(|id| seen.insert(*id))
            .map(str::to_owned)
            .collect();
        if self
            .selected
            .as_ref()
            .is_none_or(|id| !self.hits.contains(id))
        {
            self.selected = self.hits.first().cloned();
        }
    }
}

fn fold(text: &str) -> Cow<'_, str> {
    if text.is_ascii() && !text.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(
            ComposingNormalizer::new_nfc()
                .normalize(text)
                .to_lowercase(),
        )
    }
}

fn contains(text: &str, query: &str) -> bool {
    !query.is_empty() && fold(text).contains(query)
}

fn matches(message: &Message, input: Option<&serde_json::Value>, query: &str) -> bool {
    !query.is_empty()
        && (contains(message.display_body(), query)
            || message
                .thinking
                .iter()
                .any(|block| contains(&block.text, query))
            || message.tool.as_ref().is_some_and(|tool| {
                contains(&tool.name, query)
                    || tool
                        .patch
                        .as_ref()
                        .is_some_and(|patch| contains(patch, query))
            })
            || message.failure.as_ref().is_some_and(|failure| {
                contains(&failure.display_message(), query) || contains(&failure.message, query)
            })
            || message
                .delivery
                .as_ref()
                .is_some_and(|card| contains(&card.raw, query))
            || input.is_some_and(|input| contains(&input.to_string(), query)))
}

impl HiveApp {
    #[cfg(test)]
    pub(crate) fn set_conversation_find_query_for_test(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(find) = &self.conversation_find {
            let input = find.input.clone();
            input.update(cx, |input, cx| input.set_value(query, window, cx));
            self.change_conversation_find(window, cx);
        }
    }

    pub(crate) fn localize_find_placeholder(&self, window: &mut Window, cx: &mut App) {
        if let Some(find) = &self.conversation_find {
            find.input.update(cx, |input, cx| {
                input.set_placeholder(tr(FIND_PLACEHOLDER), window, cx)
            });
        }
    }
    pub fn open_conversation_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.viewing_conversation()
            || self.session.is_none()
            || !self.conversation_ready
            || self.loading
            || window.has_active_dialog(cx)
        {
            return;
        }
        if self.conversation_find.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(tr(FIND_PLACEHOLDER)));
            let subscription = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.change_conversation_find(window, cx);
                }
            });
            self.conversation_find = Some(ConversationFind {
                input,
                window: window.window_handle(),
                query: String::new(),
                loaded_hits: Vec::new(),
                hits: Vec::new(),
                selected: None,
                history: HistorySearch::Loaded,
                search_task: None,
                return_focus: window.focused(cx),
                _subscription: subscription,
            });
        }
        self.conversation_list.state.pause_following_tail();
        let focus = self
            .conversation_find
            .as_ref()
            .unwrap()
            .input
            .read(cx)
            .focus_handle(cx);
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
        cx.notify();
    }

    pub fn close_conversation_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(find) = self.conversation_find.take() {
            if let Some(focus) = find.return_focus {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    fn change_conversation_find(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(find) = &mut self.conversation_find else {
            return;
        };
        find.query = fold(find.input.read(cx).value().trim()).into_owned();
        find.selected = None;
        find.search_task = None;
        find.history = HistorySearch::Loaded;
        self.refresh_loaded_conversation_find();
        self.reveal_conversation_match(cx);
        self.search_conversation_history(cx);
        self.schedule_reading_position_save(cx);
        cx.notify();
    }

    fn search_conversation_history(&mut self, cx: &mut Context<Self>) {
        if !self
            .history_page
            .as_ref()
            .is_some_and(|page| page.has_older || page.has_newer)
        {
            return;
        }
        let (Some(find), Some(session), Some(runtime)) = (
            self.conversation_find.as_mut(),
            self.session.as_ref(),
            self.runtime.clone(),
        ) else {
            return;
        };
        if find.query.is_empty() {
            return;
        }
        let query = find.query.clone();
        let input = find.input.clone();
        let session = session.id.clone();
        let connection = self.connection_generation;
        let revision = self.open_request;
        find.history = HistorySearch::Searching;
        find.search_task = Some(cx.spawn(async move |this, cx| {
            smol::Timer::after(Duration::from_millis(180)).await;
            let result = runtime
                .search_conversation_messages(json!({
                    "sessionId": session, "viewRevision": revision, "query": query,
                }))
                .await
                .and_then(|reply| {
                    serde_json::from_value::<HistoryMatches>(reply)
                        .map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.open_request != revision
                    || this.session.as_ref().is_none_or(|item| item.id != session)
                {
                    return;
                }
                let Some(find) = &mut this.conversation_find else {
                    return;
                };
                if find.input != input || find.query != query {
                    return;
                }
                find.history = result.map_or(HistorySearch::Failed, HistorySearch::Found);
                find.merge_hits();
                this.reveal_or_open_conversation_match(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn refresh_loaded_conversation_find(&mut self) {
        let Some(find) = &mut self.conversation_find else {
            return;
        };
        find.loaded_hits = self
            .messages
            .iter()
            .filter(|m| matches(m, self.tool_inputs.get(&m.id), &find.query))
            .map(|m| m.id.clone())
            .collect();
        find.merge_hits();
    }

    pub fn move_conversation_find(
        &mut self,
        previous: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(find) = &mut self.conversation_find else {
            return;
        };
        if find.hits.is_empty() || self.loading {
            return;
        }
        let current = find
            .selected
            .as_ref()
            .and_then(|id| find.hits.iter().position(|hit| hit == id));
        let index = match current {
            Some(index) if previous => (index + find.hits.len() - 1) % find.hits.len(),
            Some(index) => (index + 1) % find.hits.len(),
            None => 0,
        };
        find.selected = Some(find.hits[index].clone());
        self.reveal_or_open_conversation_match(cx);
        self.schedule_reading_position_save(cx);
        cx.notify();
    }

    fn reveal_or_open_conversation_match(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let Some(message) = self
            .conversation_find
            .as_ref()
            .and_then(|find| find.selected.clone())
        else {
            return;
        };
        if self.message_indices.contains_key(&message) {
            self.reveal_conversation_match(cx);
        } else {
            self.open_conversation_find_match(message, cx);
        }
    }

    pub(crate) fn finish_conversation_find_jump(&mut self, message: &str, cx: &mut Context<Self>) {
        if self.conversation_find.is_none() {
            self.scroll_to_conversation_message(message, cx);
            return;
        }
        self.refresh_loaded_conversation_find();
        if self.message_indices.contains_key(message) {
            self.reveal_conversation_match(cx);
            self.schedule_reading_position_save(cx);
        } else if let Some(find) = &mut self.conversation_find {
            if matches!(find.history, HistorySearch::Found(_)) {
                find.history = HistorySearch::Failed;
                find.merge_hits();
            }
        }
        if let Some(find) = &self.conversation_find {
            let focus = find.input.read(cx).focus_handle(cx);
            let _ = find.window.update(cx, |_, window, _| {
                window.on_next_frame(move |window, cx| focus.focus(window, cx));
            });
        }
        if self
            .conversation_find
            .as_ref()
            .is_some_and(|find| matches!(find.history, HistorySearch::Loaded))
        {
            self.search_conversation_history(cx);
        }
    }

    fn reveal_conversation_match(&mut self, cx: &mut Context<Self>) {
        let Some(find) = &self.conversation_find else {
            return;
        };
        let Some(id) = &find.selected else {
            return;
        };
        if let Some(index) = self.message_indices.get(id).copied() {
            let message = &mut self.messages[index];
            for block in &mut message.thinking {
                if !block.expanded && contains(&block.text, &find.query) {
                    block.expanded = true;
                    message.layout_revision += 1;
                }
            }
            if let Some(failure) = &mut message.failure {
                if !failure.expanded && contains(&failure.message, &find.query) {
                    failure.expanded = true;
                    message.layout_revision += 1;
                }
            }
            if let Some(tool) = &mut message.tool {
                tool.expanded = true;
            }
            if let Some(card) = &mut message.delivery {
                card.expanded = true;
            }
            let id = id.clone();
            self.scroll_to_conversation_message(&id, cx);
        }
    }

    /// Only inspect the changed message while streaming. Rebuild the full set
    /// when the query changes, rather than scanning all text for every delta.
    pub(crate) fn refresh_conversation_find(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(find) = &mut self.conversation_find else {
            return;
        };
        let previous_selection = find.selected.clone();
        let matched = self
            .message_indices
            .get(id)
            .and_then(|index| self.messages.get(*index))
            .is_some_and(|m| matches(m, self.tool_inputs.get(id), &find.query));
        let present = find.loaded_hits.iter().any(|hit| hit == id);
        if matched && !present {
            find.loaded_hits.push(id.to_owned());
            find.loaded_hits
                .sort_by_key(|hit| self.message_indices.get(hit).copied());
        } else if !matched && present {
            find.loaded_hits.retain(|hit| hit != id);
        } else {
            return;
        }
        find.merge_hits();
        if find.selected != previous_selection {
            self.reveal_conversation_match(cx);
        }
    }

    pub(crate) fn conversation_find_patch_match(&self, id: &str, patch: Option<&str>) -> bool {
        self.conversation_find.as_ref().is_some_and(|find| {
            find.selected.as_deref() == Some(id) && patch.is_some_and(|p| contains(p, &find.query))
        })
    }

    pub(crate) fn conversation_find_matches(&self, id: &str) -> bool {
        self.conversation_find
            .as_ref()
            .is_some_and(|find| find.selected.as_deref() == Some(id))
    }

    pub(crate) fn render_conversation_find(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let find = self.conversation_find.as_ref().unwrap();
        let position = find
            .selected
            .as_ref()
            .and_then(|id| find.hits.iter().position(|hit| hit == id))
            .map_or(0, |i| i + 1);
        let searching = matches!(find.history, HistorySearch::Searching);
        let failed = matches!(find.history, HistorySearch::Failed);
        let partial = !find.query.is_empty()
            && (matches!(&find.history, HistorySearch::Found(result) if !result.complete || result.limited)
                || (matches!(find.history, HistorySearch::Loaded)
                    && self
                        .current_history_access()
                        .is_some_and(|access| access.history_incomplete)));
        let no_match =
            !find.query.is_empty() && find.hits.is_empty() && !searching && !partial && !failed;
        let count_label = if find.query.is_empty() {
            tr("输入关键词").into()
        } else if searching {
            tr("正在查找会话历史…").into()
        } else if find.hits.is_empty() {
            tr(if partial || failed {
                "已搜索部分没有匹配"
            } else {
                "没有匹配"
            })
            .into()
        } else if english() {
            format!("{position}/{} messages", find.hits.len())
        } else {
            format!("{position}/{} 条消息", find.hits.len())
        };
        let status_label = if failed || partial {
            format!("{count_label} · {}", tr("会话历史搜索不完整"))
        } else {
            count_label.clone()
        };
        div()
            .key_context("HiveConversationFind")
            .on_action(
                cx.listener(|this, _: &crate::NextConversationMatch, window, cx| {
                    this.move_conversation_find(false, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::PreviousConversationMatch, window, cx| {
                    this.move_conversation_find(true, window, cx);
                }),
            )
            .px_5()
            .py_2()
            .border_b_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Surface.get(cx))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap_1p5()
                    .px_2p5()
                    .py_1()
                    .rounded_xl()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Card.get(cx))
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&find.input)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .cleanable(true)
                                .prefix(
                                    Icon::new(Glyph::Search)
                                        .size(px(14.))
                                        .text_color(Color::Muted.get(cx)),
                                )
                                .aria_label(tr("查找当前会话的消息和工具")),
                        ),
                    )
                    .child(
                        div()
                            .id("conversation-find-result")
                            .role(Role::Status)
                            .aria_label(status_label)
                            .flex_shrink_0()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(if no_match {
                                Color::ErrorSurface.get(cx)
                            } else {
                                Color::Inset.get(cx)
                            })
                            .text_size(px(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if no_match {
                                Color::Error.get(cx)
                            } else {
                                Color::Muted.get(cx)
                            })
                            .child(count_label)
                            .when((partial || failed) && !find.hits.is_empty(), |badge| {
                                badge.child(format!(" · {}", tr("部分")))
                            }),
                    )
                    .when(failed, |view| {
                        view.child(
                            Button::new("retry-conversation-find")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(26.))
                                .w(px(26.))
                                .icon(Glyph::RefreshCw)
                                .accessibility_label(tr("重试搜索历史"))
                                .tooltip(tr("重试搜索历史"))
                                .disabled(self.loading)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.search_conversation_history(cx)
                                })),
                        )
                    })
                    .child(
                        div()
                            .flex_shrink_0()
                            .w(px(1.))
                            .h(px(16.))
                            .mx_0p5()
                            .bg(Color::Divider.get(cx)),
                    )
                    .child(
                        Button::new("previous-conversation-match")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_lg()
                            .icon(Glyph::ArrowUp)
                            .accessibility_label(tr("上一条匹配消息"))
                            .tooltip(tr("上一条 · Shift+Enter"))
                            .disabled(find.hits.is_empty() || self.loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.move_conversation_find(true, window, cx)
                            })),
                    )
                    .child(
                        Button::new("next-conversation-match")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_lg()
                            .icon(Glyph::ArrowDown)
                            .accessibility_label(tr("下一条匹配消息"))
                            .tooltip(tr("下一条 · Enter"))
                            .disabled(find.hits.is_empty() || self.loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.move_conversation_find(false, window, cx)
                            })),
                    )
                    .child(
                        Button::new("close-conversation-find")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(26.))
                            .w(px(26.))
                            .rounded_lg()
                            .icon(Glyph::X)
                            .accessibility_label(tr("关闭会话查找"))
                            .tooltip(tr("关闭 · Escape"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_conversation_find(window, cx)
                            })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{contains, fold};

    #[test]
    fn find_matches_canonical_forms_without_discarding_marks() {
        assert!(contains("Cafe\u{301} history", &fold("CAFÉ")));
        assert!(contains("CAFÉ history", &fold("cafe\u{301}")));
        assert!(contains("+ CAFE\u{301} patch", &fold("café patch")));
        assert!(contains(
            "낱말",
            &fold("\u{1102}\u{1161}\u{11c0}\u{1106}\u{1161}\u{11af}")
        ));
        assert!(!contains("Cafe\u{301} history", &fold("cafe")));
        assert!(!contains("fullwidth Ａ", &fold("a")));
        assert!(!contains("plain text", ""));
    }
}
