use crate::{
    app::{ErrorNotice, HiveApp},
    attachments::AttachmentRef,
    keybindings::{self, Target},
    language::tr,
    palette,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::{Deserialize, Serialize};

pub(crate) const MAX_STASH_ENTRIES: usize = 20;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StashedDraft {
    pub id: String,
    pub saved_at: i64,
    pub text: String,
    pub attachments: Vec<AttachmentRef>,
}

#[derive(Clone, Copy)]
pub(crate) enum StashAction {
    Save,
    Restore(uuid::Uuid),
    Remove(uuid::Uuid),
}

#[derive(Clone, PartialEq, Eq)]
struct Owner {
    key: String,
    workspace: String,
    connection: u64,
    view: u64,
}

impl StashedDraft {
    fn summary(&self) -> String {
        let text = self.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            self.attachments
                .first()
                .map_or_else(String::new, |file| file.name.clone())
        } else if text.chars().count() > 90 {
            format!("{}…", text.chars().take(90).collect::<String>())
        } else {
            text
        }
    }

    fn details(&self) -> String {
        let saved_at = chrono::DateTime::from_timestamp_millis(self.saved_at)
            .map(|date| {
                date.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_default();
        if self.attachments.is_empty() {
            return saved_at;
        }
        let files = self
            .attachments
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("{saved_at} · {files}")
    }
}

impl HiveApp {
    fn stash_owner(&self) -> Option<Owner> {
        let workspace = self.workspace.as_ref()?;
        let key = self.draft_target.as_ref()?;
        let expected = if let Some(session) = &self.session {
            if !self.conversation_ready
                || &session.workspace_id != workspace
                || self.inflight_inputs.contains_key(&session.id)
                || self.conversation_is_read_only()
            {
                return None;
            }
            format!("session:{}", session.id)
        } else {
            format!("workspace:{workspace}")
        };
        (key == &expected
            && self.draft_owner.as_ref() == Some(key)
            && self.connected
            && !self.loading
            && !self.closing
            && !self.settings_panel
            && !self.roles.visible
            && !self.showing_welcome_guide
            && !self.collaboration.visible
            && self.workspace_writes_allowed()
            && self.new_session_creation.is_none()
            && self.draft_store.is_some())
        .then(|| Owner {
            key: key.clone(),
            workspace: workspace.clone(),
            connection: self.connection_generation,
            view: self.open_request,
        })
    }

    fn has_stash_input(&self, owner: &Owner, cx: &App) -> bool {
        !self.input.read(cx).value().is_empty()
            || !self.attachments.refs(Some(&owner.key)).is_empty()
    }

    fn change_prompt_stash(
        &mut self,
        owner: &Owner,
        action: StashAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), ErrorNotice> {
        if self.stash_owner().as_ref() != Some(owner) {
            return Err("输入已切换，请重新打开草稿暂存".into());
        }
        if self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return Err("请完成当前输入后再操作草稿暂存".into());
        }
        if self.attachments.loading.contains(&owner.key)
            || matches!(action, StashAction::Save) && !self.attachments.ready(Some(&owner.key))
        {
            return Err("请等待附件处理完成后再暂存草稿".into());
        }
        let text = self.input.read(cx).value().to_string();
        let attachments = self.attachments.refs(Some(&owner.key));
        if matches!(action, StashAction::Restore(_))
            && (!text.is_empty() || !attachments.is_empty())
        {
            return Err("请先清空输入框，再恢复暂存的草稿".into());
        }
        if attachments
            .iter()
            .any(|item| item.workspace_id != owner.workspace)
        {
            return Err("暂存附件不属于当前工作区".into());
        }
        let failed = owner
            .key
            .strip_prefix("session:")
            .and_then(|id| self.failed_drafts.get(id))
            .cloned()
            .unwrap_or_default();
        let store = self
            .draft_store
            .as_mut()
            .expect("verified composer draft store");
        if let StashAction::Restore(id) = action {
            let id = id.to_string();
            if store
                .stashed(&owner.key)
                .iter()
                .find(|entry| entry.id == id)
                .is_some_and(|entry| {
                    entry
                        .attachments
                        .iter()
                        .any(|file| file.workspace_id != owner.workspace)
                })
            {
                return Err("暂存附件不属于当前工作区".into());
            }
        }
        if matches!(action, StashAction::Save)
            && store.stashed(&owner.key).len() >= MAX_STASH_ENTRIES
        {
            return Err("草稿暂存已满，请先移除或恢复一条".into());
        }
        let (text, attachments) = store
            .change_stash(&owner.key, action, text, attachments, failed)
            .map_err(|error| ErrorNotice::Detail("无法更新草稿暂存", error.to_string()))?;
        if !matches!(action, StashAction::Remove(_)) {
            if text.is_empty() {
                self.drafts.remove(&owner.key);
            } else {
                self.drafts.insert(owner.key.clone(), text.clone());
            }
            if attachments.is_empty() {
                self.attachments.drafts.remove(&owner.key);
            } else {
                self.attachments
                    .drafts
                    .insert(owner.key.clone(), attachments);
            }
            self.input.update(cx, |input, cx| {
                let end = text.len();
                input.set_value(text, window, cx);
                input.set_selected_range(end..end, cx);
            });
            self.prune_draft_attachments(cx);
        }
        cx.notify();
        Ok(())
    }

    pub(crate) fn prompt_stash_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) || !self.input.read(cx).focus_handle(cx).is_focused(window)
        {
            return;
        }
        let Some(owner) = self.stash_owner() else {
            return;
        };
        let action = if self.has_stash_input(&owner, cx) {
            StashAction::Save
        } else {
            let entries = self.draft_store.as_ref().unwrap().stashed(&owner.key);
            if let [entry] = entries
                && let Ok(id) = uuid::Uuid::parse_str(&entry.id)
            {
                StashAction::Restore(id)
            } else {
                self.open_prompt_stash(window, cx);
                return;
            }
        };
        if let Err(error) = self.change_prompt_stash(&owner, action, window, cx) {
            self.error = Some(error);
            cx.notify();
        }
    }

    pub(crate) fn render_prompt_stash_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = self.stash_owner();
        let count = owner
            .as_ref()
            .and_then(|owner| {
                self.draft_store
                    .as_ref()
                    .map(|store| store.stashed(&owner.key).len())
            })
            .unwrap_or(0);
        Button::new("prompt-stash")
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(28.))
            .when(count == 0, |button| button.w(px(28.)))
            .rounded_lg()
            .icon(Glyph::Archive)
            .when(count > 0, |button| button.label(count.to_string()))
            .tooltip(keybindings::hint(Target::PromptStash, "草稿暂存", cx))
            .accessibility_label(tr("草稿暂存"))
            .disabled(owner.is_none())
            .on_click(cx.listener(|this, _, window, cx| this.open_prompt_stash(window, cx)))
    }

    fn open_prompt_stash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        let Some(owner) = self.stash_owner() else {
            return;
        };
        let weak = cx.weak_entity();
        let focus = self.input.read(cx).focus_handle(cx);
        let notice = cx.new(|_| None::<ErrorNotice>);
        window.open_dialog(cx, move |dialog, _, cx| {
            let app = weak.upgrade();
            let current = app
                .as_ref()
                .filter(|app| app.read(cx).stash_owner().as_ref() == Some(&owner));
            let entries = current
                .and_then(|app| app.read(cx).draft_store.as_ref())
                .map_or(&[][..], |store| store.stashed(&owner.key));
            let has_input = current.is_some_and(|app| app.read(cx).has_stash_input(&owner, cx));
            let full = entries.len() >= MAX_STASH_ENTRIES;
            let action = |event: StashAction| {
                let (weak, owner, focus, notice) =
                    (weak.clone(), owner.clone(), focus.clone(), notice.clone());
                move |_: &ClickEvent, window: &mut Window, cx: &mut App| match weak
                    .update(cx, |app, cx| {
                        app.change_prompt_stash(&owner, event, window, cx)
                    }) {
                    Ok(Ok(())) => {
                        window.close_dialog(cx);
                        focus.focus(window, cx);
                    }
                    Ok(Err(error)) => {
                        notice.update(cx, |notice, cx| {
                            *notice = Some(error);
                            cx.notify();
                        });
                        window.refresh();
                    }
                    Err(_) => window.close_dialog(cx),
                }
            };
            dialog.title(tr("草稿暂存")).w(px(480.)).child(
                v_flex()
                    .gap_3()
                    .when_some(
                        notice.read(cx).as_ref().map(ErrorNotice::text),
                        |body, error| {
                            body.child(
                                div()
                                    .id("prompt-stash-error")
                                    .role(Role::Status)
                                    .whitespace_normal()
                                    .child(error),
                            )
                        },
                    )
                    .child(
                        Button::new("stash-current-draft")
                            .primary()
                            .label(tr("暂存草稿"))
                            .disabled(!has_input || full)
                            .on_click(action(StashAction::Save)),
                    )
                    .when(full, |body| {
                        body.child(tr("草稿暂存已满，请先移除或恢复一条"))
                    })
                    .when(entries.is_empty(), |body| body.child(tr("暂无暂存的草稿")))
                    .when(has_input && !entries.is_empty(), |body| {
                        body.child(tr("请先清空输入框，再恢复暂存的草稿"))
                    })
                    .child(
                        v_flex()
                            .id("stashed-drafts")
                            .max_h(px(300.))
                            .overflow_y_scroll()
                            .gap_2()
                            .children(entries.iter().filter_map(|entry| {
                                let id = uuid::Uuid::parse_str(&entry.id).ok()?;
                                let summary = entry.summary();
                                Some(
                                    v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .whitespace_normal()
                                                .child(summary.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .whitespace_normal()
                                                .child(entry.details()),
                                        )
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .child(
                                                    Button::new(SharedString::from(format!(
                                                        "restore-stashed-draft-{}",
                                                        entry.id
                                                    )))
                                                    .compact()
                                                    .label(tr("恢复草稿"))
                                                    .accessibility_label(format!(
                                                        "{}: {summary}",
                                                        tr("恢复草稿")
                                                    ))
                                                    .disabled(has_input)
                                                    .on_click(action(StashAction::Restore(id))),
                                                )
                                                .child(
                                                    Button::new(SharedString::from(format!(
                                                        "remove-stashed-draft-{}",
                                                        entry.id
                                                    )))
                                                    .compact()
                                                    .label(tr("丢弃暂存"))
                                                    .accessibility_label(format!(
                                                        "{}: {summary}",
                                                        tr("丢弃暂存")
                                                    ))
                                                    .on_click(action(StashAction::Remove(id))),
                                                ),
                                        ),
                                )
                            })),
                    ),
            )
        });
    }
}
