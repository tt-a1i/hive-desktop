use crate::app::{HiveApp, SettingsSection};
use crate::palette::Color;
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
        text::TextView,
        tooltip::Tooltip,
        *,
    },
    *,
};

const CHAT_COLUMN_WIDTH: f32 = 932.;

impl HiveApp {
    fn agent_name(&self, id: &str) -> String {
        self.members
            .iter()
            .find(|member| member.id == id)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| {
                if id.ends_with(":orchestrator") {
                    "主控".into()
                } else {
                    id.into()
                }
            })
    }
    pub(crate) fn session_title<'a>(&'a self, session: &'a crate::protocol::Session) -> &'a str {
        session
            .agent_id
            .as_ref()
            .and_then(|id| self.members.iter().find(|member| &member.id == id))
            .map(|member| member.name.as_str())
            .unwrap_or(&session.title)
    }
}

impl HiveApp {
    fn message_actions(
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
                    .tooltip(if delivery {
                        "复制完整原文"
                    } else {
                        "复制消息"
                    })
                    .accessibility_label(if delivery {
                        "复制完整消息原文"
                    } else {
                        "复制消息内容"
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
                            .tooltip("引用回复")
                            .accessibility_label("引用消息到草稿")
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

    fn member_session_link(
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
                    "打开 {} 的成员会话",
                    self.session_title(member_chat)
                ))
                .on_click(
                    cx.listener(move |this, _, _, cx| this.open_member_session(id.clone(), cx)),
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
                ("返回派单详情".to_owned(), ConversationSource::Dispatch(id))
            }
            Some(ConversationSource::Team) => ("返回团队成员".to_owned(), ConversationSource::Team),
            Some(ConversationSource::Session(id)) => {
                let previous = self.sessions.iter().find(|s| s.id == id)?;
                (
                    format!("返回 {}", self.session_title(previous)),
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
                    "打开主会话".to_owned(),
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
                            this.load_tasks(cx);
                            if this.collaboration.message_loading {
                                let after = this.collaboration.next_seq.unwrap_or(0);
                                this.load_dispatch_messages(after, cx);
                            }
                        }
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
                    .flex_1()
                    .min_w_0()
                    .max_h(px(160.))
                    .overflow_y_scroll()
                    .text_sm()
                    .whitespace_normal()
                    .child(self.error.clone().unwrap_or_default()),
            )
            .child(
                Button::new("dismiss-error")
                    .flex_shrink_0()
                    .label("关闭")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.error = None;
                        cx.notify();
                    })),
            )
    }
}

