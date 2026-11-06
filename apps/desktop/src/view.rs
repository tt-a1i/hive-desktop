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
                .child(h_flex().pt_4().justify_between().child(div().text_xs().text_color(rgb(0x777c75)).child("会话"))
                    .child(Button::new("new-chat").label("＋ 新会话").disabled(self.workspace.is_none() || self.loading).on_click(cx.listener(|this, _, _, cx| this.new_session(cx)))))
                .child(v_flex().id("sessions").flex_1().min_h_0().overflow_y_scroll().gap_2()
                    .children(self.sessions.iter().enumerate().map(|(i, session)| {
                        let id = session.id.clone();
                        Button::new(("session", i)).label(session.title.clone()).selected(self.session.as_ref().is_some_and(|s| s.id == session.id))
                            .on_click(cx.listener(move |this, _, _, cx| this.open_session(id.clone(), cx)))
                    })))
                .child(div().text_xs().text_color(rgb(0x777c75)).child("本地会话 · Pi")))
            .child(v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(div().min_w_0().w_full().truncate().text_xl().font_weight(FontWeight::SEMIBOLD).child(self.session.as_ref().map(|s| s.title.clone()).unwrap_or_else(|| "开始你的项目工作".into())))
                .child(div().text_xs().text_color(rgb(0x777c75)).child(self.status.clone()))
                .when(self.error.is_some(), |view| view.child(h_flex().p_3().gap_2().bg(rgb(0xffe9e4)).rounded_lg()
                    .child(div().flex_1().text_sm().child(self.error.clone().unwrap_or_default()))
                    .child(Button::new("dismiss-error").label("关闭").on_click(cx.listener(|this, _, _, cx| { this.error = None; cx.notify(); })))))
                .child(div().id("messages").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).p_3()
                    .child(v_flex().gap_5()
                        .when(self.messages.is_empty(), |view| view.child(div().py_8().text_color(rgb(0x777c75)).child(if self.loading { "正在读取…" } else if self.session.is_some() { "会话已就绪。发送第一条消息开始工作。" } else { "添加项目文件夹，再创建一个会话。" })))
                        .children(self.messages.iter().enumerate().map(|(i, message)| {
                            v_flex().id(("message", i)).gap_2().p_4().rounded_lg()
                                .bg(if message.role == "user" { rgb(0xe9eee7) } else { rgb(0xffffff) })
                                .child(h_flex().justify_between().child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(rgb(0x63715c)).child(match message.role.as_str() { "user" => "你", "assistant" => "Pi", _ => "工具" }))
                                    .child(Button::new(("copy", i)).label("复制").on_click({ let text = message.text.clone(); cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))) })))
                                .when(message.role == "assistant", |view| view.child(TextView::new(&message.markdown).selectable(true).stream_fade(true)))
                                .when(message.role != "assistant", |view| view.child(div().text_sm().child(message.text.clone())))
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
                .child(h_flex().gap_3().items_end()
                    .child(div().flex_1().child(Textarea::new(&self.input).aria_label("聊天消息")))
                    .child(if running {
                        Button::new("stop").label("停止").on_click(cx.listener(|this, _, _, cx| this.abort(cx)))
                    } else {
                        Button::new("send").primary().label("发送").disabled(self.session.is_none() || self.loading).on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
                    }))
                .child(div().text_xs().text_color(rgb(0x8a8e87)).child(if running { "Pi 正在处理 · 可随时停止" } else { "Enter 换行 · 点击发送" })))
    }
}
