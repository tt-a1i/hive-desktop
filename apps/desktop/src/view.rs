use crate::app::{HiveApp, SettingsSection};
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
    pub(crate) fn render_error_banner(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        h_flex()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .items_start()
            .p_3()
            .gap_2()
            .bg(rgb(0xffe9e4))
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
        let query = self.model_search.read(cx).value().trim().to_lowercase();
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
        let matching_models: Vec<_> = self
            .models
            .iter()
            .filter(|model| {
                model.available
                    && format!("{} {} {}", model.provider, model.id, model.name)
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect();
        let welcome = self.messages.is_empty()
            && !has_interactions
            && !running
            && pending == 0
            && self.connected
            && !self.loading;
        let narrow = window.viewport_size().width < px(820.);
        v_flex().size_full().bg(rgb(0xf8f8f6)).text_color(rgb(0x262626))
            .child(self.render_navigation_bar(cx))
            .child(h_flex().flex_1().min_h_0().w_full()
            .when(self.sidebar_open, |view| view.child(self.render_navigation(narrow, cx)))
            .when(!narrow || !self.sidebar_open, |view| view.child(if self.memory.visible { self.render_memory(cx).into_any_element() } else if self.roles.visible { v_flex().flex_1().min_w_0().h_full().child(self.render_settings_tabs(cx)).child(self.render_roles(cx)).into_any_element()
            } else if self.collaboration.visible { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(h_flex().justify_between().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("协作概览"))
                    .child(Button::new("refresh-collaboration").label("刷新").disabled(!self.connected || self.tasks_saving()).on_click(cx.listener(|this, _, _, cx| this.reload_collaboration(cx)))))
                .child(div().id("collaboration-content").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(&self.collaboration_scroll)
                  .child(v_flex().w_full().min_w_0().flex_shrink_0().gap_3()
                    .child(self.render_tasks(cx))
                    .child(div().pt_3().text_sm().font_weight(FontWeight::SEMIBOLD).child("派单"))
                    .child(h_flex().gap_1().children([("all", "全部"), ("queued", "排队中"), ("submitted", "已提交"), ("reported", "已汇报"), ("cancelled", "已取消")].into_iter().enumerate().map(|(i, (filter, label))| Button::new(("dispatch-filter", i)).label(label).selected(self.collaboration.filter == filter).on_click(cx.listener(move |this, _, _, cx| { this.collaboration.filter = filter; this.collaboration.offset = 0; this.load_dispatches(cx); })))))
                    .when(self.collaboration.list_loading, |view| view.child(div().text_sm().child("正在读取派单…")))
                    .when_some(self.collaboration.list_error.clone(), |view, error| view.child(div().text_sm().text_color(rgb(0xb54131)).child(error)))
                    .when(!self.collaboration.list_loading && self.collaboration.list_error.is_none() && self.collaboration.dispatches.is_empty(), |view| view.child(div().text_sm().child("此页没有派单")))
                    .children(self.collaboration.dispatches.iter().enumerate().map(|(i, dispatch)| {
                        let item = dispatch.clone();
                        let preview: String = dispatch.text.chars().take(100).collect();
                        Button::new(("dispatch", i)).w_full().label(format!("{} · {} · {}", crate::collaboration::dispatch_state(&dispatch.state), self.agent_name(&dispatch.to_agent_id), preview)).selected(self.collaboration.selected.as_ref().is_some_and(|selected| selected.id == dispatch.id)).on_click(cx.listener(move |this, _, _, cx| this.select_dispatch(item.clone(), cx)))
                    }))
                    .child(h_flex().gap_2()
                        .child(Button::new("dispatch-prev").label("上一页").disabled(!self.connected || self.collaboration.offset == 0 || self.collaboration.list_loading).on_click(cx.listener(|this, _, _, cx| { this.collaboration.offset = this.collaboration.offset.saturating_sub(20); this.load_dispatches(cx); })))
                        .child(div().text_sm().child(format!("第 {} 页", self.collaboration.offset / 20 + 1)))
                        .child(Button::new("dispatch-next").label("下一页").disabled(!self.connected || self.collaboration.dispatches.len() < 20 || self.collaboration.list_loading || self.collaboration.offset >= 100_000).on_click(cx.listener(|this, _, _, cx| { this.collaboration.offset += 20; this.load_dispatches(cx); }))))
                    .when_some(self.collaboration.selected.as_ref(), |view, item| view.child(v_flex().gap_2().p_3().rounded_lg().bg(rgb(0xffffff))
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("派单详情 · {}", crate::collaboration::dispatch_state(&item.state))))
                        .child(div().text_xs().child(format!("{} → {} · {}", item.from_agent_id.as_ref().map(|id| self.agent_name(id)).unwrap_or_else(|| "未记录发送者".into()), self.agent_name(&item.to_agent_id), item.id)))
                        .child(div().text_sm().child("任务原文"))
                        .child(div().text_sm().child(item.text.clone()))
                        .when_some(item.report_text.clone(), |view, text| view.child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(if item.state == "cancelled" { "取消原因" } else { "汇报原文" })).child(div().text_sm().child(text)))
                        .when(!item.artifacts.is_empty(), |view| view.child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("产物")).children(item.artifacts.iter().map(|artifact| div().text_sm().child(artifact.clone()))))
                        .child(div().pt_3().text_sm().font_weight(FontWeight::SEMIBOLD).child("派单消息"))
                        .when(self.collaboration.message_loading, |view| view.child(div().text_sm().child("正在读取消息…")))
                        .when_some(self.collaboration.message_error.clone(), |view, error| view.child(div().text_sm().text_color(rgb(0xb54131)).child(error)).child(Button::new("retry-dispatch-messages").label("重试读取消息").on_click(cx.listener(|this, _, _, cx| { let after = this.collaboration.next_seq.unwrap_or(0); this.load_dispatch_messages(after, cx); }))))
                        .when(!self.collaboration.message_loading && self.collaboration.message_error.is_none() && self.collaboration.messages.is_empty(), |view| view.child(div().text_sm().child("没有消息")))
                        .children(self.collaboration.messages.iter().map(|message| v_flex().gap_1().p_2().bg(rgb(0xf0f2ed))
                            .child(div().text_xs().child(format!("#{} · {} → {} · {} · {}", message.sequence, self.agent_name(&message.from_agent_id), self.agent_name(&message.recipient_agent_id), crate::collaboration::message_kind(&message.kind), crate::collaboration::delivery_state(&message.delivery_state))))
                            .child(div().text_sm().child(message.text.clone()))
                            .when_some(message.delivery_error.clone(), |view, error| view.child(div().text_xs().text_color(rgb(0xb54131)).child(format!("投递错误：{error}"))))))
                        .when_some(self.collaboration.next_seq, |view, seq| view.child(Button::new("more-dispatch-messages").label("加载更多消息").disabled(!self.connected || self.collaboration.message_loading).on_click(cx.listener(move |this, _, _, cx| this.load_dispatch_messages(seq, cx)))))))
                )).into_any_element()
            } else if self.settings_panel { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(self.render_settings_tabs(cx))
                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("设置"))
                .child({
                    let content = v_flex().flex_1().min_h_0().gap_3();
                    match self.settings_section {
                        SettingsSection::Team => content.id("settings-team-content").overflow_y_scroll()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("自动组队"))
                    .child(div().text_sm().child("全局设置，适用于所有工作区。开启后，Hive 会引导主控按任务需要组建团队；关闭后可自行安排成员。此设置改变主控指导语，不会直接创建或删除成员。"))
                    .child(h_flex().flex_wrap().items_center().gap_2()
                        .child(div().text_sm().child(match self.team_autostaff { Some(true) => "自动组队引导：已开启", Some(false) => "自动组队引导：已关闭", None if self.team_autostaff_loading => "正在读取自动组队设置…", None => "自动组队设置暂不可用" }))
                        .when_some(self.team_autostaff, |view, enabled| view.child(Button::new("toggle-team-autostaff").label(if enabled { "关闭引导" } else { "开启引导" }).disabled(!self.connected || self.team_autostaff_loading).on_click(cx.listener(move |this, _, _, cx| this.team_autostaff_request(Some(!enabled), cx)))))
                        .child(Button::new("reload-team-autostaff").label("重新读取").disabled(!self.connected || self.team_autostaff_loading).on_click(cx.listener(|this, _, _, cx| this.team_autostaff_request(None, cx)))))
                    .when(self.team_autostaff_loading && self.team_autostaff.is_some(), |view| view.child(div().text_xs().child("正在处理自动组队设置…")))
                    .when_some(self.team_autostaff_message.clone(), |view, message| view.child(div().text_sm().child(message))),
                        SettingsSection::Notifications => content.id("settings-notification-content").overflow_y_scroll().child(self.render_notification_settings(cx)),
                        SettingsSection::Programs => content.id("settings-programs-content").overflow_y_scroll()
                    .child(div().pt_3().text_sm().font_weight(FontWeight::SEMIBOLD).child("外部程序设置"))
                    .child(div().text_sm().child("设置外部 Pi、Codex 和 Claude 的可执行文件路径。留空时通过 PATH 查找；环境变量优先于此设置。"))
                    .child(div().text_sm().child("更改适用于之后新打开或重启的外部进程，正在运行的进程继续使用原配置。"))
                    .when_some(self.settings_message.clone(), |view, message| view.child(div().p_3().rounded_lg().bg(rgb(0xefefea)).text_sm().child(message)))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("外部 Pi 可执行文件"))
                    .child(Input::new(&self.pi_command).aria_label("外部 Pi 可执行文件路径").disabled(!self.connected || self.settings_loading || self.executable_settings.is_none()))
                    .when_some(self.executable_settings.as_ref(), |view, settings| view.child(div().text_xs().child(settings.effective.pi.label())))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Codex 可执行文件"))
                    .child(Input::new(&self.codex_command).aria_label("Codex 可执行文件路径").disabled(!self.connected || self.settings_loading || self.executable_settings.is_none()))
                    .when_some(self.executable_settings.as_ref(), |view, settings| view.child(div().text_xs().child(settings.effective.codex.label())))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Claude 可执行文件"))
                    .child(Input::new(&self.claude_command).aria_label("Claude 可执行文件路径").disabled(!self.connected || self.settings_loading || self.executable_settings.is_none()))
                    .when_some(self.executable_settings.as_ref(), |view, settings| view.child(div().text_xs().child(settings.effective.claude.label())))
                    .child(h_flex().gap_2()
                        .child(Button::new("save-executable-settings").primary().label(if self.settings_loading { "处理中…" } else { "保存设置" }).disabled(!self.connected || self.settings_loading || self.executable_settings.is_none()).on_click(cx.listener(|this, _, window, cx| this.settings_request(true, window, cx))))
                        .child(Button::new("reload-executable-settings").label("重新读取").disabled(!self.connected || self.settings_loading).on_click(cx.listener(|this, _, window, cx| this.settings_request(false, window, cx))))),
                    }
                })
                .into_any_element()
            } else if self.member_panel { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("团队成员"))
                .when(self.error.is_some(), |view| view.child(self.render_error_banner(cx)))
                .child(v_flex().id("members-panel").flex_1().min_h_0().min_w_0().overflow_y_scroll().gap_2()
                    .child(div().text_sm().child(if self.member_loading { "正在更新成员…" } else { "新建成员（默认停止）" }))
                    .child(h_flex().gap_3().child(div().flex_1().min_w_0().child(Input::new(&self.member_name))).child(div().flex_1().min_w_0().child(Textarea::new(&self.member_description).aria_label("成员职责说明"))))
                    .child(self.render_member_templates(cx))
                    .child(div().text_xs().child("角色"))
                    .child(h_flex().gap_1().children([("coder", "开发"), ("reviewer", "审查"), ("tester", "测试"), ("custom", "自定义")].into_iter().enumerate().map(|(i, (role, label))| Button::new(("member-role", i)).label(label).selected(self.member_role == role).on_click(cx.listener(move |this, _, _, cx| { this.member_role = role; cx.notify(); })))))
                    .child(div().text_xs().child("成员后端"))
                    .child(h_flex().gap_1().children([("pi", "Pi"), ("codex", "Codex"), ("pi-rpc", "外部 Pi"), ("claude", "Claude")].into_iter().enumerate().map(|(i, (backend, label))| Button::new(("member-backend", i)).label(label).selected(self.member_backend == backend).on_click(cx.listener(move |this, _, _, cx| { this.member_backend = backend; cx.notify(); })))))
                    .child(Button::new("add-member").label("添加成员").disabled(!self.connected || self.member_loading).on_click(cx.listener(|this, _, _, cx| this.add_member(cx))))
                    .child(div().text_xs().child("启动前请为对应成员选择该后端的模型"))
                    .child(h_flex().justify_between().child(div().text_sm().child("团队成员")).child(Button::new("refresh-members").label("刷新").disabled(!self.connected || self.member_loading).on_click(cx.listener(|this, _, _, cx| this.refresh_members(cx)))))
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
                        let chosen = self.member_selected_model.as_ref().filter(|(target, selected_backend, _)| target == &id && Some(selected_backend.as_str()) == backend);
                        let member_model_ready = chosen.is_some();
                        let session_id = self.sessions.iter().find(|session| session.agent_id.as_ref() == Some(&id)).map(|s| s.id.clone());
                        let start_id = id.clone(); let start_command = member.configured_command.clone();
                        let picker_id = id.clone(); let picker_command = member.configured_command.clone();
                        let edit_id = id.clone(); let edit_name = member.name.clone();
                        let save_id = id.clone(); let save_workspace = workspace.clone();
                        let delete_id = id.clone(); let delete_workspace = workspace.clone();
                        v_flex().gap_1().p_2().rounded_lg().bg(rgb(0xffffff))
                            .child(div().text_sm().child(member.name.clone()))
                            .child(div().text_xs().child(format!("{} · {} · {}", member.role_label(), member.status_label(), member.backend_label())))
                            .when(!running && backend.is_some(), |view| view.child(Button::new(("choose-member-model", i)).label(if picker_open { "重新读取模型" } else { "选择模型" }).disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, window, cx| { this.member_model_search.update(cx, |input, cx| input.set_value("", window, cx)); this.choose_member_models(picker_id.clone(), picker_command.clone(), cx); }))))
                            .when(!running && backend.is_none(), |view| view.child(div().text_xs().child("此成员的后端未配置或不受支持")))
                            .when_some(chosen, |view, (_, _, model)| view.child(div().text_xs().child(format!("已选择模型：{} / {}", model.provider, model.id))))
                            .when(!running && !member_model_ready && backend.is_some(), |view| view.child(div().text_xs().child("请选择此成员后端的模型后启动")))
                            .when(picker_open, |view| view.child(v_flex().gap_2()
                                .child(Input::new(&self.member_model_search).aria_label("搜索此成员的可用模型"))
                                .when(self.member_catalog_loading, |view| view.child(div().text_sm().child("正在读取可用模型…")))
                                .when_some(self.member_catalog_error.clone(), |view, error| view.child(div().text_sm().text_color(rgb(0xb54131)).child(error)))
                                .when(!self.member_catalog_loading && self.member_catalog_error.is_none() && matching_member_models.is_empty(), |view| view.child(div().text_sm().child("没有匹配的可用模型")))
                                .child(v_flex().id(("member-model-list", i)).h(px(168.)).min_h_0().overflow_y_scroll().gap_1()
                                    .children(matching_member_models.iter().cloned().enumerate().map(|(j, model)| {
                                        let target = id.clone(); let backend = self.member_catalog_backend.clone().unwrap_or_default();
                                        Button::new(("member-model", j)).label(format!("{} / {}", model.provider, model.name)).tooltip(model.id.clone()).on_click(cx.listener(move |this, _, _, cx| { this.member_selected_model = Some((target.clone(), backend.clone(), model.clone())); this.member_picker_target = None; cx.notify(); }))
                                    })))))
                            .when_some(session_id, |view, session_id| view.child(Button::new(("member-chat", i)).label("打开会话").on_click(cx.listener(move |this, _, _, cx| this.open_session(session_id.clone(), cx)))))
                            .child(h_flex().gap_1()
                                .child(Button::new(("member-start-stop", i)).label(if stopping { "正在停止…" } else if running { "停止" } else { "启动" }).disabled(!self.connected || self.member_loading || (!running && !member_model_ready)).on_click(cx.listener(move |this, _, _, cx| { if running { this.stop_member(start_id.clone(), cx); } else { this.start_member(start_id.clone(), start_command.clone(), cx); } })))
                                .child(Button::new(("member-edit", i)).label("改名").disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, window, cx| { this.editing_member = Some(edit_id.clone()); this.member_rename.update(cx, |input, cx| input.set_value(edit_name.clone(), window, cx)); cx.notify(); })))
                                .child(Button::new(("member-delete", i)).label(if deleting { "确认删除" } else { "删除" }).disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, _, cx| { if this.deleting_member.as_ref() == Some(&delete_id) { this.member_action("DELETE", format!("/api/workspaces/{delete_workspace}/workers/{delete_id}"), None, cx); } else { this.deleting_member = Some(delete_id.clone()); cx.notify(); } }))))
                            .when(editing, |view| view.child(Input::new(&self.member_rename)).child(Button::new(("member-save", i)).label("保存名称").disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, _, cx| { let name = this.member_rename.read(cx).value().trim().to_string(); if name.is_empty() { this.error = Some("请输入成员名称".into()); cx.notify(); } else { this.member_action("PATCH", format!("/api/workspaces/{save_workspace}/workers/{save_id}"), Some(serde_json::json!({"name":name})), cx); } }))))
                    })))
                .into_any_element()
            } else if self.terminal.visible { self.render_terminal(cx).into_any_element()
            } else { v_flex().flex_1().min_w_0().h_full().gap_3().p_5().when(welcome, |v| v.justify_center())
                .child(h_flex().flex_shrink_0().min_w_0().w_full().max_w(px(920.)).mx_auto().child({
                    let title = if welcome { "Hive".into() } else { self.session.as_ref().map(|s| self.session_title(s).to_string()).unwrap_or_else(|| "Hive".into()) };
                    div().id("session-title").flex_1().min_w_0().truncate().when(welcome, |h| h.text_3xl()).when(!welcome, |h| h.text_lg()).font_weight(FontWeight::SEMIBOLD)
                        .tooltip({ let title = title.clone(); move |window, cx| Tooltip::new(title.clone()).build(window, cx) })
                        .child(title)
                }))
                .when(!welcome, |v| v.child(div().text_xs().text_color(rgb(0x777c75)).child(self.session.as_ref().map(|s| format!("{} · {}", s.backend.label(), self.status)).unwrap_or_else(|| self.status.clone()))))
                .when(self.error.is_some(), |view| view.child(self.render_error_banner(cx)))
                .when(self.session.as_ref().is_some_and(|session| self.interaction_count(&session.id) > 0), |view| view.child(Button::new("show-pending-interactions").label(format!("等待回应 · {} 项", self.session.as_ref().map_or(0, |session| self.interaction_count(&session.id)))).on_click(cx.listener(|this, _, _, cx| { this.scroll.scroll_to_bottom(); cx.notify(); }))))
                .when(!welcome && self.session.is_some(), |surface| surface.child(div().id("messages").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).p_3()
                    .child(v_flex().w_full().max_w(px(920.)).mx_auto().gap_5()
                        .when(self.messages.is_empty() && (!has_interactions || !self.connected), |view| view.child(div().py_8().text_color(rgb(0x777c75)).child(if self.loading { "正在读取…" } else if !self.connected { "后台连接已断开，当前草稿仍保留。请先复制需要的内容，再重启应用。" } else if running { "正在等待 Agent 回复…" } else if self.session.is_some() { "会话已就绪。发送第一条消息开始工作。" } else if self.workspace.is_some() { "新建会话开始工作" } else { "添加项目文件夹，再创建一个会话。" })))
                        .children(self.messages.iter().enumerate().map(|(i, message)| {
                            if let Some(tool) = &message.tool {
                                return self.render_tool_message(i, message, tool, cx).into_any_element();
                            }
                            let heading = message.delivery.as_ref().map(|delivery| delivery.heading.clone()).unwrap_or_else(|| match message.role.as_str() { "user" if message.explicit_user => "你", "user" => "对话输入", "assistant" => self.session.as_ref().map(|s| s.backend.label()).unwrap_or("助手"), _ => "工具" }.into());
                            v_flex().id(("message", i)).min_w_0().gap_2().px_3().py_2().rounded_lg()
                                .when(message.role == "user", |bubble| bubble.bg(rgb(0xececea)).ml_auto().max_w(px(680.)))
                                .child(h_flex().min_w_0().gap_2().justify_between()
                                    .child(div().flex_1().min_w_0().truncate().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(rgb(0x63715c)).child(heading))
                                    .when(message.delivery.is_some(), |view| view.child(Button::new(("expand-delivery", i)).accessibility_label(format!("{}消息原文", if message.delivery.as_ref().is_some_and(|d| d.expanded) { "收起" } else { "查看" })).label(if message.delivery.as_ref().is_some_and(|d| d.expanded) { "收起原文" } else { "查看原文" }).on_click({ let id = message.id.clone(); cx.listener(move |this, _, _, cx| {
                                        if let Some(delivery) = this.messages.iter_mut().find(|m| m.id == id).and_then(|m| m.delivery.as_mut()) { delivery.expanded = !delivery.expanded; }
                                        cx.notify();
                                    }) })))
                                    .child(Button::new(("copy", i)).accessibility_label(if message.delivery.is_some() { "复制完整消息原文" } else { "复制消息内容" }).label(if message.delivery.is_some() { "复制原文" } else { "复制" }).on_click({ let text = message.delivery.as_ref().map(|d| d.raw.clone()).unwrap_or_else(|| message.text.clone()); cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))) })))
                                .when(message.role == "assistant", |view| view.child(TextView::new(&message.markdown).selectable(true).stream_fade(true)))
                                .when(message.role == "user" || message.delivery.is_some(), |view| view.child(div().min_w_0().text_sm().child(message.text.clone())))
                                .when(message.delivery.as_ref().is_some_and(|d| d.expanded), |view| view.child(div().min_w_0().p_3().rounded_lg().bg(rgb(0xffffff)).text_sm().child(message.delivery.as_ref().map(|d| d.raw.clone()).unwrap_or_default())))
                                .into_any_element()
                        }))
                        .child(self.render_interactions(cx)))))
                .child(v_flex().w_full().min_w_0().max_w(px(920.)).mx_auto().flex_shrink_0().gap_2().p_3().rounded_xl().border_1().border_color(rgb(0xd7d7d4)).bg(rgb(0xffffff))

                .when(self.choosing_model, |view| view.child(v_flex().flex_shrink_0().min_h_0().gap_2()
                    .child(Input::new(&self.model_search))
                    .child(v_flex().id("model-list").h(px(168.)).min_h_0().flex_shrink_0().overflow_y_scroll().gap_2()
                        .when(matching_models.is_empty(), |view| view.child(div().p_3().text_sm().child("没有匹配的可用模型")))
                        .children(matching_models.into_iter().enumerate().map(|(i, model)| {
                            let selected = self.model.as_ref().is_some_and(|m| m.id == model.id && m.provider == model.provider);
                            Button::new(("model", i)).flex_shrink_0().w_full().label(format!("{} / {}", model.provider, model.name)).tooltip(model.id.clone()).selected(selected).disabled(!self.connected || running || self.loading)
                                .on_click(cx.listener(move |this, _, _, cx| this.set_model(model.clone(), cx)))
                        })))))
                .when(pending > 0, |view| view.child(h_flex().gap_3()
                    .child(div().text_sm().child(format!("团队消息待处理：{pending}")))
                    .when(!running, |view| view.child(Button::new("resume").label("继续处理").disabled(!self.connected || self.loading).on_click(cx.listener(|this, _, _, cx| this.resume(cx)))))))
                .when_some(self.session.as_ref().and_then(|session| self.failed_drafts.get(&session.id).map(|drafts| (session.id.clone(), drafts))), |view, (session_id, drafts)| view.child(v_flex().gap_2().children(drafts.iter().enumerate().map(|(i, draft)| {
                    let session_id = session_id.clone();
                    let text = draft.clone();
                    h_flex().gap_2().p_3().rounded_lg().bg(rgb(0xffe9e4))
                        .child(div().flex_1().min_w_0().text_sm().child(format!("发送失败，内容已保留：{draft}")))
                        .child(Button::new(("copy-failed-draft", i)).label("复制内容").accessibility_label("复制发送失败的内容").on_click(cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))))
                        .child(Button::new(("dismiss-failed-draft", i)).label("移除").accessibility_label("移除发送失败的内容").on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(drafts) = this.failed_drafts.get_mut(&session_id) { if i < drafts.len() { drafts.remove(i); } }
                            cx.notify();
                        })))
                }))))
                .when(!self.connected && self.runtime.is_some(), |view| view.child(Button::new("copy-disconnected-draft").label("复制当前草稿").on_click(cx.listener(|this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(this.input.read(cx).value().to_string()));
                }))))
                .when(self.workspace.is_some(), |v| v.child(Textarea::new(&self.input).appearance(false).bordered(false).aria_label("聊天消息")))
                .child(h_flex().min_w_0().gap_2().items_center()
                    .when(self.session.is_none(), |row| row.child(if self.workspace.is_some() {
                        self.new_chat_menu("home-agent", "选择 Agent", cx).into_any_element()
                    } else { Button::new("home-folder").ghost().icon(Glyph::FolderOpen).label("选择项目文件夹").disabled(!self.connected).on_click(cx.listener(|this, _, window, cx| this.choose_workspace_folder(window, cx))).into_any_element() }))
                    .child(div().flex_1())
                    .when(self.session.is_some(), |row| row.child(Button::new("choose-model").ghost().compact().min_w_0().label(self.model.as_ref().map(|model| format!("{} / {} ▾", model.provider, model.id)).unwrap_or_else(|| "选择模型 ▾".into())).disabled(!self.connected || self.session.is_none() || running || self.loading).on_click(cx.listener(|this, _, _, cx| { this.choosing_model = !this.choosing_model; cx.notify(); }))))
                    .when(self.session.is_some(), |row| row.child(if running {
                        Button::new("stop").label("停止").disabled(!self.connected).on_click(cx.listener(|this, _, _, cx| this.abort(cx)))
                    } else {
                        Button::new("send").primary().icon(Glyph::ArrowUp).rounded(px(20.)).accessibility_label("发送消息").tooltip("发送消息").disabled(!self.connected || self.session.is_none() || self.loading || pending > 0).on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
                    })))
                .when(!self.connected || running || pending > 0, |v| v.child(div().text_xs().text_color(rgb(0x777777)).child(if !self.connected { "后台暂不可用 · 请先复制草稿，再重启应用" } else if running { "团队正在处理 · 可随时停止" } else if pending > 0 { "有待处理消息，请先继续" } else { "" }))))
                .when(welcome && self.workspace.is_some(), |surface| surface.child(
                    v_flex().w_full().max_w(px(920.)).mx_auto().items_start().gap_1()
                        .children([
                            (Glyph::FolderOpen, "了解这个项目", "请梳理当前项目的结构、主要入口和运行方式。"),
                            (Glyph::Search, "审查未提交的改动", "请审查当前项目尚未提交的改动，按严重程度列出有证据的问题。"),
                            (Glyph::Network, "拆分团队任务", "请先和我确认这次要达成的目标，再提出适合由成员并行处理的任务。"),
                        ].into_iter().enumerate().map(|(i,(icon,label,prompt))| {
                            Button::new(("starter",i)).ghost().icon(icon).label(label)
                                .on_click(cx.listener(move |this, _, window, cx| this.input.update(cx, |input,cx| {
                                    let current = input.value();
                                    let value = if current.is_empty() { prompt.to_owned() } else { format!("{current}\n{prompt}") };
                                    input.set_value(value, window, cx);
                                    input.focus(window,cx);
                                })))
                        }))
                ))
                .into_any_element() })))
    }
}
