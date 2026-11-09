use crate::language::{english, tr};
use crate::app::HiveApp;
use crate::palette::Color;
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
        tooltip::Tooltip,
        *,
    },
    *,
};

use crate::conversation_list::CHAT_COLUMN_WIDTH;

pub(crate) fn untitled_session_label(session: &crate::protocol::Session) -> Option<&'static str> {
    // New records bind to an agent before their first user turn. Keep actual
    // conversation titles and explicit renames verbatim, even for this text.
    (!session.custom_title && session.agent_id.is_none() && session.title == "新会话")
        .then(|| tr("新会话"))
}

impl HiveApp {
    pub(crate) fn agent_name(&self, id: &str) -> String {
        self.members
            .iter()
            .find(|member| member.id == id)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| {
                if id.ends_with(":orchestrator") {
                    tr("主控").into()
                } else if id.ends_with(":__workflow__") {
                    tr("工作流").into()
                } else if let Some(session) = self.sessions.iter().find(|session| session.agent_id.as_deref() == Some(id)) {
                    if english() { format!("{} · Historical conversation", self.session_title(session)) } else { format!("{} · 历史会话", self.session_title(session)) }
                } else {
                    id.into()
                }
            })
    }
    pub(crate) fn session_title<'a>(&'a self, session: &'a crate::protocol::Session) -> &'a str {
        if session.custom_title {
            return &session.title;
        }
        if let Some(title) = untitled_session_label(session) { return title; }
        session
            .agent_id
            .as_ref()
            .and_then(|id| self.members.iter().find(|member| &member.id == id))
            .map(|member| member.name.as_str())
            .unwrap_or(&session.title)
    }
}

