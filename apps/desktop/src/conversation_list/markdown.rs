use crate::app::HiveApp;
use gpui_kit::{component::text::TextViewState, *};

pub(crate) struct MarkdownView {
    pub state: Entity<TextViewState>,
    _subscription: Subscription,
}

impl HiveApp {
    pub(super) fn show_message_markdown(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Option<Entity<TextViewState>> {
        let message = &self.messages[index];
        if message.markdown_focus.is_none() || message.text.is_empty() {
            return None;
        }
        let id = message.id.clone();
        let view = self
            .conversation_list
            .markdown
            .entry(id.clone())
            .or_insert_with(|| {
                let state = cx.new(|cx| TextViewState::markdown(&message.text, cx));
                let mut rendered = state.read(cx).rendered_text();
                let subscription = cx.observe(&state, move |this, state, cx| {
                    let next = state.read(cx).rendered_text();
                    // Only a changed document invalidates the outer row height.
                    if std::ptr::eq(next.source(), rendered.source()) {
                        return;
                    }
                    rendered = next;
                    if let Some(message) = this.messages.iter_mut().find(|message| message.id == id)
                    {
                        message.layout_revision += 1;
                        cx.notify();
                    }
                });
                MarkdownView {
                    state,
                    _subscription: subscription,
                }
            });
        Some(view.state.clone())
    }
}
