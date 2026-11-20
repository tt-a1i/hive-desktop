use crate::{
    app::{DeliveryCard, ErrorNotice, HiveApp, Message, ToolOutputWindow, ToolPresentation},
    composer::SendIntent,
    general::BusySendMode,
    protocol::*,
};
use gpui_kit::{component::WindowExt, *};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::HashSet;

fn assistant_tool_calls(message: &Value) -> impl Iterator<Item = &Value> {
    message["content"].as_array()
        .filter(|_| message["role"] == "assistant")
        .into_iter()
        .flatten()
        .filter(|block| block["type"] == "toolCall")
}

fn unresolved_tool_calls(ids: &[String], history: &[Value]) -> HashSet<String> {
    let mut pending: HashSet<_> = history
        .iter()
        .flat_map(assistant_tool_calls)
        .filter_map(|call| call["id"].as_str().filter(|id| !id.is_empty()))
        .collect();
    if !pending.is_empty() {
        for (id, message) in ids.iter().zip(history) {
            if message["role"] == "toolResult" {
                pending.remove(message["toolCallId"].as_str().unwrap_or(id));
            }
        }
    }
    pending.into_iter().map(str::to_owned).collect()
}

fn accepts_background_send(value: &Value, source: &Session) -> bool {
    value["accepted"].as_bool() == Some(true)
        && value["session"]["backend"].as_str() == Some(source.backend.wire())
        && serde_json::from_value::<Session>(value["session"].clone()).is_ok_and(|acknowledged| {
            acknowledged.id == source.id
                && acknowledged.workspace_id == source.workspace_id
                && acknowledged.profile == source.profile
        })
}

impl HiveApp {
    pub(crate) fn clear_conversation_messages(&mut self, cx: &mut Context<Self>) {
        self.message_indices = Default::default();
        let images = std::mem::take(&mut self.messages)
            .into_iter()
            .flat_map(|message| message.images)
            .collect::<Vec<_>>();
        release_conversation_images(images, cx);
    }

