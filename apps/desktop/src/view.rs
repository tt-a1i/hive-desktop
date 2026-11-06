use crate::app::HiveApp;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
        text::TextView,
        *,
    },
    *,
};

impl Render for HiveApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                .child(h_flex().pt_4().justify_between().child(div().text_xs().text_color(rgb(0x777c75)).child("会话与团队"))
                    .child(Button::new("new-chat").label("＋ 新会话").disabled(self.workspace.is_none() || self.loading).on_click(cx.listener(|this, _, _, cx| this.new_session(cx)))))
                .child(v_flex().id("sessions").flex_1().min_h_0().overflow_y_scroll().gap_2()
                    .children(self.sessions.iter().enumerate().map(|(i, session)| {
                        let id = session.id.clone();
                        let role = session.agent_id.as_deref().map(|agent| if agent.ends_with(":orchestrator") { "主控" } else { "成员" }).unwrap_or("会话");
                        let activity = if self.busy.contains(&id) { " · 处理中" } else if self.pending.get(&id).copied().unwrap_or(0) > 0 { " · 已暂停" } else if self.unread.contains(&id) { " · 未读" } else { "" };
                        v_flex().min_w_0().gap_1()
                            .child(div().text_xs().text_color(rgb(0x63715c)).child(format!("{role}{activity}")))
                            .child(Button::new(("session", i)).w_full().min_w_0()
                                .child(div().flex_1().min_w_0().truncate().child(session.title.clone()))
                                .tooltip(session.agent_id.as_ref().map(|agent| format!("{}\n{agent}", session.title)).unwrap_or_else(|| session.title.clone()))
                                .selected(self.session.as_ref().is_some_and(|s| s.id == session.id))
                                .on_click(cx.listener(move |this, _, _, cx| this.open_session(id.clone(), cx))))
                    })))
                .child(div().text_xs().text_color(rgb(0x777c75)).child("本地会话 · Pi")))
            .child(v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(div().flex_shrink_0().min_w_0().w_full().truncate().text_xl().font_weight(FontWeight::SEMIBOLD).child(self.session.as_ref().map(|s| s.title.clone()).unwrap_or_else(|| "开始你的项目工作".into())))
                .child(div().text_xs().text_color(rgb(0x777c75)).child(self.status.clone()))
                .when(self.error.is_some(), |view| view.child(h_flex().p_3().gap_2().bg(rgb(0xffe9e4)).rounded_lg()
                    .child(div().flex_1().text_sm().child(self.error.clone().unwrap_or_default()))
                    .child(Button::new("dismiss-error").label("关闭").on_click(cx.listener(|this, _, _, cx| { this.error = None; cx.notify(); })))))
                .child(div().id("messages").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).p_3()
                    .child(v_flex().gap_5()
                        .when(self.messages.is_empty(), |view| view.child(div().py_8().text_color(rgb(0x777c75)).child(if self.loading { "正在读取…" } else if self.session.is_some() { "会话已就绪。发送第一条消息开始工作。" } else { "添加项目文件夹，再创建一个会话。" })))
                        .children(self.messages.iter().enumerate().map(|(i, message)| {
                            let heading = message.tool.as_ref().map(|tool| format!("{} · {}", tool.name, tool.status)).unwrap_or_else(|| message.delivery.as_ref().map(|delivery| delivery.heading.clone()).unwrap_or_else(|| match message.role.as_str() { "user" if message.explicit_user => "你", "user" => "对话输入", "assistant" => "Pi", _ => "工具" }.into()));
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
                .child(div().text_xs().text_color(rgb(0x8a8e87)).child(if running { "团队正在处理 · 可随时停止" } else if pending > 0 { "有待处理消息，请先继续" } else { "Enter 换行 · 点击发送" })))
    }
}
