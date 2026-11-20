use crate::{app::HiveApp, language::tr, palette::Color, protocol::Model};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        Icon,
        button::{Button, ButtonVariants},
        input::{Escape, Input, MoveDown, MoveUp},
        popover::Popover,
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};

#[derive(Default)]
pub(crate) struct ModelSearchNavigation {
    active: Option<(String, String)>,
    enter: Option<(String, String)>,
}

impl ModelSearchNavigation {
    pub(crate) fn cancel_enter(&mut self) {
        self.enter = None;
    }

    fn active_index(&self, models: &[&Model]) -> usize {
        self.active
            .as_ref()
            .and_then(|(provider, id)| {
                models
                    .iter()
                    .position(|model| model.provider == *provider && model.id == *id)
            })
            .unwrap_or(0)
    }
}

fn model_key(model: &Model) -> (String, String) {
    (model.provider.clone(), model.id.clone())
}

impl HiveApp {
    fn matching_picker_models(&self, cx: &App) -> Vec<&Model> {
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
            .collect::<Vec<_>>();
        matching.sort_by_key(|model| {
            !self.model.as_ref().is_some_and(|selected| {
                selected.provider == model.provider && selected.id == model.id
            })
        });
        matching
    }

    pub(crate) fn reset_model_picker_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_picker_search_changed(cx);
        self.model_search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.reasoning_picker.scroll.set_offset(point(px(0.), px(0.)));
    }

    pub(crate) fn model_picker_search_changed(&mut self, cx: &mut Context<Self>) {
        self.reasoning_picker.model_search_navigation = Default::default();
        self.reasoning_picker.model_scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn model_search_is_composing(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.model_search.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        })
    }

    fn move_model_search(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_search_is_composing(window, cx) {
            return;
        }
        cx.stop_propagation();
        let matching = self.matching_picker_models(cx);
        if matching.is_empty() {
            return;
        }
        let current = self.reasoning_picker.model_search_navigation.active_index(&matching);
        let index = current.saturating_add_signed(delta).min(matching.len() - 1);
        let key = model_key(matching[index]);
        self.reasoning_picker.model_search_navigation.active = Some(key);
        self.reasoning_picker.model_search_navigation.cancel_enter();
        self.reasoning_picker.model_scroll.scroll_to_item(index);
        cx.notify();
    }

    fn model_search_enter_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "enter"
            || event.keystroke.modifiers.modified()
            || self.model_search_is_composing(window, cx)
        {
            return;
        }
        cx.stop_propagation();
        if event.is_held
            || self.reasoning_picker.loading
            || self.reasoning_picker.model_search_navigation.enter.is_some()
        {
            return;
        }
        // Keep focus in the search until Enter is released. Otherwise a held key
        // can land in the composer as soon as the popover closes and send its draft.
        let matching = self.matching_picker_models(cx);
        let index = self.reasoning_picker.model_search_navigation.active_index(&matching);
        self.reasoning_picker.model_search_navigation.enter =
            matching.get(index).map(|model| model_key(model));
    }

    fn model_search_enter_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "enter" {
            return;
        }
        let Some((provider, id)) = self.reasoning_picker.model_search_navigation.enter.take() else {
            return;
        };
        cx.stop_propagation();
        if event.keystroke.modifiers.modified()
            || !self.choosing_model
            || self.reasoning_picker.loading
            || self.model_search_is_composing(window, cx)
        {
            return;
        }
        if let Some(model) = self
            .matching_picker_models(cx)
            .into_iter()
            .find(|model| model.provider == provider && model.id == id)
        {
            self.set_model(model.clone(), cx);
        }
    }

    fn model_search_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.reasoning_picker.model_search_navigation.cancel_enter();
        if self.model_search_is_composing(window, cx) {
            self.model_search
                .update(cx, |input, cx| input.unmark_text(window, cx));
            return;
        }
        self.choosing_model = false;
        cx.notify();
    }

    pub(crate) fn model_picker(
        &self,
        window: &Window,
        running: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let current = self.model.as_ref();
        let selected = current.and_then(|current| {
            self.models
                .iter()
                .find(|model| model.provider == current.provider && model.id == current.id)
        });
        let raw_name = selected
            .filter(|model| !model.name.trim().is_empty())
            .map(|model| model.name.clone())
            .or_else(|| current.and_then(|model| model.name.as_ref().filter(|name| !name.trim().is_empty()).cloned()))
            .or_else(|| current.map(|model| model.id.clone()))
            .unwrap_or_else(|| tr("选择模型").into());
        let name = ["Codex", "Claude", "Pi", "外部 Pi", tr("外部 Pi")]
            .into_iter()
            .find_map(|tag| raw_name.strip_suffix(&format!(" · {tag}")))
            .map(str::to_owned)
            .unwrap_or(raw_name);
        let effort = self.reasoning_label().filter(|level| !level.trim().is_empty());
        let label = effort
            .as_ref()
            .map(|level| format!("{name} · {level}"))
            .unwrap_or_else(|| name.clone());
        let identity = current
            .map(|model| format!("{} / {}", model.provider, model.id))
            .unwrap_or_else(|| tr("选择会话模型").into());
        let unavailable =
            !self.connected || !self.conversation_ready || running || self.loading || self.closing;
        let backend_icon = self
            .session
            .as_ref()
            .map(|session| crate::composer_controls::backend_glyph(session.backend));
        let profile_note = self
            .session
            .as_ref()
            .and_then(|s| s.profile.as_ref())
            .map(|profile| format!("\n{}", self.profile_label(profile)))
            .unwrap_or_default();
        let picker_label = tr("选择会话模型与推理强度");
        let accessibility = if self.session.as_ref().is_some_and(|session| {
            session.inherited_draft_model || session.inherited_draft_reasoning.is_some()
        }) {
            format!("{picker_label} · {label}")
        } else {
            picker_label.to_owned()
        };
        let trigger = Button::new("choose-model")
            .accessibility_label(accessibility)
            .custom(crate::palette::quiet_icon_variant(cx))
            .compact()
            .h(px(26.))
            .px_1p5()
            .rounded_lg()
            .selected(self.choosing_model)
            .min_w_0()
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_1p5()
                    .when_some(backend_icon, |row, glyph| {
                        row.child(
                            Icon::new(glyph)
                                .size(px(14.))
                                .text_color(Color::Muted.get(cx))
                                .flex_shrink_0(),
                        )
                    })
                    .child(
                        div()
                            .min_w_0()
                            .max_w(px(180.))
                            .truncate()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Text.get(cx))
                            .child(name.clone()),
                    )
                    .when_some(effort, |row, level| {
                        row.child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(12.))
                                .text_color(Color::Muted.get(cx))
                                .child(level),
                        )
                    })
                    .child(
                        Icon::new(Glyph::ChevronDown)
                            .size(px(11.))
                            .flex_shrink_0()
                            .text_color(Color::Muted.get(cx)),
                    ),
            )
            .tooltip(format!(
                "{label}\n{identity}{profile_note}\n{}\n{}",
                tr("只影响当前会话，不修改 Agent 全局默认"),
                crate::keybindings::hint(crate::keybindings::Target::ChooseModel, "快捷键", cx),
            ))
            .disabled(unavailable);
        if unavailable {
            return trigger.into_any_element();
        }
        let mut trigger_layout = div().min_w_0();
        let trigger_style = trigger_layout.style().clone();
        Popover::new("session-model-picker").anchor(Anchor::BottomRight)
            .trigger_style(trigger_style)
            .p_0()
            .rounded_xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Card.get(cx))
            .shadow_lg()
            .overflow_hidden()
            .w(px(620.).min(window.viewport_size().width - px(32.)))
            .open(self.choosing_model).track_focus(&self.model_search.read(cx).focus_handle(cx)).trigger(trigger)
            .on_open_change(cx.listener(|this, open, window, cx| {
                if *open && this.connect_saved_session(crate::sessions::NativeSessionPanel::Models, cx) {
                    return;
                }
                this.choosing_model = *open;
                if *open {
                    this.reset_model_picker_search(window, cx);
                    this.load_reasoning(None, cx);
                    this.load_mode(None, cx);
                    this.load_permission_mode(None, cx);
                }
                cx.notify();
            }))
            .when(self.choosing_model, |popover| {
                let matching = self.matching_picker_models(cx);
                let active = self.reasoning_picker.model_search_navigation.active_index(&matching);
                let search_focused = self.model_search.read(cx).focus_handle(cx).is_focused(window);
                let empty = matching.is_empty();
                let model_scroll = &self.reasoning_picker.model_scroll;
                let reasoning_scroll = &self.reasoning_picker.scroll;
                popover.child(
                    h_flex()
                        .min_w_0()
                        .items_stretch()
                        .h((window.viewport_size().height - px(140.)).min(px(340.)).max(px(120.)))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .px_2p5()
                                        .py_1p5()
                                        .border_b_1()
                                        .border_color(Color::Divider.get(cx))
                                        .child(
                                            div().flex_1().min_w_0()
                                                .capture_action(cx.listener(|this, _: &MoveUp, window, cx| this.move_model_search(-1, window, cx)))
                                                .capture_action(cx.listener(|this, _: &MoveDown, window, cx| this.move_model_search(1, window, cx)))
                                                .capture_action(cx.listener(Self::model_search_escape))
                                                .capture_key_down(cx.listener(Self::model_search_enter_down))
                                                .capture_key_up(cx.listener(Self::model_search_enter_up))
                                                .child(
                                                Input::new(&self.model_search)
                                                    .appearance(false)
                                                    .bordered(false)
                                                    .focus_bordered(false)
                                                    .cleanable(true)
                                                    .prefix(
                                                        Icon::new(Glyph::Search)
                                                            .size(px(14.))
                                                            .text_color(Color::Muted.get(cx)),
                                                    )
                                                    .aria_label(tr("搜索会话模型")),
                                            ),
                                        ),
                                )
                                .child(
                                    div()
                                        .px_3()
                                        .pt_2()
                                        .pb_1()
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("选择会话模型")),
                                )
                                .child(crate::focus_scroll::Viewport::new(
                                    "model-picker-viewport",
                                    model_scroll,
                                    v_flex()
                                        .id("model-list")
                                        .flex_1()
                                        .min_w_0()
                                        .min_h_0()
                                        .px_1p5()
                                        .pb_1p5()
                                        .gap_0p5()
                                        .overflow_y_scroll()
                                        .track_scroll(model_scroll)
                                        .vertical_scrollbar(model_scroll)
                                        .when(empty, |list| {
                                            list.child(
                                                div()
                                                    .p_4()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        crate::text_reading::selectable_text(
                                                            "model-picker-empty",
                                                            tr("没有匹配的可用模型"),
                                                        )
                                                        .text_sm()
                                                        .text_color(Color::Muted.get(cx)),
                                                    ),
                                            )
                                        })
                                        .children(matching.into_iter().enumerate().map(|(i, model)| {
                                            let selected = self.model.as_ref().is_some_and(|m| m.id == model.id && m.provider == model.provider);
                                            let model = model.clone();
                                            let separator = if crate::language::english() { ", " } else { "，" };
                                            let mut label = format!("{}{separator}{} / {}", model.name, model.provider, model.id);
                                            if selected {
                                                label.push_str(separator);
                                                label.push_str(tr("当前生效"));
                                            }
                                            if i == active && search_focused && !self.reasoning_picker.loading {
                                                label.push_str(separator);
                                                label.push_str(tr("按 Enter 选择"));
                                            }
                                            crate::focus_scroll::Reveal::new(
                                                ("model-option-reveal", i),
                                                model_scroll,
                                                Button::new(("model", i))
                                                    .ghost()
                                                    .compact()
                                                    .justify_start()
                                                    .flex_shrink_0()
                                                    .w_full()
                                                    .min_w_0()
                                                    .h_auto()
                                                    .px_2p5()
                                                    .py_1p5()
                                                    .rounded_lg()
                                                    .disabled(self.reasoning_picker.loading)
                                                    .accessibility_label(label)
                                                    .tooltip(format!("{}\n{} / {}", model.name, model.provider, model.id))
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .min_w_0()
                                                            .items_center()
                                                            .justify_between()
                                                            .gap_2()
                                                            .child(
                                                                v_flex()
                                                                    .flex_1()
                                                                    .min_w_0()
                                                                    .items_start()
                                                                    .gap_0p5()
                                                                    .child(
                                                                        div()
                                                                            .w_full()
                                                                            .truncate()
                                                                            .text_size(px(13.))
                                                                            .font_weight(if selected {
                                                                                FontWeight::SEMIBOLD
                                                                            } else {
                                                                                FontWeight::MEDIUM
                                                                            })
                                                                            .text_color(Color::Text.get(cx))
                                                                            .child(model.name.clone()),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .w_full()
                                                                            .truncate()
                                                                            .text_size(px(11.5))
                                                                            .text_color(Color::Muted.get(cx))
                                                                            .child(format!("{} / {}", model.provider, model.id)),
                                                                    ),
                                                            )
                                                            .when(selected, |row| {
                                                                row.child(
                                                                    Icon::new(Glyph::Check)
                                                                        .size(px(14.))
                                                                        .text_color(Color::Accent.get(cx))
                                                                        .flex_shrink_0(),
                                                                )
                                                            }),
                                                    )
                                                    .selected(i == active)
                                                    .on_click(cx.listener(move |this, _, _, cx| this.set_model(model.clone(), cx))),
                                            )
                                        })),
                                )),
                        )
                        .child(
                            v_flex()
                                .w(px(230.))
                                .flex_shrink_0()
                                .min_w_0()
                                .min_h_0()
                                .bg(Color::Inset.get(cx).opacity(0.35))
                                .border_l_1()
                                .border_color(Color::Divider.get(cx))
                                .child(crate::focus_scroll::Viewport::new(
                                    "reasoning-picker-viewport",
                                    reasoning_scroll,
                                    v_flex()
                                        .id("reasoning-settings-list")
                                        .flex_1()
                                        .min_h_0()
                                        .min_w_0()
                                        .p_2p5()
                                        .overflow_y_scroll()
                                        .track_scroll(reasoning_scroll)
                                        .vertical_scrollbar(reasoning_scroll)
                                        .child(self.render_reasoning_settings(running, cx))
                                        .child(self.render_mode_settings(running, cx)),
                                )),
                        ),
                )
            })
            .into_any_element()
    }
}
