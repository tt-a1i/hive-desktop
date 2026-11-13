use super::*;
use crate::focus_scroll::{Reveal, Viewport};
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
impl Render for Resources {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale_english = english();
        if self.locale_english != locale_english {
            self.query.update(cx,|input,cx|input.set_placeholder(tr("搜索名称、完整说明、来源或路径…"),window,cx));
            self.locale_english = locale_english;
        }
        let query = self.query.read(cx).value().to_lowercase();
        let visible: Vec<_> = self
            .commands
            .iter()
            .filter(|command| {
                (self.filter == "all" || command.source == self.filter)
                    && (query.is_empty()
                        || format!(
                            "{} {} {} {} {} {} {}",
                            command.name,
                            command.description.as_deref().unwrap_or(""),
                            kind(&command.source),
                            scope(command),
                            command.source_info,kind_key(&command.source).unwrap_or(""),tr(scope(command))
                        )
                        .to_lowercase()
                        .contains(&query))
            })
            .cloned()
            .collect();
        let height = (window.viewport_size().height - px(310.))
            .max(px(120.))
            .min(px(600.));
        v_flex().min_w_0().gap_3()
            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child(tr("只显示此 Pi 会话实际加载的资源。重新读取只更新目录；新安装的资源需由原生后端重新加载。")))
            .child(h_flex().gap_2().items_center()
                .child(div().flex_1().min_w_0().child(Input::new(&self.query)))
                .child(Button::new("resources-refresh").ghost().rounded_full().label(if self.loading {tr("读取中…")}else{tr("重新读取")}).disabled(self.loading).on_click(cx.listener(|this,_,_,cx|this.load(cx)))))
            .child(h_flex().gap_2().flex_wrap().children([("all",tr("全部")),("skill",tr("技能")),("prompt",tr("提示模板")),("extension",tr("扩展"))].into_iter().map(|(filter,label)| {
                let count=self.commands.iter().filter(|command|filter=="all" || command.source==filter).count();
                Button::new(SharedString::from(format!("resource-filter-{filter}"))).ghost().compact().rounded_full().label(format!("{label} {count}")).selected(self.filter==filter)
                    .on_click(cx.listener(move|this,_,_,cx|{this.filter=filter;this.scroll.set_offset(point(px(0.),px(0.)));cx.notify();}))
            })))
            .when_some(self.error.as_ref().map(|error|error.text()),|view,error|view.child(div().p_3().rounded_lg().bg(Color::ErrorSurface.get(cx)).text_sm().text_color(Color::Error.get(cx)).child(error)))
            .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(if english() {format!("Showing {} of {} resources · Adding to the draft does not send or execute",visible.len(),self.commands.len())} else {format!("显示 {} / {} 项 · 补入草稿不会发送或执行",visible.len(),self.commands.len())}))
            .child(div().w_full().min_w_0().max_h(height).flex().child(Viewport::new("session-resource-viewport", &self.scroll,
                v_flex().id("session-resource-results").w_full().flex_1().min_w_0().gap_3().max_h(height).overflow_y_scroll().track_scroll(&self.scroll).vertical_scrollbar(&self.scroll).pr_1()
                    .when(visible.is_empty() && !self.loading,|view|view.child(div().p_4().rounded_xl().border_1().border_color(Color::Divider.get(cx)).bg(Color::Card.get(cx)).text_sm().text_color(Color::Secondary.get(cx)).child(if self.commands.is_empty(){tr("当前会话没有返回可浏览的技能、模板或扩展命令。")}else{tr("没有匹配的资源。")})))
                    .children(visible.into_iter().enumerate().map(|(index,command)| {
                        let name=command.name.clone();let copied=format!("/{}",command.name);let path=local_path(&command);let preview=path.clone();
                        Reveal::new(("resource-card-reveal", index), &self.scroll,
                            v_flex().min_w_0().p_3().gap_2().rounded_xl().border_1().border_color(Color::Divider.get(cx)).bg(Color::Card.get(cx))
                                .child(h_flex().min_w_0().gap_2().items_center().flex_wrap()
                                    .child(div().flex_1().min_w_0().font_family(Theme::global(cx).mono_font_family.clone()).text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("/{}",command.name)))
                                    .child(div().px_2().py_0p5().rounded_full().bg(Color::Inset.get(cx)).text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} · {}",kind(&command.source),tr(scope(&command))))))
                                .child(div().text_sm().child(command.description.clone().filter(|s|!s.is_empty()).unwrap_or_else(||tr("此资源未提供说明。").into())))
                                .when_some(command.source_info["source"].as_str().filter(|s|!s.is_empty()).map(str::to_owned),|view,source|view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(if english() {format!("{}: {}",tr("来源"), match source.as_str() { "auto" => tr("原生自动发现"), "builtin" => tr("原生内置"), _ => &source })} else {format!("{}：{}",tr("来源"), match source.as_str() { "auto" => tr("原生自动发现"), "builtin" => tr("原生内置"), _ => &source })})))
                                .when_some(command.source_info["path"].as_str().map(str::to_owned),|view,path|view.child(crate::text_reading::selectable_text(format!("resource-path-{index}"), path).font_family(Theme::global(cx).mono_font_family.clone()).text_xs().text_color(Color::Secondary.get(cx))))
                                .child(h_flex().gap_2().flex_wrap()
                                    .child(Button::new(("resource-use",index)).primary().compact().rounded_full().label(tr("补入草稿")).accessibility_label(format!("{}: /{}",tr("补入草稿"),command.name)).on_click(cx.listener(move|this,_,window,cx|this.use_command(name.clone(),window,cx))))
                                    .child(Button::new(("resource-copy",index)).ghost().compact().rounded_full().label(tr("复制命令")).accessibility_label(format!("{}: /{}",tr("复制命令"),command.name)).on_click(cx.listener(move|_,_,_,cx|cx.write_to_clipboard(ClipboardItem::new_string(copied.clone())))))
                                    .when_some(preview,|row,path|row.child(Button::new(("resource-preview",index)).ghost().compact().rounded_full().label(tr("查看来源文件")).accessibility_label(format!("{}: /{}",tr("查看来源文件"),command.name)).on_click(cx.listener(move|this,_,window,cx|this.preview(path.clone(),window,cx)))))
                                    .when_some(path,|row,path| {
                                        let reveal=path.clone();row
                                            .child(Button::new(("resource-copy-path",index)).ghost().compact().rounded_full().label(tr("复制路径")).accessibility_label(format!("{}: /{}",tr("复制路径"),command.name)).on_click(cx.listener(move|_,_,_,cx|cx.write_to_clipboard(ClipboardItem::new_string(path.clone())))))
                                            .child(Button::new(("resource-reveal",index)).ghost().compact().rounded_full().label(tr("定位文件")).accessibility_label(format!("{}: /{}",tr("定位文件"),command.name)).on_click(cx.listener(move|this,_,_,cx|{if this.current(cx){cx.reveal_path(std::path::Path::new(&reveal));}else{this.stale(cx);}})))
                                    }))
                                .when(command.source_info["path"].as_str().is_some() && local_path(&command).is_none(),|view|view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(tr("原生内置或非本地文件来源，不提供文件预览。")))))
                    })))))
    }
}
