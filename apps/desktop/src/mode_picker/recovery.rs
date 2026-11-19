use super::{ModeOption, mode_label};
use crate::{app::HiveApp, language::tr, palette::Color};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct Recovery {
    working: bool,
    applied: bool,
    choices: Option<Choices>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Choices {
    session_version: u64,
    mode: Option<Axis>,
    permission_mode: Option<Axis>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Axis {
    saved: String,
    options: Vec<ModeOption>,
    can_inherit: bool,
    #[serde(skip)]
    selected: Option<Value>,
}

impl Choices {
    fn replacement(&self) -> Option<Value> {
        let mut replacement = serde_json::Map::new();
        for (key, axis) in [
            ("mode", &self.mode),
            ("permissionMode", &self.permission_mode),
        ] {
            if let Some(axis) = axis {
                replacement.insert(key.to_owned(), axis.selected.clone()?);
            }
        }
        (!replacement.is_empty()).then_some(Value::Object(replacement))
    }
}

impl HiveApp {
    pub(crate) fn needs_codex_mode_recovery(&self) -> bool {
        self.current_history_access()
            .filter(|_| self.conversation_is_read_only())
            .and_then(|access| access.identity_error.as_ref())
            .is_some_and(|error| error.code == "CODEX_SAVED_MODE_UNAVAILABLE")
    }

    pub(crate) fn can_check_saved_codex_modes(&self) -> bool {
        self.needs_codex_mode_recovery()
            || self
                .current_history_access()
                .is_some_and(|access| access.can_check_saved_codex_modes)
    }

    fn request_codex_mode_recovery(&mut self, apply: bool, cx: &mut Context<Self>) {
        if self.mode_picker.recovery.working
            || !self.can_check_saved_codex_modes()
            || !self.conversation_ready
            || !self.connected
            || self.loading
            || self.closing
            || (apply && !self.workspace_writes_allowed())
        {
            return;
        }
        let (Some(runtime), Some(session)) = (self.runtime.clone(), self.session.as_ref()) else {
            return;
        };
        let (id, workspace) = (session.id.clone(), session.workspace_id.clone());
        let connection = self.connection_generation;
        let revision = self.open_request;
        let mut params = json!({"workspaceId":workspace,"sessionId":id,"viewRevision":revision});
        let recovery = &mut self.mode_picker.recovery;
        let method = if apply {
            let Some(choices) = &recovery.choices else {
                return;
            };
            let Some(replacement) = choices.replacement() else {
                return;
            };
            params["sessionVersion"] = json!(choices.session_version);
            params["choices"] = replacement;
            "chat.codexModes.applyRecovery"
        } else {
            recovery.choices = None;
            "chat.codexModes.previewRecovery"
        };
        recovery.working = true;
        recovery.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await.and_then(|value| {
                if apply && value.get("ok").and_then(Value::as_bool) == Some(true) {
                    Ok(None)
                } else if apply {
                    Err(tr("后台未确认已保存替代模式。").to_owned())
                } else {
                    serde_json::from_value::<Choices>(value)
                        .map(Some)
                        .map_err(|error| error.to_string())
                }
            });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.open_request != revision
                    || !this
                        .session
                        .as_ref()
                        .is_some_and(|s| s.id == id && s.workspace_id == workspace)
                {
                    return;
                }
                let recovery = &mut this.mode_picker.recovery;
                recovery.working = false;
                match result {
                    Ok(Some(choices)) => recovery.choices = Some(choices),
                    Ok(None) => {
                        recovery.choices = None;
                        recovery.applied = true;
                    }
                    Err(error) => recovery.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn render_codex_mode_recovery(&self, cx: &Context<Self>) -> AnyElement {
        let recovery = &self.mode_picker.recovery;
        let disabled = recovery.working || !self.connected || self.closing || self.loading;
        let choices = recovery.choices.as_ref();
        let available_again =
            choices.is_some_and(|value| value.mode.is_none() && value.permission_mode.is_none());
        let can_reopen = recovery.applied || (available_again && self.needs_codex_mode_recovery());
        let mut view = v_flex().gap_1p5().px_1().child(
            div()
                .text_xs()
                .text_color(Color::Secondary.get(cx))
                .child(tr(
                    "检查当前 Codex 支持的模式。保存后请重新打开会话，历史和待发消息会保留。",
                )),
        );
        for (permission, axis) in [
            (false, choices.and_then(|v| v.mode.as_ref())),
            (true, choices.and_then(|v| v.permission_mode.as_ref())),
        ] {
            let Some(axis) = axis else { continue };
            let title = tr(if permission {
                "已保存的访问权限"
            } else {
                "已保存的协作模式"
            });
            let saved = mode_label(
                "codex",
                &ModeOption {
                    id: axis.saved.clone(),
                    label: axis.saved.clone(),
                    description: None,
                },
            );
            let mut options = h_flex().flex_wrap().gap_2();
            for (index, (choice, label)) in axis
                .options
                .iter()
                .map(|option| {
                    (
                        Value::String(option.id.clone()),
                        mode_label("codex", option),
                    )
                })
                .chain(
                    axis.can_inherit
                        .then(|| (Value::Null, tr("使用 Codex 默认协作设置").to_owned())),
                )
                .enumerate()
            {
                let button_group = if permission {
                    "recovery-permission"
                } else {
                    "recovery-mode"
                };
                options = options.child(
                    Button::new((button_group, index))
                        .outline()
                        .small()
                        .label(label)
                        .selected(axis.selected.as_ref() == Some(&choice))
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(choices) = this.mode_picker.recovery.choices.as_mut() {
                                let axis = if permission {
                                    &mut choices.permission_mode
                                } else {
                                    &mut choices.mode
                                };
                                if let Some(axis) = axis {
                                    axis.selected = Some(choice.clone());
                                    cx.notify();
                                }
                            }
                        })),
                );
            }
            view = view
                .child(div().text_xs().child(format!("{title} · {saved}")))
                .when(axis.options.is_empty() && !axis.can_inherit, |view| {
                    view.child(div().text_xs().text_color(Color::Error.get(cx)).child(tr(
                        "当前 Codex 没有可用的权限替代项。请检查会话使用的 Codex 配置。",
                    )))
                })
                .child(options);
        }
        view.when_some(recovery.error.as_ref(), |view, error| {
            view.child(
                div()
                    .text_xs()
                    .text_color(Color::Error.get(cx))
                    .child(error.clone()),
            )
        })
        .when(recovery.applied || available_again, |view| {
            let message = tr(if recovery.applied {
                "替代模式已保存。"
            } else {
                "已保存的模式已可用。"
            });
            view.child(
                div()
                    .id("codex-mode-recovery-saved")
                    .role(Role::Status)
                    .aria_label(message)
                    .text_xs()
                    .child(message),
            )
        })
        .child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .when(!can_reopen, |row| {
                    row.child(
                        Button::new("check-codex-modes")
                            .outline()
                            .small()
                            .label(tr(if recovery.working {
                                "正在检查 Codex…"
                            } else {
                                "检查可用模式"
                            }))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request_codex_mode_recovery(false, cx)
                            })),
                    )
                })
                .when(
                    choices.is_some() && !available_again && !can_reopen,
                    |row| {
                        row.child(
                            Button::new("apply-codex-mode-recovery")
                                .primary()
                                .small()
                                .label(tr("保存替代模式"))
                                .disabled(
                                    disabled
                                        || !self.workspace_writes_allowed()
                                        || choices.and_then(Choices::replacement).is_none(),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.request_codex_mode_recovery(true, cx)
                                })),
                        )
                    },
                )
                .when(can_reopen, |row| {
                    row.child(
                        Button::new("reopen-codex-mode-recovery")
                            .primary()
                            .small()
                            .label(tr("重新打开会话"))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(id) =
                                    this.session.as_ref().map(|session| session.id.clone())
                                {
                                    this.show_session(id, cx);
                                }
                            })),
                    )
                }),
        )
        .into_any_element()
    }
}
