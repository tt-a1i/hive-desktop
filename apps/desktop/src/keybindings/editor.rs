use super::*;
use crate::{
    focus_scroll::{Reveal, Viewport},
    palette::Color,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};

struct Recording {
    target: Target,
    window: WindowId,
    candidate: Option<Keystroke>,
}
pub(crate) struct Editor {
    focus: [FocusHandle; TARGETS.len()],
    search: Entity<InputState>,
    highlighted: Option<(Target, u64)>,
    highlight_gen: u64,
    recording: Option<Recording>,
    interceptor: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    error: Option<&'static str>,
    scroll: ScrollHandle,
}
impl Editor {
    pub(crate) fn localize_placeholders(&self, window: &mut Window, cx: &mut App) {
        self.search
            .update(cx, |input, cx| input.set_placeholder(tr("搜索快捷键或组合键…"), window, cx));
    }

    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus: [FocusHandle; TARGETS.len()] = std::array::from_fn(|_| cx.focus_handle());
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索快捷键或组合键…")));
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.highlighted = None;
                cx.notify();
            }
        }));
        for target in TARGETS {
            subscriptions.push(
                cx.on_blur(&focus[target.index()], window, move |this, _, cx| {
                    if this
                        .recording
                        .as_ref()
                        .is_some_and(|recording| recording.target == target)
                    {
                        this.stop(cx);
                    }
                }),
            );
        }
        subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.stop(cx);
            }
        }));
        Self {
            focus,
            search,
            highlighted: None,
            highlight_gen: 0,
            recording: None,
            interceptor: None,
            _subscriptions: subscriptions,
            error: None,
            scroll: ScrollHandle::new(),
        }
    }
    pub(crate) fn focus_target(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        self.stop(cx);
        self.search.update(cx, |input, cx| input.set_value("", window, cx));
        self.highlight_gen = self.highlight_gen.wrapping_add(1);
        let generation = self.highlight_gen;
        self.highlighted = Some((target, generation));
        self.focus[target.index()].focus(window, cx);
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            smol::Timer::after(std::time::Duration::from_millis(1600)).await;
            let _ = weak.update(cx, |this, cx| {
                if this
                    .highlighted
                    .is_some_and(|(_, current)| current == generation)
                {
                    this.highlighted = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
    fn stop(&mut self, cx: &mut Context<Self>) {
        self.recording = None;
        self.interceptor = None;
        self.error = None;
        cx.notify();
    }
    fn start(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        self.stop(cx);
        self.error = None;
        self.recording = Some(Recording {
            target,
            window: window.window_handle().window_id(),
            candidate: None,
        });
        self.focus[target.index()].focus(window, cx);
        let weak = cx.weak_entity();
        self.interceptor = Some(cx.intercept_keystrokes(move |event, window, cx| {
            let _ = weak.update(cx, |this, cx| this.intercept(event, window, cx));
        }));
        cx.notify();
    }
    fn intercept(&mut self, event: &KeystrokeEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(recording) = &self.recording else {
            return;
        };
        if recording.window != window.window_handle().window_id()
            || !self.focus[recording.target.index()].is_focused(window)
            || !window.is_window_active()
        {
            self.stop(cx);
            return;
        }
        let key = &event.keystroke;
        if key.key == "escape" && key.modifiers.number_of_modifiers() == 0 {
            cx.stop_propagation();
            self.stop(cx);
            return;
        }
        if key.key == "tab" && key.modifiers.number_of_modifiers() == 0 {
            self.stop(cx);
            return;
        }
        if matches!(
            key.key.as_str(),
            "shift" | "control" | "ctrl" | "alt" | "cmd" | "super" | "win" | "fn"
        ) {
            return;
        }
        // Intercept before the keymap even when the input is not recordable.
        cx.stop_propagation();
        if matches!(event.input_preference, InputPreference::CharacterInput)
            || key
                .key_char
                .as_ref()
                .is_some_and(|character| !character.is_ascii())
        {
            return;
        }
        if recording.candidate.is_some() {
            return;
        } // key-repeat latch
        let target = recording.target;
        let os = cx.platform_info().os;
        let validation = validate_chord(&key.unparse(os), os).and_then(|candidate| {
            let mut draft = cx.global::<Preferences>().draft.clone();
            draft.set(target, Some(candidate.unparse(os)));
            let normalized = draft.normalized(os)?;
            check_registered(&normalized, cx)?;
            Ok(candidate)
        });
        match validation {
            Ok(candidate) => {
                self.recording.as_mut().unwrap().candidate = Some(candidate);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }
    fn finish_candidate(&mut self, cx: &mut Context<Self>) {
        let Some(recording) = &self.recording else {
            return;
        };
        let Some(candidate) = &recording.candidate else {
            return;
        };
        let target = recording.target;
        let value = candidate.unparse(cx.platform_info().os);
        cx.global_mut::<Preferences>()
            .draft
            .set(target, Some(value));
        self.stop(cx);
        cx.refresh_windows();
    }
    fn released(&mut self, event: &KeyUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(recording) = &self.recording else {
            return;
        };
        if recording.window != window.window_handle().window_id()
            || !self.focus[recording.target.index()].is_focused(window)
            || !window.is_window_active()
        {
            self.stop(cx);
            return;
        }
        if recording
            .candidate
            .as_ref()
            .is_some_and(|candidate| event.keystroke.key == candidate.key)
        {
            self.finish_candidate(cx);
        }
    }
    fn modifiers_released(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(recording) = &self.recording else {
            return;
        };
        if recording.window != window.window_handle().window_id()
            || !self.focus[recording.target.index()].is_focused(window)
            || !window.is_window_active()
        {
            self.stop(cx);
            return;
        }
        if recording
            .candidate
            .as_ref()
            .is_some_and(|candidate| primary_released(candidate, event.modifiers))
        {
            self.finish_candidate(cx);
        }
    }
}
impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.global::<Preferences>();
        let active = state.active.clone();
        let saved = state.saved.clone();
        let draft = state.draft.clone();
        let os = cx.platform_info().os;
        let defaults = Bindings::defaults(os);
        let ready = state.root.is_some();
        let validation = draft
            .normalized(os)
            .and_then(|normalized| check_registered(&normalized, cx))
            .err();
        let message = state.notice.as_ref().map(|notice| match notice {
            Notice::Read(error) => (
                true,
                format!(
                    "{}\n{}",
                    tr("读取快捷键失败，当前快捷键保持不变。请修复或移走原文件后重新读取；不会覆盖原文件。"),
                    explain_error(error)
                ),
            ),
            Notice::Save(error) => (
                true,
                format!(
                    "{}\n{}",
                    tr("保存快捷键失败，原文件和当前快捷键均未改变。"),
                    explain_error(error)
                ),
            ),
            Notice::Saved => (false, tr("快捷键已保存并立即生效。").to_owned()),
        });
        let query = self.search.read(cx).value().trim().to_lowercase();
        let matches_target = |target: Target| {
            if query.is_empty() {
                return true;
            }
            let hay = format!(
                "{} {} {} {} {}",
                target.group().label(),
                target.label(),
                target.description(),
                display(draft.get(target), os),
                draft.get(target).as_deref().unwrap_or("")
            )
            .to_lowercase();
            hay.contains(&query)
        };
        let reveal = |id: SharedString, button: Button| Reveal::new(id, &self.scroll, button);
        let highlighted_target = self.highlighted.map(|(target, _)| target);
        Viewport::new(
            "keybindings-settings-viewport",
            &self.scroll,
            v_flex()
                .id("keybindings-settings-content")
                .flex_1()
                .min_h_0()
                .min_w_0()
                .gap_3()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .vertical_scrollbar(&self.scroll)
                .on_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                    this.released(event, window, cx)
                }))
                .on_modifiers_changed(cx.listener(
                    |this, event: &ModifiersChangedEvent, window, cx| {
                        this.modifiers_released(event, window, cx)
                    },
                ))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_sm()
                        .text_color(Color::Muted.get(cx))
                        .child(tr(
                            "录制单个 Command 或 Ctrl 组合键。保存后立即生效，无需重启应用；文字编辑与退出快捷键保持系统默认。",
                        )),
                )
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(Input::new(&self.search).cleanable(true)))
                        .when(!query.is_empty(), |row| {
                            row.child(
                                Button::new("clear-keybinding-search")
                                    .ghost()
                                    .small()
                                    .label(tr("清除"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.search.update(cx, |input, cx| {
                                            input.set_value("", window, cx)
                                        });
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                .when(!ready, |view| {
                    view.child(
                        crate::settings::settings_notice_box(
                            "keybindings-waiting-datadir",
                            tr("等待后台提供本地数据目录；默认快捷键仍可使用。"),
                            false,
                            cx,
                        ),
                    )
                })
                .when_some(message, |view, (is_error, text)| {
                    view.child(crate::settings::settings_notice_box(
                        "keybindings-persistence-notice",
                        text,
                        is_error,
                        cx,
                    ))
                })
                .when_some(validation, |view, error| {
                    view.child(crate::settings::settings_notice_box(
                        "keybindings-config-error",
                        tr(error),
                        true,
                        cx,
                    ))
                })
                .when_some(self.error, |view, error| {
                    view.child(crate::settings::settings_notice_box(
                        "keybindings-record-error",
                        tr(error),
                        true,
                        cx,
                    ))
                })
                .children({
                    let mut sections: Vec<AnyElement> = Vec::new();
                    for group in TargetGroup::ALL {
                        let items: Vec<Target> = TARGETS
                            .into_iter()
                            .filter(|target| target.group() == group && matches_target(*target))
                            .collect();
                        if items.is_empty() {
                            continue;
                        }
                        let mut card = crate::settings::settings_card(cx).flex_shrink_0();
                        for (idx, target) in items.into_iter().enumerate() {
                            let recording = self
                                .recording
                                .as_ref()
                                .filter(|recording| recording.target == target);
                            let record_label = recording
                                .map(|recording| {
                                    recording
                                        .candidate
                                        .as_ref()
                                        .map(|candidate| {
                                            format!(
                                                "{} · {}",
                                                Kbd::format(candidate),
                                                tr("松开按键以确认")
                                            )
                                        })
                                        .unwrap_or_else(|| tr("请按组合键，Esc 取消").to_owned())
                                })
                                .unwrap_or_else(|| tr("录制").to_owned());
                            let current_chord = draft.get(target);
                            let is_default =
                                current_chord.as_deref() == Some(target.default(os));
                            let is_dirty = current_chord != saved.get(target);
                            let is_highlighted = highlighted_target == Some(target);
                            let row_content = v_flex()
                                .id(target.anchor_id())
                                .gap_1p5()
                                .when(is_highlighted, |el| {
                                    el.px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(Color::AccentSoft.get(cx))
                                })
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .gap_0p5()
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .child(target.label()),
                                                        )
                                                        .when(is_dirty, |row| {
                                                            row.child(
                                                                div()
                                                                    .px_1p5()
                                                                    .py_0p5()
                                                                    .rounded_sm()
                                                                    .bg(Color::AccentSoft.get(cx))
                                                                    .text_xs()
                                                                    .text_color(
                                                                        Color::Accent.get(cx),
                                                                    )
                                                                    .child(tr("待保存")),
                                                            )
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(target.description()),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .flex_shrink_0()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(Color::Divider.get(cx))
                                                        .bg(Color::Inset.get(cx))
                                                        .text_xs()
                                                        .font_family(
                                                            Theme::global(cx)
                                                                .mono_font_family
                                                                .clone(),
                                                        )
                                                        .text_color(if current_chord.is_some() {
                                                            Color::Text.get(cx)
                                                        } else {
                                                            Color::Muted.get(cx)
                                                        })
                                                        .child(display(current_chord, os)),
                                                )
                                                .child(reveal(
                                                    format!("record-shortcut-reveal-{}", target.index()).into(),
                                                    Button::new(("record-shortcut", target.index()))
                                                        .outline()
                                                        .small()
                                                        .track_focus(&self.focus[target.index()])
                                                        .label(record_label)
                                                        .accessibility_label(format!(
                                                            "{}: {}",
                                                            tr("录制快捷键"),
                                                            target.label()
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.start(target, window, cx)
                                                            },
                                                        )),
                                                ))
                                                .when(current_chord.is_some(), |actions| {
                                                    actions.child(reveal(
                                                        format!(
                                                            "disable-shortcut-reveal-{}",
                                                            target.index()
                                                        )
                                                        .into(),
                                                        Button::new((
                                                            "disable-shortcut",
                                                            target.index(),
                                                        ))
                                                        .ghost()
                                                        .small()
                                                        .label(tr("停用"))
                                                        .accessibility_label(format!(
                                                            "{}: {}",
                                                            tr("停用"),
                                                            target.label(),
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.stop(cx);
                                                                cx.global_mut::<Preferences>()
                                                                    .draft
                                                                    .set(target, None);
                                                                cx.refresh_windows();
                                                            },
                                                        )),
                                                    ))
                                                })
                                                .when(!is_default, |actions| {
                                                    actions.child(reveal(
                                                        format!(
                                                            "reset-shortcut-reveal-{}",
                                                            target.index()
                                                        )
                                                        .into(),
                                                        Button::new((
                                                            "reset-shortcut",
                                                            target.index(),
                                                        ))
                                                        .ghost()
                                                        .small()
                                                        .icon(Glyph::RotateCcw)
                                                        .tooltip(tr("恢复默认快捷键"))
                                                        .accessibility_label(format!(
                                                            "{}: {}",
                                                            tr("恢复默认快捷键"),
                                                            target.label()
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.stop(cx);
                                                                let os = cx.platform_info().os;
                                                                cx.global_mut::<Preferences>()
                                                                    .draft
                                                                    .set(
                                                                        target,
                                                                        Some(
                                                                            target
                                                                                .default(os)
                                                                                .to_owned(),
                                                                        ),
                                                                    );
                                                                cx.refresh_windows();
                                                            },
                                                        )),
                                                    ))
                                                }),
                                        ),
                                )
                                .when(active.get(target) != current_chord, |row| {
                                    row.child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(format!(
                                                "{}: {}",
                                                tr("当前生效"),
                                                display(active.get(target), os)
                                            )),
                                    )
                                });
                            card = card
                                .child(crate::settings::settings_row(idx > 0, cx).child(row_content));
                        }
                        sections.push(
                            v_flex()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Color::Muted.get(cx))
                                        .child(group.label()),
                                )
                                .child(card)
                                .into_any_element(),
                        );
                    }
                    if sections.is_empty() {
                        sections.push(
                            crate::settings::settings_card(cx)
                                .p_4()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("没有匹配的快捷键。")),
                                )
                                .into_any_element(),
                        );
                    }
                    sections
                })
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .flex_wrap()
                        .gap_2()
                        .child(reveal(
                            "save-keybindings-reveal".into(),
                            Button::new("save-keybindings")
                                .primary()
                                .label(tr("保存并立即生效"))
                                .disabled(
                                    !ready
                                        || state.load_failed
                                        || validation.is_some()
                                        || draft == saved
                                        || self.recording.is_some(),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.stop(cx);
                                    save_pending(cx);
                                })),
                        ))
                        .child(reveal(
                            "discard-keybindings-reveal".into(),
                            Button::new("discard-keybindings")
                                .outline()
                                .label(tr("丢弃未保存更改"))
                                .disabled(draft == saved)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.stop(cx);
                                    let saved = cx.global::<Preferences>().saved.clone();
                                    cx.global_mut::<Preferences>().draft = saved;
                                    cx.refresh_windows();
                                })),
                        ))
                        .child(reveal(
                            "reset-all-keybindings-reveal".into(),
                            Button::new("reset-all-keybindings")
                                .outline()
                                .label(tr("全部恢复默认"))
                                .disabled(draft == defaults || self.recording.is_some())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.stop(cx);
                                    let defaults = Bindings::defaults(cx.platform_info().os);
                                    cx.global_mut::<Preferences>().draft = defaults;
                                    cx.refresh_windows();
                                })),
                        ))
                        .child(reveal(
                            "reload-keybindings-reveal".into(),
                            Button::new("reload-keybindings")
                                .outline()
                                .label(tr("重新读取"))
                                .disabled(!ready || draft != saved || self.recording.is_some())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.stop(cx);
                                    reload(cx);
                                })),
                        )),
                ),
        )
    }
}
