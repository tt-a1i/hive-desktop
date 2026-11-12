use crate::{app::HiveApp, focus_scroll::{Reveal, Viewport}, language::{english,tr}, palette::Color};
use gpui_kit::{assets::IconName as Glyph, component::{button::{Button,ButtonVariants}, input::{Input,Textarea}, scroll::ScrollableElement, *}, prelude::FluentBuilder, *};

impl HiveApp {
    pub(crate) fn render_members(&mut self, window: &mut Window, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        if !self.member_loading && std::mem::take(&mut self.member_focus_search) {
            let focus = self.member_search.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        let member_search = self.member_search.read(cx).value().trim().to_lowercase();
        let matching_members: Vec<_> = self.members.iter().enumerate().filter(|(_, member)| {
            (self.member_status_filter == "all" || member.status == self.member_status_filter)
                && (member_search.is_empty() || format!("{} {} {} {} {}", member.name, member.role, member.role_label(), member.backend_label(), member.description).to_lowercase().contains(&member_search))
        }).collect();
        let member_browsing = !member_search.is_empty() || self.member_status_filter != "all";
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
        v_flex().flex_1().min_w_0().min_h_0().h_full().overflow_hidden().child(Viewport::new("members-viewport", &self.member_scroll, v_flex().id("members-page").flex_1().min_h_0().min_w_0().h_full().w_full().max_w(px(960.)).mx_auto().gap_4().px_5().py_6().when(!narrow, |panel| panel.px_8().py_8()).overflow_y_scroll().track_scroll(&self.member_scroll).vertical_scrollbar(&self.member_scroll)
                .child(div().flex_shrink_0().child(Reveal::new("members-header-reveal", &self.member_scroll, h_flex().w_full().items_center().gap_2().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(tr("团队成员")))
                    .child(div().flex_1().text_xs().text_color(Color::Muted.get(cx)).child(if self.member_loading { tr("更新中").into() } else if self.members.is_empty() && self.member_error.is_some() { tr("成员状态未能更新").into() } else { if english() { format!("{} {}", self.members.len(), if self.members.len() == 1 { "member" } else { "members" }) } else { format!("{} 位", self.members.len()) } }))
                    .child(Button::new("refresh-members").ghost().compact().icon(Glyph::RefreshCw).tooltip(tr("刷新成员")).accessibility_label(tr("刷新成员")).disabled(!self.connected || self.member_loading).on_click(cx.listener(|this, _, _, cx| this.refresh_members(cx))))
                    .child(Button::new("toggle-member-form").ghost().icon(Glyph::Plus).label(if self.member_form_open { tr("收起表单") } else { tr("添加成员") }).on_click(cx.listener(|this, _, window, cx| { this.member_form_open = !this.member_form_open; let input = if this.member_form_open { &this.member_name } else { &this.member_search }; let focus = input.read(cx).focus_handle(cx); window.on_next_frame(move |window,cx|focus.focus(window,cx)); cx.notify(); }))))))
                .when(!self.members.is_empty() || self.member_error.is_none(), |view| view.child(div().flex_shrink_0().child(Reveal::new("members-filter-reveal", &self.member_scroll, self.render_member_filters(matching_members.len(), member_browsing, cx)))))
                .when(self.error.is_some(), |view| view.child(div().flex_shrink_0().child(self.render_error_banner(cx))))
                .when_some(self.member_error.clone(), |view,error| {
                    let message = format!("{}: {error}", tr("成员状态未能更新"));
                    view.child(div().id("member-refresh-error").role(Role::Label).aria_label(message.clone()).text_sm().text_color(Color::Error.get(cx)).child(message))
                })
                .child(v_flex().id("members-panel").flex_shrink_0().min_w_0().gap_2().p_1()
                    .when(self.member_form_open, |list| list.child(v_flex().gap_2().p_3().rounded_lg().border_1().border_color(Color::Divider.get(cx))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("新建成员")))
                    .child(Reveal::new("member-form-inputs-reveal", &self.member_scroll, h_flex().gap_3().child(div().flex_1().min_w_0().child(Input::new(&self.member_name).aria_label(tr("成员名称")))).child(div().flex_1().min_w_0().child(Textarea::new(&self.member_description).aria_label(tr("成员职责说明"))))))
                    .child(Reveal::new("member-form-templates-reveal", &self.member_scroll, self.render_member_templates(cx)))
                    .child(div().text_xs().child(tr("角色")))
                    .child(Reveal::new("member-form-roles-reveal", &self.member_scroll, h_flex().gap_1().children([("coder", tr("开发")), ("reviewer", tr("审查")), ("tester", tr("测试")), ("custom", tr("自定义"))].into_iter().enumerate().map(|(i, (role, label))| Button::new(("member-role", i)).label(label).selected(self.member_role == role).on_click(cx.listener(move |this, _, _, cx| { this.member_role = role; cx.notify(); }))))))
                    .child(div().text_xs().child(tr("成员后端")))
                    .child(Reveal::new("member-form-backends-reveal", &self.member_scroll, h_flex().gap_1().children([("pi", "Pi"), ("codex", "Codex"), ("pi-rpc", tr("外部 Pi")), ("claude", "Claude")].into_iter().enumerate().map(|(i, (backend, label))| Button::new(("member-backend", i)).label(label).selected(self.member_backend == backend).on_click(cx.listener(move |this, _, _, cx| { this.member_backend = backend; cx.notify(); }))))))
                    .child(Reveal::new("member-form-save-reveal", &self.member_scroll, Button::new("add-member").label(tr("添加成员")).disabled(!self.connected || self.member_loading).on_click(cx.listener(|this, _, window, cx| this.add_member(window, cx)))))))

                    .when(self.members.is_empty() && !self.member_loading && self.member_error.is_none() && !self.member_form_open, |view| view.child(v_flex().flex_1().items_center().justify_center().gap_3()
                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(tr("还没有团队成员")))
                        .child(Button::new("first-member").rounded_full().icon(Glyph::Plus).label(tr("添加第一位成员"))
                            .on_click(cx.listener(|this, _, window, cx| { this.member_form_open = true; let focus = this.member_name.read(cx).focus_handle(cx); window.on_next_frame(move |window,cx|focus.focus(window,cx)); cx.notify(); })))))
                    .when(!self.members.is_empty() && matching_members.is_empty(), |view|view.child(v_flex().items_center().gap_2().py_6()
                        .child(div().id("members-no-matches").role(Role::Label).aria_label(tr("没有匹配的成员")).text_sm().child(tr("没有匹配的成员")))
                        .child(div().id("members-no-matches-help").role(Role::Label).aria_label(tr("尝试其他名称、职责、角色或 Agent，或清除筛选。")).text_xs().text_color(Color::Muted.get(cx)).child(tr("尝试其他名称、职责、角色或 Agent，或清除筛选。")))))
                    .children(matching_members.into_iter().map(|(i, member)| {
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
                        v_flex().flex_shrink_0().id(SharedString::from(format!("team-member-{id}")))
                            .role(Role::Group).aria_label(format!("{} · {} · {}", member.name, member.role_label(), member.backend_label()))
                            .gap_2().py_3().px_2().border_b_1().border_color(Color::Divider.get(cx))
                            .child(Reveal::new(format!("member-{id}-header-reveal"), &self.member_scroll, h_flex().w_full().min_w_0().flex_wrap().items_center().gap_3()
                                .child(div().size(px(36.)).rounded_full().bg(Color::Inset.get(cx)).flex().items_center().justify_center().text_sm().font_weight(FontWeight::SEMIBOLD)
                                    .child(member.name.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default()))
                                .child(v_flex().flex_1().min_w(px(160.)).gap_1()
                                    .child(div().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).child(member.name.clone()))
                                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} · {}", member.role_label(), member.backend_label()))))
                                .child(h_flex().flex_wrap().items_center().gap_3().when(narrow, |view| view.w_full())
                                .child(div().id(SharedString::from(format!("member-workload-{id}"))).role(Role::Label)
                                    .aria_label(member.workload_label()).rounded_full().bg(Color::Inset.get(cx)).px_2().py_1().text_xs().child(member.workload_label()))
                                .when_some(session_id, |view, session_id| view.child(Button::new(("member-chat", i)).ghost().compact().icon(Glyph::MessagesSquare).label(tr("会话")).tooltip(tr("打开会话")).accessibility_label(if english() { format!("Open {}’s member conversation", member.name) } else { format!("打开 {} 的成员会话", member.name) }).on_click(cx.listener(move |this, _, window, cx| this.open_member_session(session_id.clone(), window, cx)))))
                                .child(Button::new(("member-start-stop", i)).ghost().compact().label(if stopping { tr("正在停止…") } else if running { tr("停止") } else { tr("启动") }).disabled(!self.connected || self.member_loading || (!running && !member_model_ready)).on_click(cx.listener(move |this, _, _, cx| { if running { this.stop_member(start_id.clone(), cx); } else { this.member_action("POST", format!("/api/workspaces/{start_workspace}/agents/{start_id}/start"), None, cx); } })))
                                .child(self.member_menu(member, cx)))))
                            .child(self.render_member_description(member, cx))
                            .when(!running && backend.is_some(), |view| view.child(Reveal::new(format!("member-{id}-model-trigger-reveal"), &self.member_scroll, h_flex().child(Button::new(("choose-member-model", i)).ghost().compact()
                                .label(if picker_open { tr("收起模型列表").into() } else { chosen.map(|m| format!("{} / {} ▾", m.provider, m.id)).unwrap_or_else(|| tr("选择启动模型 ▾").into()) })
                                .disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, window, cx| {
                                    if picker_open { this.member_picker_target = None; this.member_focus_search = true; cx.notify(); }
                                    else {
                                        this.member_model_search.update(cx, |input, cx| input.set_value("", window, cx));
                                        this.member_model_scroll.set_offset(point(px(0.),px(0.)));
                                        this.choose_member_models(picker_id.clone(), picker_command.clone(), cx);
                                        let focus = this.member_model_search.read(cx).focus_handle(cx);
                                        window.on_next_frame(move |window,cx|focus.focus(window,cx));
                                    }
                                }))))))
                            .when(!running && backend.is_none(), |view| view.child(div().text_xs().child(tr("此成员的后端未配置或不受支持"))))
                            .when(running, |view| view.when_some(chosen, |view, model| view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} / {}", model.provider, model.id)))))
                            .when(picker_open, |view| view.child(v_flex().gap_2()
                                .child(Reveal::new(format!("member-{id}-model-search-reveal"), &self.member_scroll, Input::new(&self.member_model_search).aria_label(tr("搜索此成员的可用模型"))))
                                .when(self.member_catalog_loading, |view| view.child(crate::text_reading::selectable_text("member-model-loading", tr("正在读取可用模型…")).text_sm()))
                                .when_some(self.member_catalog_error.clone(), |view, (backend,error)| {
                                    let repair_backend = match backend.as_str() {
                                        "pi" => Some(crate::protocol::Backend::Pi), "codex" => Some(crate::protocol::Backend::Codex),
                                        "claude" => Some(crate::protocol::Backend::Claude), "pi-rpc" => Some(crate::protocol::Backend::PiRpc), _ => None,
                                    };
                                    let retry_id = id.clone(); let retry_command = member.configured_command.clone();
                                    view.child(crate::text_reading::selectable_text("member-model-error", if english() { format!("Could not load {backend} models: {error}") } else { format!("读取 {backend} 模型失败：{error}") }).text_sm().text_color(Color::Error.get(cx)))
                                        .child(Reveal::new(format!("member-{id}-model-repair-reveal"), &self.member_scroll, h_flex().min_w_0().flex_wrap().gap_2()
                                            .when_some(repair_backend, |row, backend| row.child(self.render_backend_repair_actions(backend, !self.connected || self.member_catalog_loading, cx)))
                                            .child(Button::new("member-model-retry").ghost().compact().label(tr("重新读取模型")).disabled(!self.connected || self.member_catalog_loading)
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    this.choose_member_models(retry_id.clone(), retry_command.clone(), cx);
                                                    let focus = this.member_model_search.read(cx).focus_handle(cx);
                                                    window.on_next_frame(move |window,cx|focus.focus(window,cx));
                                                })))))
                                })
                                .when(!self.member_catalog_loading && self.member_catalog_error.is_none() && matching_member_models.is_empty(), |view| view.child(crate::text_reading::selectable_text("member-model-empty", tr("没有匹配的可用模型")).text_sm()))
                                .child(Reveal::new(format!("member-{id}-models-reveal"), &self.member_scroll, div().h(px(168.)).min_h_0().flex().child(Viewport::new("member-model-viewport", &self.member_model_scroll, v_flex().id("member-model-list").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(&self.member_model_scroll).vertical_scrollbar(&self.member_model_scroll).gap_1().p_1()
                                    .children(matching_member_models.iter().cloned().enumerate().map(|(j, model)| {
                                        let path = format!("/api/workspaces/{workspace}/agents/{id}/config"); let command = member.configured_command.clone();
                                        Reveal::new(("member-model-option-reveal",j), &self.member_model_scroll, Button::new(("member-model", j)).ghost().flex_shrink_0().w_full().h_auto().justify_start().selected(chosen.is_some_and(|m| m.provider == model.provider && m.id == model.id)).label(format!("{} / {}", model.provider, model.name)).tooltip(model.id.clone()).on_click(cx.listener(move |this, _, _, cx| { this.member_picker_target = None; this.member_focus_search = true;
                                            this.member_action("POST", path.clone(), Some(serde_json::json!({"command":command,"command_preset_id":null,"args":["--provider",model.provider,"--model",model.id]})), cx); })))
                                    }))))))))
                            .when(deleting, |view| view.child(Reveal::new(format!("member-{id}-delete-reveal"), &self.member_scroll, h_flex().flex_wrap().items_center().gap_2()
                                .child(div().id(SharedString::from(format!("member-{id}-delete-prompt"))).role(Role::Label).aria_label(if english() { format!("Delete member {}?", member.name) } else { format!("删除成员 {}？", member.name) }).text_sm().child(if english() { format!("Delete member {}?", member.name) } else { format!("删除成员 {}？", member.name) }))
                                .child(Button::new(("confirm-delete", i)).label(tr("确认删除")).disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, _, cx| this.member_action("DELETE", format!("/api/workspaces/{delete_workspace}/workers/{delete_id}"), None, cx))))
                                .child(Button::new(("cancel-delete", i)).ghost().label(tr("取消")).on_click(cx.listener(|this, _, _, cx| { this.deleting_member = None; this.member_focus_search = true; cx.notify(); }))))))
                            .when(editing, |view| view.child(Reveal::new(format!("member-{id}-rename-reveal"), &self.member_scroll, h_flex().w_full().gap_2().items_center().child(div().flex_1().min_w_0().child(Input::new(&self.member_rename).aria_label(tr("成员名称")))).child(Button::new(("member-save", i)).label(tr("保存名称")).disabled(!self.connected || self.member_loading).on_click(cx.listener(move |this, _, _, cx| { let name = this.member_rename.read(cx).value().trim().to_string(); if name.is_empty() { this.error = Some("请输入成员名称".into()); cx.notify(); } else { this.error = None; this.member_action("PATCH", format!("/api/workspaces/{save_workspace}/workers/{save_id}"), Some(serde_json::json!({"name":name})), cx); } }))).child(Button::new(("cancel-rename", i)).ghost().label(tr("取消")).on_click(cx.listener(|this, _, _, cx| { this.editing_member = None; this.member_focus_search = true; cx.notify(); }))))))
                    })))
                )).into_any_element()
    }
}
