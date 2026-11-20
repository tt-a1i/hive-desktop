use crate::{
    app::{ErrorNotice, HiveApp},
    language::tr,
    protocol::Session,
};
use gpui_kit::{component::WindowExt, *};
use serde_json::json;

impl Session {
    pub(crate) fn shared_worktree_owner(&self) -> Option<&str> {
        self.execution
            .as_ref()
            .filter(|execution| execution.kind == "worktree" && !self.worktree_released)
            .and_then(|execution| execution.owner_session_id.as_deref())
    }
}

fn same_worktree_session(source: &Session, record: &Session) -> bool {
    source.id == record.id
        && source.workspace_id == record.workspace_id
        && source.execution == record.execution
}

impl HiveApp {
    pub(crate) fn can_release_shared_worktree(&self, session: &Session) -> bool {
        session.shared_worktree_owner().is_some()
            && self.connected
            && !self.closing
            && self.workspace_removing.as_ref() != Some(&session.workspace_id)
            && !self.removed_sessions.contains(&session.id)
            && !self.worktree_release_loading.contains(&session.id)
            && !self.inflight_inputs.contains_key(&session.id)
            && !self
                .attachments
                .loading
                .contains(&format!("session:{}", session.id))
            && self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == session.workspace_id)
    }

    pub(crate) fn open_worktree_release(
        &mut self,
        session: Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_release_shared_worktree(&session) || window.has_active_dialog(cx) {
            return;
        }
        let Some(execution) = &session.execution else {
            return;
        };
        let description = format!(
            "{}\n{}\n\n{}",
            session.title,
            execution.path,
            tr(
                "此会话将进入只读状态，历史、草稿和已保存的附件仍可查看。最后一个引用解除后，原会话可重新检查工作树清理条件。"
            )
        );
        let weak = cx.weak_entity();
        let connection = self.connection_generation;
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let session = session.clone();
            dialog
                .title(tr("解除共用工作树？"))
                .description(description.clone())
                .show_cancel(true)
                .ok_text(tr("解除共用工作树"))
                .cancel_text(tr("取消"))
                .on_ok(move |_, _, cx| {
                    weak.update(cx, |this, cx| {
                        if this.connection_generation == connection {
                            this.release_shared_worktree(session.clone(), cx);
                        }
                    })
                    .is_ok()
                })
        });
    }

    fn release_shared_worktree(&mut self, session: Session, cx: &mut Context<Self>) {
        if !self.can_release_shared_worktree(&session) {
            return;
        }
        let (Some(runtime), Some(owner)) = (self.runtime.clone(), session.shared_worktree_owner())
        else {
            return;
        };
        if let Err(error) = self.flush_drafts(cx) {
            self.error = Some(ErrorNotice::Detail("无法解除共用工作树", error.to_string()));
            cx.notify();
            return;
        }
        let params = json!({
            "workspaceId": session.workspace_id,
            "sessionId": session.id,
            "expectedOwnerSessionId": owner,
        });
        let connection = self.connection_generation;
        self.worktree_release_loading.insert(session.id.clone());
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.worktrees.release", params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<Session>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.worktree_release_loading.remove(&session.id);
                match result {
                    Ok(record)
                        if record.worktree_released && same_worktree_session(&session, &record) =>
                    {
                        let viewing = this.viewing_conversation()
                            && this.workspace.as_ref() == Some(&session.workspace_id)
                            && this
                                .session
                                .as_ref()
                                .is_some_and(|current| same_worktree_session(&session, current));
                        for row in this
                            .sessions
                            .iter_mut()
                            .chain(this.project_sessions.values_mut().flatten())
                            .chain(
                                this.pinned_sessions
                                    .records
                                    .iter_mut()
                                    .map(|hit| &mut hit.session),
                            )
                            .chain(this.session.iter_mut())
                            .filter(|row| same_worktree_session(&session, row))
                        {
                            row.worktree_released = true;
                        }
                        if viewing && !this.closing {
                            this.show_session(session.id, cx);
                        }
                    }
                    Ok(_) => this.error = Some("工作树解除响应与所选会话不一致。".into()),
                    Err(error) => {
                        this.error = Some(ErrorNotice::Detail("无法解除共用工作树", error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{Session, same_worktree_session};
    use serde_json::json;

    fn record(id: &str, workspace: &str, owner: &str, released: bool) -> Session {
        serde_json::from_value(json!({
            "id": id,
            "title": "Fixture",
            "workspaceId": workspace,
            "execution": { "kind": "worktree", "path": "/fixture", "branch": "fixture", "ownerSessionId": owner },
            "worktreeReleased": released,
        })).unwrap()
    }

    #[test]
    fn release_matches_exact_session_and_worktree() {
        let source = record("borrower", "project", "owner", false);
        let released = record("borrower", "project", "owner", true);
        assert_eq!(source.shared_worktree_owner(), Some("owner"));
        assert_eq!(released.shared_worktree_owner(), None);
        assert!(same_worktree_session(&source, &released));
        assert!(!same_worktree_session(
            &source,
            &record("other", "project", "owner", true)
        ));
        assert!(!same_worktree_session(
            &source,
            &record("borrower", "other", "owner", true)
        ));
        assert!(!same_worktree_session(
            &source,
            &record("borrower", "project", "other", true)
        ));
    }
}
