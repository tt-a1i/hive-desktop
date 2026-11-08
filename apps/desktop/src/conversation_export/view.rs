use super::*;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        switch::Switch,
        *,
    },
    prelude::FluentBuilder,
    *,
};
impl Render for ConversationExport {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = (window.viewport_size().height - px(350.))
            .max(px(100.))
            .min(px(500.));
        let preview: String = self.text.chars().take(10000).collect();
        let truncated = preview.len() < self.text.len();
        v_flex().min_w_0().gap_3()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(self.snapshot.title.clone()))
            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child(format!("{} 条已加载记录的固定快照 · Markdown 格式。只记录普通附件名称及元数据、可用的图片数量，未发送草稿不会导出。",self.snapshot.count())))
            .child(h_flex().flex_wrap().gap_4().items_center()
                .child(h_flex().items_center().gap_2().child(Switch::new("export-thinking").accessibility_label("导出时包含思考过程").checked(self.thinking).disabled(self.saving)
                    .on_change(cx.listener(|this,value,_,cx|{this.thinking=*value;this.refresh(cx);}))).child("包含思考过程"))
                .child(h_flex().items_center().gap_2().child(Switch::new("export-tools").accessibility_label("导出时包含工具过程").checked(self.tools).disabled(self.saving)
                    .on_change(cx.listener(|this,value,_,cx|{this.tools=*value;this.refresh(cx);}))).child("包含工具过程")))
            .child(crate::text_reading::region("export-preview".into(),"对话快照预览，可用方向键或翻页键滚动".into(),&self.scroll,height,cx).text_sm().child(preview))
            .when(truncated,|view|view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child("预览仅显示前 10,000 字；复制和保存包含完整快照。")))
            .when_some(self.notice.clone(),|view,notice|view.child(div().text_sm().child(notice)))
            .when_some(self.error.clone(),|view,error|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)))
            .child(h_flex().flex_wrap().gap_2()
                .child(Button::new("export-copy").label("复制 Markdown").accessibility_label("复制完整对话快照").on_click(cx.listener(|this,_,_,cx|{
                    cx.write_to_clipboard(ClipboardItem::new_string(this.text.clone()));this.notice=Some("已复制完整快照".into());cx.notify();})))
                .child(Button::new("export-save").primary().label(if self.saving{"正在保存…"}else{"保存 Markdown…"}).disabled(self.saving).on_click(cx.listener(|this,_,_,cx|this.save(cx)))))
    }
}
