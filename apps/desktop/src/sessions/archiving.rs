use super::*;
use crate::language::{english, localize_owned_error, tr};
use gpui_kit::component::{
    Sizable as _, WindowExt as _,
    button::Button,
    notification::{Notification, NotificationDelivery},
};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

const UNDO_TIME: Duration = Duration::from_secs(5);
struct ArchiveUndoToast;
const INBOX_ARCHIVE_STALE: &str = "收件箱中的会话已变化，请刷新后重试。";

type InboxArchiveCheck = Rc<dyn Fn(&HiveApp, &App) -> bool>;

#[derive(Clone)]
struct InboxArchive {
    reply_revision: u64,
    nested: bool,
    still_available: InboxArchiveCheck,
}

enum ArchiveAction {
    Ordinary,
    RestoreInbox,
    OfferUndo {
        window: AnyWindowHandle,
        viewed: Option<u64>,
        inbox_reply: Option<u64>,
        global_inbox: bool,
    },
    Undo {
        window: AnyWindowHandle,
        version: u64,
        return_to_session: Option<u64>,
        restore_inbox_unread: bool,
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
        self.confirm_session_archive(session, None, window, cx);
    }

    pub(crate) fn request_inbox_session_archive(
        &mut self,
        session: Session,
        reply_revision: u64,
        nested: bool,
        still_available: impl Fn(&HiveApp, &App) -> bool + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_session_archive(
            session,
            Some(InboxArchive {
                reply_revision,
                nested,
                still_available: Rc::new(still_available),
            }),
            window,
            cx,
        );
    }

