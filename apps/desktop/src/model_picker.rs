use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        popover::Popover,
        *,
    },
    prelude::FluentBuilder,
    *,
};

impl HiveApp {
    pub(crate) fn model_picker(
        &self,
        window: &Window,
        running: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let label = self
            .model
            .as_ref()
            .map(|model| format!("{} / {} ▾", model.provider, model.id))
            .unwrap_or_else(|| "选择模型 ▾".into());
        let unavailable = !self.connected || !self.conversation_ready || running || self.loading;
        let trigger = Button::new("choose-model")
            .accessibility_label("选择会话模型")
            .ghost()
            .compact()
            .min_w_0()
            .label(label)
            .disabled(unavailable);
        if unavailable {
            return trigger.into_any_element();
        }
        Popover::new("session-model-picker")
            .anchor(Anchor::BottomRight)
            .w(px(360.).min(window.viewport_size().width - px(32.)))
            .open(self.choosing_model)
            .track_focus(&self.model_search.read(cx).focus_handle(cx))
            .trigger(trigger)
            .on_open_change(cx.listener(|this, open, window, cx| {
                this.choosing_model = *open;
                if *open {
                    this.model_search
                        .update(cx, |input, cx| input.set_value("", window, cx));
                }
                cx.notify();
            }))
            .when(self.choosing_model, |popover| {
                let query = self.model_search.read(cx).value().trim().to_lowercase();
                let mut matching = self
                    .models
                    .iter()
                    .filter(|model| {
                        model.available
                            && (query.is_empty()
                                || format!("{} {} {}", model.provider, model.id, model.name)
                                    .to_lowercase()
                                    .contains(&query))
                    })
                    .peekable();
                let empty = matching.peek().is_none();
                popover.child(
                    v_flex()
                        .min_w_0()
                        .gap_2()
                        .child(Input::new(&self.model_search).aria_label("搜索会话模型"))
                        .child(
                            v_flex()
                                .id("model-list")
                                .max_h(px(220.))
                                .min_h_0()
                                .overflow_y_scroll()
                                .when(empty, |list| {
                                    list.child(
                                        div()
                                            .p_3()
                                            .text_sm()
                                            .text_color(Color::Muted.get(cx))
                                            .child("没有匹配的可用模型"),
                                    )
                                })
                                .children(matching.enumerate().map(|(i, model)| {
                                    let selected = self.model.as_ref().is_some_and(|m| {
                                        m.id == model.id && m.provider == model.provider
                                    });
                                    let model = model.clone();
                                    Button::new(("model", i))
                                        .ghost()
                                        .compact()
                                        .justify_start()
                                        .flex_shrink_0()
                                        .w_full()
                                        .label(format!("{} / {}", model.provider, model.name))
                                        .tooltip(model.id.clone())
                                        .selected(selected)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.set_model(model.clone(), cx)
                                        }))
                                })),
                        ),
                )
            })
            .into_any_element()
    }
}
