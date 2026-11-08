use super::*;

#[derive(serde::Deserialize)]
struct ArchiveResult {
    session: Session,
}

impl HiveApp {
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
                    Ok(_) => {}
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
        cx.notify();
    }
}
