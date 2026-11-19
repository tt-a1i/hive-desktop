use crate::{app::HiveApp, language::tr, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenuItem},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::json;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModeState {
    backend: String,
    selected: Option<String>,
    applied: Option<String>,
    pending_next_turn: bool,
    options: Vec<ModeOption>,
    description: Option<String>,
}
impl ModeState {
    fn label(&self, id: &str) -> String {
        self.options
            .iter()
            .find(|option| option.id == id)
            .map(|option| mode_label(&self.backend, option))
            .unwrap_or_else(|| id.to_owned())
    }

    fn status(&self) -> String {
        let display = |id: Option<&str>| {
            id.map(|id| self.label(id))
                .unwrap_or_else(|| tr("未报告").to_owned())
        };
        let active = format!("{}: {}", tr("当前生效"), display(self.applied.as_deref()));
        if self.pending_next_turn {
            format!("{}: {} · {active}", tr("下回合使用"), display(self.selected.as_deref()))
        } else {
            active
        }
    }
}
#[derive(serde::Deserialize)]
struct ModeOption {
    id: String,
    label: String,
    description: Option<String>,
}
#[derive(Default)]
pub struct ModePicker {
    mode: ModeSlot,
    permission: ModeSlot,
}

#[derive(Default)]
struct ModeSlot {
    loading: bool,
    state: Option<ModeState>,
    error: Option<String>,
}

fn mode_label(backend: &str, option: &ModeOption) -> String {
    let label = match (backend, option.id.as_str()) {
        ("codex", "read-only") => Some(tr("只读")),
        ("codex", "workspace-write") => Some(tr("工作区写入")),
        ("codex", "full-access") => Some(tr("完全访问")),
        ("codex", "native-plan") | ("claude", "plan") => Some(tr("计划")),
        ("codex", "native-default") => Some(tr("默认协作")),
        ("claude", "default") => Some(tr("默认权限")),
        ("claude", "acceptEdits") => Some(tr("自动接受编辑")),
        _ => None,
    };
    label.unwrap_or(&option.label).to_owned()
}

impl HiveApp {
    pub(crate) fn load_mode(&mut self, mode: Option<String>, cx: &mut Context<Self>) {
        self.load_mode_axis(false, mode, cx);
    }

    /// Read native state after a turn settles, including when turn/start was
    /// rejected. The selection only becomes active after native acceptance.
    pub(crate) fn refresh_pending_session_modes(&mut self, cx: &mut Context<Self>) {
        if self.mode_picker.mode.state.as_ref().is_some_and(|state| state.pending_next_turn) {
            self.load_mode(None, cx);
        }
        if self.mode_picker.permission.state.as_ref().is_some_and(|state| state.pending_next_turn) {
            self.load_permission_mode(None, cx);
        }
    }

    pub(crate) fn cycle_session_mode(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        if self.loading
            || !self.conversation_ready
            || !self.connected
            || self.closing
            || self.conversation_is_read_only()
            || self.pending.get(&session.id).copied().unwrap_or(0) > 0
            || self.interaction_count(&session.id) > 0
            || self.mode_picker.mode.loading
        {
            return;
        }
        let next = match self.mode_picker.mode.state.as_ref() {
            None => {
                self.load_mode(None, cx);
                return;
            }
            Some(state) if state.options.len() < 2 => return,
            Some(state) => {
                let current = state.selected.as_deref().or(state.applied.as_deref());
                let idx = current
                    .and_then(|id| state.options.iter().position(|option| option.id == id))
                    .unwrap_or(0);
                state.options[(idx + 1) % state.options.len()].id.clone()
            }
        };
        self.load_mode(Some(next), cx);
    }