impl Render for HiveApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .map(|s| format!("向 {} 发送消息…", s.backend.label()))
            .unwrap_or_else(|| "输入消息…".into());
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
        let can_submit = self.conversation_ready
            && self.session.is_some()
            && !needs_pi_model
            && self.workspace_writes_allowed()
            && self.connected
            && !running
            && !self.loading
            && pending == 0
            && !self.closing;
        if can_submit != self.composer_can_submit {
            self.composer_can_submit = can_submit;
            self.input
                .update(cx, |input, cx| input.set_submit_on_enter(can_submit, cx));
        }
        let has_text = !self.input.read(cx).value().trim().is_empty();
        self.composer_has_text = has_text;
        let member_query = self
            .member_model_search
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let matching_member_models: Vec<_> = self
            .member_catalog
            .iter()
            .filter(|model| {
                format!("{} {} {}", model.provider, model.id, model.name)
                    .to_lowercase()
                    .contains(&member_query)
            })
            .cloned()
            .collect();
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
        let compact_settings =
            window.viewport_size().width < px(if self.sidebar_open { 1060. } else { 800. });
        v_flex().size_full().bg(Color::Surface.get(cx)).text_color(Color::Text.get(cx))
            .when(!self.sidebar_open, |layout| layout.child(self.render_navigation_bar(cx)))
            .child(h_flex().flex_1().min_h_0().w_full()
            .when(self.sidebar_open, |view| view.child(self.render_navigation(narrow, cx)))
            .when(!narrow || !self.sidebar_open, |view| view.child(if self.memory.visible { self.render_memory(cx).into_any_element() } else if self.roles.visible { h_flex().items_stretch().when(compact_settings, |layout| layout.flex_col()).flex_1().min_w_0().h_full().child(self.render_settings_tabs(compact_settings, cx)).child(self.render_roles(cx)).into_any_element()
            } else if self.collaboration.visible { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(h_flex().justify_between().items_center()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(if self.collaboration.selected.is_some() { "派单详情" } else { "协作概览" }))
                    .when(self.collaboration.selected.is_some(), |row| row.child(Button::new("back-dispatch-list").ghost().icon(Glyph::ArrowLeft).label("返回派单列表").on_click(cx.listener(|this, _, _, cx| {
                        this.collaboration.selected = None;
                        cx.notify();
                    }))))
                    .when(self.collaboration.selected.is_none(), |row| row.child(Button::new("refresh-collaboration").label("刷新").disabled(!self.connected || self.tasks_saving()).on_click(cx.listener(|this, _, _, cx| this.reload_collaboration(cx))))))
                .child(div().id("collaboration-content").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(if self.collaboration.selected.is_some() { &self.collaboration.detail_scroll } else { &self.collaboration_scroll })
                  .child(v_flex().w_full().min_w_0().flex_shrink_0().gap_3()
                    .when(self.collaboration.selected.is_none(), |list| list
                    .child(self.render_tasks(cx))
                    .child(div().pt_3().text_sm().font_weight(FontWeight::SEMIBOLD).child("派单"))
                    .child(h_flex().gap_1().children([("all", "全部"), ("queued", "排队中"), ("submitted", "已提交"), ("reported", "已汇报"), ("cancelled", "已取消")].into_iter().enumerate().map(|(i, (filter, label))| Button::new(("dispatch-filter", i)).label(label).selected(self.collaboration.filter == filter).on_click(cx.listener(move |this, _, _, cx| { this.collaboration.filter = filter; this.collaboration.offset = 0; this.load_dispatches(cx); })))))
                    .when(self.collaboration.list_loading, |view| view.child(div().text_sm().child("正在读取派单…")))
                    .when_some(self.collaboration.list_error.clone(), |view, error| view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)))
                    .when(!self.collaboration.list_loading && self.collaboration.list_error.is_none() && self.collaboration.dispatches.is_empty(), |view| view.child(div().text_sm().child("此页没有派单")))
                    .children(self.collaboration.dispatches.iter().enumerate().map(|(i, dispatch)| {
                        let item = dispatch.clone();
                        let preview: String = dispatch.text.chars().take(100).collect();
                        Button::new(("dispatch", i)).w_full().label(format!("{} · {} · {}", crate::collaboration::dispatch_state(&dispatch.state), self.agent_name(&dispatch.to_agent_id), preview)).on_click(cx.listener(move |this, _, _, cx| this.select_dispatch(item.clone(), cx)))
                    }))
                    .child(h_flex().gap_2()
                        .child(Button::new("dispatch-prev").label("上一页").disabled(!self.connected || self.collaboration.offset == 0 || self.collaboration.list_loading).on_click(cx.listener(|this, _, _, cx| { this.collaboration.offset = this.collaboration.offset.saturating_sub(20); this.load_dispatches(cx); })))
                        .child(div().text_sm().child(format!("第 {} 页", self.collaboration.offset / 20 + 1)))
                        .child(Button::new("dispatch-next").label("下一页").disabled(!self.connected || self.collaboration.dispatches.len() < 20 || self.collaboration.list_loading || self.collaboration.offset >= 100_000).on_click(cx.listener(|this, _, _, cx| { this.collaboration.offset += 20; this.load_dispatches(cx); }))))
                    )
                    .when_some(self.collaboration.selected.as_ref(), |view, item| view.child(v_flex().gap_2().p_3().rounded_lg().bg(Color::Card.get(cx))
                        .child(h_flex().items_center().gap_2()
                            .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(crate::collaboration::dispatch_state(&item.state).to_owned()))
                            .when_some(self.member_session_link(&item.id, Some(&item.to_agent_id), "负责人的会话", cx), |row, link| row.child(link)))
                        .child(div().text_xs().child(format!("{} → {} · {}", item.from_agent_id.as_ref().map(|id| self.agent_name(id)).unwrap_or_else(|| "未记录发送者".into()), self.agent_name(&item.to_agent_id), item.id)))
                        .child(div().text_sm().child("任务原文"))
                        .child(div().text_sm().child(item.text.clone()))
                        .when_some(item.report_text.clone(), |view, text| view.child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(if item.state == "cancelled" { "取消原因" } else { "汇报原文" })).child(div().text_sm().child(text)))
                        .when(!item.artifacts.is_empty(), |view| view.child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("产物")).children(item.artifacts.iter().map(|artifact| div().text_sm().child(artifact.clone()))))
                        .child(div().pt_3().text_sm().font_weight(FontWeight::SEMIBOLD).child("派单消息"))
                        .when(self.collaboration.message_loading, |view| view.child(div().text_sm().child("正在读取消息…")))
                        .when_some(self.collaboration.message_error.clone(), |view, error| view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)).child(Button::new("retry-dispatch-messages").label("重试读取消息").on_click(cx.listener(|this, _, _, cx| { let after = this.collaboration.next_seq.unwrap_or(0); this.load_dispatch_messages(after, cx); }))))
                        .when(!self.collaboration.message_loading && self.collaboration.message_error.is_none() && self.collaboration.messages.is_empty(), |view| view.child(div().text_sm().child("没有消息")))
                        .children(self.collaboration.messages.iter().map(|message| v_flex().gap_1().p_2().bg(Color::Inset.get(cx))
                            .child(div().text_xs().child(format!("#{} · {} → {} · {} · {}", message.sequence, self.agent_name(&message.from_agent_id), self.agent_name(&message.recipient_agent_id), crate::collaboration::message_kind(&message.kind), crate::collaboration::delivery_state(&message.delivery_state))))
                            .child(div().text_sm().child(message.text.clone()))
                            .when_some(message.delivery_error.clone(), |view, error| view.child(div().text_xs().text_color(Color::Error.get(cx)).child(format!("投递错误：{error}"))))))
                        .when_some(self.collaboration.next_seq, |view, seq| view.child(Button::new("more-dispatch-messages").label("加载更多消息").disabled(!self.connected || self.collaboration.message_loading).on_click(cx.listener(move |this, _, _, cx| this.load_dispatch_messages(seq, cx)))))))
                )).into_any_element()
            } else if self.settings_panel { h_flex().items_stretch().when(compact_settings, |layout| layout.flex_col()).flex_1().min_w_0().h_full()
                .child(self.render_settings_tabs(compact_settings, cx))
                .child(v_flex().flex_1().min_h_0().min_w_0().p_5().gap_3()
                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(match self.settings_section { SettingsSection::Team => "团队", SettingsSection::Notifications => "通知", SettingsSection::Programs => "外部程序" }))
                .child({
                    let content = v_flex().flex_1().min_h_0().gap_3();
                    match self.settings_section {
                        SettingsSection::Team => content.id("settings-team-content").overflow_y_scroll()
                    .child(h_flex().gap_3().items_center().pb_3().border_b_1().border_color(Color::Divider.get(cx))
                        .child(v_flex().flex_1().min_w_0().gap_1()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("自动组队引导"))
                            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child("在所有工作区引导主控按任务需要组建团队。")))
                        .when_some(self.team_autostaff, |row, enabled| row.child(gpui_kit::component::switch::Switch::new("toggle-team-autostaff")
                            .accessibility_label("自动组队引导").checked(enabled).disabled(!self.connected || self.team_autostaff_loading)
                            .on_change(cx.listener(|this, enabled, _, cx| this.team_autostaff_request(Some(*enabled), cx))))))
                    .when(self.team_autostaff_loading, |view| view.child(div().text_xs().text_color(Color::Muted.get(cx)).child("正在同步设置…")))
                    .when(!self.team_autostaff_loading, |view| view.child(h_flex().child(Button::new("reload-team-autostaff").ghost().compact().label("重新读取").disabled(!self.connected).on_click(cx.listener(|this, _, _, cx| this.team_autostaff_request(None, cx))))))
                    .when_some(self.team_autostaff_message.clone(), |view, message| view.child(div().text_sm().child(message))),
                        SettingsSection::Notifications => content.id("settings-notification-content").overflow_y_scroll().child(self.render_notification_settings(cx)),
                        SettingsSection::Programs => content.id("settings-programs-content").overflow_y_scroll()
                    .child(div().text_sm().text_color(Color::Secondary.get(cx)).child("路径留空时使用 PATH 查找。环境变量优先，修改在下次启动对应 Agent 时生效。"))
                    .when_some(self.settings_message.clone(), |view, message| view.child(div().p_3().rounded_lg().bg(Color::Inset.get(cx)).text_sm().child(message)))
                    .child(self.executable_field("外部 Pi", &self.pi_command, self.executable_settings.as_ref().map(|s| &s.effective.pi), cx))
                    .child(self.executable_field("Codex", &self.codex_command, self.executable_settings.as_ref().map(|s| &s.effective.codex), cx))
                    .child(self.executable_field("Claude Code", &self.claude_command, self.executable_settings.as_ref().map(|s| &s.effective.claude), cx))
                    .child(h_flex().gap_2()
                        .child(Button::new("save-executable-settings").primary().label(if self.settings_loading { "处理中…" } else { "保存设置" }).disabled(!self.connected || self.settings_loading || self.executable_settings.is_none()).on_click(cx.listener(|this, _, window, cx| this.settings_request(true, window, cx))))
                        .child(Button::new("reload-executable-settings").label("重新读取").disabled(!self.connected || self.settings_loading).on_click(cx.listener(|this, _, window, cx| this.settings_request(false, window, cx))))),
                    }
                }))
                .into_any_element()
            } else if self.member_panel { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(h_flex().w_full().items_center().gap_2().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("团队成员"))
                    .child(div().flex_1().text_xs().text_color(Color::Muted.get(cx)).child(if self.member_loading { "更新中".into() } else { format!("{} 位", self.members.len()) }))
                    .child(Button::new("refresh-members").ghost().compact().icon(Glyph::RefreshCw).tooltip("刷新成员").accessibility_label("刷新成员").disabled(!self.connected || self.member_loading).on_click(cx.listener(|this, _, _, cx| this.refresh_members(cx))))
                    .child(Button::new("toggle-member-form").ghost().icon(Glyph::Plus).label(if self.member_form_open { "收起表单" } else { "添加成员" }).on_click(cx.listener(|this, _, _, cx| { this.member_form_open = !this.member_form_open; cx.notify(); }))))
                .when(self.error.is_some(), |view| view.child(self.render_error_banner(cx)))
                .child(v_flex().id("members-panel").flex_1().min_h_0().min_w_0().overflow_y_scroll().gap_2()
                    .when(self.member_form_open, |list| list.child(v_flex().gap_2().p_3().rounded_lg().border_1().border_color(Color::Divider.get(cx))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("新建成员"))
                    .child(h_flex().gap_3().child(div().flex_1().min_w_0().child(Input::new(&self.member_name))).child(div().flex_1().min_w_0().child(Textarea::new(&self.member_description).aria_label("成员职责说明"))))
                    .child(self.render_member_templates(cx))
                    .child(div().text_xs().child("角色"))
                    .child(h_flex().gap_1().children([("coder", "开发"), ("reviewer", "审查"), ("tester", "测试"), ("custom", "自定义")].into_iter().enumerate().map(|(i, (role, label))| Button::new(("member-role", i)).label(label).selected(self.member_role == role).on_click(cx.listener(move |this, _, _, cx| { this.member_role = role; cx.notify(); })))))
                    .child(div().text_xs().child("成员后端"))
                    .child(h_flex().gap_1().children([("pi", "Pi"), ("codex", "Codex"), ("pi-rpc", "外部 Pi"), ("claude", "Claude")].into_iter().enumerate().map(|(i, (backend, label))| Button::new(("member-backend", i)).label(label).selected(self.member_backend == backend).on_click(cx.listener(move |this, _, _, cx| { this.member_backend = backend; cx.notify(); })))))
                    .child(Button::new("add-member").label("添加成员").disabled(!self.connected || self.member_loading).on_click(cx.listener(|this, _, _, cx| this.add_member(cx))))))

                    .when(self.members.is_empty() && !self.member_loading, |view| view.child(div().text_xs().child("暂无成员")))
                    .children(self.members.iter().enumerate().map(|(i, member)| {
                        let id = member.id.clone();
                        let workspace = self.workspace.clone().unwrap_or_default();
                        let editing = self.editing_member.as_ref() == Some(&id);
                        let deleting = self.deleting_member.as_ref() == Some(&id);
                        let running = member.status != "stopped";
                        let stopping = self.member_stopping.as_ref() == Some(&id);
                        let backend = crate::members::command_backend(member.configured_command.as_deref());
                        let picker_open = self.member_picker_target.as_ref() == Some(&id);
                        let chosen = member.desktop_model.as_ref();
                        let member_model_ready = chosen.is_some();
                        let session_id = self.sessions.iter().find(|session| session.agent_id.as_ref() == Some(&id)).map(|s| s.id.clone());
                        let start_id = id.clone(); let start_workspace = workspace.clone();
                        let picker_id = id.clone(); let picker_command = member.configured_command.clone();
                        let save_id = id.clone(); let save_workspace = workspace.clone();
                        let delete_id = id.clone(); let delete_workspace = workspace.clone();
                        v_flex().gap_2().py_3().px_2().border_b_1().border_color(Color::Divider.get(cx))
                            .child(h_flex().w_full().items_center().gap_3()
                                .child(v_flex().flex_1().min_w_0().gap_1()
                                    .child(div().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).child(member.name.clone()))
                                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} · {}", member.role_label(), member.backend_label()))))
                                .child(div().rounded_full().bg(Color::Inset.get(cx)).px_2().py_1().text_xs().child(member.status_label().to_owned()))
                                .when_some(session_id, |view, session_id| view.child(Button::new(("member-chat", i)).ghost().compact().icon(Glyph::MessagesSquare).label("会话").tooltip("打开会话").accessibility_label(format!("打开 {} 的成员会话", member.name)).on_click(cx.listener(move |this, _, _, cx| this.open_member_session(session_id.clone(), cx)))))
                                .child(Button::new(("member-start-stop", i)).ghost().compact().label(if stopping { "正在停止…" } else if running { "停止" } else { "启动" }).disabled(!self.connected || self.member_loading || (!running && !member_model_ready)).on_click(cx.listener(move |this, _, _, cx| { if running { this.stop_member(start_id.clone(), cx); } else { this.member_action("POST", format!("/api/workspaces/{start_workspace}/agents/{start_id}/start"), None, cx); } })))
                                .child(self.member_menu(member, cx)))
                            .when(!running && backend.is_some(), |view| view.child(h_flex().child(Button::new(("choose-member-model", i)).ghost().compact()
                                .label(if picker_open { "收起模型列表".into() } else { chosen.map(|m| format!("{} / {} ▾", m.provider, m.id)).unwrap_or_else(|| "选择启动模型 ▾".into()) })
                                .disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, window, cx| {
                                    if picker_open { this.member_picker_target = None; cx.notify(); }
                                    else {
                                        this.member_model_search.update(cx, |input, cx| input.set_value("", window, cx));
                                        this.choose_member_models(picker_id.clone(), picker_command.clone(), cx);
                                    }
                                })))))
                            .when(!running && backend.is_none(), |view| view.child(div().text_xs().child("此成员的后端未配置或不受支持")))
                            .when(running, |view| view.when_some(chosen, |view, model| view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} / {}", model.provider, model.id)))))
                            .when(picker_open, |view| view.child(v_flex().gap_2()
                                .child(Input::new(&self.member_model_search).aria_label("搜索此成员的可用模型"))
                                .when(self.member_catalog_loading, |view| view.child(div().text_sm().child("正在读取可用模型…")))
                                .when_some(self.member_catalog_error.clone(), |view, error| view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)))
                                .when(!self.member_catalog_loading && self.member_catalog_error.is_none() && matching_member_models.is_empty(), |view| view.child(div().text_sm().child("没有匹配的可用模型")))
                                .child(v_flex().id(("member-model-list", i)).h(px(168.)).min_h_0().overflow_y_scroll().gap_1()
                                    .children(matching_member_models.iter().cloned().enumerate().map(|(j, model)| {
                                        let path = format!("/api/workspaces/{workspace}/agents/{id}/config"); let command = member.configured_command.clone();
                                        Button::new(("member-model", j)).ghost().selected(chosen.is_some_and(|m| m.provider == model.provider && m.id == model.id)).label(format!("{} / {}", model.provider, model.name)).tooltip(model.id.clone()).on_click(cx.listener(move |this, _, _, cx| { this.member_picker_target = None;
                                            this.member_action("POST", path.clone(), Some(serde_json::json!({"command":command,"command_preset_id":null,"args":["--provider",model.provider,"--model",model.id]})), cx); }))
                                    })))))
                            .when(deleting, |view| view.child(h_flex().flex_wrap().items_center().gap_2()
                                .child(div().text_sm().child(format!("删除成员 {}？", member.name)))
                                .child(Button::new(("confirm-delete", i)).label("确认删除").disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, _, cx| this.member_action("DELETE", format!("/api/workspaces/{delete_workspace}/workers/{delete_id}"), None, cx))))
                                .child(Button::new(("cancel-delete", i)).ghost().label("取消").on_click(cx.listener(|this, _, _, cx| { this.deleting_member = None; cx.notify(); })))))
                            .when(editing, |view| view.child(h_flex().w_full().gap_2().items_center().child(div().flex_1().min_w_0().child(Input::new(&self.member_rename).aria_label("成员名称"))).child(Button::new(("member-save", i)).label("保存名称").disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, _, cx| { let name = this.member_rename.read(cx).value().trim().to_string(); if name.is_empty() { this.error = Some("请输入成员名称".into()); cx.notify(); } else { this.member_action("PATCH", format!("/api/workspaces/{save_workspace}/workers/{save_id}"), Some(serde_json::json!({"name":name})), cx); } }))).child(Button::new(("cancel-rename", i)).ghost().label("取消").on_click(cx.listener(|this, _, _, cx| { this.editing_member = None; cx.notify(); })))))
                    })))
                .into_any_element()
            } else if self.terminal.visible { self.render_terminal(cx).into_any_element()
            } else { v_flex().flex_1().min_w_0().h_full().gap_3().py_4().px_5().when(!narrow, |v| v.px(px(44.))).id("conversation-surface").when(welcome, |v| v.overflow_y_scroll()).when(welcome && window.viewport_size().height > px(600.), |v| v.justify_center())
                .child(h_flex().flex_shrink_0().min_w_0().w_full().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().items_center().gap_2()
                    .when_some(self.conversation_return(cx), |header, back| header.child(back))
                    .child({
                    let title = if welcome && self.workspace.is_none() { "打开项目".into() } else if welcome { "Hive".into() } else { self.session.as_ref().map(|s| self.session_title(s).to_string()).unwrap_or_else(|| "Hive".into()) };
                    div().id("session-title").font_weight(FontWeight::SEMIBOLD).flex_1().min_w_0().truncate().when(welcome, |h| h.text_size(px(40.)).line_height(px(50.)).font_weight(FontWeight::BOLD)).when(!welcome, |h| h.text_lg())
                        .tooltip({ let title = title.clone(); move |window, cx| Tooltip::new(title.clone()).build(window, cx) })
                        .child(title)
                })
                .when(welcome && self.workspace.is_some(), |header| header.child(
                    Button::new("home-team").ghost().rounded_full().icon(Glyph::Network)
                        .label("团队成员").on_click(cx.listener(|this, _, window, cx| this.show_team(window, cx)))
                )).when(!welcome && !open_failed, |header| header.child(
                    div().min_w_0().truncate().text_xs().text_color(Color::Muted.get(cx)).child(
                        self.session.as_ref().map(|s| if self.status == "后台已连接" { s.backend.label().to_owned() }
                        else { format!("{} · {}", s.backend.label(), self.status) }).unwrap_or_else(|| self.status.clone())
                    )
                )).when(!welcome && self.reading_above_latest(), |header| header.child(
                    Button::new("jump-to-latest").ghost().compact().icon(Glyph::ArrowDown)
                        .label("回到最新").accessibility_label("回到最新消息")
                        .on_click(cx.listener(|this, _, _, cx| { this.jump_to_latest(cx); }))
                )))
                .when(self.error.is_some(), |view| view.child(self.render_error_banner(cx)))
                .when(self.session.as_ref().is_some_and(|session| self.interaction_count(&session.id) > 0), |view| view.child(Button::new("show-pending-interactions").label(format!("等待回应 · {} 项", self.session.as_ref().map_or(0, |session| self.interaction_count(&session.id)))).on_click(cx.listener(|this, _, _, cx| { this.jump_to_latest(cx); }))))
                .when(open_failed, |surface| surface.child(v_flex().flex_1().min_h_0().items_center().justify_center().gap_3()
                    .child(div().text_sm().child("未能打开会话，草稿已保留。"))
                    .when(self.session.as_ref().is_some_and(|s| !matches!(s.backend, crate::protocol::Backend::Pi)), |panel| panel.child(
                        Button::new("open-program-settings").ghost().label("外部程序设置").on_click(cx.listener(|this, _, window, cx| {
                            this.settings_section = SettingsSection::Programs;
                            this.open_settings(window, cx);
                        }))
                    ))
                    .child(Button::new("retry-open-session").label("重新打开会话").on_click(cx.listener(|this, _, _, cx| {
                        if let Some(session) = &this.session {
                            this.error = None;
                            this.show_session(session.id.clone(), cx);
                        }
                    })))))
                .when(!welcome && !open_failed && self.session.is_some(), |surface| surface.child(div().id("messages").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).on_scroll(cx.listener(|_, _, _, cx| cx.notify())).py_3()
                    .child(v_flex().w_full().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().gap_5()
                        .when(self.messages.is_empty() && self.pending_reports.is_empty() && (!has_interactions || !self.connected), |view| view.child(div().py_8().text_color(Color::Muted.get(cx)).child(if self.loading { "正在读取…" } else if !self.connected { "后台连接已断开，当前草稿仍保留。请先复制需要的内容，再重启应用。" } else if running { "正在等待 Agent 回复…" } else if needs_pi_model { "还没有消息。" } else if self.session.is_some() { "会话已就绪。发送第一条消息开始工作。" } else if self.workspace.is_some() { "新建会话开始工作" } else { "添加项目文件夹，再创建一个会话。" })))
                        .children(self.messages.chunk_by(|left, right| left.completed_tool() && right.completed_tool())
                            .scan(0usize, |offset, group| {
                                let start = *offset;
                                *offset += group.len();
                                Some((start, group))
                            }).map(|(i, group)| {
                            if group.len() > 1 { return self.render_completed_tools(i, group, cx).into_any_element(); }
                            let message = &group[0];
                            if let Some(tool) = &message.tool {
                                return self.render_tool_message(i, message, tool, cx).into_any_element();
                            }
                            let heading = message.delivery.as_ref().map(|delivery| delivery.heading.clone()).unwrap_or_else(|| match message.role.as_str() { "user" if message.explicit_user => "你", "user" => "对话输入", "assistant" => self.session.as_ref().map(|s| s.backend.label()).unwrap_or("助手"), "notice" => "会话通知", _ => "工具" }.into());
                            v_flex().id(("message", i)).min_w_0().gap_2().px_3().py_2().rounded_lg()
                                .when(message.role == "user", |bubble| bubble.ml_auto().max_w(px(680.)))
                                .when(!message.explicit_user, |view| view.child(h_flex().min_w_0().gap_2().justify_between()
                                    .child(div().flex_1().min_w_0().truncate().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(Color::Secondary.get(cx)).child(heading))
                                    .when(message.delivery.as_ref().is_some_and(|card| card.report), |row| {
                                        let state = if self.completed_deliveries.contains(&message.id) {
                                            "处理回合已完成"
                                        } else {
                                            self.pending_reports.iter().find(|report| report.delivery_id == message.id)
                                                .map(|report| report.status()).unwrap_or("已进入会话")
                                        };
                                        row.child(div().text_xs().text_color(Color::Muted.get(cx)).child(state))
                                    })
                                    .when_some(self.member_session_link(&message.id, message.delivery.as_ref().and_then(|card| card.worker_id.as_deref()), "成员会话", cx), |row, link| row.child(link))
                                    .when(message.delivery.is_some(), |view| view.child(Button::new(("expand-delivery", i)).accessibility_label(format!("{}消息原文", if message.delivery.as_ref().is_some_and(|d| d.expanded) { "收起" } else { "查看" })).label(if message.delivery.as_ref().is_some_and(|d| d.expanded) { "收起原文" } else { "查看原文" }).on_click({ let id = message.id.clone(); cx.listener(move |this, _, _, cx| {
                                        if let Some(delivery) = this.messages.iter_mut().find(|m| m.id == id).and_then(|m| m.delivery.as_mut()) { delivery.expanded = !delivery.expanded; }
                                        cx.notify();
                                    }) })))
                                ))
                                .when_some(message.markdown.as_ref(), |view, markdown| view.child(TextView::new(markdown).selectable(true).stream_fade(true)))
                                .when(message.markdown.is_none(), |view| view.child(div().min_w_0().text_size(px(15.)).line_height(px(24.)).when(message.role == "user", |bubble| bubble.px_4().py_2().rounded_xl().bg(Color::Card.get(cx)).border_1().border_color(Color::Divider.get(cx))).child(message.text.clone())))
                                .when_some(message.delivery.as_ref(), |view, delivery| view.children(delivery.artifacts.iter().map(|artifact|
                                    div().min_w_0().text_sm().text_color(Color::Secondary.get(cx)).child(format!("交付物 · {artifact}"))
                                )))
                                .when(message.delivery.as_ref().is_some_and(|d| d.expanded), |view| view.child(div().min_w_0().p_3().rounded_lg().bg(Color::Card.get(cx)).text_sm().child(message.delivery.as_ref().map(|d| d.raw.clone()).unwrap_or_default())))
                                .child(self.message_actions(i, message, cx))
                                .into_any_element()
                        }))
                        .children(self.pending_reports.iter().filter(|report|
                            !self.messages.iter().any(|message| message.id == report.delivery_id)
                        ).map(|report| {
                            v_flex().min_w_0().gap_2().px_3().py_2()
                                .child(h_flex().gap_2().items_center()
                                    .child(div().flex_1().text_xs().font_weight(FontWeight::SEMIBOLD).child(format!("{} · 成员汇报", report.worker_name.as_deref().unwrap_or("Worker"))))
                                    .child(div().text_xs().text_color(Color::Muted.get(cx)).child(report.status()))
                                    .when_some(self.member_session_link(&report.delivery_id, report.worker_id.as_deref(), "成员会话", cx), |row, link| row.child(link)))
                                .child(div().text_sm().child(report.text.clone()))
                                .children(report.artifacts.iter().map(|artifact| div().text_sm().text_color(Color::Secondary.get(cx)).child(format!("交付物 · {artifact}"))))
                        }))
                        .child(self.render_interactions(cx)))))
                .child(v_flex().w_full().min_w_0().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().flex_shrink_0().gap_2().px(px(11.)).pt(px(11.)).pb(px(6.)).when(welcome && self.workspace.is_some(), |composer| composer.min_h(px(140.)).justify_between()).rounded_xl().border_1().border_color(Color::Divider.get(cx)).bg(Color::Card.get(cx))

                .when(pending > 0, |view| view.child(h_flex().gap_3()
                    .child(div().text_sm().child(format!("团队消息待处理：{pending}")))
                    .when(!running, |view| view.child(Button::new("resume").label("继续处理").disabled(!self.connected || !self.conversation_ready || self.loading).on_click(cx.listener(|this, _, _, cx| this.resume(cx)))))))
                .when_some(self.session.as_ref().and_then(|session| self.failed_drafts.get(&session.id).map(|drafts| (session.id.clone(), drafts))), |view, (session_id, drafts)| view.child(v_flex().gap_2().children(drafts.iter().enumerate().map(|(i, draft)| {
                    let session_id = session_id.clone();
                    let text = draft.clone();
                    h_flex().gap_2().p_3().rounded_lg().bg(Color::ErrorSurface.get(cx))
                        .child(div().flex_1().min_w_0().text_sm().child(format!("发送失败，内容已保留：{draft}")))
                        .child(Button::new(("copy-failed-draft", i)).label("复制内容").accessibility_label("复制发送失败的内容").on_click(cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))))
                        .child(Button::new(("dismiss-failed-draft", i)).label("移除").accessibility_label("移除发送失败的内容").on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(drafts) = this.failed_drafts.get_mut(&session_id) { if i < drafts.len() { drafts.remove(i); } }
                            this.schedule_draft_save(cx);
                            cx.notify();
                        })))
                }))))
                .when(!self.connected && self.runtime.is_some(), |view| view.child(Button::new("copy-disconnected-draft").label("复制当前草稿").on_click(cx.listener(|this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(this.input.read(cx).value().to_string()));
                }))))
                .when(welcome && self.workspace.is_none(), |surface| surface.child(div().text_sm().p_2().text_color(Color::Secondary.get(cx)).child("选择本地项目文件夹，开始与 Agent 协作。")))

                .when(self.workspace.is_some(), |v| v.child(Textarea::new(&self.input).appearance(false).bordered(false).aria_label("聊天消息")))
                .child(h_flex().min_w_0().gap_2().items_center()
                    .when(self.session.is_none(), |row| row.child(if self.workspace.is_some() {
                        self.new_chat_menu("home-agent", "选择 Agent", cx).into_any_element()
                    } else { Button::new("home-folder").ghost().icon(Glyph::FolderOpen).label("选择项目文件夹").disabled(!self.connected).on_click(cx.listener(|this, _, window, cx| this.choose_workspace_folder(window, cx))).into_any_element() }))
                    .child(div().flex_1())
                    .when(self.session.is_some(), |row| row.child(self.model_picker(window, running, cx)))
                    .when(self.session.is_some(), |row| row.child(if running {
                        Button::new("stop").label("停止").disabled(!self.connected).on_click(cx.listener(|this, _, _, cx| this.abort(cx)))
                    } else {
                        Button::new("send").primary().icon(Glyph::ArrowUp).rounded(px(20.)).accessibility_label("发送消息").tooltip("Enter 发送，Shift+Enter 换行").disabled(!can_submit || !has_text).on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
                    })))
                .when(!self.connected || running || pending > 0, |v| v.child(div().text_xs().text_color(Color::Muted.get(cx)).child(if !self.connected { "后台暂不可用 · 请先复制草稿，再重启应用" } else if running { "团队正在处理 · 可随时停止" } else if pending > 0 { "有待处理消息，请先继续" } else { "" }))))
                .when(needs_pi_model && self.connected, |surface| surface.child(
                    h_flex().w_full().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().min_w_0().flex_wrap().items_center().gap_2()
                        .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(if let Some(model) = &self.unavailable_model {
                            format!("{} / {} 暂不可用", model.provider, model.id)
                        } else if self.models.iter().any(|m| m.available) { "选择模型后即可发送。".into() }
                        else { "请在 Pi 中登录或配置模型提供商，然后刷新。".into() }))
                        .when_some(self.pi_agent_dir.clone(), |row, dir| row.child(Button::new("locate-pi-config").ghost().compact().label("定位配置目录").tooltip(dir.display().to_string())
                            .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&dir)))))
                        .child(Button::new("refresh-pi-models").ghost().compact().label(if self.refreshing_models { "刷新中…" } else { "刷新模型" })
                            .disabled(self.refreshing_models || running).on_click(cx.listener(|this, _, _, cx| this.refresh_pi_models(cx))))))
                .when(welcome && self.workspace.is_some() && !needs_pi_model, |surface| surface.child(
                    v_flex().w_full().max_w(px(CHAT_COLUMN_WIDTH)).mx_auto().items_start().mt_1().gap_1()
                        .children([
                            (Glyph::FolderOpen, "了解这个项目", "请梳理当前项目的结构、主要入口和运行方式。"),
                            (Glyph::Search, "审查未提交的改动", "请审查当前项目尚未提交的改动，按严重程度列出有证据的问题。"),
                            (Glyph::Network, "拆分团队任务", "请先和我确认这次要达成的目标，再提出适合由成员并行处理的任务。"),
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