    fn confirm_session_archive(
        &mut self,
        session: Session,
        inbox: Option<InboxArchive>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if inbox
            .as_ref()
            .is_some_and(|inbox| !(inbox.still_available)(self, cx))
        {
            return;
        }
        if !crate::general::current(cx).confirm_archive_session {
            self.archive_with_undo(session, window.window_handle(), inbox.as_ref(), cx);
            return;
        }
        if !self.connected
            || self.closing
            || self.session_pin_loading.contains(&session.id)
            || self.session_archive_loading.contains(&session.id)
            || self.workspace_removing.as_ref() == Some(&session.workspace_id)
            || (window.has_active_dialog(cx) && !inbox.as_ref().is_some_and(|inbox| inbox.nested))
        {
            return;
        }
        let connection = self.connection_generation;
        let parent = window.window_handle();
        let weak = cx.weak_entity();
        let title = &session.title;
        let description = if inbox.is_some() {
            let Some(project) = self
                .workspaces
                .iter()
                .find(|item| item.id == session.workspace_id)
            else {
                return;
            };
            let name = if self
                .workspaces
                .iter()
                .filter(|item| item.name == project.name)
                .count()
                > 1
            {
                &project.path
            } else {
                &project.name
            };
            let template =
                tr("项目：{project}。确定归档会话“{title}”吗？归档后可在设置的“归档会话”中恢复。");
            let (before_project, after_project) =
                template.split_once("{project}").unwrap_or(("", template));
            let (before_title, after_title) = after_project
                .split_once("{title}")
                .unwrap_or((after_project, ""));
            format!("{before_project}{name}{before_title}{title}{after_title}")
        } else if english() {
            format!(
                "Archive \"{title}\"? You can restore it anytime from Archived Sessions in Settings."
            )
        } else {
            format!("确定归档会话“{title}”吗？归档后可随时在设置的“归档会话”中恢复。")
        };
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let confirm_session = session.clone();
            let inbox = inbox.clone();
            dialog
                .title(tr("归档会话？"))
                .description(description.clone())
                .show_cancel(true)
                .ok_text(tr("归档会话"))
                .cancel_text(tr("取消"))
                .on_ok(move |_, window, cx| {
                    weak.update(cx, |this, cx| {
                        if this.connection_generation != connection {
                            return;
                        }
                        if inbox
                            .as_ref()
                            .is_some_and(|inbox| !(inbox.still_available)(this, cx))
                        {
                            window.push_notification(
                                Notification::error(tr(INBOX_ARCHIVE_STALE))
                                    .delivery(NotificationDelivery::InApp),
                                cx,
                            );
                            return;
                        }
                        this.archive_with_undo(confirm_session.clone(), parent, inbox.as_ref(), cx);
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

    pub(crate) fn restore_inbox_session(&mut self, session: Session, cx: &mut Context<Self>) {
        if session.archived
            && self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == session.workspace_id)
        {
            self.change_session_archive(session, false, ArchiveAction::RestoreInbox, cx);
        }
    }

    fn archive_with_undo(
        &mut self,
        session: Session,
        window: AnyWindowHandle,
        inbox: Option<&InboxArchive>,
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
            ArchiveAction::OfferUndo {
                window,
                viewed,
                inbox_reply: inbox.map(|value| value.reply_revision),
                global_inbox: inbox.is_some_and(|value| value.nested),
            },
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
        let restore_inbox = matches!(&action, ArchiveAction::RestoreInbox);
        if restore_inbox {
            params["restoreWorkspaceId"] = json!(workspace);
        }
        let inbox_window = match &action {
            ArchiveAction::OfferUndo {
                window,
                inbox_reply: Some(reply_revision),
                ..
            } => {
                params["inbox"] = json!({"workspaceId":workspace,"replyRevision":reply_revision});
                Some(*window)
            }
            _ => None,
        };
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
                        if restore_inbox {
                            this.show_inbox_restore_error(&workspace, &id, &error, cx);
                        }
                        if let Some(window) = inbox_window {
                            let message = if error.contains("ARCHIVE_INBOX_STALE") {
                                tr(INBOX_ARCHIVE_STALE).to_owned()
                            } else {
                                localize_owned_error(&error)
                            };
                            let _ = window.update(cx, |_, window, cx| {
                                window.push_notification(
                                    Notification::error(message)
                                        .delivery(NotificationDelivery::InApp),
                                    cx,
                                );
                            });
                        }
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
                        if restore_inbox {
                            this.show_inbox_restore_error(
                                &workspace,
                                &id,
                                tr(INBOX_ARCHIVE_STALE),
                                cx,
                            );
                        }
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
                            this.finish_archive_action(
                                action,
                                saved.session.clone(),
                                mutation.version,
                                cx,
                            );
                            this.apply_inbox_archive_session(&saved.session, cx);
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
            ArchiveAction::Ordinary | ArchiveAction::RestoreInbox => (),
            ArchiveAction::OfferUndo {
                window,
                viewed,
                inbox_reply,
                global_inbox,
            } => {
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
                self.offer_archive_undo(
                    window,
                    session,
                    version,
                    return_to_session,
                    inbox_reply.is_some(),
                    global_inbox,
                    cx,
                );
            }
            ArchiveAction::Undo {
                window,
                return_to_session,
                restore_inbox_unread,
                ..
            } => {
                if restore_inbox_unread
                    && !self
                        .session
                        .as_ref()
                        .is_some_and(|current| current.id == session.id)
                {
                    self.unread.insert(session.id.clone());
                }
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
        restore_inbox_unread: bool,
        global_inbox: bool,
        cx: &mut Context<Self>,
    ) {
        let connection = self.connection_generation;
        let app = cx.weak_entity();
        let expires = Instant::now() + UNDO_TIME;
        let title = session.title.clone();
        let apply = Rc::new(move |window: &mut Window, cx: &mut App| {
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
                            restore_inbox_unread,
                        },
                        cx,
                    );
                }
            });
        });
        if global_inbox && self.offer_inbox_archive_undo(title.clone(), expires, apply.clone(), cx)
        {
            return;
        }
        let key = format!("{connection}:{version}");
        let action_key = key.clone();
        let toast = Notification::success(tr("会话已归档"))
            .id1::<ArchiveUndoToast>(key.clone())
            .title(title)
            .delivery(NotificationDelivery::InApp)
            .action(move |_, _, _| {
                let apply = apply.clone();
                let key = action_key.clone();
                Button::new("undo-session-archive")
                    .compact()
                    .small()
                    .label(tr("撤销归档"))
                    .on_click(move |_, window, cx| {
                        window.remove_notification1::<ArchiveUndoToast>(key.clone(), cx);
                        apply(window, cx);
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
