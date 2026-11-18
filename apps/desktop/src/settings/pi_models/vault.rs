use super::{HiveApp, InputState, tr};
use crate::{focus_scroll::Reveal, palette::Color, text_reading::selectable_text};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Provider {
    provider: String,
    endpoint: String,
    #[serde(default)]
    stale: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    revision: String,
    config_revision: String,
    eligible: Vec<Provider>,
    configured: Vec<Provider>,
}

struct Editor {
    provider: String,
    reference: Entity<InputState>,
}

#[derive(Default)]
pub(in crate::settings) struct VaultSettings {
    visible: bool,
    loading: bool,
    generation: u64,
    snapshot: Option<Snapshot>,
    editor: Option<Editor>,
    notice: Option<Result<&'static str, String>>,
}

impl VaultSettings {
    pub(in crate::settings) fn close(&mut self) {
        *self = Self {
            generation: self.generation + 1,
            ..Self::default()
        };
    }
}

impl HiveApp {
    fn pi_vault_request(
        &mut self,
        change: Option<(String, Option<String>)>,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.pi_models.vault;
        if !self.connected || !state.visible || state.loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let (method, params, confirmation) = match (change, &state.snapshot) {
            (Some((provider, reference)), Some(snapshot)) => (
                "pi.vault.change",
                json!({"revision":snapshot.revision,"configRevision":snapshot.config_revision,"provider":provider,"reference":reference}),
                Some(if reference.is_some() {
                    "1Password 引用已保存。请回到会话刷新模型。"
                } else {
                    "1Password 引用已移除。请回到会话刷新模型。"
                }),
            ),
            (None, _) => ("pi.vault.get", json!({}), None),
            _ => return,
        };
        state.loading = true;
        state.notice = None;
        let generation = state.generation;
        let connection = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await.and_then(|value| {
                serde_json::from_value::<Snapshot>(value).map_err(|error| error.to_string())
            });
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.pi_models.vault;
                if state.generation != generation || this.connection_generation != connection {
                    return;
                }
                state.loading = false;
                match result {
                    Ok(snapshot) => {
                        state.snapshot = Some(snapshot);
                        state.editor = None;
                        state.notice = confirmation.map(Ok);
                    }
                    Err(error) => state.notice = Some(Err(error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_pi_vault(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.pi_models.vault;
        let disabled = !self.connected || state.loading;
        let mut card = super::super::settings_card(cx).gap_3().p_3().child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .items_center()
                .child(
                    Button::new("pi-vault-toggle")
                        .outline()
                        .small()
                        .label(if state.visible {
                            tr("关闭 1Password 设置")
                        } else {
                            tr("配置 1Password")
                        })
                        .disabled(!self.connected)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.pi_models.vault.visible {
                                this.pi_models.vault.close();
                                cx.notify();
                            } else {
                                this.pi_models.vault.visible = true;
                                this.pi_vault_request(None, cx);
                            }
                        })),
                )
                .when(state.visible, |row| {
                    row.child(
                        Button::new("pi-vault-refresh")
                            .ghost()
                            .small()
                            .disabled(disabled)
                            .label(if state.loading {
                                tr("处理中…")
                            } else {
                                tr("重新读取列表")
                            })
                            .on_click(
                                cx.listener(|this, _, _, cx| this.pi_vault_request(None, cx)),
                            ),
                    )
                }),
        );
        if !state.visible {
            return card;
        }
        card = card.child(
            selectable_text("pi-vault-description", tr(
                "需安装 1Password CLI 并启用桌面应用集成。仅保存 op:// 引用，用于默认内置 Pi 的自定义 OpenAI 兼容接口。",
            )).text_sm().text_color(Color::Secondary.get(cx)),
        );
        if let Some(notice) = &state.notice {
            let (body, error) = match notice {
                Ok(key) => (tr(key).to_owned(), false),
                Err(error) => (crate::language::localize_owned_error(error), true),
            };
            card = card.child(super::super::settings_notice_box(
                "pi-vault-notice",
                body,
                error,
                cx,
            ));
            if !error
                && self.session.as_ref().is_some_and(|session| {
                    matches!(session.backend, crate::protocol::Backend::Pi)
                        && !self.busy.contains(&session.id)
                })
                && self.conversation_ready
                && !self.conversation_is_read_only()
            {
                card = card.child(
                    h_flex().child(
                        Button::new("pi-vault-refresh-session")
                            .primary()
                            .small()
                            .label(tr("返回会话并刷新模型"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.leave_preferences(window, cx);
                                this.refresh_pi_models(cx);
                            })),
                    ),
                );
            }
        }
        let Some(snapshot) = &state.snapshot else {
            return card;
        };
        if snapshot.eligible.is_empty() && snapshot.configured.is_empty() {
            return card.child(
                selectable_text(
                    "pi-vault-no-providers",
                    tr("先添加未设置其他认证的自定义 OpenAI 兼容接口。"),
                )
                .text_sm(),
            );
        }
        for provider in snapshot
            .eligible
            .iter()
            .chain(snapshot.configured.iter().filter(|bound| {
                !snapshot
                    .eligible
                    .iter()
                    .any(|item| item.provider == bound.provider)
            }))
        {
            let id = &provider.provider;
            let configured = snapshot.configured.iter().find(|item| item.provider == *id);
            let eligible = snapshot.eligible.iter().any(|item| item.provider == *id);
            let mut row = v_flex()
                .min_w_0()
                .gap_2()
                .p_2()
                .border_t_1()
                .border_color(Color::Divider.get(cx))
                .child(
                    selectable_text(
                        SharedString::from(format!("pi-vault-provider-{id}")),
                        id.clone(),
                    )
                    .text_sm(),
                )
                .child(
                    selectable_text(
                        SharedString::from(format!("pi-vault-endpoint-{id}")),
                        provider.endpoint.clone(),
                    )
                    .text_sm()
                    .text_color(Color::Secondary.get(cx)),
                );
            if let Some(configured) = configured {
                row = row.child(
                    selectable_text(
                        SharedString::from(format!("pi-vault-status-{id}")),
                        if configured.stale {
                            tr("接口配置已变更，请重新关联引用。")
                        } else {
                            tr("1Password 引用已关联")
                        },
                    )
                    .text_sm(),
                );
            }
            let edit_id = id.clone();
            let remove_id = id.clone();
            row = row.child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .when(eligible, |controls| {
                        controls.child(
                            Button::new(SharedString::from(format!("bind-pi-vault-{id}")))
                                .outline()
                                .small()
                                .disabled(disabled)
                                .label(tr(if configured.is_some() {
                                    "更换引用"
                                } else {
                                    "关联引用"
                                }))
                                .accessibility_label(format!(
                                    "{}: {id}",
                                    tr(if configured.is_some() {
                                        "更换引用"
                                    } else {
                                        "关联引用"
                                    })
                                ))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let reference = cx.new(|cx| {
                                        InputState::new(window, cx)
                                            .placeholder("op://Vault/Item/field")
                                    });
                                    reference.focus_handle(cx).focus(window, cx);
                                    this.pi_models.vault.editor = Some(Editor {
                                        provider: edit_id.clone(),
                                        reference,
                                    });
                                    this.pi_models.vault.notice = None;
                                    cx.notify();
                                })),
                        )
                    })
                    .when(configured.is_some(), |controls| {
                        controls.child(
                            Button::new(SharedString::from(format!("remove-pi-vault-{id}")))
                                .ghost()
                                .small()
                                .disabled(disabled)
                                .label(tr("移除引用"))
                                .accessibility_label(format!("{}: {id}", tr("移除引用")))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.pi_vault_request(Some((remove_id.clone(), None)), cx);
                                })),
                        )
                    }),
            );
            if let Some(editor) = state
                .editor
                .as_ref()
                .filter(|editor| editor.provider == *id)
            {
                let input = editor.reference.clone();
                let focus = input.focus_handle(cx);
                let save_id = id.clone();
                row = row.child(
                    Reveal::new(
                        "pi-vault-reference-reveal",
                        &self.pi_models.scroll,
                        v_flex()
                            .gap_2()
                            .child(
                                Input::new(&input)
                                    .aria_label(tr("1Password 字段引用"))
                                    .disabled(disabled),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .flex_wrap()
                                    .child(
                                        Button::new("save-pi-vault-reference")
                                            .primary()
                                            .small()
                                            .disabled(disabled)
                                            .label(tr("保存此接口的引用"))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                let reference =
                                                    input.read(cx).value().trim().to_owned();
                                                this.pi_vault_request(
                                                    Some((save_id.clone(), Some(reference))),
                                                    cx,
                                                );
                                            })),
                                    )
                                    .child(
                                        Button::new("cancel-pi-vault-reference")
                                            .ghost()
                                            .small()
                                            .disabled(disabled)
                                            .label(tr("取消"))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.pi_models.vault.editor = None;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .track_focus(&focus),
                );
            }
            card = card.child(row);
        }
        card
    }
}
