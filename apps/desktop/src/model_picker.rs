use crate::{app::HiveApp, language::tr, palette::Color};
use gpui_kit::{component::{button::{Button, ButtonVariants}, input::Input, popover::Popover, scroll::ScrollableElement, *}, prelude::FluentBuilder, *};

impl HiveApp {
    pub(crate) fn model_picker(&self, window: &Window, running: bool, cx: &Context<Self>) -> AnyElement {
        let current = self.model.as_ref();
        let selected = current.and_then(|current| self.models.iter().find(|model| model.provider == current.provider && model.id == current.id));
        let name = selected.filter(|model| !model.name.trim().is_empty()).map(|model| model.name.clone())
            .or_else(|| current.map(|model| model.id.clone())).unwrap_or_else(|| tr("选择模型").into());
        let label = self.reasoning_label().map(|level| format!("{name} · {level}")).unwrap_or(name);
        let identity = current.map(|model| format!("{} / {}", model.provider, model.id)).unwrap_or_else(|| tr("选择会话模型").into());
        let unavailable = !self.connected || !self.conversation_ready || running || self.loading || self.closing;
        let trigger = Button::new("choose-model").accessibility_label(tr("选择会话模型与推理强度"))
            .ghost().compact().min_w_0().max_w(px(if window.viewport_size().width < px(900.) {260.} else {340.}))
            .label(label.clone()).dropdown_caret(true).tooltip(format!("{label}\n{identity}\n{}",tr("只影响当前会话，不修改 Agent 全局默认"))).disabled(unavailable);
        if unavailable { return trigger.into_any_element(); }
        Popover::new("session-model-picker").anchor(Anchor::BottomRight)
            .w(px(600.).min(window.viewport_size().width - px(32.)))
            .open(self.choosing_model).track_focus(&self.model_search.read(cx).focus_handle(cx)).trigger(trigger)
            .on_open_change(cx.listener(|this, open, window, cx| {
                this.choosing_model = *open;
                if *open {
                    this.model_search.update(cx, |input, cx| input.set_value("", window, cx));
                    this.reasoning_picker.model_scroll.set_offset(point(px(0.),px(0.)));
                    this.reasoning_picker.scroll.set_offset(point(px(0.),px(0.)));
                    this.load_reasoning(None, cx);
                }
                cx.notify();
            }))
            .when(self.choosing_model, |popover| {
                let query = self.model_search.read(cx).value().trim().to_lowercase();
                let mut matching = self.models.iter().filter(|model| model.available && (query.is_empty()
                    || format!("{} {} {}",model.provider,model.id,model.name).to_lowercase().contains(&query))).collect::<Vec<_>>();
                matching.sort_by_key(|model| !self.model.as_ref().is_some_and(|selected| selected.provider == model.provider && selected.id == model.id));
                let empty = matching.is_empty();
                let model_scroll = &self.reasoning_picker.model_scroll;
                let reasoning_scroll = &self.reasoning_picker.scroll;
                popover.child(h_flex().min_w_0().items_stretch().gap_3()
                    .h((window.viewport_size().height - px(140.)).min(px(320.)).max(px(100.)))
                    .child(v_flex().flex_1().min_w_0().min_h_0().gap_2()
                        .child(Input::new(&self.model_search).aria_label(tr("搜索会话模型")))
                        .child(crate::focus_scroll::Viewport::new("model-picker-viewport", model_scroll,
                            v_flex().id("model-list").flex_1().min_w_0().min_h_0().p_1().overflow_y_scroll().track_scroll(model_scroll).vertical_scrollbar(model_scroll)
                                .when(empty, |list| list.child(crate::text_reading::selectable_text("model-picker-empty", tr("没有匹配的可用模型")).p_3().text_sm().text_color(Color::Muted.get(cx))))
                                .children(matching.into_iter().enumerate().map(|(i, model)| {
                                    let selected = self.model.as_ref().is_some_and(|m| m.id == model.id && m.provider == model.provider);
                                    let model = model.clone();
                                    crate::focus_scroll::Reveal::new(("model-option-reveal",i), model_scroll,
                                        Button::new(("model", i)).ghost().compact().justify_start().flex_shrink_0().w_full().min_w_0().h_auto().py_2()
                                            .disabled(self.reasoning_picker.loading)
                                            .accessibility_label(format!("{}，{} / {}",model.name,model.provider,model.id))
                                            .tooltip(format!("{}\n{} / {}",model.name,model.provider,model.id))
                                            .child(v_flex().flex_1().min_w_0().items_start().gap_1()
                                                .child(div().w_full().truncate().text_sm().child(model.name.clone()))
                                                .child(div().w_full().truncate().text_xs().text_color(Color::Muted.get(cx)).child(format!("{} / {}",model.provider,model.id))))
                                            .selected(selected).on_click(cx.listener(move |this, _, _, cx| this.set_model(model.clone(),cx))))
                                })))))
                    .child(v_flex().flex_1().min_w_0().min_h_0().pl_3().border_l_1().border_color(Color::Divider.get(cx))
                        .child(crate::focus_scroll::Viewport::new("reasoning-picker-viewport", reasoning_scroll,
                            v_flex().id("reasoning-settings-list").flex_1().min_h_0().min_w_0().p_1().overflow_y_scroll().track_scroll(reasoning_scroll).vertical_scrollbar(reasoning_scroll)
                                .child(self.render_reasoning_settings(running,cx))))))
            }).into_any_element()
    }
}
