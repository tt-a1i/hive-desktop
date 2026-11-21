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
        if key.key == "tab"
            && (key.modifiers == Modifiers::none() || key.modifiers == Modifiers::shift())
        {
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
        let validation = target.validate(&key.unparse(os), os).and_then(|candidate| {
            let existing_default = Some(candidate.unparse(os).as_str()) == target.default(os);
            if !existing_default
                && protected_reason_in_scope(
                    &dispatch_key(candidate.clone(), cx),
                    target.context_scope(),
                    cx,
                )
                .is_some()
            {
                return Err("此组合键已用于现有组件或文字编辑，请选择其他组合。");
            }
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
        let conflicts = analyze_conflicts(
            &draft,
            os,
            |key| dispatch_key(key, cx),
            |key, scope| protected_reason_in_scope(key, scope, cx),
        );
        let conflict_summary = if conflicts.is_empty() {
            validation.map(|err| tr(err).to_owned())
        } else {
            let mut pairs: Vec<String> = Vec::new();
            for c in &conflicts {
                let desc = match c.kind {
                    ConflictKind::SameScope(other) | ConflictKind::OverlappingScope(other) => {
                        if c.target.index() < other.index() {
                            Some(format!("「{}」↔「{}」", c.target.label(), other.label()))
                        } else {
                            None
                        }
                    }
                    ConflictKind::ProtectedComponent(comp) => {
                        Some(format!("「{}」↔ {}", c.target.label(), tr(comp)))
                    }
                };
                if let Some(d) = desc
                    && !pairs.contains(&d)
                {
                    pairs.push(d);
                }
            }
            let count = pairs.len().max(1);
            let joined = pairs.join("、");
            Some(if crate::language::english() {
                format!("{count} shortcut conflict(s): {joined}. Reassign or disable before saving.")
            } else {
                format!("检测到 {count} 处快捷键冲突：{joined}。请调整或停用冲突项后再保存。")
            })
        };
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
                .when_some(state.layout_error.as_ref(), |view, error| {
                    view.child(crate::settings::settings_notice_box(
                        "keybindings-layout-error",
                        format!("{}\n{}", tr("键盘布局变化后无法更新快捷键。现有绑定和配置未修改；请检查快捷键后重试。"), explain_error(error)),
                        true,
                        cx,
                    ))
                })
                .when_some(conflict_summary, |view, error| {
                    view.child(crate::settings::settings_notice_box(
                        "keybindings-config-error",
                        error,
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
                                current_chord.as_deref() == target.default(os);
                            let is_dirty = current_chord != saved.get(target);
                            let is_highlighted = highlighted_target == Some(target);
                            let scope = target.context_scope();
                            let row_conflicts: Vec<&TargetConflict> = conflicts
                                .iter()
                                .filter(|c| c.target == target)
                                .collect();
                            let has_conflict = !row_conflicts.is_empty();
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
                                                        .flex_wrap()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .child(target.label()),
                                                        )
                                                        .child(
                                                            div()
                                                                .id(SharedString::from(format!(
                                                                    "keybinding-scope-{}",
                                                                    target.index()
                                                                )))
                                                                .px_1p5()
                                                                .py(px(1.))
                                                                .rounded_sm()
                                                                .bg(Color::Inset.get(cx))
                                                                .border_1()
                                                                .border_color(Color::Divider.get(cx))
                                                                .text_size(px(10.5))
                                                                .text_color(Color::Secondary.get(cx))
                                                                .child(tr(scope.badge_key()))
                                                                .tooltip(move |window, cx| {
                                                                    gpui_kit::component::tooltip::Tooltip::new(
                                                                        format!(
                                                                            "{} ({})",
                                                                            tr(scope.detail_key()),
                                                                            scope.when_expr()
                                                                        ),
                                                                    )
                                                                    .build(window, cx)
                                                                }),
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
                                                        })
                                                        .when(has_conflict, |row| {
                                                            row.child(
                                                                div()
                                                                    .id(SharedString::from(format!(
                                                                        "keybinding-conflict-badge-{}",
                                                                        target.index()
                                                                    )))
                                                                    .px_1p5()
                                                                    .py_0p5()
                                                                    .rounded_sm()
                                                                    .bg(Color::ErrorSurface.get(cx))
                                                                    .text_xs()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(Color::Error.get(cx))
                                                                    .child(tr("冲突")),
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
                                                        .border_color(if has_conflict {
                                                            Color::Error.get(cx)
                                                        } else {
                                                            Color::Divider.get(cx)
                                                        })
                                                        .bg(Color::Inset.get(cx))
                                                        .text_xs()
                                                        .font_family(
                                                            Theme::global(cx)
                                                                .mono_font_family
                                                                .clone(),
                                                        )
                                                        .text_color(if has_conflict {
                                                            Color::Error.get(cx)
                                                        } else if current_chord.is_some() {
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
                                                                        target.default(os).map(str::to_owned),
                                                                    );
                                                                cx.refresh_windows();
                                                            },
                                                        )),
                                                    ))
                                                }),
                                        ),
                                )
                                .when(has_conflict, |row| {
                                    row.children(row_conflicts.iter().enumerate().map(|(ci, conflict)| {
                                        let chord_label = display(&Some(conflict.chord.clone()), os);
                                        let (explanation, jump_target) = match conflict.kind {
                                            ConflictKind::SameScope(other) => {
                                                let msg = if crate::language::english() {
                                                    format!(
                                                        "Same-scope conflict ({}): shares {} with \"{}\"",
                                                        tr(scope.badge_key()),
                                                        chord_label,
                                                        other.label()
                                                    )
                                                } else {
                                                    format!(
                                                        "同作用域（{}）冲突：与「{}」共享 {}",
                                                        tr(scope.badge_key()),
                                                        other.label(),
                                                        chord_label
                                                    )
                                                };
                                                (msg, Some(other))
                                            }
                                            ConflictKind::OverlappingScope(other) => {
                                                let other_scope = other.context_scope();
                                                let msg = tr("重叠作用域冲突：「{scope}」与「{other_scope}」（{target}）在 {chord} 上重叠")
                                                    .replace("{scope}", tr(scope.badge_key()))
                                                    .replace("{other_scope}", tr(other_scope.badge_key()))
                                                    .replace("{target}", other.label())
                                                    .replace("{chord}", &chord_label);
                                                (msg, Some(other))
                                            }
                                            ConflictKind::ProtectedComponent(comp) => {
                                                let msg = if crate::language::english() {
                                                    format!(
                                                        "Conflicts with built-in {} binding ({})",
                                                        tr(comp),
                                                        chord_label
                                                    )
                                                } else {
                                                    format!(
                                                        "与内置「{}」快捷键（{}）冲突",
                                                        tr(comp),
                                                        chord_label
                                                    )
                                                };
                                                (msg, None)
                                            }
                                        };
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .flex_wrap()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(Color::ErrorSurface.get(cx))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_xs()
                                                    .text_color(Color::Error.get(cx))
                                                    .child(explanation),
                                            )
                                            .when_some(jump_target, |line, other| {
                                                line.child(
                                                    Button::new((
                                                        "jump-conflict-target",
                                                        target.index() * 16 + ci,
                                                    ))
                                                    .ghost()
                                                    .xsmall()
                                                    .h(px(20.))
                                                    .px_1p5()
                                                    .label(tr("查看冲突项"))
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.focus_target(other, window, cx)
                                                        },
                                                    )),
                                                )
                                            })
                                    }))
                                })
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