    pub(crate) fn load_permission_mode(&mut self, mode: Option<String>, cx: &mut Context<Self>) {
        if !self
            .session
            .as_ref()
            .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Codex))
        {
            return;
        }
        self.load_mode_axis(true, mode, cx);
    }

    fn load_mode_axis(&mut self, permission: bool, mode: Option<String>, cx: &mut Context<Self>) {
        let picker = if permission {
            &self.mode_picker.permission
        } else {
            &self.mode_picker.mode
        };
        if self.current_history_access().is_some_and(|a| a.handoff_passive())
            || self.conversation_is_read_only()
            || picker.loading
            || !self.conversation_ready
            || self.loading
            || !self.connected
            || self.closing
        {
            return;
        }
        let (Some(runtime), Some(session)) = (self.runtime.clone(), self.session.as_ref()) else {
            return;
        };
        let id = session.id.clone();
        if mode.is_some()
            && (self.pending.get(&id).copied().unwrap_or(0) > 0 || self.interaction_count(&id) > 0)
        {
            return;
        }
        let connection = self.connection_generation;
        let open = self.open_request;
        let picker = if permission {
            &mut self.mode_picker.permission
        } else {
            &mut self.mode_picker.mode
        };
        picker.loading = true;
        picker.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mutation = mode.is_some();
            let read_rpc = if permission {
                "chat.permissionMode"
            } else {
                "chat.mode"
            };
            let write_rpc = if permission {
                "chat.setPermissionMode"
            } else {
                "chat.setMode"
            };
            let result = if let Some(mode) = mode {
                runtime
                    .request(write_rpc, json!({"sessionId":id,"mode":mode}))
                    .await
            } else {
                runtime.request(read_rpc, json!({"sessionId":id,"viewRevision":open})).await
            };
            let failure = result.as_ref().err().cloned();
            let result = if mutation && failure.is_some() {
                runtime.request(read_rpc, json!({"sessionId":id,"viewRevision":open})).await
            } else {
                result
            };
            let state = result.and_then(|value| {
                serde_json::from_value::<ModeState>(value).map_err(|e| e.to_string())
            });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.open_request != open
                    || !this.session.as_ref().is_some_and(|s| s.id == id)
                {
                    return;
                }
                let picker = if permission {
                    &mut this.mode_picker.permission
                } else {
                    &mut this.mode_picker.mode
                };
                picker.loading = false;
                match state {
                    Ok(state) => picker.state = Some(state),
                    Err(error) => {
                        picker.state = None;
                        picker.error = Some(error);
                    }
                }
                if let Some(error) = failure {
                    picker.error = Some(error);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_composer_mode_pill(
        &self,
        permission: bool,
        disabled: bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let picker = if permission {
            &self.mode_picker.permission
        } else {
            &self.mode_picker.mode
        };
        let state = picker.state.as_ref()?;
        if state.options.is_empty() {
            return None;
        }
        let current_id = state.selected.as_deref().or(state.applied.as_deref());
        let label = current_id
            .map(|id| state.label(id))
            .unwrap_or_else(|| tr(if permission { "权限" } else { "模式" }).into());
        let heading = if permission {
            tr("Codex 访问权限")
        } else {
            tr("会话模式")
        };
        let accessibility_label = format!("{heading}. {}", state.status());
        let options: Vec<(String, String, bool)> = state
            .options
            .iter()
            .map(|option| {
                (
                    option.id.clone(),
                    mode_label(&state.backend, option),
                    current_id == Some(option.id.as_str()),
                )
            })
            .collect();
        let weak = cx.entity().downgrade();
        let id = if permission {
            "composer-permission-menu"
        } else {
            "composer-mode-menu"
        };
        Some(
            Button::new(id)
                .custom(crate::palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(22.))
                .px_1p5()
                .rounded_md()
                .min_w_0()
                .disabled(disabled || picker.loading)
                .tooltip(accessibility_label.clone())
                .accessibility_label(accessibility_label)
                .child(
                    h_flex()
                        .min_w_0()
                        .items_center()
                        .gap_1()
                        .child(
                            Icon::new(if permission {
                                Glyph::Lock
                            } else {
                                Glyph::Sparkles
                            })
                            .size(px(11.5))
                            .text_color(Color::Muted.get(cx))
                            .flex_shrink_0(),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .max_w(px(120.))
                                .truncate()
                                .text_size(px(12.))
                                .text_color(Color::Muted.get(cx))
                                .child(label),
                        )
                        .child(
                            Icon::new(Glyph::ChevronDown)
                                .size(px(10.5))
                                .text_color(Color::Muted.get(cx))
                                .flex_shrink_0(),
                        ),
                )
                .dropdown_menu(move |mut menu, _, _| {
                    for (option_id, option_label, selected) in &options {
                        let weak_item = weak.clone();
                        let next_id = option_id.clone();
                        menu = menu.item(
                            PopupMenuItem::new(option_label.clone())
                                .checked(*selected)
                                .on_click(move |_, _, cx| {
                                    let _ = weak_item.update(cx, |this, cx| {
                                        if permission {
                                            this.load_permission_mode(Some(next_id.clone()), cx);
                                        } else {
                                            this.load_mode(Some(next_id.clone()), cx);
                                        }
                                    });
                                }),
                        );
                    }
                    menu
                })
                .into_any_element(),
        )
    }

    pub(crate) fn render_composer_mode_pills(
        &self,
        running: bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let session = self.session.as_ref()?;
        let blocked = self.pending.get(&session.id).copied().unwrap_or(0) > 0
            || self.interaction_count(&session.id) > 0;
        let disabled = running
            || self.loading
            || !self.conversation_ready
            || !self.connected
            || self.closing
            || self.conversation_is_read_only()
            || blocked;
        let mode_pill = self.render_composer_mode_pill(false, disabled, cx);
        let permission_pill = matches!(session.backend, crate::protocol::Backend::Codex)
            .then(|| self.render_composer_mode_pill(true, disabled, cx))
            .flatten();
        if mode_pill.is_none() && permission_pill.is_none() {
            return None;
        }
        Some(
            h_flex()
                .flex_shrink_0()
                .items_center()
                .gap_1()
                .children(mode_pill)
                .children(permission_pill)
                .into_any_element(),
        )
    }

    pub(crate) fn render_mode_settings(&self, running: bool, cx: &Context<Self>) -> AnyElement {
        let codex = self
            .session
            .as_ref()
            .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Codex));
        v_flex()
            .child(self.render_mode_axis(false, running, cx))
            .when(codex, |view| {
                view.child(self.render_mode_axis(true, running, cx))
            })
            .into_any_element()
    }

    fn render_mode_axis(&self, permission: bool, running: bool, cx: &Context<Self>) -> AnyElement {
        let heading = if permission {
            tr("Codex 访问权限")
        } else {
            tr("会话模式")
        };
        let disabled = running
            || self.loading
            || !self.conversation_ready
            || !self.connected
            || self.closing
            || self.conversation_is_read_only();
        let blocked = self.session.as_ref().is_some_and(|session| {
            self.pending.get(&session.id).copied().unwrap_or(0) > 0
                || self.interaction_count(&session.id) > 0
        });
        let picker = if permission {
            &self.mode_picker.permission
        } else {
            &self.mode_picker.mode
        };
        v_flex()
            .id(format!("mode-picker-content-{permission}"))
            .role(Role::Group)
            .aria_label(heading)
            .min_w_0()
            .gap_1p5()
            .pt_2()
            .border_t_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        crate::text_reading::selectable_text(format!("mode-heading-{permission}"), heading)
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Muted.get(cx)),
                    )
                    .child(
                        Button::new(format!("refresh-mode-{permission}"))
                            .ghost()
                            .compact()
                            .xsmall()
                            .label(tr("重新读取"))
                            .disabled(picker.loading || disabled)
                            .on_click(cx.listener(move |this, _, _, cx| this.load_mode_axis(permission, None, cx))),
                    ),
            )
            .when(permission, |view| {
                view.child(crate::text_reading::selectable_text(
                    "codex-permission-scope",
                    tr("仅适用于当前 Codex 原生会话，不限制其他 Agent。完全访问会关闭 Codex 沙箱和审批。"),
                ).text_xs().text_color(Color::Muted.get(cx)))
            })
            .when(picker.loading, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("正在读取原生设置…")),
                )
            })
            .when_some(picker.error.as_ref(), |view, error| {
                view.child(
                    crate::text_reading::selectable_text(
                        format!("mode-error-{permission}"),
                        crate::language::localize_owned_error(error),
                    )
                    .text_xs()
                    .text_color(Color::Error.get(cx)),
                )
            })
            .when_some(picker.state.as_ref(), |view, state| {
                let status = state.status();
                view.child(
                    div()
                        .id(format!("mode-current-status-{permission}"))
                        .role(Role::Status)
                        .aria_label(status.clone())
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(status),
                )
                .when_some(state.description.as_ref(), |view, text| {
                    view.child(
                        crate::text_reading::selectable_text(format!("mode-description-{permission}"), match text.as_str() {
                            "Native Claude permission mode applies to this session. It does not constrain other agents." => tr("Claude 原生权限模式仅适用于当前会话，不限制其他 Agent。").to_owned(),
                            "This backend does not support native session modes." => tr("此后端不支持会话模式。").to_owned(),
                            "Codex 未提供权限策略要求。" => tr("Codex 未提供权限策略要求。").to_owned(),
                            "无法确定 Codex 权限策略要求。" => tr("无法确定 Codex 权限策略要求。").to_owned(),
                            "此 Codex 程序不支持原生协作模式。" => tr("此 Codex 程序不支持原生协作模式。").to_owned(),
                            "无法确定 Codex 协作模式。" => tr("无法确定 Codex 协作模式。").to_owned(),
                            _ => text.clone(),
                        })
                            .text_xs()
                            .text_color(Color::Muted.get(cx)),
                    )
                })
                .when(state.options.is_empty(), |view| {
                    view.child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(format!(
                                "{}: {}",
                                state.backend,
                                tr("当前没有可用的原生模式选项。")
                            )),
                    )
                })
                .children(state.options.iter().enumerate().map(|(i, option)| {
                    let selected = state.selected.as_ref() == Some(&option.id);
                    let id = option.id.clone();
                    let label = mode_label(&state.backend, option);
                    let description = option.description.clone();
                    v_flex()
                        .min_w_0()
                        .child(
                            Button::new(format!("mode-option-{permission}-{i}"))
                                .ghost()
                                .compact()
                                .w_full()
                                .justify_start()
                                .selected(selected)
                                .disabled(picker.loading || disabled || blocked)
                                .accessibility_label(format!("{}: {}", heading, label))
                                .label(label)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.load_mode_axis(permission, Some(id.clone()), cx)
                                })),
                        )
                        .when_some(description, |view, text| {
                            view.child(
                                div()
                                    .px_2()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(text),
                            )
                        })
                }))
            })
            .into_any_element()
    }
}
