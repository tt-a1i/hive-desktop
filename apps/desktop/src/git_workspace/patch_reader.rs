use crate::{language::tr, palette::Color, text_reading};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub(crate) struct PatchReader {
    text: text_reading::TextPages,
    pub scroll: ScrollHandle,
    copy_button: bool,
}

impl PatchReader {
    pub fn new(patch: SharedString) -> Self {
        Self {
            text: text_reading::TextPages::new(patch),
            scroll: ScrollHandle::new(),
            copy_button: true,
        }
    }

    pub fn without_copy_button(mut self) -> Self {
        self.copy_button = false;
        self
    }

    fn show(&mut self, page: usize) {
        if self.text.show(page) {
            self.scroll = ScrollHandle::new();
        }
    }

    pub fn view<T: 'static>(
        &self,
        id: &'static str,
        region: Stateful<Div>,
        reader: fn(&mut T) -> Option<&mut Self>,
        cx: &Context<T>,
    ) -> impl IntoElement {
        self.view_with_wrap(id, region, false, reader, cx)
    }

    pub fn view_with_wrap<T: 'static>(
        &self,
        id: &'static str,
        region: Stateful<Div>,
        nowrap: bool,
        reader: fn(&mut T) -> Option<&mut Self>,
        cx: &Context<T>,
    ) -> impl IntoElement {
        let page = self.text.page;
        let count = self.text.count();
        let group_id = ElementId::from(id);
        let text_el = text_reading::selectable_text(
            format!("{id}-{page}"),
            self.text.visible.clone(),
        );
        let body: AnyElement = if nowrap {
            h_flex()
                .w_full()
                .min_w_0()
                .items_start()
                .child(div().flex_none().whitespace_nowrap().child(text_el))
                .into_any_element()
        } else {
            text_el.into_any_element()
        };
        v_flex()
            .min_w_0()
            .gap_1()
            .child(region.child(body))
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
                        .when(self.copy_button, |controls| controls.child(
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
                        )),
                )
            })
    }
}
