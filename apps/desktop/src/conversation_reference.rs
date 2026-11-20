use crate::{app::HiveApp, language::tr, palette::Color, protocol::Session};
use gpui_kit::{
    base::Disableable,
    component::{
        WindowExt as _,
        button::{Button, ButtonVariants},
        input::{Enter, Input, InputEvent, InputState},
        v_flex,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::json;

const PREFIX: &str = "hive:conversation:v1/";

#[derive(PartialEq, Eq)]
pub(crate) struct ConversationReference {
    workspace: String,
    session: String,
}

impl ConversationReference {
    pub(crate) fn for_session(session: &Session) -> Option<Self> {
        Self::new(&session.workspace_id, &session.id)
    }

    fn new(workspace: &str, session: &str) -> Option<Self> {
        // Current IDs are UUIDs. Older records can have opaque string IDs.
        let safe_id = |id: &str| {
            id.len() <= 128
                && id.bytes().any(|byte| byte.is_ascii_alphanumeric())
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        };
        (safe_id(workspace) && safe_id(session)).then(|| Self {
            workspace: workspace.to_owned(),
            session: session.to_owned(),
        })
    }

    pub(crate) fn clipboard_text(&self) -> String {
        format!("{PREFIX}{}/{}", self.workspace, self.session)
    }

    fn parse(text: &str) -> Option<Self> {
        let (workspace, session) = text.trim().strip_prefix(PREFIX)?.split_once('/')?;
        Self::new(workspace, session)
    }

    fn resolve(&self, value: serde_json::Value) -> Result<Session, &'static str> {
        let session: Option<Session> =
            serde_json::from_value(value).map_err(|_| "无法读取引用指向的会话。")?;
        session
            .filter(|session| {
                crate::history_navigation::open_session_identity_matches(
                    session,
                    &self.session,
                    &self.workspace,
                )
            })
            .ok_or("在引用指定的项目中找不到该会话。")
    }
}

struct ReferenceDialog {
    app: WeakEntity<HiveApp>,
    input: Entity<InputState>,
    focus: FocusHandle,
    lookup: Option<Task<()>>,
    error: Option<&'static str>,
}

impl HiveApp {
    pub(crate) fn open_conversation_reference(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.closing || window.has_active_dialog(cx) {
            return;
        }
        let app = cx.weak_entity();
        let dialog = cx.new(|cx: &mut Context<ReferenceDialog>| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("粘贴会话引用")));
            cx.subscribe(&input, |dialog, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    dialog.lookup = None;
                    dialog.error = None;
                    cx.notify();
                }
            })
            .detach();
            ReferenceDialog {
                app,
                input,
                focus: cx.focus_handle(),
                lookup: None,
                error: None,
            }
        });
        let input_focus = dialog.read(cx).input.read(cx).focus_handle(cx);
        window.open_dialog(cx, move |surface, _, _| {
            let cancel = dialog.clone();
            surface
                .title(tr("打开会话引用"))
                .w(px(520.))
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| {
                    cancel.update(cx, |dialog, _| dialog.lookup = None);
                    true
                })
                .child(dialog.clone())
        });
        let active_dialog = window.focused(cx);
        window.on_next_frame(move |window, cx| {
            if active_dialog.is_some_and(|dialog| dialog.contains_focused(window, cx)) {
                input_focus.focus(window, cx);
            }
        });
    }
}

impl ReferenceDialog {
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.lookup.is_some() {
            return;
        }
        self.input.read(cx).focus_handle(cx).focus(window, cx);
        let value = self.input.read(cx).value();
        let Some(reference) = ConversationReference::parse(&value) else {
            self.show_error("请粘贴有效的 Hive 会话引用。", cx);
            return;
        };
        let Some(owner) = self.app.upgrade() else {
            return;
        };
        let app = owner.read(cx);
        let (Some(runtime), Some(origin)) = (app.runtime.clone(), app.history_link_target()) else {
            self.show_error("后台未连接，无法打开会话引用。", cx);
            return;
        };
        if !origin.current(app) {
            self.show_error("后台未连接，无法打开会话引用。", cx);
            return;
        }
        if !workspace_available(app, &reference.workspace) {
            self.show_error("此会话引用的项目在本机不可用。", cx);
            return;
        }

