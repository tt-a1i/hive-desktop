use crate::{
    app::{HiveApp, Message},
    palette::Color,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        *,
    },
    *,
};

pub struct ConversationFind {
    input: Entity<InputState>,
    query: String,
    hits: Vec<String>,
    selected: Option<String>,
    return_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

fn matches(message: &Message, input: Option<&serde_json::Value>, query: &str) -> bool {
    !query.is_empty()
        && (message.text.to_lowercase().contains(query)
            || message
                .tool
                .as_ref()
                .is_some_and(|tool| tool.name.to_lowercase().contains(query))
            || message
                .delivery
                .as_ref()
                .is_some_and(|card| card.raw.to_lowercase().contains(query))
            || input.is_some_and(|input| input.to_string().to_lowercase().contains(query)))
}

impl HiveApp {
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
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder("查找已加载消息和工具…"));
            let subscription =
                cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                    InputEvent::Change => this.change_conversation_find(window, cx),
                    InputEvent::PressEnter {
                        secondary: false,
                        shift,
                    } => this.move_conversation_find(*shift, window, cx),
                    _ => (),
                });
            self.conversation_find = Some(ConversationFind {
                input,
                query: String::new(),
                hits: Vec::new(),
                selected: None,
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
        focus.focus(window, cx);
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

    fn change_conversation_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(find) = &mut self.conversation_find else {
            return;
        };
        find.query = find.input.read(cx).value().trim().to_lowercase();
        find.hits = self
            .messages
            .iter()
            .filter(|m| matches(m, self.tool_inputs.get(&m.id), &find.query))
            .map(|m| m.id.clone())
            .collect();
        find.selected = find.hits.first().cloned();
        self.reveal_conversation_match(window, cx);
        cx.notify();
    }

    pub fn move_conversation_find(
        &mut self,
        previous: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(find) = &mut self.conversation_find else {
            return;
        };
        if find.hits.is_empty() {
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
        self.reveal_conversation_match(window, cx);
        cx.notify();
    }

    fn reveal_conversation_match(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(find) = &self.conversation_find else {
            return;
        };
        let Some(id) = &find.selected else {
            return;
        };
        if let Some(message) = self.messages.iter_mut().find(|m| &m.id == id) {
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
    pub(crate) fn refresh_conversation_find(&mut self, id: &str) {
        let Some(find) = &mut self.conversation_find else {
            return;
        };
        let matched = self
            .messages
            .iter()
            .find(|m| m.id == id)
            .is_some_and(|m| matches(m, self.tool_inputs.get(id), &find.query));
        let present = find.hits.iter().any(|hit| hit == id);
        if matched && !present {
            find.hits.push(id.to_owned());
            find.hits
                .sort_by_key(|hit| self.messages.iter().position(|m| &m.id == hit));
        } else if !matched && present {
            find.hits.retain(|hit| hit != id);
        }
        if find
            .selected
            .as_ref()
            .is_none_or(|id| !find.hits.contains(id))
        {
            find.selected = find.hits.first().cloned();
        }
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
        h_flex()
            .px_5()
            .py_2()
            .gap_2()
            .border_b_1()
            .border_color(Color::Divider.get(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&find.input).aria_label("查找当前会话的消息和工具")),
            )
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(
                if find.query.is_empty() {
                    "输入关键词".into()
                } else if find.hits.is_empty() {
                    "没有匹配".into()
                } else {
                    format!("{position}/{} 条消息", find.hits.len())
                },
            ))
            .child(
                Button::new("previous-conversation-match")
                    .ghost()
                    .compact()
                    .icon(Glyph::ArrowUp)
                    .accessibility_label("上一条匹配消息")
                    .tooltip("上一条 · Shift+Enter")
                    .disabled(find.hits.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.move_conversation_find(true, window, cx)
                    })),
            )
            .child(
                Button::new("next-conversation-match")
                    .ghost()
                    .compact()
                    .icon(Glyph::ArrowDown)
                    .accessibility_label("下一条匹配消息")
                    .tooltip("下一条 · Enter")
                    .disabled(find.hits.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.move_conversation_find(false, window, cx)
                    })),
            )
            .child(
                Button::new("close-conversation-find")
                    .ghost()
                    .compact()
                    .icon(Glyph::X)
                    .accessibility_label("关闭会话查找")
                    .tooltip("关闭 · Escape")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.close_conversation_find(window, cx)),
                    ),
            )
    }
}
