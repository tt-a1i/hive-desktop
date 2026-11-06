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
        let member_model_ready = !self.member_provider.read(cx).value().trim().is_empty()
            && !self.member_model.read(cx).value().trim().is_empty();
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
                .child(Button::new("toggle-members").label(if self.member_panel { "返回会话" } else { "管理团队成员" }).disabled(self.workspace.is_none()).on_click(cx.listener(|this, _, _, cx| { this.member_panel = !this.member_panel; if this.member_panel { this.refresh_members(cx); } cx.notify(); })))
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
                                .on_click(cx.listener(move |this, _, _, cx| this.open_session(id.clone(), cx))))
                    })))
                .child(div().text_xs().text_color(rgb(0x777c75)).child("本地会话")))
            .child(if self.member_panel { v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("团队成员"))
                .when(self.error.is_some(), |view| view.child(h_flex().p_3().gap_2().bg(rgb(0xffe9e4)).rounded_lg()
                    .child(div().flex_1().text_sm().child(self.error.clone().unwrap_or_default()))
                    .child(Button::new("dismiss-member-error").label("关闭").on_click(cx.listener(|this, _, _, cx| { this.error = None; cx.notify(); })))))
                .child(v_flex().id("members-panel").flex_1().min_h_0().min_w_0().overflow_y_scroll().gap_2()
                    .child(div().text_sm().child(if self.member_loading { "正在更新成员…" } else { "新建成员（默认停止）" }))
                    .child(h_flex().gap_3().child(div().flex_1().min_w_0().child(Input::new(&self.member_name))).child(div().flex_1().min_w_0().child(Input::new(&self.member_description))))
                    .child(div().text_xs().child("角色"))
                    .child(h_flex().gap_1().children([("coder", "开发"), ("reviewer", "审查"), ("tester", "测试"), ("custom", "自定义")].into_iter().enumerate().map(|(i, (role, label))| Button::new(("member-role", i)).label(label).selected(self.member_role == role).on_click(cx.listener(move |this, _, _, cx| { this.member_role = role; cx.notify(); })))))
                    .child(div().text_xs().child("成员后端"))
                    .child(h_flex().gap_1().children([("pi", "Pi"), ("codex", "Codex"), ("pi-rpc", "外部 Pi")].into_iter().enumerate().map(|(i, (backend, label))| Button::new(("member-backend", i)).label(label).selected(self.member_backend == backend).on_click(cx.listener(move |this, _, _, cx| { this.member_backend = backend; cx.notify(); })))))
                    .child(Button::new("add-member").label("添加成员").disabled(self.member_loading).on_click(cx.listener(|this, _, _, cx| this.add_member(cx))))
                    .child(div().text_xs().child("启动成员前指定模型"))
                    .child(h_flex().gap_3().child(div().flex_1().min_w_0().child(Input::new(&self.member_provider))).child(div().flex_1().min_w_0().child(Input::new(&self.member_model))))
                    .when_some(self.model.clone(), |view, model| view.child(Button::new("use-current-model").label(format!("使用当前模型 · {} / {}", model.provider, model.id)).on_click(cx.listener(move |this, _, window, cx| { this.member_provider.update(cx, |input, cx| input.set_value(model.provider.clone(), window, cx)); this.member_model.update(cx, |input, cx| input.set_value(model.id.clone(), window, cx)); cx.notify(); }))))
                    .child(h_flex().justify_between().child(div().text_sm().child("团队成员")).child(Button::new("refresh-members").label("刷新").disabled(self.member_loading).on_click(cx.listener(|this, _, _, cx| this.refresh_members(cx)))))
                    .when(self.members.is_empty() && !self.member_loading, |view| view.child(div().text_xs().child("暂无成员")))
                    .children(self.members.iter().enumerate().map(|(i, member)| {
                        let id = member.id.clone();
                        let workspace = self.workspace.clone().unwrap_or_default();
                        let editing = self.editing_member.as_ref() == Some(&id);
                        let deleting = self.deleting_member.as_ref() == Some(&id);
                        let running = member.status != "stopped";
                        let stopping = self.member_stopping.as_ref() == Some(&id);
                        let session_id = self.sessions.iter().find(|session| session.agent_id.as_ref() == Some(&id)).map(|s| s.id.clone());
                        let start_id = id.clone(); let start_command = member.configured_command.clone();
                        let edit_id = id.clone(); let edit_name = member.name.clone();
                        let save_id = id.clone(); let save_workspace = workspace.clone();
                        let delete_id = id.clone(); let delete_workspace = workspace.clone();
                        v_flex().gap_1().p_2().rounded_lg().bg(rgb(0xffffff))
                            .child(div().text_sm().child(member.name.clone()))
                            .child(div().text_xs().child(format!("{} · {} · {}", member.role_label(), member.status_label(), member.backend_label())))
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
