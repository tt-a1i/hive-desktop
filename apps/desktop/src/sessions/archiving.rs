use super::*;
use crate::language::{english, tr};
use gpui_kit::component::{
    Sizable as _, WindowExt as _,
    button::Button,
    notification::{Notification, NotificationDelivery},
};
use std::time::{Duration, Instant};

const UNDO_TIME: Duration = Duration::from_secs(5);
struct ArchiveUndoToast;

enum ArchiveAction {
    Ordinary,
    OfferUndo {
        window: AnyWindowHandle,
        viewed: Option<u64>,
    },
    Undo {
        window: AnyWindowHandle,
        version: u64,
        return_to_session: Option<u64>,
    },
}

#[derive(serde::Deserialize)]
struct ArchiveResult {
    session: Session,
    mutation: Option<ArchiveMutation>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveMutation {
    version: u64,
    event_sequence: u64,
}

impl HiveApp {
    pub(crate) fn request_session_archived(
        &mut self,
        session: Session,
        archived: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !archived {
            self.set_session_archived(session, archived, cx);
            return;
        }
        if !crate::general::current(cx).confirm_archive_session {
            self.archive_with_undo(session, window.window_handle(), cx);
            return;
        }
        if !self.connected
            || self.closing
            || self.session_pin_loading.contains(&session.id)
            || self.session_archive_loading.contains(&session.id)
            || self.workspace_removing.as_ref() == Some(&session.workspace_id)
            || window.has_active_dialog(cx)
        {
            return;
        }
        let connection = self.connection_generation;
        let parent = window.window_handle();
        let weak = cx.weak_entity();
        let title = session.title.clone();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let confirm_session = session.clone();
            dialog
                .title(tr("归档会话？"))
                .description(if english() {
                    format!(
                        "Archive \"{title}\"? You can restore it anytime from Archived Sessions in Settings."
                    )
                } else {
                    format!(
                        "确定归档会话“{title}”吗？归档后可随时在设置的“归档会话”中恢复。"
                    )
                })
                .show_cancel(true)
                .ok_text(tr("归档会话"))
                .cancel_text(tr("取消"))
                .on_ok(move |_, _, cx| {
                    weak.update(cx, |this, cx| {
                        if this.connection_generation == connection {
                            this.archive_with_undo(confirm_session.clone(), parent, cx);
                        }
                    })
                    .is_ok()
                })
        });
    }

    pub(crate) fn set_session_archived(
        &mut self,
        session: Session,
        archived: bool,
        cx: &mut Context<Self>,
    ) {
        self.change_session_archive(session, archived, ArchiveAction::Ordinary, cx);
    }

    fn archive_with_undo(
        &mut self,
        session: Session,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let viewed = (self.viewing_conversation()
            && self.session.as_ref().is_some_and(|viewed| {
                viewed.id == session.id
                    && viewed.workspace_id == session.workspace_id
                    && !viewed.archived
            }))
        .then_some(self.open_request);
        self.change_session_archive(
            session,
            true,
            ArchiveAction::OfferUndo { window, viewed },
            cx,
        );
    }

    fn change_session_archive(
        &mut self,
        session: Session,
        archived: bool,
        action: ArchiveAction,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self.closing
            || self.session_pin_loading.contains(&session.id)
            || self.session_archive_loading.contains(&session.id)
            || self.workspace_removing.as_ref() == Some(&session.workspace_id)
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let id = session.id;
        let workspace = session.workspace_id;
        let connection = self.connection_generation;
        let mut params = json!({"sessionId":id,"archived":archived});
        if let ArchiveAction::Undo { version, .. } = &action {
            params["undo"] = json!({"workspaceId":workspace,"version":version});
        }
        self.session_archive_loading.insert(id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.archive", params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<ArchiveResult>(value)
                        .map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.session_archive_loading.remove(&id);
                match result {
                    Err(error) => {
                        this.error = Some(crate::app::ErrorNotice::Detail(
                            "更新会话归档失败",
                            error.to_string(),
                        ))
                    }
                    Ok(saved)
                        if saved.session.id != id
                            || saved.session.workspace_id != workspace
                            || saved.session.archived != archived =>
                    {
                        this.error = Some("归档响应中的会话身份或状态不匹配，请刷新核对。".into())
                    }
                    Ok(saved) => {
                        if this.settings_panel
                            && this.settings_section == crate::app::SettingsSection::Archive
                        {
                            this.load_archived_settings(false, cx);
                        }
                        if let Some(mutation) = saved.mutation
                            && this
                                .background_sequences
                                .get(&id)
                                .copied()
                                .unwrap_or_default()
                                <= mutation.event_sequence
                            && !this.pending_events.iter().any(|event| {
                                event.session_id() == id
                                    && event.sequence() > mutation.event_sequence
                            })
                            && !this.removed_sessions.contains(&id)
                            && this.workspace_removing.as_ref() != Some(&workspace)
                            && this.workspaces.iter().any(|entry| entry.id == workspace)
                        {
                            this.finish_archive_action(action, saved.session, mutation.version, cx);
                        }
                    }
                }
                // Ordered navigation events carry the saved state. Do not apply
                // an older RPC acknowledgement over a newer incoming delivery.
                cx.notify();
            });
        })
        .detach();
    }

    fn finish_archive_action(
        &mut self,
        action: ArchiveAction,
        session: Session,
        version: u64,
        cx: &mut Context<Self>,
    ) {
        match action {
            ArchiveAction::Ordinary => (),
            ArchiveAction::OfferUndo { window, viewed } => {
                let return_to_session = if viewed == Some(self.open_request)
                    && self.workspace.as_ref() == Some(&session.workspace_id)
                    && self
                        .session
                        .as_ref()
                        .is_some_and(|current| current.id == session.id)
                    && self.viewing_conversation()
                {
                    self.remember_conversation_position(cx);
                    self.clear_active_session_only(cx);
                    self.switch_draft(Some(format!("workspace:{}", session.workspace_id)), cx);
                    if self.navigation.sessions.get(&session.workspace_id) == Some(&session.id) {
                        self.navigation.sessions.remove(&session.workspace_id);
                        self.save_navigation(cx);
                    }
                    self.loading = false;
                    self.sync_git_workspace(cx);
                    Some(self.open_request)
                } else {
                    None
                };
                self.offer_archive_undo(window, session, version, return_to_session, cx);
            }
            ArchiveAction::Undo {
                window,
                return_to_session,
                ..
            } => {
                if return_to_session == Some(self.open_request)
                    && self.workspace.as_ref() == Some(&session.workspace_id)
                    && self.session.is_none()
                    && self.viewing_conversation()
                    && window
                        .update(cx, |_, window, cx| !window.has_active_dialog(cx))
                        .unwrap_or(false)
                {
                    self.show_session(session.id, cx);
                }
            }
        }
    }

    fn offer_archive_undo(
        &self,
        window: AnyWindowHandle,
        session: Session,
        version: u64,
        return_to_session: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        let connection = self.connection_generation;
        let app = cx.weak_entity();
        let expires = Instant::now() + UNDO_TIME;
        let key = format!("{connection}:{version}");
        let action_key = key.clone();
        let toast = Notification::success(tr("会话已归档"))
            .id1::<ArchiveUndoToast>(key.clone())
            .title(session.title.clone())
            .delivery(NotificationDelivery::InApp)
            .action(move |_, _, _| {
                let app = app.clone();
                let session = session.clone();
                let key = action_key.clone();
                Button::new("undo-session-archive")
                    .compact()
                    .small()
                    .label(tr("撤销归档"))
                    .on_click(move |_, window, cx| {
                        window.remove_notification1::<ArchiveUndoToast>(key.clone(), cx);
                        if Instant::now() >= expires {
                            return;
                        }
                        let _ = app.update(cx, |this, cx| {
                            if this.connection_generation == connection {
                                this.change_session_archive(
                                    session.clone(),
                                    false,
                                    ArchiveAction::Undo {
                                        window: window.window_handle(),
                                        version,
                                        return_to_session,
                                    },
                                    cx,
                                );
                            }
                        });
                    })
            });
        if window
            .update(cx, |_, window, cx| window.push_notification(toast, cx))
            .is_err()
        {
            return;
        }
        let timer = cx.background_executor().timer(UNDO_TIME);
        cx.spawn(async move |_, cx| {
            timer.await;
            let _ = window.update(cx, |_, window, cx| {
                window.remove_notification1::<ArchiveUndoToast>(key, cx)
            });
        })
        .detach();
    }

    pub(crate) fn apply_archive_navigation(
        &mut self,
        id: &str,
        archived: bool,
        pinned: bool,
        updated_at: i64,
        cx: &mut Context<Self>,
    ) {
        for session in self
            .sessions
            .iter_mut()
            .chain(self.project_sessions.values_mut().flatten())
            .chain(self.session.iter_mut())
        {
            if session.id == id {
                session.archived = archived;
                session.updated_at = updated_at;
            }
        }
        let order = |a: &Session, b: &Session| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| {
                    if a.pinned && b.pinned {
                        a.pin_rank
                            .unwrap_or(i64::MAX)
                            .cmp(&b.pin_rank.unwrap_or(i64::MAX))
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        };
        self.sessions.sort_by(order);
        for sessions in self.project_sessions.values_mut() {
            sessions.sort_by(order);
        }
        for hit in &mut self.pinned_sessions.records {
            if hit.session.id == id {
                hit.session.archived = archived;
                hit.session.updated_at = updated_at;
            }
        }
        if archived {
            self.unread.remove(id);
            self.pinned_sessions
                .records
                .retain(|hit| hit.session.id != id);
        }
        if pinned {
            self.refresh_pinned_sessions(cx);
        }
        if self.settings_panel && self.settings_section == crate::app::SettingsSection::Archive {
            self.load_archived_settings(false, cx);
        }
        cx.notify();
    }
}
