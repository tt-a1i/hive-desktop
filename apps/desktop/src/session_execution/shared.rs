use crate::{
    app::{ErrorNotice, HiveApp},
    protocol::Session,
};
use gpui_kit::*;
use serde_json::json;

pub(crate) struct WorktreeDraftCreation {
    workspace: String,
    source: String,
    request: String,
    connection: Option<u64>,
}

fn matches_shared_draft(source: &Session, request: &str, draft: &Session) -> bool {
    let Some(original) = &source.execution else {
        return false;
    };
    draft.id == request
        && draft.id != source.id
        && draft.workspace_id == source.workspace_id
        && draft.backend == source.backend
        && draft.profile == source.profile
        && !draft.archived
        && draft.agent_id.is_none()
        && draft.origin_agent_id.is_none()
        && draft.identity_state.as_deref() == Some("new")
        && draft.execution.as_ref().is_some_and(|execution| {
            original.kind == "worktree"
                && execution.kind == original.kind
                && execution.path == original.path
                && execution.branch == original.branch
                && execution.owner_session_id.as_deref()
                    == Some(original.owner_session_id.as_deref().unwrap_or(&source.id))
        })
}

impl HiveApp {
    pub(crate) fn shared_worktree_draft_source(&self) -> Option<&Session> {
        self.session.as_ref().filter(|session| {
            self.workspace.as_ref() == Some(&session.workspace_id)
                && !session.archived
                && session.profile.is_some()
                && session.execution.as_ref().is_some_and(|execution| {
                    execution.kind == "worktree" && !execution.path.is_empty()
                })
        })
    }

    pub(crate) fn can_create_shared_worktree_draft(&self) -> bool {
        self.connected
            && !self.closing
            && !self.loading
            && self.conversation_ready
            && self.viewing_conversation()
            && self.workspace_writes_allowed()
            && !self.conversation_is_read_only()
            && self.shared_worktree_draft_source().is_some()
            && self
                .worktree_draft_creation
                .as_ref()
                .is_none_or(|pending| pending.connection != Some(self.connection_generation))
    }

    pub(crate) fn create_shared_worktree_draft(&mut self, source_id: &str, cx: &mut Context<Self>) {
        let Some(source) = self
            .shared_worktree_draft_source()
            .filter(|session| session.id == source_id && self.can_create_shared_worktree_draft())
            .cloned()
        else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        if let Err(error) = self.flush_drafts(cx) {
            self.error = Some(ErrorNotice::Detail(
                "无法在此工作树新建会话",
                error.to_string(),
            ));
            cx.notify();
            return;
        }
        let connection = self.connection_generation;
        let origin = self.open_request;
        let request = self
            .worktree_draft_creation
            .as_ref()
            .filter(|pending| {
                pending.workspace == source.workspace_id && pending.source == source.id
            })
            .map(|pending| pending.request.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.worktree_draft_creation = Some(WorktreeDraftCreation {
            workspace: source.workspace_id.clone(),
            source: source.id.clone(),
            request: request.clone(),
            connection: Some(connection),
        });
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.createInWorktreeRecord",
                    json!({"workspaceId":source.workspace_id,"sourceSessionId":source.id,"requestId":request}),
                )
                .await
                .and_then(|value| serde_json::from_value::<Session>(value).map_err(|error| error.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || !this.worktree_draft_creation.as_ref().is_some_and(|pending| {
                        pending.request == request && pending.connection == Some(connection)
                    })
                {
                    return;
                }
                let current = this.connected && !this.closing && this.open_request == origin
                    && this.viewing_conversation()
                    && this.shared_worktree_draft_source().is_some_and(|session| {
                        session.id == source.id && session.execution == source.execution
                    });
                match result {
                    Ok(draft) if matches_shared_draft(&source, &request, &draft) => {
                        this.worktree_draft_creation = None;
                        if current && this.workspace_writes_allowed() {
                            this.accept_created_session(draft, cx);
                        } else if !this.closing
                            && this.workspace_removing.as_ref() != Some(&draft.workspace_id)
                            && this.workspaces.iter().any(|workspace| workspace.id == draft.workspace_id)
                        {
                            let rows = if this.workspace.as_ref() == Some(&draft.workspace_id) {
                                &mut this.sessions
                            } else {
                                this.project_sessions.entry(draft.workspace_id.clone()).or_default()
                            };
                            if !rows.iter().any(|session| session.id == draft.id) {
                                rows.insert(0, draft);
                            }
                        }
                    }
                    result => {
                        if let Some(pending) = &mut this.worktree_draft_creation {
                            pending.connection = None;
                        }
                        if current {
                            this.error = Some(match result {
                                Err(error) => ErrorNotice::Detail("无法在此工作树新建会话", error),
                                Ok(_) => "新会话返回的工作树或身份与请求不一致。".into(),
                            });
                        }
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
    use super::{Session, matches_shared_draft};
    use serde_json::json;

    #[test]
    fn shared_draft_requires_the_exact_owner_scope_and_request() {
        let session = |id, owner, path, workspace| -> Session {
            serde_json::from_value(json!({
                "id": id,
                "title": "Conversation",
                "workspaceId": workspace,
                "identityState": "new",
                "profile": {"id":"default-pi","revision":1},
                "execution":{"kind":"worktree","path":path,"branch":"wip","ownerSessionId":owner}
            }))
            .unwrap()
        };
        let source = session("source", None::<&str>, "/tree", "project");
        let draft = session("request", Some("source"), "/tree", "project");
        assert!(matches_shared_draft(&source, "request", &draft));
        assert!(matches_shared_draft(
            &draft,
            "request-2",
            &session("request-2", Some("source"), "/tree", "project"),
        ));
        for rejected in [
            session("other-request", Some("source"), "/tree", "project"),
            session("request", Some("other-owner"), "/tree", "project"),
            session("request", Some("source"), "/project-root", "project"),
            session("request", Some("source"), "/tree", "other-project"),
        ] {
            assert!(!matches_shared_draft(&source, "request", &rejected));
        }
    }
}