    pub(crate) fn send(&mut self, intent: SendIntent, window: &mut Window, cx: &mut Context<Self>) {
        if self.conversation_is_read_only() || !self.conversation_ready || !self.workspace_writes_allowed() {
            return;
        }
        let Some(source) = self.session.as_ref() else {
            return;
        };
        let fresh_draft = if matches!(intent, SendIntent::AndStartDraft) {
            if window.has_active_dialog(cx) || !self.can_send_and_start_draft(cx) {
                return;
            }
            Some(source.clone())
        } else {
            None
        };
        let id = source.id.clone();
        self.showing_welcome_guide = false;
        let text = self.input.read(cx).value().to_string();
        if self.current_history_access().is_some_and(|a| a.handoff_blocked()) { self.error = Some("接续状态需要先刷新核验。".into()); cx.notify(); return; }
        if self.current_history_access().is_some_and(|a| a.handoff_unconsumed()) && (text.trim().is_empty() || text.trim_start().starts_with('/')) {
            self.error = Some(if crate::language::english() { "The first handoff input must be ordinary nonempty text." } else { "接续会话的第一条输入必须是普通非空文本。" }.into());
            cx.notify(); return;
        }
        let attachment_key = format!("session:{id}");
        let refs = self.attachments.refs(Some(&attachment_key));
        if self.session.as_ref().is_some_and(|s| matches!(s.backend, Backend::Acp)) && refs.iter().any(|item| item.is_image()) { self.error = Some("当前 ACP 不支持图片，请移除图片后发送。".into()); cx.notify(); return; }
        let was_busy = self.busy.contains(&id);
        let configured = crate::general::current(cx).busy_send_mode;
        if (text.trim().is_empty() && refs.is_empty())
            || !self.attachments.ready(Some(&attachment_key))
            || self.inflight_inputs.contains_key(&id)
            || (!was_busy && self.pending.get(&id).copied().unwrap_or(0) > 0)
            || (was_busy && configured == BusySendMode::BlockUntilIdle)
            || self.loading
            || !self.connected
        {
            return;
        }
        if self.closing {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let follow_up = if was_busy && matches!(intent, SendIntent::AlternateWhenBusy) {
            configured.alternate().unwrap_or(configured)
        } else {
            configured
        };
        let busy_mode = match follow_up {
            BusySendMode::SteerWhenSupported => "steer",
            _ => "queue",
        };
        self.busy.insert(id.clone());
        self.inflight_inputs.insert(id.clone(), text.clone());
        self.drafts.remove(&format!("session:{id}"));
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.schedule_draft_save(cx);
        let window = window.window_handle();
        let connection = self.connection_generation;
        let open_request = self.open_request;
        let mut prompt = json!({
            "sessionId": id, "text": text,
            "attachmentIds": refs.iter().map(|item| &item.id).collect::<Vec<_>>(),
        });
        if fresh_draft.is_some() {
            prompt["requireIdle"] = json!(true);
        } else {
            prompt["busyMode"] = json!(busy_mode);
        }
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.prompt", prompt)
                .await
                .map_err(ErrorNotice::from)
                .and_then(|value| {
                    if fresh_draft.as_ref().is_some_and(|source| !accepts_background_send(&value, source)) {
                        Err(ErrorNotice::Message("未能确认原会话已接收消息"))
                    } else {
                        Ok(value)
                    }
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.inflight_inputs.remove(&id);
                if this.removed_sessions.contains(&id) {
                    return;
                }
                if result.is_ok() {
                    if let Some(items)=this.attachments.drafts.get_mut(&attachment_key) {items.retain(|item| !refs.iter().any(|sent| sent.id==item.id));}
                    this.prune_draft_attachments(cx);
                    this.schedule_draft_save(cx);
                }
                match result {
                    Err(error) => {
                        let refresh_handoff = this.session.as_ref().is_some_and(|s| s.id == id)
                            && this.current_history_access().is_some_and(|a| a.handoff_unconsumed());
                        if refresh_handoff {
                            this.history_navigation.access.handoff_delivery = Some(json!({"status":"unknown"}));
                        }
                        if !was_busy { this.busy.remove(&id); }
                        let showing_source = this.session.as_ref().is_some_and(|s| s.id == id);
                        let source_owns_editor = showing_source && (fresh_draft.is_none()
                            || this.draft_owner == this.draft_target
                                && this.draft_target.as_deref() == Some(attachment_key.as_str()));
                        if source_owns_editor && this.input.read(cx).value().is_empty()
                        {
                            let input = this.input.clone();
                            if window
                                .update(cx, |_, window, cx| {
                                    input.update(cx, |state, cx| {
                                        state.set_value(text.clone(), window, cx)
                                    });
                                })
                                .is_err()
                            {
                                this.failed_drafts
                                    .entry(id.clone())
                                    .or_default()
                                    .push(text.clone());
                            }
                        } else if !source_owns_editor && !this.drafts.contains_key(&attachment_key)
                        {
                            this.drafts.insert(format!("session:{id}"), text.clone());
                        } else {
                            this.failed_drafts
                                .entry(id.clone())
                                .or_default()
                                .push(text.clone());
                        }
                        this.schedule_draft_save(cx);
                        this.error = Some(error);
                        if refresh_handoff {
                            this.refresh_handoff_target_state(id.clone(), cx);
                        }
                    }
                    Ok(value) => {
                        let showing_source = this.session.as_ref().is_some_and(|s| s.id == id);
                        let refresh_handoff = showing_source
                            && this.current_history_access().is_some_and(|a| a.handoff_unconsumed());
                        let reopen_after_activation = showing_source && this.open_request == open_request
                            && value["passive"].as_bool() == Some(false)
                            && this.current_history_access().is_some_and(|access| {
                                access.passive == Some(true) && access.handoff_snapshot.is_none()
                            });
                        if refresh_handoff {
                            if value.get("passive").and_then(Value::as_bool) == Some(false) {
                                this.history_navigation.access.passive = Some(false);
                            }
                            this.history_navigation.access.handoff_delivery = Some(json!({"status":"unknown"}));
                        }
                        match serde_json::from_value::<Session>(value["session"].clone()) {
                            Ok(session) if session.id == id => {
                                if let Some(item) =
                                    this.sessions.iter_mut().find(|item| item.id == id)
                                {
                                    *item = session.clone();
                                }
                                if this.session.as_ref().is_some_and(|item| item.id == id) {
                                    this.session = Some(session);
                                }
                                if let Some(source) = &fresh_draft
                                    && showing_source && this.open_request == open_request
                                    && this.workspace.as_ref() == Some(&source.workspace_id)
                                    && this.viewing_conversation() && this.can_start_new_conversation()
                                {
                                    let width = window.update(cx, |_, window, cx| {
                                        (!window.has_active_dialog(cx)).then(|| window.viewport_size().width)
                                    }).ok().flatten();
                                    if let Some(width) = width {
                                        this.create_fresh_draft_after_send(source, window, width < px(820.), cx);
                                    }
                                } else if reopen_after_activation {
                                    this.show_session(id.clone(), cx);
                                }
                            }
                            Ok(_) => this.error = Some("发送响应中的会话身份不匹配".into()),
                            Err(error) => this.error = Some(crate::app::ErrorNotice::Detail("发送响应格式错误",error.to_string())),
                        }
                        if refresh_handoff && !this.busy.contains(&id) {
                            this.refresh_handoff_target_state(id.clone(), cx);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn resume(&mut self, cx: &mut Context<Self>) {
        if self.conversation_is_read_only() || !self.conversation_ready || !self.workspace_writes_allowed() {
            return;
        }
        let Some(id) = self.session.as_ref().map(|s| s.id.clone()) else {
            return;
        };
        if self.loading
            || self.busy.contains(&id)
            || self.pending.get(&id).copied().unwrap_or(0) == 0
        {
            return;
        }
        self.request("chat.resume", json!({"sessionId":id}), cx, |this, _, cx| {
            this.refresh_sessions(cx)
        });
    }
    fn apply_activity(&mut self, event: &PiEvent) {
        let id = event.session_id().to_string();
        match event {
            PiEvent::State {
                busy,
                pending,
                pending_reports,
                pending_user_inputs,
                completed_report_id,
                delivery_failure,
                handoff_delivery,
                ..
            } => {
                if self.session.as_ref().is_some_and(|s| s.id == id) {
                    self.pending_reports.clone_from(pending_reports);
                    self.pending_user_inputs.clone_from(pending_user_inputs);
                    let was_busy = self.busy.contains(&id);
                    if *busy
                        && matches!(self.status, crate::app::Status::Message("已停止当前回复"))
                    {
                        self.status = "后台已连接".into();
                    }
                    if (*busy && !was_busy)
                        || (!*busy
                            && was_busy
                            && delivery_failure.is_some()
                            && !self
                                .agent_diagnostic
                                .as_ref()
                                .is_some_and(|d| d.diagnostic_write_failed))
                    {
                        self.agent_diagnostic = None;
                    }
                    self.set_delivery_failure(delivery_failure.clone());
                    if let Some(delivery) = handoff_delivery {
                        if self.current_history_access().is_some_and(|access| access.handoff_snapshot.is_some()) {
                            self.history_navigation.access.handoff_delivery = Some(delivery.clone());
                        }
                    }
                    if let Some(id) = completed_report_id {
                        self.completed_deliveries.insert(id.clone());
                    }
                }
                if *busy {
                    if delivery_failure.is_some() {
                        self.sidebar_failed_turns.insert(id.clone());
                    } else if !self.busy.contains(&id) {
                        self.sidebar_failed_turns.remove(&id);
                    }
                    self.busy.insert(id.clone());
                } else {
                    self.busy.remove(&id);
                    self.sidebar_failed_turns.remove(&id);
                }
                self.pending.insert(id, *pending);
            }
            PiEvent::Settled { .. } => {
                self.busy.remove(&id);
                self.sidebar_failed_turns.remove(&id);
            }
            // A backend can report failure before the queue finishes stopping.
            // Return the row to the regular sidebar immediately.
            PiEvent::Error { .. } if self.busy.contains(&id) => {
                self.sidebar_failed_turns.insert(id);
            }
            PiEvent::AssistantDelta { .. } | PiEvent::ToolStart { .. } => {
                self.busy.insert(id);
            }
            _ => {}
        }
        // Background deliveries can start a turn while the model menu is open.
        // Clear its controlled state before the busy trigger replaces the popover.
        if self.session.as_ref().is_some_and(|session| self.busy.contains(&session.id)) {
            self.choosing_model = false;
        }
    }
    pub fn abort(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &self.session {
            self.request(
                "chat.abort",
                json!({"sessionId":session.id}),
                cx,
                |_, _, _| {},
            );
        }
    }
    pub fn refresh_pi_models(&mut self, cx: &mut Context<Self>) {
        if self.conversation_is_read_only() || !self.conversation_ready || self.refreshing_models || !self.connected {
            return;
        }
        let (Some(session), Some(runtime)) = (&self.session, self.runtime.clone()) else {
            return;
        };
        let id = session.id.clone();
        let open = self.open_request;
        self.refreshing_models = true;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.refreshPiModels", json!({"sessionId":id}))
                .await
                .and_then(|v| serde_json::from_value::<Vec<Model>>(v).map_err(|e| e.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                if this.open_request != open {
                    return;
                }
                this.refreshing_models = false;
                match result {
                    Ok(models) => {
                        let selected = this
                            .model
                            .as_ref()
                            .or(this.unavailable_model.as_ref())
                            .cloned();
                        if let Some(selected) = selected {
                            if let Some(current) = models.iter().find(|model| {
                                model.available
                                    && model.provider == selected.provider
                                    && model.id == selected.id
                            }) {
                                this.model = Some(ModelIdentity {
                                    provider: current.provider.clone(),
                                    id: current.id.clone(),
                                });
                                this.unavailable_model = None;
                            } else {
                                this.unavailable_model = Some(selected);
                                this.model = None;
                            }
                        }
                        this.choosing_model = models.iter().any(|m| m.available);
                        this.models = models;
                    }
                    Err(e) => {
                        if let Some(selected) = this.model.take() {
                            this.unavailable_model = Some(selected);
                        }
                        this.error = Some(crate::app::ErrorNotice::Detail("刷新 Pi 模型失败",e.to_string()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn set_model(&mut self, model: Model, cx: &mut Context<Self>) {
        if self.conversation_is_read_only() || !self.conversation_ready || !self.workspace_writes_allowed() {
            return;
        }
        if let Some(session) = &self.session {
            let id = session.id.clone();
            self.request(
                "chat.setModel",
                json!({"sessionId":id,"provider":model.provider,"modelId":model.id}),
                cx,
                move |this, _, _| {
                    if this.session.as_ref().map(|s| &s.id) == Some(&id) {
                        this.choosing_model = false;
                        this.reasoning_picker = Default::default();
                        this.mode_picker = Default::default();
                        this.reset_usage();
                        this.unavailable_model = None;
                        this.model = Some(ModelIdentity {
                            provider: model.provider,
                            id: model.id,
                        });
                    }
                },
            );
        }
    }
    pub(crate) fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        if self.history_navigation.live_start.is_some()
            && let Some(session) = self.session.as_ref()
        {
            if self.connected && !self.closing {
                self.show_latest_session(session.id.clone(), cx);
            }
            return;
        }
        self.conversation_find = None;
        self.conversation_list.pending_anchor = None;
        // Update the header action now, then pin to the final offset after layout.
        self.conversation_list
            .state
            .set_follow_mode(FollowMode::Tail);
        self.conversation_list.state.scroll_to_end();
        self.schedule_reading_position_save(cx);
        cx.notify();
    }

    pub(crate) fn reading_above_latest(&self) -> bool {
        self.history_navigation.live_start.is_some()
            || !self.conversation_list.state.is_following_tail()
    }

    pub(crate) fn message_index(&self, id: &str) -> Option<usize> {
        self.message_indices.get(id).copied()
    }

    pub(crate) fn message_mut(&mut self, id: &str) -> Option<&mut Message> {
        let index = self.message_index(id)?;
        self.messages.get_mut(index)
    }

    pub fn push_message(&mut self, id: String, role: String, text: String, cx: &mut Context<Self>) {
        if !matches!(
            role.as_str(),
            "user" | "assistant" | "tool" | "toolResult" | "notice"
        ) || self.message_indices.contains_key(&id) {
            return;
        }
        // The row's focus region is stable. GPUI can keep a focused message
        // alive during scrolling even when the parsed body is viewport-owned.
        let markdown_focus = (role == "assistant").then(|| cx.focus_handle());
        self.message_indices.insert(id.clone(), self.messages.len());
        self.messages.push(Message {
            layout_revision: 0,
            id,
            role,
            native_user_item_id: None,
            handoff_item_id: None,
            source_session_id: None,
            text,
            markdown_focus,
            tool: None,
            thinking: vec![],
            delivery: None,
            explicit_user: false,
            failure: None,
            images: vec![],
            files: vec![],
            image_sources: vec![],
            history_images: None,
            images_loading: false,
            images_error: None,
            image_task: None,
        });
    }
    pub(crate) fn restore_conversation_history(
        &mut self,
        message_ids: Vec<String>,
        history: Vec<Value>,
        cx: &mut Context<Self>,
    ) {
        let mut unresolved = unresolved_tool_calls(&message_ids, &history);
        for (message_id, message) in message_ids.into_iter().zip(history) {
            let handoff_id = crate::session_handoff::historical_item_id(&message);
            self.receive_message(message_id.clone(), &message, cx);
            if let Some(rendered) = self.message_mut(&message_id)
                && matches!(rendered.role.as_str(), "user" | "assistant")
            {
                rendered.handoff_item_id = handoff_id;
            }
            // Saved requests can outlive a stopped provider. Keep the original
            // call ID available to search, and leave completed rows in place.
            for call in assistant_tool_calls(&message) {
                if let Some(id) = call["id"].as_str()
                    && unresolved.remove(id)
                    && self.message_index(id).is_none()
                {
                    let name = call["name"].as_str().unwrap_or("工具").to_owned();
                    self.upsert_tool(id.to_owned(), name, "未记录结果", String::new(), cx);
                    if let Some(rendered) = self.message_mut(id) {
                        rendered.source_session_id =
                            message["sourceSessionId"].as_str().map(str::to_owned);
                    }
                }
            }
        }
    }

    pub(crate) fn prepend_conversation_history(
        &mut self,
        message_ids: Vec<String>,
        history: Vec<Value>,
        cx: &mut Context<Self>,
    ) {
        if message_ids.is_empty() {
            return;
        }
        let live_start = self.history_navigation.live_start.take();
        let existing_messages = std::mem::take(&mut self.messages);
        let existing_indices = std::mem::take(&mut self.message_indices);
        self.restore_conversation_history(message_ids, history, cx);
        let mut duplicate_images = Vec::new();
        self.messages.retain_mut(|message| {
            if existing_indices.contains_key(&message.id) {
                duplicate_images.append(&mut message.images);
                return false;
            }
            true
        });
        self.message_indices = self.messages.iter().enumerate()
            .map(|(index, message)| (message.id.clone(), index))
            .collect();
        let prepended_len = self.messages.len();
        self.messages.extend(existing_messages);
        for (id, idx) in existing_indices {
            self.message_indices.insert(id, idx + prepended_len);
        }
        self.history_navigation.live_start = live_start.map(|index| index + prepended_len);
        release_conversation_images(duplicate_images, cx);
        self.conversation_list.state.pause_following_tail();
    }

    pub(crate) fn append_conversation_history(
        &mut self,
        message_ids: Vec<String>,
        history: Vec<Value>,
        has_newer: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(live_start) = self.history_navigation.live_start.take() else {
            return;
        };
        let was_at_gap = self.conversation_list.anchor().is_none_or(|anchor| {
            use crate::conversation_list::RowKey;
            matches!(anchor.key, RowKey::LoadNewer | RowKey::Tail)
        });
        let live = self.messages.split_off(live_start);
        for message in &live {
            self.message_indices.remove(&message.id);
        }
        self.restore_conversation_history(message_ids, history, cx);
        let new_live_start = self.messages.len();
        self.history_navigation.live_start = has_newer.then_some(new_live_start);
        let mut duplicate_images = Vec::new();
        for mut message in live {
            if let Some(index) = self.message_index(&message.id) {
                // A live update can race the page read. Keep the state already
                // delivered to this view, at the position the saved page supplies.
                message.layout_revision =
                    message.layout_revision.max(self.messages[index].layout_revision) + 1;
                let mut duplicate = std::mem::replace(&mut self.messages[index], message);
                duplicate_images.append(&mut duplicate.images);
            } else {
                self.message_indices.insert(message.id.clone(), self.messages.len());
                self.messages.push(message);
            }
        }
        release_conversation_images(duplicate_images, cx);
        self.conversation_list.state.pause_following_tail();
        if was_at_gap && let Some(message) = self.messages.get(live_start) {
            self.conversation_list.pending_anchor = Some(crate::conversation_list::ReadingAnchor {
                key: crate::conversation_list::RowKey::Message(message.id.clone()),
                offset: px(0.),
            });
        }
    }

    // Both snapshots and live events use the same provenance contract. Never infer
    // a source from message text: users may paste a complete Hive envelope.
    pub(crate) fn receive_message(
        &mut self,
        mut id: String,
        message: &Value,
        cx: &mut Context<Self>,
    ) {
        let role = message["role"].as_str().unwrap_or("assistant");
        if role == "assistant" {
            for block in assistant_tool_calls(message) {
                if let (Some(id), Some(args)) = (block["id"].as_str(), block.get("arguments")) {
                    self.tool_inputs.insert(id.to_owned(), args.clone());
                }
            }
            self.sync_thinking(&id, message, false, cx);
        }
        let (text, output_windows) = if role == "toolResult" {
            history_tool_output(&id, message)
        } else {
            (message_text(message), Vec::new())
        };
        let presentation = if matches!(role, "custom" | "user") {
            delivery_presentation(message)
        } else {
            None
        };
        if (role == "custom" && message["customType"] == "hive-delivery")
            || (role == "user" && presentation.is_some())
        {
            id = presentation
                .as_ref()
                .map(|p| p.delivery_id.clone())
                .unwrap_or(id);
            let explicit_user = presentation
                .as_ref()
                .is_some_and(|p| p.origin == DeliveryOrigin::User);
            let missing_provenance = presentation.is_none();
            let display = presentation
                .as_ref()
                .map(|p| {
                    // A system delivery without a separate display summary is
                    // internal context. Keep it in the existing raw-message
                    // disclosure rather than repeating the envelope in chat.
                    if p.origin == DeliveryOrigin::System && p.display_text == text {
                        String::new()
                    } else {
                        p.display_text.clone()
                    }
                })
                .unwrap_or_default();
            let heading = presentation.as_ref().map(|p| p.origin.heading()).unwrap_or("来源未记录");
            let worker_name = presentation.as_ref().and_then(|p| p.worker_name.clone());
            let expanded = self
                .message_index(&id)
                .and_then(|index| self.messages.get(index))
                .and_then(|m| m.delivery.as_ref())
                .is_some_and(|d| d.expanded);
            // Notice is a rendered role, never an engine role.
            self.upsert(
                id.clone(),
                if explicit_user { "user" } else { "notice" },
                display,
                false,
                cx,
            );
            self.load_message_images(id.clone(), message, cx);
            if let Some(rendered) = self.message_mut(&id) {
                rendered.explicit_user = explicit_user;
                let files = presentation
                    .as_ref()
                    .map(|p| {
                        p.attachments
                            .iter()
                            .filter(|item| !item.is_image())
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if rendered.files != files {
                    rendered.files = files;
                    rendered.layout_revision += 1;
                }

                let next_missing = rendered
                    .delivery
                    .as_ref()
                    .is_none_or(|d| d.missing_provenance != missing_provenance);
                rendered.delivery = if explicit_user {
                    None
                } else {
                    Some(DeliveryCard {
                        dispatch_id: presentation.as_ref().and_then(|p| p.dispatch_id.clone()),
                        report: presentation
                            .as_ref()
                            .is_some_and(|p| p.origin == DeliveryOrigin::Report),
                        heading,
                        worker_name,
                        worker_id: presentation.as_ref().and_then(|p| p.worker_id.clone()),
                        artifacts: presentation.map(|p| p.artifacts).unwrap_or_default(),
                        raw: text,
                        expanded,
                        missing_provenance,
                    })
                };
                if next_missing && rendered.delivery.is_some() {
                    rendered.layout_revision += 1;
                }
            }
        } else if role == "toolResult" {
            if let (Some(call), Some(args)) =
                (message["toolCallId"].as_str(), message.get("toolInput"))
            {
                self.tool_inputs.insert(call.to_owned(), args.clone());
            }
            self.upsert_tool(
                message["toolCallId"].as_str().unwrap_or(&id).into(),
                message["toolName"].as_str().unwrap_or("工具").into(),
                crate::tool_message::result_status(message, message["isError"] == true),
                text,
                cx,
            );
            if let Some(rendered) = self.message_mut(message["toolCallId"].as_str().unwrap_or(&id)) {
                if let Some(tool) = &mut rendered.tool {
                    tool.output_windows = output_windows;
                }
            }
            self.retain_tool_patch(message["toolCallId"].as_str().unwrap_or(&id), message, cx);
        } else {
            self.upsert(id.clone(), role, text, false, cx);
            if role == "assistant" {
                let failure = (message["stopReason"] == "error").then(|| {
                    message["errorMessage"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                });
                if failure.is_some() && self.message_index(&id).is_none() {
                    self.push_message(id.clone(), role.into(), String::new(), cx);
                }
                if let Some(rendered) = self.message_mut(&id) {
                    if rendered.failure.as_ref().map(|f| &f.message) != failure.as_ref() {
                        rendered.failure = failure.map(|message| crate::failures::Diagnostic {
                            message,
                            expanded: false,
                            diagnostic_write_failed: false,
                        });
                        rendered.layout_revision += 1;
                    }
                }
            }
            self.load_message_images(id.clone(), message, cx);
        }
        let rendered_id = if role == "toolResult" { message["toolCallId"].as_str().unwrap_or(&id) } else { &id };
        if let Some(rendered) = self.message_mut(rendered_id) {
            rendered.native_user_item_id = (message["rawBackendItem"]["type"] == "userMessage")
                .then(|| message["rawBackendItem"]["id"].as_str().map(str::to_owned)).flatten();
            rendered.source_session_id = message["sourceSessionId"].as_str().map(str::to_owned);
        }
        self.refresh_conversation_find(&id, cx);
    }
    pub(crate) fn upsert_tool(
        &mut self,
        id: String,
        name: String,
        status: &'static str,
        output: String,
        cx: &mut Context<Self>,
    ) {
        self.upsert(id.clone(), "toolResult", output, false, cx);
        if let Some(message) = self.message_mut(&id) {
            if let Some(tool) = &mut message.tool {
                tool.name = name;
                tool.status = status;
                tool.output_windows.clear();
            } else {
                message.tool = Some(ToolPresentation {
                    patch: None,
                    name,
                    status,
                    expanded: false,
                    details_scroll: ScrollHandle::new(),
                    output_windows: Vec::new(),
                });
            }
        }
        self.refresh_conversation_find(&id, cx);
    }
    pub(crate) fn upsert(
        &mut self,
        id: String,
        role: &str,
        text: String,
        append: bool,
        cx: &mut Context<Self>,
    ) {
        let index = self.message_index(&id);
        if !append
            && role == "assistant"
            && text.trim().is_empty()
            && index.is_none_or(|index| self.messages[index].thinking.is_empty())
        {
            if let Some(index) = index {
                let message = self.messages.remove(index);
                if let Some(live_start) = &mut self.history_navigation.live_start
                    && index < *live_start
                {
                    *live_start -= 1;
                }
                self.message_indices.remove(&id);
                for (offset, shifted) in self.messages[index..].iter().enumerate() {
                    *self.message_indices.get_mut(&shifted.id).unwrap() = index + offset;
                }
                self.conversation_list.markdown.remove(&id);
                release_conversation_images(message.images, cx);
            }
            self.refresh_conversation_find(&id, cx);
            return;
        }
        if let Some(index) = index {
            let message = &mut self.messages[index];
            message.layout_revision += 1;
            if append {
                message.text.push_str(&text);
            } else {
                message.text = text;
            }
            if let Some(markdown) = self.conversation_list.markdown.get(&id) {
                markdown.state.update(cx, |state, cx| state.set_text(&message.text, cx));
            }
        } else {
            self.push_message(id.clone(), role.into(), text, cx);
        }
        self.refresh_conversation_find(&id, cx);
    }
    pub(crate) fn agent_event(&mut self, event: PiEvent, cx: &mut Context<Self>) {
        if self.removed_sessions.contains(event.session_id()) {
            return;
        }
        if let PiEvent::MessageEnd { session_id, inbox_reply: Some(reply), .. } = &event {
            self.note_live_inbox_reply(&reply.workspace_id, session_id, reply.revision, reply.received_at);
        }
        // Keep current-session navigation in the same snapshot replay as chat
        // events, so an in-flight open cannot replace newer metadata.
        if self.loading && self.session.as_ref().map(|s| s.id.as_str()) == Some(event.session_id())
        {
            self.pending_events.push(event);
            return;
        }
        if let PiEvent::SessionNavigation {
            session_id,
            sequence,
            archived,
            pinned,
            updated_at,
        } = &event
        {
            let seen = self
                .background_sequences
                .get(session_id)
                .copied()
                .unwrap_or(0)
                .max(
                    if self
                        .session
                        .as_ref()
                        .is_some_and(|session| session.id == *session_id)
                    {
                        self.event_sequence
                    } else {
                        0
                    },
                );
            if *sequence > seen {
                self.background_sequences
                    .insert(session_id.clone(), *sequence);
                self.apply_archive_navigation(session_id, *archived, *pinned, *updated_at, cx);
            }
            return;
        }
        if self.session.as_ref().map(|s| s.id.as_str()) != Some(event.session_id()) {
            let previous = self
                .background_sequences
                .entry(event.session_id().into())
                .or_default();
            if event.sequence() <= *previous {
                return;
            }
            *previous = event.sequence();
            if matches!(&event, PiEvent::SessionModeChanged { .. }) {
                return;
            }
            self.local_notification_event(&event, cx);
            self.apply_activity(&event);
            self.apply_interaction_event(&event);
            if !matches!(&event, PiEvent::State { .. } | PiEvent::Settled { .. }) {
                self.unread.insert(event.session_id().into());
            }
            self.refresh_sessions(cx);
            return;
        }
        if event.sequence() <= self.event_sequence {
            return;
        }
        self.event_sequence = event.sequence();
        self.background_sequences
            .entry(event.session_id().into())
            .and_modify(|seen| *seen = (*seen).max(event.sequence()))
            .or_insert(event.sequence());
        self.local_notification_event(&event, cx);
        self.apply_activity(&event);
        self.apply_interaction_event(&event);
        if matches!(&event, PiEvent::State { .. } | PiEvent::Settled { .. }) {
            self.refresh_sessions(cx);
        }
        // GPUI stores a positive max_offset and a negative current offset.
        let follow = self.conversation_find.is_none() && !self.reading_above_latest();
        match event {
            PiEvent::SessionModeChanged { .. } => self.refresh_advertised_modes(cx),
            PiEvent::SessionNavigation { .. }
            | PiEvent::State { .. }
            | PiEvent::Interaction { .. }
            | PiEvent::InteractionResolved { .. } => {}
            PiEvent::AssistantDelta {
                message_id,
                kind,
                content_index,
                delta,
                ..
            } => {
                if kind == "thinking" {
                    self.append_thinking(message_id, content_index, delta, cx);
                } else if kind == "text" {
                    self.finish_thinking_before(&message_id, content_index);
                    self.upsert(message_id, "assistant", delta, true, cx);
                }
            }
            PiEvent::MessageEnd {
                message_id,
                message,
                ..
            } => {
                self.receive_message(message_id, &message, cx);
            }
            PiEvent::ToolStart {
                tool_call_id,
                tool_name,
                args,
                ..
            } => {
                self.tool_inputs.insert(tool_call_id.clone(), args);
                self.upsert_tool(tool_call_id, tool_name, "执行中", String::new(), cx);
            }
            PiEvent::ToolUpdate {
                tool_call_id,
                tool_name,
                result,
                ..
            } => {
                self.upsert_tool(tool_call_id, tool_name, "执行中", message_text(&result), cx);
            }
            PiEvent::ToolEnd {
                tool_call_id,
                tool_name,
                result,
                is_error,
                ..
            } => {
                let call_id = tool_call_id.clone();
                self.upsert_tool(
                    tool_call_id,
                    tool_name,
                    crate::tool_message::result_status(&result, is_error),
                    message_text(&result),
                    cx,
                );
                self.retain_tool_patch(&call_id, &result, cx);
            }
            PiEvent::Settled { aborted, .. } => {
                self.load_usage(cx);
                self.refresh_pending_session_modes(cx);
                self.finish_thinking(aborted);
                if aborted {
                    self.status = "已停止当前回复".into();
                }
                if let Some(id) = self.session.as_ref().map(|s| s.id.clone()) {
                    if self.current_history_access().is_some_and(|a| a.handoff_unconsumed()) {
                        self.refresh_handoff_target_state(id, cx);
                    }
                }
            }
            PiEvent::Error {
                message,
                diagnostic_write_failed,
                ..
            } => {
                self.failure_scroll.set_offset(point(px(0.),px(0.)));
                self.agent_diagnostic = Some(crate::failures::Diagnostic {
                    message,
                    expanded: false,
                    diagnostic_write_failed,
                });
                if let Some(id) = self.session.as_ref().map(|s| s.id.clone()) {
                    if self.current_history_access().is_some_and(|a| a.handoff_unconsumed()) {
                        self.refresh_handoff_target_state(id, cx);
                    }
                }
            }
        }
        if follow {
            self.conversation_list
                .state
                .set_follow_mode(FollowMode::Tail);
        }
    }
}

fn release_conversation_images(
    images: Vec<std::sync::Arc<RenderImage>>,
    cx: &mut Context<HiveApp>,
) {
    if !images.is_empty() {
        cx.defer(move |cx| {
            for image in images {
                cx.drop_image(image, None);
            }
        });
    }
}

fn text_part(block: &Value, include_tool_calls: bool) -> Option<Cow<'_, str>> {
    match block["type"].as_str() {
        Some("text") => block["text"].as_str().map(Cow::Borrowed),
        Some("toolCall") if include_tool_calls => Some(Cow::Owned(format!(
            "{}\n{}",
            block["name"].as_str().unwrap_or("工具"),
            block["arguments"],
        ))),
        _ => None,
    }
}

pub(crate) fn message_text(message: &Value) -> String {
    let content = &message["content"];
    if let Some(text) = content.as_str() {
        return text.to_owned();
    }
    let Some(blocks) = content.as_array() else {
        return message.to_string();
    };
    // Join borrowed text so a small trailing block does not double the
    // retained buffer of a long first block.
    let include_tool_calls = message["role"] != "assistant";
    let parts: Vec<_> = blocks
        .iter()
        .filter_map(|block| text_part(block, include_tool_calls))
        .collect();
    parts.join("\n\n")
}

pub(crate) fn history_tool_output(id: &str, message: &Value) -> (String, Vec<ToolOutputWindow>) {
    let Some(blocks) = message["content"].as_array() else {
        return (message_text(message), Vec::new());
    };
    let mut parts = Vec::new();
    let mut windows = Vec::new();
    let mut byte_offset = 0;
    let mut text_block_index = 0;
    for block in blocks {
        let Some(part) = text_part(block, true) else {
            continue;
        };
        let start = byte_offset;
        let end = start + part.len();
        byte_offset = end + 2;
        if block["type"] == "text" {
            // The runtime indexes text blocks, including empty and complete ones.
            let block_index = text_block_index;
            text_block_index += 1;
            if block["hasMoreBytes"] == true
                && let (Some(loaded_bytes), Some(total_bytes)) =
                    (block["loadedBytes"].as_u64(), block["totalBytes"].as_u64())
                && loaded_bytes < total_bytes
                && let Ok(initial_prefix_bytes) = usize::try_from(loaded_bytes)
                && part.is_char_boundary(initial_prefix_bytes)
            {
                windows.push(ToolOutputWindow {
                    history_message_id: id.to_owned(),
                    block_index,
                    display_range: start..end,
                    generation: std::sync::Arc::new(()),
                    window_start_bytes: 0,
                    loaded_bytes,
                    total_bytes,
                    has_more: true,
                    loading: false,
                    error: None,
                });
            }
        }
        parts.push(part);
    }
    (parts.join("\n\n"), windows)
}

#[cfg(test)]
mod tests {
    use super::{Session, accepts_background_send, message_text, unresolved_tool_calls};
    use serde_json::json;

    #[test]
    fn fresh_draft_requires_positive_acknowledgement_from_the_exact_source() {
        let saved = json!({
            "id": "original", "workspaceId": "project", "title": "New chat", "updatedAt": 1,
            "backend": "codex", "profile": {"id": "configured-profile", "revision": 2},
        });
        let source: Session = serde_json::from_value(saved.clone()).unwrap();
        let accepted = json!({"accepted": true, "session": saved});
        assert!(accepts_background_send(&accepted, &source));
        for (field, wrong) in [
            ("id", json!("different-chat")),
            ("workspaceId", json!("different-project")),
            ("backend", json!("claude")),
            ("profile", json!({"id": "configured-profile", "revision": 3})),
        ] {
            let mut reply = accepted.clone();
            reply["session"][field] = wrong;
            assert!(!accepts_background_send(&reply, &source), "{field} mismatch");
        }
        for missing in [json!({}), json!(false), json!("true"), json!(null)] {
            let mut reply = accepted.clone();
            reply["accepted"] = missing;
            assert!(!accepts_background_send(&reply, &source));
        }
    }

    #[test]
    fn text_blocks_keep_separators_without_doubling_a_large_buffer() {
        let long = "x".repeat(16 * 1024);
        let message = json!({"role":"assistant","content":[
            {"type":"text","text":long},
            {"type":"thinking","thinking":"skip"},
            {"type":"text","text":""},
            {"type":"text","text":"last"},
        ]});
        let body = message_text(&message);
        assert_eq!(body, format!("{long}\n\n\n\nlast"));
        assert!(body.capacity() < body.len() + 1024);
    }

    #[test]
    fn history_adds_saved_calls_only_when_the_result_row_is_missing() {
        let ids = ["assistant", "native-result", "legacy-call"].map(str::to_owned);
        let history = [
            json!({"role":"assistant","content":[
                {"type":"toolCall","id":"completed"},
                {"type":"toolCall","id":"legacy-call"},
                {"type":"toolCall","id":"interrupted"},
                {"type":"toolCall","id":""},
            ]}),
            json!({"role":"toolResult","toolCallId":"completed"}),
            json!({"role":"toolResult"}),
        ];
        assert_eq!(
            unresolved_tool_calls(&ids, &history),
            ["interrupted".to_owned()].into_iter().collect()
        );
    }
}

#[derive(serde::Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum DeliveryOrigin {
    User,
    Dispatch,
    Report,
    Cancel,
    Recovery,
    System,
    Unknown,
}
impl DeliveryOrigin {
    fn heading(&self) -> &'static str {
        match self {
            Self::User => "你",
            Self::Dispatch => "派单",
            Self::Report => "成员汇报",
            Self::Cancel => "取消",
            Self::Recovery => "会话恢复",
            Self::System => "团队通知",
            Self::Unknown => "来源未记录",
        }
    }
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryPresentation {
    dispatch_id: Option<String>,
    version: u32,
    delivery_id: String,
    origin: DeliveryOrigin,
    display_text: String,
    worker_name: Option<String>,
    worker_id: Option<String>,
    #[serde(default)]
    artifacts: Vec<String>,
    #[serde(default)]
    attachments: Vec<crate::attachments::AttachmentRef>,
}
fn delivery_presentation(message: &Value) -> Option<DeliveryPresentation> {
    let presentation = <DeliveryPresentation as serde::Deserialize>::deserialize(message.get("presentation")?).ok()?;
    (presentation.version == 1 && !presentation.delivery_id.is_empty()).then_some(presentation)
}
