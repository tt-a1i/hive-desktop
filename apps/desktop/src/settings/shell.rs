use super::*;
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::component::{button::ButtonVariants, dialog::DialogEntrance};
use std::cell::Cell;

impl HiveApp {
    /// Use the same window-owned modal stack as confirmations and file previews.
    /// It owns nested Escape handling, focus trapping and focus restoration.
    pub(crate) fn ensure_preferences_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Restart/navigation may have closed the entire window dialog stack.
        if self.preferences_modal_open && !window.has_active_dialog(cx) {
            self.preferences_modal_open = false;
        }
        if !(self.settings_panel || self.roles.visible) || self.preferences_modal_open {
            return;
        }
        if self.preferences_return_focus.is_none() {
            self.preferences_return_focus = window.focused(cx);
        }
        self.preferences_modal_open = true;
        let weak = cx.weak_entity();
        let close = weak.clone();
        let focus = self.preferences_focus.clone();
        let initial_focus = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            // WindowState initially focuses its own shell, outside our action
            // scope. Enter this content scope once per opening, never again on
            // redraw or while a child confirmation owns focus.
            if initial_focus.replace(false) {
                focus.focus(window, cx);
            }
            let size = window.viewport_size();
            let content = weak.clone();
            dialog
                .centered()
                .w(
                    (size.width - px(if size.width >= px(1024.) { 80. } else { 48. }))
                        .min(px(1280.)),
                )
                .h(
                    (size.height - px(if size.width >= px(1024.) { 80. } else { 64. }))
                        .min(px(960.)),
                )
                .p_0()
                .rounded(px(16.))
                .shadow_xl()
                .overflow_hidden()
                .close_button(false)
                .entrance(DialogEntrance::Fade)
                // Enter belongs to the active form, never to the outer modal.
                .on_ok(|_, _, _| false)
                .on_close({
                    let close = close.clone();
                    move |_, window, cx| {
                        let _ = close.update(cx, |this, cx| this.leave_preferences(window, cx));
                    }
                })
                .content(move |body, window, cx| {
                    let shell = content
                        .update(cx, |this, cx| {
                            this.render_preferences(
                                window.viewport_size().width < px(768.),
                                window,
                                cx,
                            )
                            .into_any_element()
                        })
                        .unwrap_or_else(|_| div().into_any_element());
                    body.p_0().min_h_0().size_full().child(shell)
                })
        });
    }

    pub(super) fn render_settings_shell(
        &self,
        content: AnyElement,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let title = if self.roles.visible {
            tr("角色模板")
        } else {
            tr(super::section_meta(self.settings_section).0)
        };
        let search_expanded = compact && self.preferences_search_open;
        // WindowState owns child dialogs as sibling layers. Their focus is
        // outside this content scope; hide the parent controls from AX.
        // The child keeps its own native focus trap and Escape handling.
        let child_layer_focused =
            window.focused(cx).is_some() && !self.preferences_focus.contains_focused(window, cx);
        v_flex()
            .id("settings-modal-shell")
            .role(Role::Group)
            .aria_label(tr("设置"))
            .aria_hidden(child_layer_focused)
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .bg(Color::Surface.get(cx))
            .track_focus(&self.preferences_focus)
            .key_context("HivePreferences")
            .on_action(
                cx.listener(|this, _: &crate::OpenConversationFind, window, cx| {
                    this.preferences_search_open = true;
                    let focus = this.settings_search.read(cx).focus_handle(cx);
                    window.on_next_frame(move |window, cx| focus.focus(window, cx));
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::ClosePreferences, window, cx| {
                    if !this.settings_search.read(cx).value().is_empty() {
                        this.settings_search.update(cx, |input, cx| {
                            input.set_value("", window, cx);
                            input.focus(window, cx);
                        });
                        cx.notify();
                    } else {
                        this.leave_preferences(window, cx);
                        window.close_dialog(cx);
                    }
                }),
            )
            .when(
                !self.connected && !self.closing && (!self.loading || self.restarting),
                |shell| {
                    shell.child(self.render_connection_notice(px(0.), cx)).when(
                        self.error.is_some(),
                        |shell| {
                            shell.child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .flex_shrink_0()
                                    .child(self.render_error_banner(cx)),
                            )
                        },
                    )
                },
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.render_settings_tabs(compact, window, cx))
                    .when(!search_expanded, |shell| {
                        shell.child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .h_full()
                                .child(
                                    h_flex()
                                        .h(px(48.))
                                        .flex_shrink_0()
                                        .w_full()
                                        .min_w_0()
                                        .items_center()
                                        .gap_2()
                                        .pl_5()
                                        .pr_2()
                                        .border_b_1()
                                        .border_color(Color::Divider.get(cx))
                                        .child(
                                            div()
                                                .text_size(px(13.))
                                                .text_color(Color::Muted.get(cx))
                                                .child(tr("设置")),
                                        )
                                        .child(
                                            Icon::new(Glyph::ChevronRight)
                                                .size(px(12.))
                                                .text_color(Color::Muted.get(cx)),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_size(px(13.))
                                                .text_color(Color::Text.get(cx))
                                                .child(title),
                                        )
                                        .child(
                                            Button::new("close-settings-modal")
                                                .ghost()
                                                .compact()
                                                .h(px(28.))
                                                .w(px(28.))
                                                .rounded_md()
                                                .icon(Glyph::X)
                                                .tooltip(tr("关闭设置"))
                                                .accessibility_label(tr("关闭设置"))
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.leave_preferences(window, cx);
                                                    window.close_dialog(cx);
                                                })),
                                        ),
                                )
                                .when(
                                    self.settings_section == crate::app::SettingsSection::Programs
                                        && !self.roles.visible,
                                    |pane| {
                                        pane.children(self.render_program_save_feedback(window, cx))
                                    },
                                )
                                .child(content),
                        )
                    }),
            )
    }
}
