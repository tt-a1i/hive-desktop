use crate::app::HiveApp;
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
    fn session_title<'a>(&'a self, session: &'a crate::protocol::Session) -> &'a str {
        session
            .agent_id
            .as_ref()
            .and_then(|id| self.members.iter().find(|member| &member.id == id))
            .map(|member| member.name.as_str())
            .unwrap_or(&session.title)
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
        let running = self
            .session
            .as_ref()
            .is_some_and(|s| self.busy.contains(&s.id));
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
        h_flex().size_full().bg(rgb(0xfaf9f6)).text_color(rgb(0x282c2a))
            .child(v_flex().w(px(270.)).h_full().bg(rgb(0xefefea)).p_4().gap_3()
                .child(h_flex().gap_2().child(div().text_2xl().font_weight(FontWeight::BOLD).child("Hive"))
                    .child(div().text_xs().text_color(rgb(0x777c75)).child("桌面工作台")))
                .child(div().pt_3().text_xs().text_color(rgb(0x777c75)).child("工作区"))
                .children(self.workspaces.iter().enumerate().map(|(i, workspace)| {
                    let id = workspace.id.clone();
                    Button::new(("workspace", i)).label(workspace.name.clone()).tooltip(workspace.path.clone()).selected(self.workspace.as_ref() == Some(&workspace.id))
                        .on_click(cx.listener(move |this, _, _, cx| this.select_workspace(id.clone(), cx)))
                }))
                .child(Input::new(&self.folder))
                .child(Button::new("add-workspace").label("添加文件夹").disabled(self.runtime.is_none()).on_click(cx.listener(|this, _, window, cx| this.add_workspace(window, cx))))
                .child(div().pt_4().text_xs().text_color(rgb(0x777c75)).child("会话与团队 · 新建"))
                .child(h_flex().gap_2()
                    .child(Button::new("new-pi-chat").label("＋ Pi").accessibility_label("新建 Pi 会话").disabled(self.workspace.is_none() || self.loading).on_click(cx.listener(|this, _, _, cx| this.new_session("pi", cx))))
                    .child(Button::new("new-codex-chat").label("＋ Codex").accessibility_label("新建 Codex 会话").disabled(self.workspace.is_none() || self.loading).on_click(cx.listener(|this, _, _, cx| this.new_session("codex", cx)))))
                .child(Button::new("new-pi-rpc-chat").label("＋ 外部 Pi").accessibility_label("新建外部 Pi 会话").disabled(self.workspace.is_none() || self.loading).on_click(cx.listener(|this, _, _, cx| this.new_session("pi-rpc", cx))))
                .child(Button::new("toggle-members").label(if self.member_panel { "返回会话" } else { "管理团队成员" }).disabled(self.workspace.is_none()).on_click(cx.listener(|this, _, _, cx| { this.close_settings(); this.close_collaboration(); this.close_roles(); this.member_panel = !this.member_panel; if this.member_panel { this.refresh_members(cx); this.load_roles(cx); } cx.notify(); })))
                .child(Button::new("open-settings").label(if self.settings_panel { "返回会话" } else { "外部程序设置" }).on_click(cx.listener(|this, _, window, cx| { if this.settings_panel { this.close_settings(); cx.notify(); } else { this.open_settings(window, cx); } })))
                .child(Button::new("open-roles").label(if self.roles.visible { "返回会话" } else { "角色模板" }).on_click(cx.listener(|this, _, _, cx| { if this.roles.visible { this.close_roles(); } else { this.open_roles(cx); } cx.notify(); })))
                .child(Button::new("open-collaboration").label(if self.collaboration.visible { "返回会话" } else { "协作概览" }).disabled(self.workspace.is_none()).on_click(cx.listener(|this, _, _, cx| { if this.collaboration.visible { this.close_collaboration(); } else { this.open_collaboration(cx); } cx.notify(); })))
                .child(v_flex().id("sessions").flex_1().min_h_0().overflow_y_scroll().gap_2()
                    .children(self.sessions.iter().enumerate().map(|(i, session)| {
                        let id = session.id.clone();
                        let role = session.agent_id.as_deref().map(|agent| if agent.ends_with(":orchestrator") { "主控" } else { "成员" }).unwrap_or("会话");
                        let activity = if self.busy.contains(&id) { " · 处理中" } else if self.pending.get(&id).copied().unwrap_or(0) > 0 { " · 已暂停" } else if self.unread.contains(&id) { " · 未读" } else { "" };
                        v_flex().min_w_0().gap_1()
                            .child(div().text_xs().text_color(rgb(0x63715c)).child(format!("{role} · {}{activity}", session.backend.label())))
                            .child(Button::new(("session", i)).w_full().min_w_0()
                                .child(div().flex_1().min_w_0().truncate().child(self.session_title(session).to_string()))
                                .tooltip(session.agent_id.as_ref().map(|agent| format!("{}\n{agent}", self.session_title(session))).unwrap_or_else(|| self.session_title(session).to_string()))
                                .selected(self.session.as_ref().is_some_and(|s| s.id == session.id))
                                .on_click(cx.listener(move |this, _, _, cx| { this.close_collaboration(); this.open_session(id.clone(), cx); })))
                    })))
                .child(div().text_xs().text_color(rgb(0x777c75)).child("本地会话")))
            .child(if self.roles.visible { self.render_roles(cx).into_any_element()
            } else if self.collaboration.visible { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(h_flex().justify_between().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("协作概览"))
                    .child(Button::new("refresh-collaboration").label("刷新").on_click(cx.listener(|this, _, _, cx| this.reload_collaboration(cx)))))
                .child(div().id("collaboration-content").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(&self.collaboration_scroll)
                  .child(v_flex().w_full().min_w_0().flex_shrink_0().gap_3()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("任务 · 只读"))
                    .when(self.collaboration.task_loading, |view| view.child(div().text_sm().child("正在读取任务…")))
                    .when_some(self.collaboration.task_error.clone(), |view, error| view.child(div().text_sm().text_color(rgb(0xb54131)).child(error)))
                    .when(!self.collaboration.task_loading && self.collaboration.task_error.is_none() && self.collaboration.task_text.is_empty(), |view| view.child(div().text_sm().child("任务文档为空")))
                    .when(!self.collaboration.task_text.is_empty(), |view| view.child(div().p_3().rounded_lg().bg(rgb(0xffffff)).child(TextView::new(&self.collaboration.tasks).selectable(true))))
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
                        .child(Button::new("dispatch-prev").label("上一页").disabled(self.collaboration.offset == 0 || self.collaboration.list_loading).on_click(cx.listener(|this, _, _, cx| { this.collaboration.offset = this.collaboration.offset.saturating_sub(20); this.load_dispatches(cx); })))
                        .child(div().text_sm().child(format!("第 {} 页", self.collaboration.offset / 20 + 1)))
                        .child(Button::new("dispatch-next").label("下一页").disabled(self.collaboration.dispatches.len() < 20 || self.collaboration.list_loading || self.collaboration.offset >= 100_000).on_click(cx.listener(|this, _, _, cx| { this.collaboration.offset += 20; this.load_dispatches(cx); }))))
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
                        .when_some(self.collaboration.next_seq, |view, seq| view.child(Button::new("more-dispatch-messages").label("加载更多消息").disabled(self.collaboration.message_loading).on_click(cx.listener(move |this, _, _, cx| this.load_dispatch_messages(seq, cx)))))))
                )).into_any_element()
            } else if self.settings_panel { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("外部程序设置"))
                .child(div().text_sm().child("设置外部 Pi 和 Codex 的可执行文件路径。留空时通过 PATH 查找；环境变量优先于此设置。"))
                .child(div().text_sm().child("更改适用于之后新打开或重启的外部进程，正在运行的进程继续使用原配置。"))
                .when_some(self.settings_message.clone(), |view, message| view.child(div().p_3().rounded_lg().bg(rgb(0xefefea)).text_sm().child(message)))
                .child(v_flex().id("settings-content").flex_1().min_h_0().overflow_y_scroll().gap_3()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("外部 Pi 可执行文件"))
                    .child(Input::new(&self.pi_command).aria_label("外部 Pi 可执行文件路径").disabled(self.settings_loading || self.executable_settings.is_none()))
                    .when_some(self.executable_settings.as_ref(), |view, settings| view.child(div().text_xs().child(settings.effective.pi.label())))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Codex 可执行文件"))
                    .child(Input::new(&self.codex_command).aria_label("Codex 可执行文件路径").disabled(self.settings_loading || self.executable_settings.is_none()))
                    .when_some(self.executable_settings.as_ref(), |view, settings| view.child(div().text_xs().child(settings.effective.codex.label())))
                    .child(h_flex().gap_2()
                        .child(Button::new("save-executable-settings").primary().label(if self.settings_loading { "处理中…" } else { "保存设置" }).disabled(self.settings_loading || self.executable_settings.is_none()).on_click(cx.listener(|this, _, window, cx| this.settings_request(true, window, cx))))
                        .child(Button::new("reload-executable-settings").label("重新读取").disabled(self.settings_loading).on_click(cx.listener(|this, _, window, cx| this.settings_request(false, window, cx))))))
                .into_any_element()
            } else if self.member_panel { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("团队成员"))
                .when(self.error.is_some(), |view| view.child(h_flex().p_3().gap_2().bg(rgb(0xffe9e4)).rounded_lg()
                    .child(div().flex_1().text_sm().child(self.error.clone().unwrap_or_default()))
                    .child(Button::new("dismiss-member-error").label("关闭").on_click(cx.listener(|this, _, _, cx| { this.error = None; cx.notify(); })))))
                .child(v_flex().id("members-panel").flex_1().min_h_0().min_w_0().overflow_y_scroll().gap_2()
                    .child(div().text_sm().child(if self.member_loading { "正在更新成员…" } else { "新建成员（默认停止）" }))
                    .child(h_flex().gap_3().child(div().flex_1().min_w_0().child(Input::new(&self.member_name))).child(div().flex_1().min_w_0().child(Textarea::new(&self.member_description).aria_label("成员职责说明"))))
                    .child(self.render_member_templates(cx))
                    .child(div().text_xs().child("角色"))
                    .child(h_flex().gap_1().children([("coder", "开发"), ("reviewer", "审查"), ("tester", "测试"), ("custom", "自定义")].into_iter().enumerate().map(|(i, (role, label))| Button::new(("member-role", i)).label(label).selected(self.member_role == role).on_click(cx.listener(move |this, _, _, cx| { this.member_role = role; cx.notify(); })))))
                    .child(div().text_xs().child("成员后端"))
                    .child(h_flex().gap_1().children([("pi", "Pi"), ("codex", "Codex"), ("pi-rpc", "外部 Pi")].into_iter().enumerate().map(|(i, (backend, label))| Button::new(("member-backend", i)).label(label).selected(self.member_backend == backend).on_click(cx.listener(move |this, _, _, cx| { this.member_backend = backend; cx.notify(); })))))
                    .child(Button::new("add-member").label("添加成员").disabled(self.member_loading).on_click(cx.listener(|this, _, _, cx| this.add_member(cx))))
                    .child(div().text_xs().child("启动前请为对应成员选择该后端的模型"))
                    .child(h_flex().justify_between().child(div().text_sm().child("团队成员")).child(Button::new("refresh-members").label("刷新").disabled(self.member_loading).on_click(cx.listener(|this, _, _, cx| this.refresh_members(cx)))))
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
                            .when(!running && backend.is_some(), |view| view.child(Button::new(("choose-member-model", i)).label(if picker_open { "重新读取模型" } else { "选择模型" }).disabled(self.member_loading).on_click(cx.listener(move |this, _, window, cx| { this.member_model_search.update(cx, |input, cx| input.set_value("", window, cx)); this.choose_member_models(picker_id.clone(), picker_command.clone(), cx); }))))
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
                                .child(Button::new(("member-start-stop", i)).label(if stopping { "正在停止…" } else if running { "停止" } else { "启动" }).disabled(self.member_loading || (!running && !member_model_ready)).on_click(cx.listener(move |this, _, _, cx| { if running { this.stop_member(start_id.clone(), cx); } else { this.start_member(start_id.clone(), start_command.clone(), cx); } })))
                                .child(Button::new(("member-edit", i)).label("改名").disabled(self.member_loading).on_click(cx.listener(move |this, _, window, cx| { this.editing_member = Some(edit_id.clone()); this.member_rename.update(cx, |input, cx| input.set_value(edit_name.clone(), window, cx)); cx.notify(); })))
                                .child(Button::new(("member-delete", i)).label(if deleting { "确认删除" } else { "删除" }).disabled(self.member_loading).on_click(cx.listener(move |this, _, _, cx| { if this.deleting_member.as_ref() == Some(&delete_id) { this.member_action("DELETE", format!("/api/workspaces/{delete_workspace}/workers/{delete_id}"), None, cx); } else { this.deleting_member = Some(delete_id.clone()); cx.notify(); } }))))
                            .when(editing, |view| view.child(Input::new(&self.member_rename)).child(Button::new(("member-save", i)).label("保存名称").disabled(self.member_loading).on_click(cx.listener(move |this, _, _, cx| { let name = this.member_rename.read(cx).value().trim().to_string(); if name.is_empty() { this.error = Some("请输入成员名称".into()); cx.notify(); } else { this.member_action("PATCH", format!("/api/workspaces/{save_workspace}/workers/{save_id}"), Some(serde_json::json!({"name":name})), cx); } }))))
                    })))
                .into_any_element()
            } else { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(h_flex().flex_shrink_0().min_w_0().w_full().child({
                    let title = self.session.as_ref().map(|s| self.session_title(s).to_string()).unwrap_or_else(|| "开始你的项目工作".into());
                    div().id("session-title").flex_1().min_w_0().truncate().text_xl().font_weight(FontWeight::SEMIBOLD)
                        .tooltip({ let title = title.clone(); move |window, cx| Tooltip::new(title.clone()).build(window, cx) })
                        .child(title)
                }))
                .child(div().text_xs().text_color(rgb(0x777c75)).child(self.session.as_ref().map(|s| format!("{} · {}", s.backend.label(), self.status)).unwrap_or_else(|| self.status.clone())))
                .when(self.error.is_some(), |view| view.child(h_flex().p_3().gap_2().bg(rgb(0xffe9e4)).rounded_lg()
                    .child(div().flex_1().text_sm().child(self.error.clone().unwrap_or_default()))
                    .child(Button::new("dismiss-error").label("关闭").on_click(cx.listener(|this, _, _, cx| { this.error = None; cx.notify(); })))))
                .child(div().id("messages").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).p_3()
                    .child(v_flex().gap_5()
                        .when(self.messages.is_empty(), |view| view.child(div().py_8().text_color(rgb(0x777c75)).child(if self.loading { "正在读取…" } else if self.session.is_some() { "会话已就绪。发送第一条消息开始工作。" } else { "添加项目文件夹，再创建一个会话。" })))
                        .children(self.messages.iter().enumerate().map(|(i, message)| {
                            let heading = message.tool.as_ref().map(|tool| format!("{} · {}", tool.name, tool.status)).unwrap_or_else(|| message.delivery.as_ref().map(|delivery| delivery.heading.clone()).unwrap_or_else(|| match message.role.as_str() { "user" if message.explicit_user => "你", "user" => "对话输入", "assistant" => self.session.as_ref().map(|s| s.backend.label()).unwrap_or("助手"), _ => "工具" }.into()));
                            let expanded = message.tool.as_ref().is_some_and(|tool| tool.expanded);
                            v_flex().id(("message", i)).min_w_0().gap_2().p_4().rounded_lg()
                                .bg(if message.role == "user" { rgb(0xe9eee7) } else if message.delivery.is_some() { rgb(0xf0f2ed) } else { rgb(0xffffff) })
                                .child(h_flex().min_w_0().gap_2().justify_between()
                                    .child(div().flex_1().min_w_0().truncate().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(if message.tool.as_ref().is_some_and(|tool| tool.status == "失败") { rgb(0xb54131) } else { rgb(0x63715c) }).child(heading))
                                    .when(message.tool.is_some(), |view| view.child(Button::new(("expand-tool", i)).accessibility_label(format!("{}工具详情", if expanded { "收起" } else { "展开" })).label(if expanded { "收起" } else { "展开" }).on_click({ let id = message.id.clone(); cx.listener(move |this, _, _, cx| {
                                        if let Some(tool) = this.messages.iter_mut().find(|m| m.id == id).and_then(|m| m.tool.as_mut()) { tool.expanded = !tool.expanded; }
                                        cx.notify();
                                    }) })))
                                    .when(message.delivery.is_some(), |view| view.child(Button::new(("expand-delivery", i)).accessibility_label(format!("{}消息原文", if message.delivery.as_ref().is_some_and(|d| d.expanded) { "收起" } else { "查看" })).label(if message.delivery.as_ref().is_some_and(|d| d.expanded) { "收起原文" } else { "查看原文" }).on_click({ let id = message.id.clone(); cx.listener(move |this, _, _, cx| {
                                        if let Some(delivery) = this.messages.iter_mut().find(|m| m.id == id).and_then(|m| m.delivery.as_mut()) { delivery.expanded = !delivery.expanded; }
                                        cx.notify();
                                    }) })))
                                    .child(Button::new(("copy", i)).accessibility_label(if message.delivery.is_some() { "复制完整消息原文" } else { "复制消息内容" }).label(if message.delivery.is_some() { "复制原文" } else { "复制" }).on_click({ let text = message.delivery.as_ref().map(|d| d.raw.clone()).unwrap_or_else(|| message.text.clone()); cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))) })))
                                .when(message.role == "assistant", |view| view.child(TextView::new(&message.markdown).selectable(true).stream_fade(true)))
                                .when(message.role == "user" || message.delivery.is_some() || expanded, |view| view.child(div().min_w_0().text_sm().child(message.text.clone())))
                                .when(message.delivery.as_ref().is_some_and(|d| d.expanded), |view| view.child(div().min_w_0().p_3().rounded_lg().bg(rgb(0xffffff)).text_sm().child(message.delivery.as_ref().map(|d| d.raw.clone()).unwrap_or_default())))
                                .when(!expanded && message.tool.as_ref().is_some_and(|tool| tool.status == "失败"), |view| view.child(div().text_sm().text_color(rgb(0xb54131)).child(message.text.chars().take(160).collect::<String>())))
                        }))))
                .child(Button::new("choose-model").label(self.model.as_ref().map(|model| format!("模型 · {} / {} ▾", model.provider, model.id)).unwrap_or_else(|| "选择模型 ▾".into())).disabled(self.session.is_none() || running || self.loading).on_click(cx.listener(|this, _, _, cx| { this.choosing_model = !this.choosing_model; cx.notify(); })))
                .when(self.choosing_model, |view| view.child(v_flex().flex_shrink_0().min_h_0().gap_2()
                    .child(Input::new(&self.model_search))
                    .child(v_flex().id("model-list").h(px(168.)).min_h_0().flex_shrink_0().overflow_y_scroll().gap_2()
                        .when(matching_models.is_empty(), |view| view.child(div().p_3().text_sm().child("没有匹配的可用模型")))
                        .children(matching_models.into_iter().enumerate().map(|(i, model)| {
                            let selected = self.model.as_ref().is_some_and(|m| m.id == model.id && m.provider == model.provider);
                            Button::new(("model", i)).flex_shrink_0().w_full().label(format!("{} / {}", model.provider, model.name)).tooltip(model.id.clone()).selected(selected).disabled(running || self.loading)
                                .on_click(cx.listener(move |this, _, _, cx| this.set_model(model.clone(), cx)))
                        })))))
                .when(pending > 0, |view| view.child(h_flex().gap_3()
                    .child(div().text_sm().child(format!("团队消息待处理：{pending}")))
                    .when(!running, |view| view.child(Button::new("resume").label("继续处理").disabled(self.loading).on_click(cx.listener(|this, _, _, cx| this.resume(cx)))))))
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
                .child(h_flex().gap_3().items_end()
                    .child(div().flex_1().child(Textarea::new(&self.input).aria_label("聊天消息")))
                    .child(if running {
                        Button::new("stop").label("停止").on_click(cx.listener(|this, _, _, cx| this.abort(cx)))
                    } else {
                        Button::new("send").primary().label("发送").disabled(self.session.is_none() || self.loading || pending > 0).on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
                    }))
                .child(div().text_xs().text_color(rgb(0x8a8e87)).child(if running { "团队正在处理 · 可随时停止" } else if pending > 0 { "有待处理消息，请先继续" } else { "Enter 换行 · 点击发送" }))
                .into_any_element() })
    }
}
