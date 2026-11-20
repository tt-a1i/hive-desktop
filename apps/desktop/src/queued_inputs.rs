mod editor;
use crate::{
    app::HiveApp,
    language::{english, tr},
    palette::Color,
    protocol::PendingUserInput,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use std::ops::Range;

gpui_kit::actions!(hive_desktop, [SteerPendingInput]);

fn latest_queued_input(inputs: &[PendingUserInput]) -> Option<&PendingUserInput> {
    inputs
        .last()
        .filter(|input| input.editable && !input.processing)
}

fn can_edit_queue_at_caret(selection: Range<usize>, composing: bool) -> bool {
    selection == (0..0) && !composing
}

impl HiveApp {
    pub(crate) fn render_pending_user_inputs(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let running = self
            .session
            .as_ref()
            .is_some_and(|session| self.busy.contains(&session.id));
        let undisplayed: Vec<_> = self
            .pending_user_inputs
            .iter()
            .filter(|input| {
                !self
                    .messages
                    .iter()
                    .any(|message| message.id == input.delivery_id)
            })
            .collect();
        if undisplayed.is_empty() {
            return None;
        }
        let count = undisplayed.len();
        let latest_editable_id = self
            .latest_queue_edit_target()
            .map(|input| input.delivery_id.as_str());
        Some(
            v_flex()
                .w_full()
                .min_w_0()
                .gap_1p5()
                .p_2p5()
                .rounded_xl()
                .bg(Color::Inset.get(cx))
                .border_1()
                .border_color(Color::Divider.get(cx))
                .child(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .px_0p5()
                        .child(
                            div()
                                .size(px(6.))
                                .rounded_full()
                                .flex_shrink_0()
                                .bg(if running {
                                    Color::Accent.get(cx)
                                } else {
                                    Color::Muted.get(cx)
                                }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Color::Secondary.get(cx))
                                .child(tr("已接收消息")),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py(px(1.))
                                .rounded_full()
                                .bg(Color::Card.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .text_size(px(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Color::Muted.get(cx))
                                .child(count.to_string()),
                        ),
                )
                .child(v_flex().id("queued-user-inputs").gap_1p5()
                    .key_context("HiveQueuedInputs")
                    .on_action(cx.listener(Self::steer_next_queued_input))
                    .children(
                    undisplayed.into_iter().map(|input| {
                        let text = input.text.clone();
                        let edit_input = input.clone();
                        let cancel_input = input.clone();
                        let steering = self.steering_queued_input(&input.delivery_id);
                        let status = match input.reason.as_deref() {
                            Some("completed") => tr("消息已处理，不能编辑或撤回"),
                            Some("attempted") => tr("消息已尝试发送，不能编辑或撤回"),
                            Some("legacy") => tr("旧版待发消息缺少原始记录，不能编辑或撤回"),
                            _ if steering => tr("正在发送到当前回合…"),
                            _ if input.processing => tr("已接收，正在处理"),
                            _ if running => tr("已入队，等待当前回复结束"),
                            _ => tr("已入队，等待继续处理"),
                        };
                        let label = if input.text.is_empty() {
                            tr("附件消息").to_string()
                        } else {
                            input.text.clone()
                        };
                        let detail = if input.attachments.is_empty() {
                            status.to_string()
                        } else {
                            format!(
                                "{} · {}: {}",
                                status,
                                tr("附件"),
                                input
                                    .attachments
                                    .iter()
                                    .map(|item| item.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(if english() { ", " } else { "、" })
                            )
                        };
                        crate::focus_scroll::Reveal::new(
                            SharedString::from(format!(
                                "received-input-reveal-{}",
                                input.delivery_id
                            )),
                            &self.composer_scroll,
                            h_flex()
                                .id(SharedString::from(format!(
                                    "received-input-{}",
                                    input.delivery_id
                                )))
                                .role(Role::Group)
                                .aria_label(format!("{label} · {detail}"))
                                .items_center()
                                .min_w_0()
                                .gap_2()
                                .px_2p5()
                                .py_1p5()
                                .rounded_lg()
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .bg(Color::Card.get(cx))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Text.get(cx))
                                                .truncate()
                                                .child(label),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(Color::Muted.get(cx))
                                                .truncate()
                                                .child(detail),
                                        ),
                                )
                                .when(input.steerable && running, |row| {
                                    let delivery = input.delivery_id.clone();
                                    let expected = input.text.clone();
                                    let shortcut = if cfg!(target_os = "macos") {
                                        "⌘⇧↵"
                                    } else {
                                        "Ctrl+Shift+Enter"
                                    };
                                    row.child(
                                        Button::new(SharedString::from(format!("steer-queued-{delivery}")))
                                            .custom(crate::palette::subtle_icon_variant(cx))
                                            .compact()
                                            .h(px(24.))
                                            .w(px(24.))
                                            .rounded_md()
                                            .icon(Glyph::ArrowUp)
                                            .tooltip(format!("{} ({shortcut})", tr("发送到当前回合")))
                                            .accessibility_label(tr("发送到当前回合"))
                                            .disabled(!self.can_steer_queued_input(input))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.steer_queued_input_now(&delivery, &expected, cx);
                                            })),
                                    )
                                })
                                .when(input.editable && !input.processing, |row| {
                                    let edit_tooltip = if latest_editable_id == Some(input.delivery_id.as_str()) {
                                        let shortcut = if cfg!(target_os = "macos") {
                                            "⌥↑"
                                        } else {
                                            "Alt+↑"
                                        };
                                        format!("{} ({shortcut})", tr("编辑待发消息"))
                                    } else {
                                        tr("编辑待发消息").to_owned()
                                    };
                                    row.child(
                                        Button::new(SharedString::from(format!(
                                            "edit-queued-{}",
                                            input.delivery_id
                                        )))
                                        .custom(crate::palette::subtle_icon_variant(cx))
                                        .compact()
                                        .h(px(24.))
                                        .w(px(24.))
                                        .rounded_md()
                                        .icon(Glyph::Pencil)
                                        .tooltip(edit_tooltip)
                                        .accessibility_label(tr("编辑待发消息"))
                                        .disabled(!self.connected || self.closing || steering)
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.open_queue_editor(
                                                    edit_input.clone(),
                                                    false,
                                                    window,
                                                    cx,
                                                )
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "cancel-queued-{}",
                                            input.delivery_id
                                        )))
                                        .custom(crate::palette::subtle_icon_variant(cx))
                                        .compact()
                                        .h(px(24.))
                                        .w(px(24.))
                                        .rounded_md()
                                        .icon(Glyph::X)
                                        .tooltip(tr("撤回待发消息"))
                                        .accessibility_label(tr("撤回待发消息"))
                                        .disabled(!self.connected || self.closing || steering)
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                if crate::general::current(cx)
                                                    .confirm_cancel_queued_input
                                                {
                                                    this.open_queue_editor(
                                                        cancel_input.clone(),
                                                        true,
                                                        window,
                                                        cx,
                                                    );
                                                } else {
                                                    this.cancel_queued_input_now(
                                                        cancel_input.clone(),
                                                        cx,
                                                    );
                                                }
                                            }),
                                        ),
                                    )
                                })
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "copy-queued-{}",
                                        input.delivery_id
                                    )))
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .h(px(24.))
                                    .w(px(24.))
                                    .rounded_md()
                                    .icon(Glyph::Copy)
                                    .tooltip(tr("复制"))
                                    .accessibility_label(tr("复制已接收消息"))
                                    .disabled(input.text.is_empty())
                                    .on_click(
                                        move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                text.clone(),
                                            ))
                                        },
                                    ),
                                ),
                        )
                    }),
                ))
                .into_any_element(),
        )
    }

    pub(crate) fn edit_latest_queued_input(
        &mut self,
        _: &crate::EditLatestQueuedInput,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_session = self.session.as_ref().is_some_and(|session| {
            self.workspace.as_ref() == Some(&session.workspace_id)
                && self
                    .draft_target
                    .as_deref()
                    .and_then(|target| target.strip_prefix("session:"))
                    == Some(session.id.as_str())
                && self.draft_owner == self.draft_target
        });
        if !active_session
            || !self.connected
            || self.closing
            || self.loading
            || !self.conversation_ready
            || !self.viewing_conversation()
            || !self.workspace_writes_allowed()
            || self.conversation_is_read_only()
            || self.command_picker.open
            || window.has_active_dialog(cx)
            || !self.input.read(cx).focus_handle(cx).is_focused(window)
        {
            cx.propagate();
            return;
        }
        let Some(input) = self.latest_queue_edit_target().cloned() else {
            cx.propagate();
            return;
        };
        if self.input.update(cx, |state, cx| {
            can_edit_queue_at_caret(
                state.selected_range(),
                state.marked_text_range(window, cx).is_some(),
            )
        }) {
            cx.stop_propagation();
            self.open_queue_editor(input, false, window, cx);
        } else {
            cx.propagate();
        }
    }

    fn latest_queue_edit_target(&self) -> Option<&PendingUserInput> {
        latest_queued_input(&self.pending_user_inputs).filter(|input| {
            !self.steering_queued_input(&input.delivery_id)
                && !self.message_indices.contains_key(&input.delivery_id)
        })
    }

    fn steering_queued_input(&self, delivery: &str) -> bool {
        self.session.as_ref().is_some_and(|session| {
            self.steering_queued_inputs
                .iter()
                .any(|(connection, session_id, delivery_id)| {
                    *connection == self.connection_generation
                        && *session_id == session.id
                        && delivery_id == delivery
                })
        })
    }

    fn can_steer_queued_input(&self, input: &PendingUserInput) -> bool {
        input.steerable
            && input.editable
            && !input.processing
            && self.connected
            && !self.closing
            && !self.loading
            && self.workspace_writes_allowed()
            && !self.conversation_is_read_only()
            && self.viewing_conversation()
            && self
                .session
                .as_ref()
                .is_some_and(|session| self.busy.contains(&session.id))
            && !self.steering_queued_input(&input.delivery_id)
            && !self.message_indices.contains_key(&input.delivery_id)
    }

    fn steer_next_queued_input(
        &mut self,
        _: &SteerPendingInput,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .pending_user_inputs
            .iter()
            .find(|input| self.can_steer_queued_input(input));
        if let Some(input) = target {
            let (delivery, expected) = (input.delivery_id.clone(), input.text.clone());
            self.steer_queued_input_now(&delivery, &expected, cx);
        } else {
            cx.propagate();
        }
    }

    fn steer_queued_input_now(&mut self, delivery: &str, expected: &str, cx: &mut Context<Self>) {
        if !self.pending_user_inputs.iter().any(|input| {
            input.delivery_id == delivery
                && input.text == expected
                && self.can_steer_queued_input(input)
        }) {
            return;
        }
        let (Some(runtime), Some(session)) = (self.runtime.clone(), self.session.as_ref()) else {
            return;
        };
        let session_id = session.id.clone();
        let delivery = delivery.to_owned();
        let connection = self.connection_generation;
        let view = self.open_request;
        let operation = (connection, session_id.clone(), delivery.clone());
        self.steering_queued_inputs.insert(operation.clone());
        let params = serde_json::json!({
            "sessionId": session_id,
            "deliveryId": delivery,
            "expectedText": expected,
        });
        cx.spawn(async move |this, cx| {
            let result = runtime.request("chat.queue.steer", params).await;
            let _ = this.update(cx, |this, cx| {
                this.steering_queued_inputs.remove(&operation);
                if this.connection_generation != connection || this.open_request != view {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(value) if value["deliveryId"].as_str() == Some(delivery.as_str()) => {
                        if let Some(input) = this
                            .pending_user_inputs
                            .iter_mut()
                            .find(|input| input.delivery_id == delivery)
                        {
                            input.steerable = false;
                        }
                    }
                    Ok(_) => {
                        this.error = Some("后台未确认待发消息操作。".into());
                    }
                    Err(error) => {
                        this.error = Some(crate::app::ErrorNotice::Detail(
                            "无法将待发消息发送到当前回合",
                            error,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn cancel_queued_input_now(
        &mut self,
        input: crate::protocol::PendingUserInput,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self.closing
            || !self.workspace_writes_allowed()
            || !input.editable
            || input.processing
        {
            return;
        }
        let Some(session_id) = self.session.as_ref().map(|session| session.id.clone()) else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.queue.cancel",
                    serde_json::json!({
                        "sessionId": session_id,
                        "deliveryId": input.delivery_id,
                        "expectedText": input.text,
                    }),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                match result {
                    Ok(value) if value["ok"] == true => {}
                    Ok(_) => {
                        this.error = Some("后台未确认待发消息操作。".into());
                    }
                    Err(error) => {
                        this.error = Some(crate::app::ErrorNotice::Detail(
                            "撤回待发消息失败",
                            error,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{can_edit_queue_at_caret, latest_queued_input};
    use crate::protocol::PendingUserInput;

    fn input(id: &str, editable: bool, processing: bool) -> PendingUserInput {
        PendingUserInput {
            delivery_id: id.into(),
            text: format!("Queued {id}"),
            attachments: Vec::new(),
            processing,
            editable,
            steerable: false,
            reason: None,
        }
    }

    #[test]
    fn queue_edit_requires_a_collapsed_selection_at_document_start() {
        assert!(can_edit_queue_at_caret(0..0, false));
        assert!(!can_edit_queue_at_caret(0..0, true));
        assert!(!can_edit_queue_at_caret(0..4, false));
        assert!(!can_edit_queue_at_caret(4..4, false));
    }

    #[test]
    fn queue_edit_targets_the_latest_row_and_does_not_fall_back() {
        assert!(latest_queued_input(&[]).is_none());
        let first = input("first", true, false);
        let mut tail = input("latest", true, false);
        let queue = [first.clone(), tail.clone()];
        assert_eq!(latest_queued_input(&queue).unwrap().delivery_id, "latest");
        tail.processing = true;
        assert!(latest_queued_input(&[first.clone(), tail.clone()]).is_none());
        tail.processing = false;
        tail.editable = false;
        assert!(latest_queued_input(&[first, tail]).is_none());
    }
}
