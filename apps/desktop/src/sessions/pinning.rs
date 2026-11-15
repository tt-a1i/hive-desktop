use super::*;

#[derive(Default)]
pub(crate) struct PinnedSessions {
    pub records: Vec<SessionHit>,
    pub collapsed: bool,
    pub loading: bool,
    pub dirty: bool,
    pub error: Option<String>,
    generation: u64,
}

#[derive(serde::Deserialize)]
struct PinResult {
    session: Session,
    sessions: Vec<Session>,
}

use crate::language::{english, tr};
use gpui_kit::component::WindowExt as _;

impl HiveApp {
    pub(crate) fn request_session_pinned(
        &mut self,
        session: Session,
        pinned: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if pinned || !crate::general::current(cx).confirm_unpin_session {
            self.set_session_pinned(session, pinned, cx);
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
                .title(tr("取消置顶？"))
                .description(if english() {
                    format!(
                        "Unpin \"{title}\"? It will return to its normal chronological position."
                    )
                } else {
                    format!("确定取消置顶“{title}”吗？该会话将回到工作区的常规时间位置。")
                })
                .show_cancel(true)
                .ok_text(tr("取消置顶"))
                .cancel_text(tr("取消"))
                .on_ok(move |_, _, cx| {
                    weak.update(cx, |this, cx| {
                        if this.connection_generation == connection {
                            this.set_session_pinned(confirm_session.clone(), false, cx);
                        }
                    })
                    .is_ok()
                })
        });
    }

    pub(crate) fn set_session_pinned(
        &mut self,
        session: Session,
        pinned: bool,
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
        let connection = self.connection_generation;
        let id = session.id;
        let project = session.workspace_id;
        self.session_pin_loading.insert(id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.pin", json!({"sessionId":id,"pinned":pinned}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<PinResult>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.session_pin_loading.remove(&id);
                match result {
                    Ok(result) => {
                        if this
                            .workspaces
                            .iter()
                            .any(|workspace| workspace.id == project)
                        {
                            this.refresh_pinned_sessions(cx);
                            if this.workspace.as_ref() == Some(&project) {
                                this.sessions = result.sessions;
                            } else {
                                this.project_sessions.insert(project, result.sessions);
                            }
                            if let Some(selected) =
                                this.session.as_mut().filter(|selected| selected.id == id)
                            {
                                selected.pinned = result.session.pinned;
                            }
                        }
                    }
                    Err(error) => this.error = Some(crate::app::ErrorNotice::Detail("保存会话置顶失败",error.to_string())),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl HiveApp {
    pub(crate) fn refresh_pinned_sessions(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.closing {
            return;
        }
        self.pinned_sessions.generation += 1;
        self.pinned_sessions.dirty = true;
        if self.pinned_sessions.loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.pinned_sessions.loading = true;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(generation)) = this.update(cx, |this, _| {
                    if this.connection_generation != connection {
                        return None;
                    }
                    this.pinned_sessions.dirty = false;
                    Some(this.pinned_sessions.generation)
                }) else {
                    return;
                };
                let result = runtime
                    .request("chat.pinned", json!({}))
                    .await
                    .and_then(|value| {
                        serde_json::from_value::<Vec<SessionHit>>(value)
                            .map_err(|error| error.to_string())
                    });
                let Ok(again) = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return false;
                    }
                    if generation == this.pinned_sessions.generation && !this.pinned_sessions.dirty
                    {
                        match result {
                            Ok(records) => {
                                this.pinned_sessions.records = records;
                                this.pinned_sessions.error = None;
                            }
                            Err(error) => this.pinned_sessions.error = Some(error),
                        }
                    }
                    let again = this.pinned_sessions.dirty;
                    if !again {
                        this.pinned_sessions.loading = false;
                    }
                    cx.notify();
                    again
                }) else {
                    return;
                };
                if !again {
                    return;
                }
            }
        })
        .detach();
    }
}

impl HiveApp {
    pub(crate) fn sync_pinned_project(&mut self) {
        // Conversation events already refresh the active project's list. Merge that
        // snapshot instead of rereading every pinned project's team on each burst.
        for hit in &mut self.pinned_sessions.records {
            if let Some(session) = self
                .sessions
                .iter()
                .find(|session| session.id == hit.session.id)
            {
                hit.session.title = session.title.clone();
                hit.session.custom_title = session.custom_title;
                hit.session.agent_id = session.agent_id.clone();
                hit.session.updated_at = session.updated_at;
                hit.session.archived = session.archived;
                hit.title = if !session.custom_title {
                    session
                        .agent_id
                        .as_ref()
                        .and_then(|id| self.members.iter().find(|member| &member.id == id))
                        .map(|member| member.name.clone())
                        .unwrap_or_else(|| session.title.clone())
                } else {
                    session.title.clone()
                };
            }
        }
        self.pinned_sessions
            .records
            .retain(|hit| !hit.session.archived);
        self.pinned_sessions.records.sort_by(|a, b| {
            b.session
                .updated_at
                .cmp(&a.session.updated_at)
                .then_with(|| a.session.id.cmp(&b.session.id))
        });
    }
}
