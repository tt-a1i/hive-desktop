use crate::language::tr;
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
        let size = item.size_label();
        let name = item.name.clone();
        h_flex()
            .h(px(30.))
            .max_w(px(280.))
            .min_w_0()
            .items_center()
            .gap_0p5()
            .pl_1()
            .pr_1()
            .rounded_lg()
            .bg(Color::Inset.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                Button::new(SharedString::from(format!("file-draft-{}", item.id)))
                    .custom(crate::palette::quiet_icon_variant(cx))
                    .compact()
                    .h(px(24.))
                    .px_1p5()
                    .rounded_md()
                    .min_w_0()
                    .flex_1()
                    .justify_start()
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(Glyph::FileText)
                                    .size(px(13.))
                                    .text_color(Color::Muted.get(cx)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(11.))
                                    .text_color(Color::Muted.get(cx))
                                    .child(size),
                            ),
                    )
                    .tooltip(format!("{}: {}", tr("预览附件副本"), name))
                    .accessibility_label(format!("{}: {}", tr("预览附件副本"), name))
                    .disabled(!self.connected)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_attachment_preview(item.clone(), window, cx);
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("remove-image-{}", id)))
                    .custom(crate::palette::subtle_icon_variant(cx))
                    .compact()
                    .h(px(22.))
                    .w(px(22.))
                    .rounded_md()
                    .icon(Glyph::X)
                    .accessibility_label(format!("{}: {}", tr("移除附件"), name))
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
        let name = item.name.clone();
        let size = item.size_label();
        Button::new(SharedString::from(format!("file-{owner}-{}", item.id)))
            .custom(crate::palette::quiet_icon_variant(cx))
            .compact()
            .h(px(28.))
            .px_2()
            .rounded_lg()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Inset.get(cx))
            .min_w_0()
            .max_w_full()
            .justify_start()
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Icon::new(Glyph::FileText)
                            .size(px(13.))
                            .text_color(Color::Muted.get(cx)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(11.))
                            .text_color(Color::Muted.get(cx))
                            .child(size),
                    ),
            )
            .tooltip(format!("{}: {}", tr("预览附件副本"), name))
            .accessibility_label(format!("{}: {}", tr("预览附件副本"), name))
            .disabled(!self.connected)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_attachment_preview(item.clone(), window, cx);
            }))
    }
}
