use super::GitWorkspace;
use crate::{language::tr, palette::Color, text_reading};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub(super) struct PatchReader {
    text: text_reading::TextPages,
    pub scroll: ScrollHandle,
}

impl PatchReader {
    pub fn new(patch: SharedString) -> Self {
        Self {
            text: text_reading::TextPages::new(patch),
            scroll: ScrollHandle::new(),
        }
    }

    fn show(&mut self, page: usize) {
        if self.text.show(page) {
            self.scroll = ScrollHandle::new();
        }
    }

    pub fn view(
        &self,
        id: &'static str,
        region: Stateful<Div>,
        reader: fn(&mut GitWorkspace) -> Option<&mut Self>,
        cx: &Context<GitWorkspace>,
    ) -> impl IntoElement {
        let page = self.text.page;
        let count = self.text.count();
        let group_id = ElementId::from(id);
        v_flex()
            .min_w_0()
            .gap_1()
            .child(region.child(text_reading::selectable_text(
                format!("{id}-{page}"),
                self.text.visible.clone(),
            )))
            .when(count > 1, |view| {
                let label = format!("{} {} / {count}", tr("补丁页"), page + 1);
                view.child(
                    h_flex()
                        .flex_wrap()
                        .items_center()
                        .gap_1p5()
                        .child(
                            Button::new((group_id.clone(), "previous"))
                                .ghost()
                                .xsmall()
                                .label(tr("上一页补丁"))
                                .disabled(page == 0)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(reader) = reader(this).filter(|r| r.text.page > 0) {
                                        reader.show(reader.text.page - 1);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            div()
                                .id((group_id.clone(), "position"))
                                .role(Role::Status)
                                .aria_label(label.clone())
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(label),
                        )
                        .child(
                            Button::new((group_id.clone(), "next"))
                                .ghost()
                                .xsmall()
                                .label(tr("下一页补丁"))
                                .disabled(page + 1 == count)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(reader) = reader(this) {
                                        reader.show(reader.text.page + 1);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            Button::new((group_id, "copy"))
                                .ghost()
                                .xsmall()
                                .label(tr("复制完整补丁"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(reader) = reader(this) {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            reader.text.full.to_string(),
                                        ));
                                    }
                                })),
                        ),
                )
            })
    }
}