impl HiveApp {
    pub(crate) fn message_actions(
        &self,
        index: usize,
        message: &crate::app::Message,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let copy_id = message.id.clone();
        let reply_id = message.id.clone();
        let delivery = message.delivery.is_some();
        h_flex()
            .gap_1()
            .when(message.role == "user", |row| row.justify_end())
            .child(
                Button::new(("copy", index))
                    .ghost()
                    .compact()
                    .icon(Glyph::Copy)
                    .disabled(!delivery && message.text.is_empty())
                    .tooltip(if delivery {
                        tr("复制完整原文")
                    } else {
                        tr("复制消息")
                    })
                    .accessibility_label(if delivery {
                        tr("复制完整消息原文")
                    } else {
                        tr("复制消息内容")
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(message) = this.messages.iter().find(|m| m.id == copy_id) {
                            let text = message.delivery.as_ref().map_or(&message.text, |d| &d.raw);
                            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                        }
                    })),
            )
            .when(
                !message.text.is_empty()
                    && (message.role == "assistant" || message.explicit_user || delivery),
                |row| {
                    row.child(
                        Button::new(("quote", index))
                            .ghost()
                            .compact()
                            .icon(Glyph::Reply)
                            .tooltip(tr("引用回复"))
                            .accessibility_label(tr("引用消息到草稿"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(message) =
                                    this.messages.iter().find(|m| m.id == reply_id)
                                {
                                    let quote = message
                                        .text
                                        .lines()
                                        .map(|line| format!("> {line}"))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    this.input.update(cx, |input, cx| {
                                        let value = format!("{quote}\n\n{}", input.value());
                                        input.replace_all(value, window, cx);
                                        input.focus(window, cx);
                                    });
                                }
                            })),
                    )
                },
            )
    }

    pub(crate) fn dispatch_link(
        &self,
        source: &str,
        dispatch: Option<&str>,
        cx: &Context<Self>,
    ) -> Option<Button> {
        let id = dispatch?.to_owned();
        if uuid::Uuid::try_parse(&id).is_err() {
            return None;
        }
        Some(
            Button::new(SharedString::from(format!("dispatch-link-{source}")))
                .ghost()
                .compact()
                .label(tr("派单详情"))
                .accessibility_label(tr("打开关联派单"))
                .disabled(!self.connected)
                .on_click(
                    cx.listener(move |this, _, _, cx| this.open_dispatch_from_chat(id.clone(), cx)),
                ),
        )
    }

    pub(crate) fn member_session_link(
        &self,
        source_id: &str,
        worker_id: Option<&str>,
        label: &'static str,
        cx: &Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let worker = worker_id?;
        let member_chat = self
            .sessions
            .iter()
            .find(|s| s.agent_id.as_deref() == Some(worker))?;
        let id = member_chat.id.clone();
        Some(
            Button::new(SharedString::from(format!("member-link-{source_id}")))
                .ghost()
                .compact()
                .label(label)
                .accessibility_label(format!(
                    "{}: {}",tr("打开成员会话"),
                    self.session_title(member_chat)
                ))
                .on_click(
                    cx.listener(move |this, _, window, cx| this.open_member_session(id.clone(), window, cx)),
                ),
        )
    }

    fn conversation_return(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        use crate::app::ConversationSource;
        let session = self.session.as_ref()?;
        let origin = self.conversation_sources.last().cloned();
        let (label, source) = match origin {
            Some(ConversationSource::Dispatch(id)) => {
                if !self
                    .collaboration
                    .selected
                    .as_ref()
                    .is_some_and(|d| d.id == id)
                {
                    return None;
                }
                (tr("返回派单详情").to_owned(), ConversationSource::Dispatch(id))
            }
            Some(ConversationSource::Workflow) => (tr("返回工作流").to_owned(), ConversationSource::Workflow),
            Some(ConversationSource::Team) => (tr("返回团队成员").to_owned(), ConversationSource::Team),
            Some(ConversationSource::Session(id)) => {
                let previous = self.sessions.iter().find(|s| s.id == id)?;
                (
                    if english() { format!("Back to {}", self.session_title(previous)) } else { format!("返回 {}", self.session_title(previous)) },
                    ConversationSource::Session(id),
                )
            }
            None => {
                let lead = format!("{}:orchestrator", session.workspace_id);
                if session.agent_id.as_deref().is_none_or(|id| id == lead) {
                    return None;
                }
                let previous = self.sessions.iter().find(|s| {
                    s.workspace_id == session.workspace_id && s.agent_id.as_deref() == Some(&lead)
                })?;
                (
                    tr("打开主会话").to_owned(),
                    ConversationSource::Session(previous.id.clone()),
                )
            }
        };
        Some(
            Button::new("conversation-return")
                .ghost()
                .compact()
                .icon(Glyph::ArrowLeft)
                .accessibility_label(label.clone())
                .tooltip(label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.conversation_sources.pop();
                    match &source {
                        ConversationSource::Dispatch(_) => {
                            this.collaboration.visible = true;
                            this.collaboration.focus_navigation = true;
                            this.load_dispatch_record(cx);
                            if this.collaboration.chat_origin.is_none() {
                                this.load_tasks(cx);
                            }
                            if this.collaboration.message_loading {
                                let after = this.collaboration.requested_after.clone();
                                this.load_dispatch_messages(after, cx);
                            }
                        }
                        ConversationSource::Workflow => this.show_workflows(window, cx),
                        ConversationSource::Team => this.show_team(window, cx),
                        ConversationSource::Session(id) => this.show_session(id.clone(), cx),
                    }
                })),
        )
    }

    pub(crate) fn render_error_banner(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        h_flex()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .items_start()
            .p_3()
            .gap_2()
            .bg(Color::ErrorSurface.get(cx))
            .rounded_lg()
            .child(
                div()
                    .id("error-banner-text")
                    .role(Role::Label)
                    .aria_label(self.error.as_ref().map(|error| error.text()).unwrap_or_default())
                    .flex_1()
                    .min_w_0()
                    .max_h(px(160.))
                    .overflow_y_scroll()
                    .text_sm()
                    .whitespace_normal()
                    .child(self.error.as_ref().map(|error|error.text()).unwrap_or_default()),
            )
            .child(
                Button::new("dismiss-error")
                    .flex_shrink_0()
                    .label(tr("关闭"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.error = None;
                        cx.notify();
                    })),
            )
    }
}

