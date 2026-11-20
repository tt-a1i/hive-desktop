use crate::{app::HiveApp, language::tr, protocol::Session};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        h_flex,
    },
    *,
};
use serde_json::json;
use std::{borrow::Cow, collections::HashSet};

// The source travels with the editable quotation through saved drafts, the
// stash, and the transcript. Only an explicit desktop action resolves it.
const PREFIX: &str = "hive:quote:v1/";
const LABEL: &str = "引用原消息";
const MAX_MESSAGE_ID: usize = 1024;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct QuoteSource {
    workspace: String,
    session: String,
    message: String,
}

fn safe_scope(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn safe_message(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_MESSAGE_ID && !id.chars().any(char::is_control)
}

pub(crate) fn assistant_quote_source_line(
    workspace: &str,
    session: &str,
    message: &str,
) -> Option<String> {
    if !safe_scope(workspace) || !safe_scope(session) || !safe_message(message) {
        return None;
    }
    let message = URL_SAFE_NO_PAD.encode(message);
    Some(format!(
        "> [{}]({PREFIX}{workspace}/{session}/{message})",
        tr(LABEL)
    ))
}

impl QuoteSource {
    fn from_line(line: &str) -> Option<Self> {
        let (label, target) = line
            .trim_end_matches(['\r', '\n'])
            .strip_prefix("> [")?
            .split_once("](")?;
        if !matches!(label, LABEL | "Quoted message") {
            return None;
        }
        let (workspace, target) = target
            .strip_suffix(')')?
            .strip_prefix(PREFIX)?
            .split_once('/')?;
        let (session, encoded) = target.split_once('/')?;
        if !safe_scope(workspace)
            || !safe_scope(session)
            || encoded.len() > MAX_MESSAGE_ID.div_ceil(3) * 4
        {
            return None;
        }
        let message = String::from_utf8(URL_SAFE_NO_PAD.decode(encoded).ok()?).ok()?;
        safe_message(&message).then(|| Self {
            workspace: workspace.into(),
            session: session.into(),
            message,
        })
    }
}

pub(crate) fn body_without_quote_source(body: &str) -> Cow<'_, str> {
    if !body.contains(PREFIX) {
        return Cow::Borrowed(body);
    }
    Cow::Owned(
        body.split_inclusive('\n')
            .filter(|line| QuoteSource::from_line(line).is_none())
            .collect(),
    )
}

impl HiveApp {
    pub(crate) fn render_assistant_quote_sources(
        &self,
        text: &str,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if !text.contains(PREFIX) {
            return None;
        }
        let mut seen = HashSet::new();
        let sources: Vec<_> = text
            .lines()
            .filter_map(QuoteSource::from_line)
            .filter(|source| seen.insert(source.clone()))
            .collect();
        (!sources.is_empty()).then(|| {
            h_flex()
                .flex_wrap()
                .gap_1()
                .children(sources.into_iter().enumerate().map(|(index, source)| {
                    Button::new(("assistant-quote-source", index))
                        .custom(crate::palette::quiet_icon_variant(cx))
                        .compact()
                        .xsmall()
                        .icon(Glyph::Reply)
                        .label(tr(LABEL))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_assistant_quote_source(source.clone(), window, cx);
                        }))
                }))
                .into_any_element()
        })
    }

    fn open_assistant_quote_source(
        &mut self,
        source: QuoteSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.closing || self.loading {
            return;
        }
        self.assistant_quote_navigation = None;
        let narrow = window.viewport_size().width < px(820.);
        if let Some(session) = self
            .session
            .as_ref()
            .filter(|session| {
                self.workspace.as_ref() == Some(&source.workspace)
                    && session.workspace_id == source.workspace
                    && session.id == source.session
            })
            .cloned()
        {
            if self.message_index(&source.message).is_some() {
                self.finish_assistant_quote_jump(&source.message, cx);
            } else if !self.navigate_to_assistant_quote(session, source.message, narrow, cx) {
                self.unavailable_assistant_quote(cx);
            }
            return;
        }
        let (Some(runtime), Some(origin)) = (self.runtime.clone(), self.history_link_target())
        else {
            self.unavailable_assistant_quote(cx);
            return;
        };
        if !origin.current(self)
            || !self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == source.workspace)
        {
            self.unavailable_assistant_quote(cx);
            return;
        }
        self.assistant_quote_navigation = Some(cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.reference.resolve",
                    json!({
                        "workspaceId": source.workspace, "sessionId": source.session,
                    }),
                )
                .await
                .ok()
                .and_then(|value| serde_json::from_value::<Session>(value).ok());
            let _ = this.update(cx, |this, cx| {
                if !origin.current(this) {
                    return;
                }
                if let Some(session) = result.filter(|session| {
                    crate::history_navigation::open_session_identity_matches(
                        session,
                        &source.session,
                        &source.workspace,
                    )
                }) && this.navigate_to_assistant_quote(session, source.message, narrow, cx)
                {
                    return;
                }
                this.unavailable_assistant_quote(cx);
            });
        }));
    }

    pub(crate) fn finish_assistant_quote_jump(&mut self, message: &str, cx: &mut Context<Self>) {
        if self
            .message_index(message)
            .is_some_and(|index| self.messages[index].role == "assistant")
            && self.scroll_to_conversation_message(message, cx)
        {
            self.schedule_reading_position_save(cx);
            cx.notify();
        } else {
            self.unavailable_assistant_quote(cx);
        }
    }

    fn unavailable_assistant_quote(&mut self, cx: &mut Context<Self>) {
        self.error = Some(crate::app::ErrorNotice::Conversation(Box::new(
            "引用的原消息暂不可用。".into(),
        )));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{QuoteSource, assistant_quote_source_line, body_without_quote_source};

    #[test]
    fn source_round_trips_without_altering_the_quoted_snapshot_or_surrounding_input() {
        let line = assistant_quote_source_line("project", "session", "codex/item:原文(2)").unwrap();
        let text = format!("前缀\n\n> café\n{line}\n\n后缀");
        assert_eq!(
            QuoteSource::from_line(&line).unwrap(),
            QuoteSource {
                workspace: "project".into(),
                session: "session".into(),
                message: "codex/item:原文(2)".into(),
            }
        );
        assert_eq!(body_without_quote_source(&text), "前缀\n\n> café\n\n后缀");
        assert!(QuoteSource::from_line(&format!("> {line}")).is_none());
        assert!(assistant_quote_source_line("../project", "session", "item").is_none());
        let malformed = "> [引用原消息](hive:quote:v1/project/session/***)";
        assert_eq!(body_without_quote_source(malformed), malformed);
    }
}
