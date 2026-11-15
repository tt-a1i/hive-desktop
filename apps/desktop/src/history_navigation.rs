use crate::{
    app::HiveApp,
    language::tr,
    protocol::{Initialization, Session},
};
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct HistoryNavigation {
    generation: u64,
    pub(crate) loading: bool,
    pub(crate) access: HistoryAccess,
    pub(crate) initialization: Option<Initialization>,
}
impl HistoryNavigation {
    pub(crate) fn invalidate(&mut self) {
        self.generation += 1;
        self.loading = false;
        self.access = Default::default();
        self.initialization = None;
    }
}
#[derive(Default)]
pub(crate) struct HistoryAccess {
    workspace: String,
    session: String,
    read_only: bool,
    pub(crate) identity_error: Option<crate::protocol::SnapshotIdentityError>,
    pub(crate) history_incomplete: bool,
    pub(crate) history_warning: Option<String>,
    pub(crate) handoff_snapshot: Option<serde_json::Value>,
    pub(crate) handoff_delivery: Option<serde_json::Value>,
    pub(crate) passive: Option<bool>,
}
impl HistoryAccess {
    pub(crate) fn snapshot(open: &crate::protocol::OpenSession) -> Self {
        Self {
            workspace: open.session.workspace_id.clone(),
            session: open.session.id.clone(),
            read_only: open.read_only,
            identity_error: open.identity_error.clone(),
            history_incomplete: open.history_incomplete,
            handoff_snapshot: open.handoff_snapshot.clone(),
            handoff_delivery: open.handoff_delivery.clone(),
            passive: open.passive,
            history_warning: open
                .history_warning
                .clone()
                .filter(|warning| !warning.trim().is_empty()),
        }
    }
    pub(crate) fn handoff_unconsumed(&self) -> bool {
        self.handoff_snapshot.is_some() && self.handoff_delivery.as_ref().and_then(|d|d["status"].as_str()) != Some("consumed")
    }
    pub(crate) fn handoff_blocked(&self) -> bool {
        self.handoff_snapshot.is_some() && !matches!(self.handoff_delivery.as_ref().and_then(|d|d["status"].as_str()), Some("pending" | "consumed"))
    }
    pub(crate) fn handoff_passive(&self) -> bool {
        self.handoff_unconsumed() && self.passive != Some(false)
    }
    fn matches(&self, workspace: Option<&str>, session: Option<&str>) -> bool {
        workspace == Some(self.workspace.as_str()) && session == Some(self.session.as_str())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DispatchSessionRequest {
    workspace_id: String,
    dispatch_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_agent_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DispatchSessionResponse {
    session: Session,
    source_session_id: String,
    source_agent_id: String,
}
fn validated_session(
    response: DispatchSessionResponse,
    request: &DispatchSessionRequest,
) -> Result<Session, String> {
    if response.session.id.is_empty()
        || response.session.id != response.source_session_id
        || response.session.workspace_id != request.workspace_id
        || response.source_agent_id.is_empty()
        || request
            .source_agent_id
            .as_ref()
            .is_some_and(|agent| agent != &response.source_agent_id)
    {
        return Err(tr("历史派单返回的会话来源不匹配。").into());
    }
    // The historical record may no longer own this agent's current binding.
    Ok(response.session)
}
#[derive(Clone, PartialEq, Eq)]
struct NavigationContext {
    workspace: Option<String>,
    session: Option<String>,
    open_request: u64,
    collaboration: bool,
    dispatch: Option<String>,
    workflow: bool,
    workflow_selected: Option<String>,
    workflow_generation: u64,
    member_panel: bool,
}
impl NavigationContext {
    fn capture(app: &HiveApp) -> Self {
        Self {
            workspace: app.workspace.clone(),
            session: app.session.as_ref().map(|session| session.id.clone()),
            open_request: app.open_request,
            collaboration: app.collaboration.visible,
            dispatch: app
                .collaboration
                .selected
                .as_ref()
                .map(|dispatch| dispatch.id.clone()),
            workflow: app.workflows.visible,
            workflow_selected: app.workflows.selected.clone(),
            workflow_generation: app.workflows.generation,
            member_panel: app.member_panel,
        }
    }
}
pub(crate) fn open_session_identity_matches(
    session: &Session,
    expected_id: &str,
    expected_workspace: &str,
) -> bool {
    session.id == expected_id && session.workspace_id == expected_workspace
}
pub(crate) struct HistoryLinkTarget {
    context: NavigationContext,
    runtime: Arc<crate::transport::Runtime>,
    connection: u64,
}
impl HistoryLinkTarget {
    pub(crate) fn current(&self, app: &HiveApp) -> bool {
        app.connected
            && !app.closing
            && app.connection_generation == self.connection
            && app
                .runtime
                .as_ref()
                .is_some_and(|runtime| Arc::ptr_eq(runtime, &self.runtime))
            && NavigationContext::capture(app) == self.context
    }
}
impl HiveApp {
    pub(crate) fn history_link_target(&self) -> Option<HistoryLinkTarget> {
        Some(HistoryLinkTarget {
            context: NavigationContext::capture(self),
            runtime: self.runtime.clone()?,
            connection: self.connection_generation,
        })
    }
    pub(crate) fn current_history_access(&self) -> Option<&HistoryAccess> {
        let access = &self.history_navigation.access;
        access
            .matches(
                self.workspace.as_deref(),
                self.session.as_ref().map(|session| session.id.as_str()),
            )
            .then_some(access)
    }
    pub(crate) fn conversation_is_read_only(&self) -> bool {
        self.current_history_access()
            .is_some_and(|access| access.read_only)
    }
    pub(crate) fn refresh_initialization(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(session)) = (
            self.workspace.clone(),
            self.session.as_ref().map(|s| s.id.clone()),
        ) else {
            return;
        };
        let generation = self.open_request;
        self.request(
            "worktree.initialization.get",
            serde_json::json!({"workspaceId":workspace,"sessionId":session}),
            cx,
            move |this, value, cx| {
                if this.open_request != generation
                    || this.workspace.as_ref() != Some(&workspace)
                    || this.session.as_ref().map(|s| &s.id) != Some(&session)
                {
                    return;
                }
                match serde_json::from_value::<Option<Initialization>>(value) {
                    Ok(status) => this.history_navigation.initialization = status,
                    Err(_) => this.error = Some("无法读取工作树初始化状态".into()),
                }
                cx.notify();
            },
        );
    }
    pub(crate) fn change_initialization(&mut self, retry: bool, cx: &mut Context<Self>) {
        let (Some(workspace), Some(session), Some(status)) = (
            self.workspace.clone(),
            self.session.as_ref().map(|s| s.id.clone()),
            self.history_navigation.initialization.as_ref(),
        ) else {
            return;
        };
        let uncertain =
            matches!(status.status.as_str(), "unknown" | "interrupted") && status.run_id.is_some();
        let allowed = status
            .actions
            .as_ref()
            .map(|actions| if retry { actions.retry } else { actions.skip })
            .unwrap_or(!uncertain);
        if !allowed {
            return;
        }
        let version = status.version.clone();
        let generation = self.open_request;
        self.request(if retry { "worktree.initialization.retry" } else { "worktree.initialization.skip" }, serde_json::json!({"workspaceId":workspace,"sessionId":session,"expectedVersion":version}), cx, move |this, _, cx| {
            if this.open_request == generation && this.workspace.as_ref() == Some(&workspace) && this.session.as_ref().map(|s| &s.id) == Some(&session) { this.refresh_initialization(cx); }
        });
    }
    pub(crate) fn open_historical_member_session(
        &mut self,
        dispatch: Option<String>,
        agent: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.history_navigation.generation += 1;
        self.history_navigation.loading = false;
        let generation = self.history_navigation.generation;
        let Some(dispatch_id) = dispatch.filter(|id| !id.trim().is_empty()) else {
            self.error = Some(
                tr("旧报告未记录派单来源，无法确定原成员会话。")
                    .to_owned()
                    .into(),
            );
            cx.notify();
            return;
        };
        let (Some(workspace_id), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        if !self.connected || self.closing || !self.workspace_writes_allowed() {
            return;
        }
        self.history_navigation.loading = true;
        let context = NavigationContext::capture(self);
        let connection = self.connection_generation;
        let window = window.window_handle();
        let weak = cx.weak_entity();
        let request = DispatchSessionRequest {
            workspace_id,
            dispatch_id,
            source_agent_id: agent.filter(|id| !id.is_empty()),
        };
        cx.notify();
        // Start outside the click's HiveApp borrow; completion also enters through a weak owner.
        cx.defer(move |cx| {
            let _ = weak.update(cx, |_, cx| {
                cx.spawn(async move |owner, cx| {
                    let result = runtime
                        .request(
                            "dispatch.session",
                            serde_json::to_value(&request).expect("dispatch identity serializes"),
                        )
                        .await
                        .and_then(|value| {
                            serde_json::from_value::<DispatchSessionResponse>(value)
                                .map_err(|_| tr("历史成员会话响应格式错误。").to_owned())
                        })
                        .and_then(|response| validated_session(response, &request));
                    let _ = window.update(cx, |_, window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if this.history_navigation.generation != generation {
                                return;
                            }
                            this.history_navigation.loading = false;
                            cx.notify();
                            if !this.connected
                                || this.closing
                                || this.connection_generation != connection
                                || !this
                                    .runtime
                                    .as_ref()
                                    .is_some_and(|current| Arc::ptr_eq(current, &runtime))
                                || NavigationContext::capture(this) != context
                            {
                                return;
                            }
                            match result {
                                Ok(session) => {
                                    let id = session.id.clone();
                                    if let Some(existing) =
                                        this.sessions.iter_mut().find(|existing| existing.id == id)
                                    {
                                        *existing = session;
                                    } else {
                                        this.sessions.push(session);
                                    }
                                    this.open_member_session(id, window, cx);
                                }
                                Err(error) => {
                                    this.error = Some(crate::app::ErrorNotice::Detail(
                                        "无法打开原成员会话",
                                        error,
                                    ))
                                }
                            }
                            cx.notify();
                        });
                    });
                })
                .detach();
            });
        });
    }
}
#[cfg(test)]
mod tests {
    use super::{DispatchSessionRequest, DispatchSessionResponse, validated_session};
    use serde_json::json;
    #[test]
    fn dispatch_navigation_accepts_historical_unbound_session_and_rejects_wrong_identity() {
        let request = DispatchSessionRequest {
            workspace_id: "w".into(),
            dispatch_id: "dispatch".into(),
            source_agent_id: Some("worker".into()),
        };
        let value = json!({"session":{"id":"old-A","workspaceId":"w","title":"history","agentId":null},"sourceSessionId":"old-A","sourceAgentId":"worker"});
        let decode = |value| serde_json::from_value::<DispatchSessionResponse>(value).unwrap();
        assert_eq!(
            validated_session(decode(value.clone()), &request)
                .unwrap()
                .id,
            "old-A"
        );
        for (key, replacement) in [
            ("sourceSessionId", "current-B"),
            ("sourceAgentId", "other-worker"),
        ] {
            let mut wrong = value.clone();
            wrong[key] = json!(replacement);
            assert!(validated_session(decode(wrong), &request).is_err());
        }
        let mut wrong = value;
        wrong["session"]["workspaceId"] = json!("other-workspace");
        assert!(validated_session(decode(wrong), &request).is_err());
        let absent = DispatchSessionRequest {
            source_agent_id: None,
            ..request
        };
        assert!(
            serde_json::to_value(absent)
                .unwrap()
                .get("sourceAgentId")
                .is_none()
        );
    }
}