        self.error = None;
        let window = window.window_handle();
        self.lookup = Some(cx.spawn(async move |dialog, cx| {
            let result = runtime
                .request(
                    "chat.reference.resolve",
                    json!({
                        "workspaceId": reference.workspace,
                        "sessionId": reference.session,
                    }),
                )
                .await
                .map_err(|_| "无法读取引用指向的会话。")
                .and_then(|value| reference.resolve(value));
            let _ = window.update(cx, |_, window, cx| {
                let _ = dialog.update(cx, |dialog, cx| {
                    if !dialog.focus.contains_focused(window, cx)
                        || ConversationReference::parse(&dialog.input.read(cx).value()).as_ref()
                            != Some(&reference)
                    {
                        dialog.lookup = None;
                        cx.notify();
                        return;
                    }
                    dialog.lookup = None;
                    let result = owner.update(cx, |app, cx| {
                        if !origin.current(app) {
                            return Err("当前视图已变化，请重新打开会话引用。");
                        }
                        if !workspace_available(app, &reference.workspace) {
                            return Err("此会话引用的项目在本机不可用。");
                        }
                        let session = result?;
                        if !app.navigate_to_session(session, window, cx) {
                            return Err("在引用指定的项目中找不到该会话。");
                        }
                        app.focus_project_input = true;
                        Ok(())
                    });
                    match result {
                        Ok(()) => window.close_dialog(cx),
                        Err(error) => dialog.show_error(error, cx),
                    }
                });
            });
        }));
        cx.notify();
    }

    fn show_error(&mut self, error: &'static str, cx: &mut Context<Self>) {
        self.error = Some(error);
        cx.notify();
    }
}

fn workspace_available(app: &HiveApp, workspace: &str) -> bool {
    app.workspace_removing.as_deref() != Some(workspace)
        && app.workspaces.iter().any(|project| project.id == workspace)
}

impl Render for ReferenceDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loading = self.lookup.is_some();
        v_flex()
            .track_focus(&self.focus)
            .w_full()
            .gap_2()
            .child(
                div()
                    .capture_action(cx.listener(|this, enter: &Enter, window, cx| {
                        if !enter.secondary
                            && !enter.shift
                            && !this.input.update(cx, |input, cx| {
                                input.marked_text_range(window, cx).is_some()
                            })
                        {
                            cx.stop_propagation();
                            this.submit(window, cx);
                        }
                    }))
                    .child(Input::new(&self.input).aria_label(tr("粘贴会话引用"))),
            )
            .when_some(self.error, |form, error| {
                form.child(
                    div()
                        .id("conversation-reference-error")
                        .role(Role::Alert)
                        .aria_label(tr(error))
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(tr(error)),
                )
            })
            .when(loading, |form| {
                form.child(
                    div()
                        .id("conversation-reference-pending")
                        .role(Role::Status)
                        .aria_label(tr("正在查找会话…"))
                        .text_sm()
                        .child(tr("正在查找会话…")),
                )
            })
            .child(
                Button::new("open-conversation-reference-submit")
                    .primary()
                    .label(tr("打开"))
                    .disabled(loading)
                    .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{ConversationReference, json};

    #[test]
    fn conversation_references_round_trip_without_rewriting_legacy_ids() {
        let text =
            "hive:conversation:v1/00000000-0000-5000-8000-000000000001/legacy_Conversation-2";
        let reference = ConversationReference::parse(&format!("  {text}\n")).unwrap();
        assert_eq!(reference.clipboard_text(), text);
    }

    #[test]
    fn conversation_references_reject_ambiguous_or_external_text() {
        for text in [
            "https://example.test/x",
            "hive:conversation:v2/a/b",
            "hive:conversation:v1//b",
            "hive:conversation:v1/a/b/extra",
            "hive:conversation:v1/a/b?prompt=test",
            "hive:conversation:v1/a/b#extra",
            "hive:conversation:v1/a/%2fb",
            "hive:conversation:v1/a/..",
            "hive:conversation:v1/a/b\nmore",
        ] {
            assert!(ConversationReference::parse(text).is_none(), "{text}");
        }
        assert!(ConversationReference::new(&"a".repeat(129), "b").is_none());
    }

    #[test]
    fn conversation_reference_uses_exact_ids_across_title_changes_and_archival() {
        let reference = ConversationReference::new("workspace-b", "target").unwrap();
        let target =
            json!({"id":"target", "title":"Renamed", "workspaceId":"workspace-b", "archived":true});
        let other_project = json!({"id":"target", "title":"Renamed", "workspaceId":"workspace-a"});
        let other_conversation =
            json!({"id":"other", "title":"Renamed", "workspaceId":"workspace-b"});
        assert!(reference.resolve(other_project).is_err());
        assert!(reference.resolve(other_conversation).is_err());
        assert!(reference.resolve(serde_json::Value::Null).is_err());
        assert!(reference.resolve(json!([target.clone()])).is_err());
        let exact = reference.resolve(target).unwrap();
        assert!(exact.archived);
        assert_eq!(exact.workspace_id, "workspace-b");
        assert_eq!(exact.title, "Renamed");
    }
}
