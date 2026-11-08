use super::AttachmentRef;
use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    *,
};
impl HiveApp {
    pub(crate) fn draft_file_attachment(
        &self,
        item: AttachmentRef,
        busy: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let key = self.draft_target.clone();
        let id = item.id.clone();
        h_flex()
            .w(px(280.))
            .max_w_full()
            .min_w_0()
            .gap_1()
            .p_1()
            .rounded_lg()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                self.file_attachment_link("draft", item.clone(), cx)
                    .flex_1(),
            )
            .child(
                Button::new(SharedString::from(format!("remove-image-{}", item.id)))
                    .ghost()
                    .compact()
                    .icon(Glyph::X)
                    .accessibility_label(format!("移除附件 {}", item.name))
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(items) = key
                            .as_ref()
                            .and_then(|key| this.attachments.drafts.get_mut(key))
                        {
                            items.retain(|item| item.id != id);
                        }
                        this.attachments.prune();
                        this.schedule_draft_save(cx);
                        cx.notify();
                    })),
            )
    }

    pub(crate) fn file_attachment_link(
        &self,
        owner: &str,
        item: AttachmentRef,
        cx: &Context<Self>,
    ) -> Button {
        let label = format!("{} · {}", item.name, item.size_label());
        Button::new(SharedString::from(format!("file-{owner}-{}", item.id)))
            .ghost()
            .compact()
            .min_w_0()
            .icon(Glyph::File)
            .max_w_full()
            .justify_start()
            .child(div().min_w_0().truncate().child(label))
            .tooltip(format!("预览附件副本：{}", item.name))
            .accessibility_label(format!("预览附件副本 {}", item.name))
            .disabled(!self.connected)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_attachment_preview(item.clone(), window, cx);
            }))
    }
}
