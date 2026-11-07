use super::*;

#[derive(serde::Deserialize)]
struct PinResult {
    session: Session,
    sessions: Vec<Session>,
}

impl HiveApp {
    pub(crate) fn set_session_pinned(
        &mut self,
        session: Session,
        pinned: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self.closing
            || self.session_pin_loading.contains(&session.id)
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
                    Err(error) => this.error = Some(format!("保存会话置顶失败：{error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
