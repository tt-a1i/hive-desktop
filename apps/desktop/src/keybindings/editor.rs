use super::*;
use crate::{
    focus_scroll::{Reveal, Viewport},
    palette::Color,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
};

struct Recording {
    target: Target,
    window: WindowId,
    candidate: Option<Keystroke>,
}
pub(crate) struct Editor {
    focus: [FocusHandle; 3],
    recording: Option<Recording>,
    interceptor: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    error: Option<&'static str>,
    scroll: ScrollHandle,
}
impl Editor {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus: [FocusHandle; 3] = std::array::from_fn(|_| cx.focus_handle());
        let mut subscriptions = Vec::new();
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
            recording: None,
            interceptor: None,
            _subscriptions: subscriptions,
            error: None,
            scroll: ScrollHandle::new(),
        }
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
        let ready = state.root.is_some();
        let protected = state.load_failed || state.preview;
        let validation = draft
            .normalized(os)
            .and_then(|normalized| check_registered(&normalized, cx))
            .err();
        let message = state.notice.as_ref().map(|notice| match notice {
            Notice::Read(error) => format!("{}\n{}", tr("读取快捷键失败，当前快捷键保持不变。请修复或移走原文件后重新读取；不会覆盖原文件。"), explain_error(error)),
            Notice::Save(error) => format!("{}\n{}", tr("保存快捷键失败，原文件和当前快捷键均未改变。"), explain_error(error)),
            Notice::Saved => tr("快捷键已保存，请退出并重新启动 Hive Desktop 后使用。重新启动后台不会应用这些更改。").to_owned(),
        });
        let reveal = |id: SharedString, button: Button| Reveal::new(id, &self.scroll, button);
        Viewport::new("keybindings-settings-viewport", &self.scroll,
            v_flex().id("keybindings-settings-content").flex_1().min_h_0().min_w_0().gap_3()
                .overflow_y_scroll().track_scroll(&self.scroll).vertical_scrollbar(&self.scroll)
                .on_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| this.released(event, window, cx)))
                .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, window, cx| this.modifiers_released(event, window, cx)))
                .child(div().flex_shrink_0().text_sm().text_color(Color::Muted.get(cx)).child(tr("录制单个 Command 或 Ctrl 组合键。更改保存后需重启应用；当前快捷键、发送方式和文字编辑保持不变。")))
                .when(!ready, |view| view.child(div().text_sm().child(tr("等待后台提供本地数据目录；默认快捷键仍可使用。"))))
                .when(saved != active, |view| view.child(div().id("keybindings-restart-required").role(Role::Status).text_sm().child(tr("已保存的快捷键将在下次启动应用时生效。"))))
                .when_some(message, |view, message| view.child(div().id("keybindings-persistence-notice").role(Role::Status).text_sm().child(message)))
                .when_some(validation, |view, error| view.child(div().id("keybindings-config-error").role(Role::Status).text_sm().text_color(Color::Error.get(cx)).child(tr(error))))
                .when_some(self.error, |view, error| view.child(div().id("keybindings-record-error").role(Role::Status).text_sm().text_color(Color::Error.get(cx)).child(tr(error))))
                .children(TARGETS.into_iter().map(|target| {
                    let recording = self.recording.as_ref().filter(|recording| recording.target == target);
                    let record_label = recording.map(|recording| recording.candidate.as_ref().map(|candidate| format!("{} · {}", Kbd::format(candidate), tr("松开按键以确认"))).unwrap_or_else(|| tr("请按组合键，Esc 取消").to_owned())).unwrap_or_else(|| tr("录制快捷键").to_owned());
                    crate::settings::settings_card(cx).flex_shrink_0().p_3().gap_2()
                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(target.title()))
                        .child(div().text_xs().child(format!("{}: {}\n{}: {}\n{}: {}", tr("当前生效"), display(active.get(target), os), tr("已保存，重启后生效"), display(saved.get(target), os), tr("待保存"), display(draft.get(target), os))))
                        .child(h_flex().flex_wrap().gap_2()
                            .child(reveal(format!("record-shortcut-reveal-{}", target.index()).into(), Button::new(("record-shortcut", target.index())).outline().small().track_focus(&self.focus[target.index()])
                                .label(record_label).accessibility_label(format!("{}: {}", tr("录制快捷键"), target.title()))
                                .on_click(cx.listener(move |this, _, window, cx| this.start(target, window, cx)))))
                            .child(reveal(format!("disable-shortcut-reveal-{}", target.index()).into(), Button::new(("disable-shortcut", target.index())).outline().small().label(tr("停用快捷键"))
                                .on_click(cx.listener(move |this, _, _, cx| { this.stop(cx); cx.global_mut::<Preferences>().draft.set(target, None); cx.refresh_windows(); }))))
                            .child(reveal(format!("reset-shortcut-reveal-{}", target.index()).into(), Button::new(("reset-shortcut", target.index())).outline().small().label(tr("恢复默认"))
                                .on_click(cx.listener(move |this, _, _, cx| { this.stop(cx); let os = cx.platform_info().os; cx.global_mut::<Preferences>().draft.set(target, Some(target.default(os).to_owned())); cx.refresh_windows(); })))))
                }))
                .child(h_flex().flex_shrink_0().flex_wrap().gap_2()
                    .child(reveal("save-keybindings-reveal".into(), Button::new("save-keybindings").primary().label(tr("保存快捷键，重启后生效")).disabled(!ready || protected || validation.is_some() || draft == saved || self.recording.is_some())
                        .on_click(cx.listener(|this, _, _, cx| { this.stop(cx); save_pending(cx); }))))
                    .child(reveal("discard-keybindings-reveal".into(), Button::new("discard-keybindings").outline().label(tr("丢弃未保存更改")).disabled(draft == saved)
                        .on_click(cx.listener(|this, _, _, cx| { this.stop(cx); let saved = cx.global::<Preferences>().saved.clone(); cx.global_mut::<Preferences>().draft = saved; cx.refresh_windows(); }))))
                    .child(reveal("reload-keybindings-reveal".into(), Button::new("reload-keybindings").outline().label(tr("重新读取快捷键")).disabled(!ready || draft != saved || self.recording.is_some())
                        .on_click(cx.listener(|this, _, _, cx| { this.stop(cx); reload(cx); })))))
        )
    }
}