#[cfg(test)]
mod access_tests {
    use super::{HistoryAccess, NavigationContext, open_session_identity_matches};
    use crate::protocol::OpenSession;
    use serde_json::json;
    #[test]
    fn snapshot_read_only_and_incomplete_identity_are_scoped_without_affecting_old_snapshots() {
        let value = json!({"id":"A","workspaceId":"w","title":"history","historyMessageIds":[],"eventSequence":0,"readOnly":true,"identityError":{"code":"AGENT_MISSING","message":"The original member was deleted"},"historyIncomplete":true,"historyWarning":"Some earlier messages are unavailable"});
        let open: OpenSession = serde_json::from_value(value).unwrap();
        let access = HistoryAccess::snapshot(&open);
        assert!(access.read_only && access.history_incomplete);
        assert_eq!(
            access.identity_error.as_ref().unwrap().code,
            "AGENT_MISSING"
        );
        assert_eq!(
            access.identity_error.as_ref().unwrap().message,
            "The original member was deleted"
        );
        assert_eq!(
            access.history_warning.as_deref(),
            Some("Some earlier messages are unavailable")
        );
        assert!(access.matches(Some("w"), Some("A")));
        assert!(!access.matches(Some("w"), Some("B")));
        assert!(!access.matches(Some("other"), Some("A")));
        assert!(!access.matches(None, None));
        let ordinary:OpenSession=serde_json::from_value(json!({"id":"B","workspaceId":"w","title":"ordinary","historyMessageIds":[],"eventSequence":0})).unwrap();
        let ordinary = HistoryAccess::snapshot(&ordinary);
        assert!(
            !ordinary.read_only
                && !ordinary.history_incomplete
                && ordinary.identity_error.is_none()
        );
    }
    #[test]
    fn navigation_context_detects_session_reopen_even_when_ids_match() {
        let before = NavigationContext {
            workspace: Some("w".into()),
            session: Some("A".into()),
            open_request: 1,
            collaboration: false,
            dispatch: None,
            workflow: false,
            workflow_selected: None,
            workflow_generation: 0,
            member_panel: false,
        };
        let after = NavigationContext {
            open_request: 2,
            ..NavigationContext {
                workspace: Some("w".into()),
                session: Some("A".into()),
                open_request: 1,
                collaboration: false,
                dispatch: None,
                workflow: false,
                workflow_selected: None,
                workflow_generation: 0,
                member_panel: false,
            }
        };
        assert!(before != after);
    }
    #[test]
    fn workflow_detail_switch_and_refresh_invalidate_pending_history_navigation() {
        let original = NavigationContext {
            workspace: Some("w".into()),
            session: Some("same-session".into()),
            open_request: 7,
            collaboration: false,
            dispatch: None,
            workflow: true,
            workflow_selected: Some("workflow-A".into()),
            workflow_generation: 3,
            member_panel: false,
        };
        let mut changed = original.clone();
        changed.workflow_selected = Some("workflow-B".into());
        assert!(changed != original);
        let mut refreshed = original.clone();
        refreshed.workflow_generation += 1;
        assert!(refreshed != original);
    }
    #[test]
    fn open_snapshot_must_match_requested_workspace_before_access_is_applied() {
        let mut response:OpenSession=serde_json::from_value(json!({"id":"same-id","workspaceId":"requested-workspace","title":"history","historyMessageIds":[],"eventSequence":0,"readOnly":true})).unwrap();
        assert!(open_session_identity_matches(
            &response.session,
            "same-id",
            "requested-workspace"
        ));
        response.session.workspace_id = "wrong-workspace".into();
        assert!(!open_session_identity_matches(
            &response.session,
            "same-id",
            "requested-workspace"
        ));
        response.session.workspace_id = "requested-workspace".into();
        response.session.id = "wrong-session".into();
        assert!(!open_session_identity_matches(
            &response.session,
            "same-id",
            "requested-workspace"
        ));
    }
}