impl Render for HiveApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = cx.global::<crate::language::Language>().locale;
        if self.interface_locale != Some(locale) {
            self.localize_placeholders(window, cx);
            self.interface_locale = Some(locale);
        }
        self.restore_task_editor(window, cx);
        if self.draft_target != self.draft_owner {
            let text = self
                .draft_target
                .as_ref()
                .and_then(|key| self.drafts.get(key))
                .cloned()
                .unwrap_or_default();
            self.input
                .update(cx, |input, cx| input.set_value(text, window, cx));
            self.draft_owner = self.draft_target.clone();
        }
        let placeholder = self
            .session
            .as_ref()
            .map(|s| if crate::language::english() { format!("Message {}…", crate::language::tr(s.backend.label())) } else { format!("向 {} 发送消息…", s.backend.label()) })
            .unwrap_or_else(|| crate::language::tr("输入消息…").into());
        if self.input_placeholder != placeholder {
            self.input_placeholder = placeholder.clone();
            self.input.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
        self.prepare_interactions(window, cx);
        self.prepare_terminal_view(cx);
        let running = self
            .session
            .as_ref()
            .is_some_and(|s| self.busy.contains(&s.id));
        let has_interactions = self
            .session
            .as_ref()
            .and_then(|session| self.interactions.sessions.get(&session.id))
            .is_some_and(|cards| !cards.is_empty());
        let pending = self
            .session
            .as_ref()
            .and_then(|s| self.pending.get(&s.id))
            .copied()
            .unwrap_or(0);
        let open_failed = self.session.is_some()
            && !self.conversation_ready
            && !self.loading
            && self.connected
            && !self.closing;
        let needs_pi_model = self
            .session
            .as_ref()
            .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Pi))
            && self.conversation_ready
            && self.model.is_none()
            && !self.loading;
        self.ensure_attachment_previews(cx);
        let can_submit = self.attachments.ready(self.draft_target.as_ref())
            && self.conversation_ready
            && self.session.is_some()
            && !needs_pi_model
            && self.workspace_writes_allowed()
            && self.connected
            && self.session.as_ref().is_none_or(|session| !self.inflight_inputs.contains_key(&session.id))
            && !self.loading
            && (pending == 0 || running)
            && !self.closing;
        self.composer_can_submit = can_submit;
        let submit_on_enter = can_submit
            && cx.global::<crate::composer::ComposerPreferences>().mode
                == crate::composer::SendMode::Enter;
        if submit_on_enter != self.composer_submit_on_enter {
            self.composer_submit_on_enter = submit_on_enter;
            self.input.update(cx, |input, cx| {
                input.set_submit_on_enter(submit_on_enter, cx)
            });
        }
        self.sync_conversation_list(cx);
        let has_text = !self.input.read(cx).value().trim().is_empty()
            || !self.attachments.refs(self.draft_target.as_ref()).is_empty();
        self.composer_has_text = has_text;
        let welcome = self.session.as_ref().is_none_or(|s| s.agent_id.is_none())
            && !open_failed
            && self.pending_reports.is_empty()
            && self.messages.is_empty()
            && !has_interactions
            && !running
            && pending == 0
            && self.connected
            && !self.loading;
        let narrow = window.viewport_size().width < px(820.);
        let preferences = self.settings_panel || self.roles.visible;
        let compact_settings = window.viewport_size().width < px(720.);
        if !self.loading && (!narrow || !self.sidebar_open) && self.viewing_conversation()
            && std::mem::take(&mut self.focus_project_input)
        {
            let focus = self.input.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        if !preferences && (!narrow || !self.sidebar_open) && self.collaboration.visible
            && std::mem::take(&mut self.collaboration.focus_navigation)
        {
            let focus = self.collaboration.navigation_focus.clone();
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        v_flex().size_full().bg(Color::Surface.get(cx)).text_color(Color::Text.get(cx))
            .when(!self.connected && !self.closing && (!self.loading || self.restarting), |view| view.child(
                h_flex().flex_shrink_0().items_center().gap_3().p_3().bg(Color::Attention.get(cx))
                    .child(div().flex_1().min_w_0().text_sm().child(if self.restarting { tr("正在重新启动后台，当前草稿已保留。") } else { tr("后台未连接，当前草稿已保留。") }))
                    .child(Button::new("restart-backend").label(tr("重新启动后台")).disabled(self.restarting)
                        .on_click(cx.listener(|this, _, window, cx| this.restart_backend(window, cx))))))
            .when(self.connected && self.remote.valid && self.remote.pending.iter().any(|p| p.expires_at > crate::settings::remote::now()) && !(self.settings_panel && self.settings_section == crate::app::SettingsSection::Remote), |view| view.child(
                h_flex().flex_shrink_0().items_center().gap_3().p_3().bg(Color::Attention.get(cx))
                    .child(div().flex_1().min_w_0().text_sm().child(crate::language::tr("有新设备等待配对，请核对两端安全短码后确认。")))
                    .child(Button::new("remote-pending-banner").label(crate::language::tr("查看配对请求")).on_click(cx.listener(|this, _, window, cx| this.select_settings_section(crate::app::SettingsSection::Remote, window, cx))))))
            .when(!preferences && !self.sidebar_open, |layout| layout.child(self.render_navigation_bar(cx)))
            .child(h_flex().flex_1().min_h_0().w_full()
            .when(!preferences && self.sidebar_open, |view| view.child(self.render_navigation(narrow, cx)))
            .when(preferences || !narrow || !self.sidebar_open, |view| view.child(if preferences { self.render_preferences(compact_settings, cx).into_any_element()
            } else if self.memory.visible { self.render_memory(cx).into_any_element()
            } else if self.workflows.visible { self.render_workflows(cx)
            } else if self.collaboration.visible { self.render_collaboration(cx)
            } else if self.member_panel { self.render_members(window, narrow, cx)
            } else if self.terminal.visible { self.render_terminal(cx).into_any_element()
            } else if self.workspace.is_none() && self.session.is_none() { self.render_welcome(window, cx).into_any_element()
            } else { v_flex().flex_1().min_w_0().h_full().gap_3().py_4().px_5().when(!narrow && welcome, |v| v.px(px(44.))).when(!welcome, |v| v.px_0().pt_0()).id("conversation-surface").on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| this.import_image_paths(paths.paths().to_vec(), cx))).drag_over::<ExternalPaths>(|style, _, _, cx|style.bg(Color::Attention.get(cx))).key_context("HiveConversation").on_action(cx.listener(|this, _: &crate::CloseConversationFind, window, cx| this.close_conversation_find(window, cx))).when(welcome, |v| v.overflow_y_scroll()).when(welcome && window.viewport_size().height > px(600.), |v| v.justify_center())
                .when(welcome && self.workspace.is_some(), |view| view.child(self.render_project_context(cx)))
                .child(h_flex().flex_shrink_0().min_w_0().w_full().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().items_center().gap_2()
                    .when(!welcome, |header| header.max_w_full().min_h(px(54.)).px_5().border_b_1().border_color(Color::Divider.get(cx)))
                    .when_some(self.conversation_return(cx), |header, back| header.child(back))
                    .when(self.session.is_none() || self.session_name_target.as_deref() != self.session.as_ref().map(|s| s.id.as_str()), |header| header.child({
                    let title = if welcome && self.workspace.is_none() { tr("打开项目").into() } else if welcome { tr("今天想推进什么？").into() } else { self.session.as_ref().map(|s| self.session_title(s).to_string()).unwrap_or_else(|| "Hive".into()) };
                    div().id("session-title").role(Role::Label).aria_label(title.clone()).font_weight(FontWeight::SEMIBOLD).flex_1().min_w_0().when(welcome, |h| h.font_weight(FontWeight::MEDIUM).text_size(px(if window.viewport_size().width < px(1000.) { 28. } else { 32. })).line_height(px(40.)).whitespace_normal()).when(!welcome, |h| h.text_lg().truncate())
                        .tooltip({ let title = title.clone(); move |window, cx| Tooltip::new(title.clone()).build(window, cx) })
                        .child(title)
                }))
                    .when(self.session.is_some() && self.session_name_target.as_deref() == self.session.as_ref().map(|s| s.id.as_str()), |header| header
                        .key_context("HiveSessionName")
                        .on_action(cx.listener(|this, _: &crate::CancelSessionRename, _, cx| { if !this.session_name_loading { this.session_name_target = None; cx.notify(); } }))
                        .child(div().flex_1().min_w_0().child(Input::new(&self.session_name).aria_label(tr("会话名称")).disabled(self.session_name_loading)))
                        .child(Button::new("save-session-name").primary().label(tr("保存名称")).disabled(!self.connected || self.session_name_loading).on_click(cx.listener(|this, _, _, cx| this.save_session_name(cx))))
                        .child(Button::new("cancel-session-name").ghost().label(tr("取消")).disabled(self.session_name_loading).on_click(cx.listener(|this, _, _, cx| { this.session_name_target = None; cx.notify(); }))))
                    .when(!welcome && self.session_name_target.as_deref() != self.session.as_ref().map(|s| s.id.as_str()), |header| header
                        .child(Button::new("rename-session").ghost().compact().icon(Glyph::SquarePen).tooltip(tr("重命名会话")).accessibility_label(tr("重命名当前会话"))
                            .disabled(!self.connected || self.session_name_loading || !self.workspace_writes_allowed()).on_click(cx.listener(|this, _, window, cx| this.begin_session_rename(window, cx)))))
                .when(!welcome && self.conversation_ready && self.session.is_some() && !self.loading, |header| header.child(
                    Button::new("find-in-conversation").ghost().compact().icon(Glyph::Search).accessibility_label(tr("查找当前会话")).tooltip(if cfg!(target_os = "macos") { tr("查找会话内容 · ⌘F") } else { tr("查找会话内容 · Ctrl+F") }).on_click(cx.listener(|this, _, window, cx| this.open_conversation_find(window, cx)))
                ).child(Button::new("export-conversation").ghost().compact().icon(Glyph::FileDown).accessibility_label(tr("导出当前对话")).tooltip(tr("导出对话为 Markdown")).disabled(self.messages.is_empty()).on_click(cx.listener(|this, _, window, cx| this.open_conversation_export(window,cx)))))
                .when(welcome && self.workspace.is_some(), |header| header.child(
                    Button::new("home-team").ghost().compact().rounded_full().icon(Glyph::Network)
                        .accessibility_label(tr("团队成员")).tooltip(tr("团队成员")).on_click(cx.listener(|this, _, window, cx| this.show_team(window, cx)))
                )).when(!welcome && self.reading_above_latest(), |header| header.child(
                    Button::new("jump-to-latest").ghost().compact().icon(Glyph::ArrowDown)
                        .label(tr("回到最新")).accessibility_label(tr("回到最新消息"))
                        .on_click(cx.listener(|this, _, _, cx| { this.jump_to_latest(cx); }))
                )))
                .when(self.conversation_find.is_some(), |view| view.child(self.render_conversation_find(cx)))
                .when(self.error.is_some(), |view| view.child(self.render_error_banner(cx)))
                .when(self.session.as_ref().is_some_and(|session| self.interaction_count(&session.id) > 0), |view| view.child(Button::new("show-pending-interactions").label(if english() { format!("Waiting for response · {}", self.session.as_ref().map_or(0, |session| self.interaction_count(&session.id))) } else { format!("等待回应 · {} 项", self.session.as_ref().map_or(0, |session| self.interaction_count(&session.id))) }).on_click(cx.listener(|this, _, _, cx| { this.jump_to_latest(cx); }))))
                .when(self.session.as_ref().is_some_and(|session|session.archived), |surface| {
                    let session=self.session.as_ref().unwrap().clone();
                    surface.child(h_flex().flex_wrap().gap_2().px_4().items_center()
                        .child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr("已归档 · 发送或收到新消息后自动恢复")))
                        .child(Button::new("restore-current-session").ghost().label(tr("恢复会话"))
                            .disabled(!self.connected || self.session_archive_loading.contains(&session.id) || self.session_pin_loading.contains(&session.id))
                            .on_click(cx.listener(move |this,_,_,cx|this.set_session_archived(session.clone(),false,cx)))))
                })
                .when(open_failed, |surface| surface.child(v_flex().flex_1().min_h_0().items_center().justify_center().gap_3()
                    .child(div().text_sm().child(tr("未能打开会话，草稿已保留。")))
                    .when(self.session.as_ref().is_some_and(|s| !matches!(s.backend, crate::protocol::Backend::Pi)), |panel| panel.child(
                        Button::new("open-program-settings").ghost().label(tr("外部程序设置")).on_click(cx.listener(|this, _, window, cx| {
                            let backend = this.session.as_ref().and_then(|session| match session.backend {
                                crate::protocol::Backend::Codex => Some("codex"),
                                crate::protocol::Backend::Claude => Some("claude"),
                                crate::protocol::Backend::PiRpc => Some("pi"),
                                crate::protocol::Backend::Pi => None,
                            });
                            if let Some(backend) = backend { this.open_program_setup(backend, window, cx); }
                        }))
                    ))
                    .child(Button::new("retry-open-session").label(tr("重新打开会话")).on_click(cx.listener(|this, _, _, cx| {
                        if let Some(session) = &this.session {
                            this.error = None;
                            this.show_session(session.id.clone(), cx);
                        }
                    })))))
                .when(!welcome && !open_failed && self.session.is_some(), |surface| surface.child(self.render_conversation_list(narrow, cx)))
                .when(welcome && self.session.is_none(), |surface| surface.child(div().id("home-agent-hint").role(Role::Label)
                    .aria_label(tr("先写下需求，再选择主控 Agent。草稿会带入新会话。"))
                    .w_full().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().text_sm().text_color(Color::Muted.get(cx))
                    .child(tr("先写下需求，再选择主控 Agent。草稿会带入新会话。"))))
                .child(v_flex().w_full().min_w_0().flex_shrink_0().gap_2().when(!welcome, |dock| dock.px_5().when(!narrow, |dock| dock.px(px(44.))))
                .when(!welcome, |dock| dock.child(h_flex().w_full().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().items_center().gap_2()
                    .child(div().rounded_full().bg(Color::Inset.get(cx)).px_2().py_1().text_xs().font_weight(FontWeight::SEMIBOLD)
                        .child(self.session.as_ref().map(|s| tr(s.backend.label())).unwrap_or("Hive")))
                    .when(!self.status.is_background_connected() || running, |row| row.child(div().text_xs().text_color(Color::Muted.get(cx))
                        .child(if running { tr("正在回复…").to_string() } else { self.status.text() })))))
                .child(v_flex().relative().w_full().min_w_0().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().flex_shrink_0().gap_2().px(px(11.)).pt(px(11.)).pb(px(6.)).when(welcome && self.workspace.is_some(), |composer| composer.min_h(px(140.)).justify_between()).rounded_xl().border_1().border_color(Color::Divider.get(cx)).bg(Color::Card.get(cx))

                .child(self.render_session_failure(running,cx))
                .child(self.render_pending_user_inputs(cx))
                .when(!running && pending > 0 && (self.delivery_failure.is_none() || self.agent_diagnostic.is_some()), |view| view.child(h_flex().gap_3()
                    .child(div().text_sm().child(if english() { format!("Queued messages: {pending}") } else { format!("待处理消息：{pending}") }))
                    .when(!running, |view| view.child(Button::new("resume").label(tr("继续处理")).disabled(!self.connected || !self.conversation_ready || self.loading).on_click(cx.listener(|this, _, _, cx| this.resume(cx)))))))
                .when_some(self.session.as_ref().and_then(|session| self.failed_drafts.get(&session.id).map(|drafts| (session.id.clone(), drafts))), |view, (session_id, drafts)| view.child(v_flex().gap_2().children(drafts.iter().enumerate().map(|(i, draft)| {
                    let session_id = session_id.clone();
                    let text = draft.clone();
                    h_flex().gap_2().p_3().rounded_lg().bg(Color::ErrorSurface.get(cx))
                        .child(div().flex_1().min_w_0().text_sm().child(format!("{}: {draft}", tr("发送失败，内容已保留"))))
                        .child(Button::new(("copy-failed-draft", i)).label(tr("复制内容")).accessibility_label(tr("复制发送失败的内容")).on_click(cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))))
                        .child(Button::new(("dismiss-failed-draft", i)).label(tr("移除")).accessibility_label(tr("移除发送失败的内容")).on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(drafts) = this.failed_drafts.get_mut(&session_id) { if i < drafts.len() { drafts.remove(i); } }
                            this.schedule_draft_save(cx);
                            cx.notify();
                        })))
                }))))
                .when(!self.connected && self.runtime.is_some(), |view| view.child(Button::new("copy-disconnected-draft").label(tr("复制当前草稿")).on_click(cx.listener(|this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(this.input.read(cx).value().to_string()));
                }))))
                .when(welcome && self.workspace.is_none(), |surface| surface.child(div().text_sm().p_2().text_color(Color::Secondary.get(cx)).child(tr("选择本地项目文件夹，开始与 Agent 协作。"))))

                .when(self.command_picker.open, |view| view.child(deferred(anchored().anchor(Anchor::BottomLeft).snap_to_window().offset(point(px(11.),px(-8.))).child(self.render_commands(window,cx))).with_priority(1)))
                .child(self.render_input_attachments(cx))
                .when(self.workspace.is_some(), |v| v.child(div().capture_action(cx.listener(Self::composer_enter)).capture_action(cx.listener(Self::command_up)).capture_action(cx.listener(Self::command_down)).capture_action(cx.listener(Self::command_escape)).child(Textarea::new(&self.input).appearance(false).bordered(false).aria_label(tr("聊天消息")).on_paste({ let app = cx.weak_entity(); move |item, _, cx| app.update(cx, |this, cx| this.paste_images(item, cx)).unwrap_or(false) }))))
                .child(self.render_composer_controls(window, running, can_submit && has_text, cx))
                .when(!self.connected || running || pending > 0, |v| v.child(div().text_xs().text_color(Color::Muted.get(cx)).child(if !self.connected { tr("后台暂不可用 · 可在窗口顶部重新启动后台") } else if running { tr("Agent 正在处理 · 新消息会按顺序排队") } else if pending > 0 { tr("有待处理消息，请先继续") } else { "" }))))
                .when(needs_pi_model && self.connected, |surface| surface.child(
                    h_flex().w_full().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().min_w_0().flex_wrap().items_center().gap_2()
                        .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(if let Some(model) = &self.unavailable_model {
                            if english() { format!("{} / {} is unavailable", model.provider, model.id) } else { format!("{} / {} 暂不可用", model.provider, model.id) }
                        } else if self.models.iter().any(|m| m.available) { tr("选择模型后即可发送。").into() }
                        else { tr("请先连接 Pi 账号或配置模型提供商，然后刷新。").into() }))
                        .when_some(self.pi_agent_dir.clone(), |row, dir| row.child(Button::new("locate-pi-config").ghost().compact().label(tr("定位配置目录")).tooltip(dir.display().to_string())
                            .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&dir)))))
                        .child(Button::new("open-pi-accounts").ghost().compact().label(tr("连接账号")).on_click(cx.listener(|this, _, window, cx| { this.settings_section = crate::app::SettingsSection::PiAccounts; this.open_settings(window, cx); })))
                        .child(Button::new("refresh-pi-models").ghost().compact().label(if self.refreshing_models { tr("刷新中…") } else { tr("刷新模型") })
                            .disabled(self.refreshing_models || running).on_click(cx.listener(|this, _, _, cx| this.refresh_pi_models(cx))))))
                )
                .when(welcome && self.workspace.is_some(), |surface| surface.child(
                    h_flex().w_full().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().flex_wrap().items_start().mt_1().gap_2()
                        .children([
                            (Glyph::FolderOpen, tr("了解这个项目"), tr("请梳理当前项目的结构、主要入口和运行方式。")),
                            (Glyph::Search, tr("审查未提交的改动"), tr("请审查当前项目尚未提交的改动，按严重程度列出有证据的问题。")),
                            (Glyph::Network, tr("拆分团队任务"), tr("请先和我确认这次要达成的目标，再提出适合由成员并行处理的任务。")),
                        ].into_iter().enumerate().map(|(i,(icon,label,prompt))| {
                            Button::new(("starter",i)).ghost().h(px(35.)).rounded_full().text_color(Color::Muted.get(cx)).icon(icon).label(label)
                                .on_click(cx.listener(move |this, _, window, cx| this.input.update(cx, |input,cx| {
                                    let current = input.value();
                                    let value = if current.is_empty() { prompt.to_owned() } else { format!("{current}\n{prompt}") };
                                    input.replace_all(value, window, cx);
                                    input.focus(window,cx);
                                })))
                        }))
                ))
                .into_any_element() })))
    }
}
