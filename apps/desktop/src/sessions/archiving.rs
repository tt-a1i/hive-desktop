use super::*;
use crate::language::{english, tr};
use gpui_kit::component::WindowExt as _;

#[derive(serde::Deserialize)]
struct ArchiveResult {
    session: Session,
}

impl HiveApp {
    pub(crate) fn request_session_archived(
        &mut self,
        session: Session,
        archived: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !archived || !crate::general::current(cx).confirm_archive_session {
            self.set_session_archived(session, archived, cx);
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
                            this.set_session_archived(confirm_session.clone(), true, cx);
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
        self.session_archive_loading.insert(id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.archive", json!({"sessionId":id,"archived":archived}))
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
                    Err(error) => this.error = Some(crate::app::ErrorNotice::Detail("更新会话归档失败",error.to_string())),
                    Ok(saved)
                        if saved.session.id != id
                            || saved.session.workspace_id != workspace
                            || saved.session.archived != archived =>
                    {
                        this.error = Some("归档响应中的会话身份或状态不匹配，请刷新核对。".into())
                    }
                    Ok(_) => {
                        if this.settings_panel
                            && this.settings_section == crate::app::SettingsSection::Archive
                        {
                            this.load_archived_settings(false, cx);
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
