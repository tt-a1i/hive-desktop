use super::*;
use crate::protocol::PendingUserInput;
use gpui_kit::{
    component::input::{InputEvent, Textarea, TextareaState},
    prelude::FluentBuilder,
};
use serde_json::json;

enum QueueError {
    Owned(&'static str),
    Request(String),
}
impl QueueError {
    fn text(&self) -> String {
        match self {
            Self::Owned(message) => tr(message).to_owned(),
            Self::Request(error) => crate::language::localize_owned_error(error),
        }
    }
}

struct QueueEditor {
    app: WeakEntity<HiveApp>,
    session: String,
    connection: u64,
    original: PendingUserInput,
    input: Entity<TextareaState>,
    cancel: bool,
    saving: bool,
    acknowledged: bool,
    dialog_focus: Option<FocusHandle>,
    error: Option<QueueError>,
}
impl HiveApp {
    pub(super) fn open_queue_editor(
        &mut self,
        original: PendingUserInput,
        cancel: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self.closing
            || !original.editable
            || original.processing
            || window.has_active_dialog(cx)
        {
            return;
        }
        let Some(session) = self.session.as_ref().map(|session| session.id.clone()) else {
            return;
        };
        let app = cx.weak_entity();
        let connection = self.connection_generation;
        let editor = cx.new(|cx| {
            let input = cx.new(|cx| {
                let mut input = TextareaState::new(window, cx).auto_grow(4, 12);
                input.set_value(original.text.clone(), window, cx);
                input
            });
            cx.subscribe(&input, |_, _, _: &InputEvent, cx| cx.notify())
                .detach();
            if let Some(parent) = app.upgrade() {
                cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            }
            QueueEditor {
                app,
                session,
                connection,
                original,
                input,
                cancel,
                saving: false,
                acknowledged: false,
                dialog_focus: None,
                error: None,
            }
        });
        let focus = editor.read(cx).input.read(cx).focus_handle(cx);
        let dialog_editor = editor.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let editor = &dialog_editor;
            let saving = editor.read(cx).saving;
            let cancel_guard = editor.clone();
            dialog
                .title(if cancel {
                    tr("撤回待发消息")
                } else {
                    tr("编辑待发消息")
                })
                .w(px(560.))
                .on_ok(|_, _, _| false)
                .keyboard(!saving)
                .close_button(!saving)
                .overlay_closable(!saving)
                .on_cancel(move |_, _, cx| !cancel_guard.read(cx).saving)
                .child(editor.clone())
        });
        let dialog_focus = window.focused(cx);
        editor.update(cx, |editor, _| editor.dialog_focus = dialog_focus.clone());
        if !cancel {
            window.on_next_frame(move |window, cx| {
                if dialog_focus.is_some_and(|root| root.contains_focused(window, cx)) {
                    focus.focus(window, cx);
                }
            });
        }
    }
}
impl QueueEditor {
    fn unavailable(&self, cx: &App) -> Option<&'static str> {
        let Some(app) = self.app.upgrade() else {
            return Some("原会话已关闭，未修改待发消息。");
        };
        let app = app.read(cx);
        if !app.connected || app.closing || app.connection_generation != self.connection {
            return Some("后台连接已变化，请关闭后重新打开待发消息。");
        }
        if app.session.as_ref().map(|session| session.id.as_str()) != Some(self.session.as_str()) {
            return Some("会话已切换，未修改待发消息。请返回原会话重新打开。");
        }
        let Some(input) = app
            .pending_user_inputs
            .iter()
            .find(|input| input.delivery_id == self.original.delivery_id)
        else {
            return Some("消息已被处理或撤回，请关闭后核对队列。");
        };
        if input.text != self.original.text {
            return Some("待发消息已更改。当前编辑内容已保留，请关闭后重新核对。");
        }
        if !input.editable || input.processing {
            return Some("消息已开始处理或尝试发送，不能再编辑或撤回。");
        }
        None
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.acknowledged {
            return;
        }
        if let Some(message) = self.unavailable(cx) {
            self.error = Some(QueueError::Owned(message));
            cx.notify();
            return;
        }
        let text = self.input.read(cx).value().to_string();
        if !self.cancel && text.trim().is_empty() && self.original.attachments.is_empty() {
            return;
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let Some(runtime) = app.read(cx).runtime.clone() else {
            return;
        };
        let session = self.session.clone();
        let id = self.original.delivery_id.clone();
        let expected = self.original.text.clone();
        let cancel = self.cancel;
        let mut params = json!({"sessionId":session,"deliveryId":id,"expectedText":expected});
        if !cancel {
            params["text"] = json!(text);
        }
        self.saving = true;
        self.error = None;
        cx.notify();
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    if cancel {
                        "chat.queue.cancel"
                    } else {
                        "chat.queue.edit"
                    },
                    params,
                )
                .await
                .map_err(QueueError::Request)
                .and_then(|value| {
                    if value["ok"] == true {
                        Ok(())
                    } else {
                        Err(QueueError::Owned("后台未确认待发消息操作。"))
                    }
                });
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.saving = false;
                    match result {
                        Ok(()) => {
                            this.acknowledged = true;
                            // The queue snapshot may already reflect our mutation. Only
                            // compare session/connection here, not the old text or ID.
                            let current = this.app.upgrade().is_some_and(|app| {
                                let app = app.read(cx);
                                app.connected
                                    && app.connection_generation == this.connection
                                    && app
                                        .session
                                        .as_ref()
                                        .is_some_and(|session| session.id == this.session)
                            });
                            let owns_focus = this
                                .dialog_focus
                                .as_ref()
                                .is_some_and(|focus| focus.contains_focused(window, cx));
                            if current && owns_focus {
                                window.close_dialog(cx);
                            } else {
                                this.error = Some(QueueError::Owned(
                                    "后台已确认操作，请关闭此窗口并核对队列。",
                                ));
                            }
                        }
                        Err(error) => this.error = Some(error),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }
}
impl Render for QueueEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let unavailable = if self.saving || self.acknowledged {
            None
        } else {
            self.unavailable(cx)
        };
        let empty =
            self.input.read(cx).value().trim().is_empty() && self.original.attachments.is_empty();
        let disabled = self.saving
            || self.acknowledged
            || unavailable.is_some()
            || (!self.cancel
                && (empty || self.input.read(cx).value().as_str() == self.original.text));
        let error = unavailable
            .map(|message| tr(message).to_owned())
            .or_else(|| self.error.as_ref().map(QueueError::text));
        v_flex()
            .min_w_0()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(Color::Muted.get(cx))
                    .child(if self.cancel {
                        tr("撤回后，此消息和它的附件将不再等待发送。")
                    } else {
                        tr("仅修改这条待发消息。附件保持不变，聊天输入框草稿不受影响。")
                    }),
            )
            .child(
                Textarea::new(&self.input)
                    .disabled(self.saving || self.cancel)
                    .aria_label(tr("待发消息内容")),
            )
            .when(!self.original.attachments.is_empty(), |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(format!(
                            "{}: {}",
                            tr("附件"),
                            self.original
                                .attachments
                                .iter()
                                .map(|item| item.name.as_str())
                                .collect::<Vec<_>>()
                                .join(if english() { ", " } else { "、" })
                        )),
                )
            })
            .when_some(error, |view, error| {
                view.child(
                    div()
                        .id("queue-editor-error")
                        .role(Role::Status)
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("queue-editor-dismiss")
                            .outline()
                            .label(tr("取消"))
                            .disabled(self.saving)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("queue-editor-submit")
                            .primary()
                            .label(if self.saving {
                                tr("处理中…")
                            } else if self.cancel {
                                tr("确认撤回")
                            } else {
                                tr("保存修改")
                            })
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use crate::protocol::PendingUserInput;
    use serde_json::json;

    #[test]
    fn pending_input_capabilities_require_explicit_backend_opt_in() {
        let legacy: PendingUserInput = serde_json::from_value(json!({
            "deliveryId":"legacy", "text":"queued", "processing":false
        }))
        .unwrap();
        assert!(!legacy.editable);
        assert!(legacy.reason.is_none());
        let editable: PendingUserInput = serde_json::from_value(json!({
            "deliveryId":"editable", "text":"", "processing":false,
            "editable":true, "attachments":[]
        }))
        .unwrap();
        assert!(editable.editable);
        let attempted: PendingUserInput = serde_json::from_value(json!({
            "deliveryId":"attempted", "text":"queued", "processing":false,
            "editable":false, "reason":"attempted"
        }))
        .unwrap();
        assert!(!attempted.editable);
        assert_eq!(attempted.reason.as_deref(), Some("attempted"));
    }
}
