use super::*;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        *,
    },
    prelude::FluentBuilder,
    *,
};
impl Render for Resources {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.query.read(cx).value().to_lowercase();
        let visible: Vec<_> = self
            .commands
            .iter()
            .filter(|command| {
                (self.filter == "all" || command.source == self.filter)
                    && (query.is_empty()
                        || format!(
                            "{} {} {} {} {}",
                            command.name,
                            command.description.as_deref().unwrap_or(""),
                            kind(&command.source),
                            scope(command),
                            command.source_info
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
            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child("只显示此 Pi 会话实际加载的资源。重新读取只更新目录；新安装的资源需由原生后端重新加载。"))
            .child(h_flex().gap_2().items_center()
                .child(div().flex_1().min_w_0().child(Input::new(&self.query)))
                .child(Button::new("resources-refresh").ghost().label(if self.loading {"读取中…"}else{"重新读取"}).disabled(self.loading).on_click(cx.listener(|this,_,_,cx|this.load(cx)))))
            .child(h_flex().gap_2().flex_wrap().children([("all","全部"),("skill","技能"),("prompt","提示模板"),("extension","扩展")].into_iter().map(|(filter,label)| {
                let count=self.commands.iter().filter(|command|filter=="all" || command.source==filter).count();
                Button::new(SharedString::from(format!("resource-filter-{filter}"))).ghost().label(format!("{label} {count}")).selected(self.filter==filter)
                    .on_click(cx.listener(move|this,_,_,cx|{this.filter=filter;this.scroll.set_offset(point(px(0.),px(0.)));cx.notify();}))
            })))
            .when_some(self.error.clone(),|view,error|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)))
            .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("显示 {} / {} 项 · 补入草稿不会发送或执行",visible.len(),self.commands.len())))
            .child(v_flex().id("session-resource-results").min_w_0().gap_3().max_h(height).overflow_y_scroll().track_scroll(&self.scroll)
                .when(visible.is_empty() && !self.loading,|view|view.child(div().p_3().text_sm().child(if self.commands.is_empty(){"当前会话没有返回可浏览的技能、模板或扩展命令。"}else{"没有匹配的资源。"})))
                .children(visible.into_iter().enumerate().map(|(index,command)| {
                    let name=command.name.clone();let copied=format!("/{}",command.name);let path=local_path(&command);let preview=path.clone();
                    v_flex().min_w_0().p_3().gap_2().rounded_lg().border_1().border_color(Color::Divider.get(cx))
                        .child(h_flex().min_w_0().gap_2().flex_wrap()
                            .child(div().flex_1().min_w_0().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("/{}",command.name)))
                            .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} · {}",kind(&command.source),scope(&command)))))
                        .child(div().text_sm().child(command.description.clone().filter(|s|!s.is_empty()).unwrap_or_else(||"此资源未提供说明。".into())))
                        .when_some(command.source_info["source"].as_str().filter(|s|!s.is_empty()).map(str::to_owned),|view,source|view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("来源：{}", match source.as_str() { "auto" => "原生自动发现", "builtin" => "原生内置", _ => &source }))))
                        .when_some(command.source_info["path"].as_str().map(str::to_owned),|view,path|view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(path)))
                        .child(h_flex().gap_2().flex_wrap()
                            .child(Button::new(("resource-use",index)).label("补入草稿").accessibility_label(format!("将 /{} 补入当前会话草稿",command.name)).on_click(cx.listener(move|this,_,window,cx|this.use_command(name.clone(),window,cx))))
                            .child(Button::new(("resource-copy",index)).ghost().label("复制命令").accessibility_label(format!("复制 /{} 命令",command.name)).on_click(cx.listener(move|_,_,_,cx|cx.write_to_clipboard(ClipboardItem::new_string(copied.clone())))))
                            .when_some(preview,|row,path|row.child(Button::new(("resource-preview",index)).ghost().label("查看来源文件").accessibility_label(format!("查看 /{} 的来源文件",command.name)).on_click(cx.listener(move|this,_,window,cx|this.preview(path.clone(),window,cx)))))
                            .when_some(path,|row,path| {
                                let reveal=path.clone();row
                                    .child(Button::new(("resource-copy-path",index)).ghost().label("复制路径").accessibility_label(format!("复制 /{} 的来源路径",command.name)).on_click(cx.listener(move|_,_,_,cx|cx.write_to_clipboard(ClipboardItem::new_string(path.clone())))))
                                    .child(Button::new(("resource-reveal",index)).ghost().label("定位文件").accessibility_label(format!("定位 /{} 的来源文件",command.name)).on_click(cx.listener(move|this,_,_,cx|{if this.current(cx){cx.reveal_path(std::path::Path::new(&reveal));}else{this.stale(cx);}})))
                            }))
                        .when(command.source_info["path"].as_str().is_some() && local_path(&command).is_none(),|view|view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child("原生内置或非本地文件来源，不提供文件预览。")))
                })))
    }
}
