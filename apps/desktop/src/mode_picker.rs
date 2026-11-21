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

mod recovery;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModeState {
    backend: String,
    selected: Option<String>,
    applied: Option<String>,
    pending_next_turn: bool,
    #[serde(default)]
    connect_available: bool,
    options: Vec<ModeOption>,
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
    recovery: recovery::Recovery,
}

#[derive(Default)]
struct ModeSlot {
    loading: bool,
    refresh_queued: bool,
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

    pub(crate) fn refresh_advertised_modes(&mut self, cx: &mut Context<Self>) {
        if self.mode_picker.mode.loading {
            self.mode_picker.mode.refresh_queued = true;
        } else {
            self.load_mode(None, cx);
        }
        if self.mode_picker.permission.loading {
            self.mode_picker.permission.refresh_queued = true;
        } else {
            self.load_permission_mode(None, cx);
        }
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
        if self.connect_saved_session(crate::sessions::NativeSessionPanel::Models, cx) {
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
            .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Codex | crate::protocol::Backend::Acp))
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
                let failed = failure.is_some();
                if let Some(error) = failure {
                    picker.error = Some(error);
                }
                if std::mem::take(&mut picker.refresh_queued) && !failed {
                    this.load_mode_axis(permission, None, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn connect_acp_modes(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_ref() else { return };
        if !self.connected || self.loading || self.closing || !self.conversation_ready
            || self.conversation_is_read_only() || !self.workspace_writes_allowed()
            || self.mode_picker.mode.loading || !self.mode_picker.mode.state.as_ref().is_some_and(|mode| mode.connect_available)
        { return; }
        let Some(runtime) = self.runtime.clone() else { return; };
        let id = session.id.clone();
        let connection = self.connection_generation;
        let open = self.open_request;
        self.mode_picker.mode.loading = true;
        self.mode_picker.mode.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request("chat.acp.prepareModes", json!({"sessionId":id})).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection || this.open_request != open
                    || !this.session.as_ref().is_some_and(|session| session.id == id) { return; }
                this.mode_picker.mode.loading = false;
                match result {
                    Ok(_) => this.show_session(id, cx),
                    Err(error) => {
                        this.mode_picker.mode.error = Some(error.clone());
                        this.error = Some(crate::app::ErrorNotice::Detail("连接 ACP 会话失败", error));
                        cx.notify();
                    }
                }
            });
        }).detach();
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
        let heading = if permission && state.backend == "acp" {
            tr("ACP 访问权限")
        } else if permission {
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
        let mode_pill = self.render_composer_mode_pill(false, disabled, cx).or_else(|| {
            session.inherited_draft_work_mode.map(|mode| {
                let status = format!("{} · {} · {}", tr("会话模式"), tr("下回合使用"), mode.label());
                Button::new("inherited-draft-work-mode")
                    .custom(crate::palette::quiet_icon_variant(cx))
                    .compact()
                    .xsmall()
                    .h(px(22.))
                    .rounded_md()
                    .icon(Glyph::Sparkles)
                    .label(mode.label())
                    .tooltip(status.clone())
                    .accessibility_label(status)
                    .disabled(disabled || self.unavailable_inherited_work_mode().is_some())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.connect_saved_session(crate::sessions::NativeSessionPanel::Models, cx);
                    }))
                    .into_any_element()
            })
        }).or_else(|| {
            self.mode_picker.mode.state.as_ref().is_some_and(|mode| mode.connect_available).then(|| {
                Button::new("composer-connect-acp-modes").compact().xsmall()
                    .label(tr("连接 ACP 以设置模式"))
                    .disabled(disabled || self.mode_picker.mode.loading)
                    .on_click(cx.listener(|this, _, _, cx| this.connect_acp_modes(cx))).into_any_element()
            })
        });
        let permission_pill = matches!(session.backend, crate::protocol::Backend::Codex | crate::protocol::Backend::Acp)
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

    pub(crate) fn render_mode_settings(
        &self,
        running: bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let codex = self
            .session
            .as_ref()
            .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Codex));
        let acp_permission = self
            .mode_picker
            .permission
            .state
            .as_ref()
            .is_some_and(|mode| mode.backend == "acp" && !mode.options.is_empty());
        let mode_axis = self.render_mode_axis(false, running, cx);
        let perm_axis = (codex || acp_permission)
            .then(|| self.render_mode_axis(true, running, cx))
            .flatten();
        if mode_axis.is_none() && perm_axis.is_none() {
            return None;
        }
        Some(
            v_flex()
                .w_full()
                .min_w_0()
                .children(mode_axis)
                .children(perm_axis)
                .into_any_element(),
        )
    }

    fn render_mode_axis(
        &self,
        permission: bool,
        running: bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let picker = if permission {
            &self.mode_picker.permission
        } else {
            &self.mode_picker.mode
        };
        let has_options = picker.state.as_ref().is_some_and(|s| !s.options.is_empty());
        if !has_options && picker.error.is_none() {
            return None;
        }
        let heading = if permission {
            if self
                .session
                .as_ref()
                .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Acp))
            {
                tr("访问权限")
            } else {
                tr("访问权限")
            }
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
        Some(
            v_flex()
                .id(format!("mode-picker-content-{permission}"))
                .role(Role::Group)
                .aria_label(heading)
                .w_full()
                .min_w_0()
                .px_2p5()
                .py_2()
                .gap_1p5()
                .border_t_1()
                .border_color(Color::Divider.get(cx))
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Color::Muted.get(cx))
                                .child(heading),
                        )
                        .when(picker.error.is_some(), |row| {
                            row.child(
                                Button::new(format!("refresh-mode-{permission}"))
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(20.))
                                    .w(px(20.))
                                    .rounded_md()
                                    .icon(Glyph::RefreshCw)
                                    .tooltip(tr("重新读取"))
                                    .disabled(picker.loading || disabled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.load_mode_axis(permission, None, cx)
                                    })),
                            )
                        }),
                )
                .when_some(picker.error.as_ref(), |view, error| {
                    view.child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(Color::ErrorSurface.get(cx))
                            .text_size(px(11.5))
                            .text_color(Color::Error.get(cx))
                            .child(crate::language::localize_owned_error(error)),
                    )
                })
                .when_some(
                    picker.state.as_ref().filter(|s| !s.options.is_empty()),
                    |view, state| {
                        view.child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .gap(px(2.))
                                .p(px(2.))
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .children(state.options.iter().enumerate().map(|(i, option)| {
                                    let selected = state.selected.as_ref() == Some(&option.id)
                                        || (state.selected.is_none()
                                            && state.applied.as_ref() == Some(&option.id));
                                    let id = option.id.clone();
                                    let label = mode_label(&state.backend, option);
                                    let tooltip = option
                                        .description
                                        .clone()
                                        .unwrap_or_else(|| label.clone());
                                    Button::new(format!("mode-option-{permission}-{i}"))
                                        .custom(crate::palette::segment_variant(selected, cx))
                                        .compact()
                                        .xsmall()
                                        .flex_1()
                                        .min_w_0()
                                        .h(px(24.))
                                        .px_1p5()
                                        .rounded_md()
                                        .justify_center()
                                        .selected(selected)
                                        .disabled(picker.loading || disabled || blocked)
                                        .tooltip(tooltip)
                                        .accessibility_label(format!("{heading}: {label}"))
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(11.5))
                                                .font_weight(if selected {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::NORMAL
                                                })
                                                .text_color(if selected {
                                                    Color::Text.get(cx)
                                                } else {
                                                    Color::Muted.get(cx)
                                                })
                                                .child(label),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.load_mode_axis(permission, Some(id.clone()), cx)
                                        }))
                                })),
                        )
                    },
                )
                .into_any_element(),
        )
    }
}