#[cfg(test)]
mod handoff_state_tests {
    use super::HistoryAccess;
    use crate::protocol::OpenSession;
    use serde_json::json;
    #[test]
    fn handoff_requires_known_delivery_state_and_only_passive_pi_skips_model_gate() {
        let make = |delivery: serde_json::Value, passive: Option<bool>| {
            let mut value=json!({"id":"s","workspaceId":"w","title":"t","backend":"pi","historyMessageIds":[],"eventSequence":0,"handoffSnapshot":{},"handoffDelivery":delivery});
            if let Some(passive)=passive { value["passive"]=json!(passive); }
            HistoryAccess::snapshot(&serde_json::from_value::<OpenSession>(value).unwrap())
        };
        let cold=make(json!({"status":"pending"}),Some(true));assert!(cold.handoff_passive());assert!(!cold.handoff_blocked());
        let live=make(json!({"status":"pending"}),Some(false));assert!(!live.handoff_passive());assert!(!live.handoff_blocked());
        let uncertain=make(json!({"status":"uncertain"}),Some(false));assert!(uncertain.handoff_blocked());
        let unknown=make(json!({}),None);assert!(unknown.handoff_passive());assert!(unknown.handoff_blocked());
        let in_flight=make(json!({"status":"unknown"}),Some(false));assert!(!in_flight.handoff_passive());assert!(in_flight.handoff_blocked());assert!(in_flight.handoff_unconsumed());
        let consumed=make(json!({"status":"consumed"}),Some(false));assert!(!consumed.handoff_unconsumed());assert!(!consumed.handoff_blocked());assert!(!consumed.handoff_passive());
    }
}
