use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileRef {
    pub id: String,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentRef {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_file: Option<ProjectFileReference>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectFileReference {
    pub relative_path: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_session_id: Option<String>,
}
#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryFailure {
    pub delivery_id: String,
    pub message: String,
}

#[derive(Clone, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxReply {
    pub revision: u64,
    pub received_at: Option<u64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxReplyEvent {
    pub workspace_id: String,
    pub revision: u64,
    pub received_at: Option<u64>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub custom_title: bool,
    #[serde(default)]
    pub pinned: bool,
    pub pin_rank: Option<i64>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub updated_at: i64,
    pub inbox_reply: Option<InboxReply>,
    pub workspace_id: String,
    pub agent_id: Option<String>,
    pub origin_agent_id: Option<String>,
    pub identity_state: Option<String>,
    #[serde(default)]
    pub backend: Backend,
    #[serde(default)]
    pub execution: Option<SessionExecution>,
    #[serde(default)]
    pub worktree_released: bool,
    #[serde(default)]
    pub can_start_fresh_worktree_draft: bool,
    #[serde(default)]
    pub execution_path: Option<String>,
    #[serde(default)]
    pub scratch: bool,
    #[serde(default)]
    pub profile: Option<ProfileRef>,
}
impl Session {
    pub(crate) fn inbox_reply_revision(&self) -> u64 {
        self.inbox_reply.as_ref().map_or(0, |reply| reply.revision)
    }

    /// Presentation only: original identity is never a live run or token credential.
    pub(crate) fn display_agent_id(&self) -> Option<&str> {
        self.origin_agent_id
            .as_deref()
            .filter(|_| self.identity_state.as_deref() == Some("known"))
            .or(self.agent_id.as_deref())
    }
    pub(crate) fn display_role_key(&self) -> &'static str {
        match self.display_agent_id() {
            Some(id) if id.ends_with(":orchestrator") => "主控",
            Some(id) if id.ends_with(":__workflow__") => "工作流",
            Some(_) => "成员",
            None => "会话",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExecution {
    pub kind: String,
    pub path: String,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_session_id: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHit {
    pub session: Session,
    pub workspace_name: String,
    #[serde(default)]
    pub workspace_path: String,
    pub title: String,
    #[serde(default)]
    pub body_match: Option<SessionBodyMatch>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBodyMatch {
    pub message_id: String,
    pub snippet: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncompleteHistory {
    pub session_id: String,
    pub title: String,
    pub message: String,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Pi,
    Codex,
    #[serde(rename = "pi-rpc")]
    PiRpc,
    Claude,
    Acp,
}
impl Backend {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::Codex => "codex",
            Self::PiRpc => "pi-rpc",
            Self::Claude => "claude",
            Self::Acp => "acp",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Pi => "Pi",
            Self::Codex => "Codex",
            Self::PiRpc => "外部 Pi",
            Self::Claude => "Claude",
            Self::Acp => "ACP",
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberDesktopActivity {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub runtime_state: String,
    #[serde(default)]
    pub activity_kind: Option<String>,
    #[serde(default)]
    pub active_tool: Option<String>,
    #[serde(default)]
    pub active_target: Option<String>,
    #[serde(default)]
    pub pending_interaction_count: u64,
    #[serde(default)]
    pub pending_interaction_ids: Vec<String>,
    #[serde(default)]
    pub queued_input_count: u64,
    #[serde(default)]
    pub pending_report_count: u64,
    #[serde(default)]
    pub delivery_failure_message: Option<String>,
    #[serde(default)]
    pub worktree_branch: Option<String>,
    #[serde(default)]
    pub worktree_removed: bool,
}

#[derive(Clone, Deserialize)]
pub struct Member {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub pending_task_count: u64,
    #[serde(default)]
    pub configured_command: Option<String>,
    pub desktop_model: Option<ModelIdentity>,
    #[serde(default)]
    pub desktop_profile: Option<ProfileRef>,
    #[serde(default)]
    pub desktop_launch_error: Option<String>,
    #[serde(default)]
    pub desktop_activity: Option<MemberDesktopActivity>,
}

#[derive(Deserialize)]
pub struct MemberRun {
    pub agent_id: String,
    pub run_id: String,
    pub status: String,
}
#[derive(Clone, Deserialize)]
pub struct Model {
    pub provider: String,
    pub id: String,
    pub name: String,
    pub available: bool,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingReport {
    pub dispatch_id: Option<String>,
    pub delivery_id: String,
    pub text: String,
    pub worker_name: Option<String>,
    pub worker_id: Option<String>,
    pub artifacts: Vec<String>,
    pub processing: bool,
}
impl PendingReport {
    pub fn status(&self) -> &'static str {
        if self.processing {
            "正在处理"
        } else {
            "已收到 · 待处理"
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingUserInput {
    pub delivery_id: String,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<AttachmentRef>,
    pub processing: bool,
    #[serde(default)]
    pub editable: bool,
    #[serde(default)]
    pub reason: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Initialization {
    pub(crate) version: String,
    pub(crate) status: String,
    pub(crate) script: Option<InitializationScript>,
    pub(crate) error_code: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) actions: Option<InitializationActions>,
}
#[derive(Clone, Deserialize)]
pub(crate) struct InitializationActions {
    pub(crate) retry: bool,
    pub(crate) skip: bool,
    pub(crate) reason: Option<String>,
}
#[derive(Clone, Deserialize)]
pub(crate) struct InitializationScript {
    pub(crate) name: String,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceStatus {
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub scope: String,
    pub transport: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub config_path: Option<String>,
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendReadiness {
    pub backend: String,
    pub level: String,
    #[serde(default)]
    pub credential_state: String,
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default)]
    pub config_error: Option<String>,
    #[serde(default)]
    pub remediation_key: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolInfo {
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProbeResult {
    pub ok: bool,
    pub status: String,
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub tools: Vec<McpToolInfo>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLocalBranch {
    pub name: String,
    #[serde(default)]
    pub is_current: bool,
    #[serde(default)]
    pub ahead: u32,
    #[serde(default)]
    pub behind: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchesSnapshot {
    #[serde(default)]
    pub current: Option<String>,
    #[serde(default)]
    pub local: Vec<GitLocalBranch>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStashEntry {
    #[serde(rename = "ref")]
    pub ref_name: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub relative_time: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolOutputSliceResponse {
    pub chunk: String,
    #[serde(default)]
    pub next_byte_offset: u64,
    #[serde(default)]
    pub total_bytes: u64,
    #[serde(default)]
    pub has_more: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitSummary {
    pub oid: String,
    #[serde(default)]
    pub short_oid: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub relative_time: String,
    #[serde(default)]
    pub subject: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitDiffResult {
    #[serde(default)]
    pub patch: String,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOrRuleEntry {
    pub name: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub excerpt: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsAndRulesReport {
    #[serde(default)]
    pub rules: Vec<SkillOrRuleEntry>,
    #[serde(default)]
    pub skills: Vec<SkillOrRuleEntry>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendUsageBreakdown {
    pub backend: String,
    #[serde(default)]
    pub total_sessions: usize,
    #[serde(default)]
    pub active_last7_days: usize,
    #[serde(default)]
    pub worktree_sessions: usize,
    #[serde(default)]
    pub archived_sessions: usize,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalUsageSummary {
    #[serde(default)]
    pub total_sessions: usize,
    #[serde(default)]
    pub active_last7_days: usize,
    #[serde(default)]
    pub active_last30_days: usize,
    #[serde(default)]
    pub worktree_sessions: usize,
    #[serde(default)]
    pub archived_sessions: usize,
    #[serde(default)]
    pub sampled_transcripts: usize,
    #[serde(default)]
    pub total_input_tokens: u64,
    #[serde(default)]
    pub total_output_tokens: u64,
    #[serde(default)]
    pub total_cache_read_tokens: u64,
    #[serde(default)]
    pub backends: Vec<BackendUsageBreakdown>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessReport {
    #[serde(default)]
    pub backends: Vec<BackendReadiness>,
    #[serde(default)]
    pub mcp_resources: Vec<McpResourceStatus>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPageMeta {
    #[serde(default)]
    pub snapshot: Option<String>,
    #[serde(default)]
    pub start_index: usize,
    #[serde(default)]
    pub has_older: bool,
    #[serde(default)]
    pub next_before_index: Option<usize>,
    #[serde(default)]
    pub next_before_message_id: Option<String>,
    #[serde(default)]
    pub has_newer: bool,
    #[serde(default)]
    pub next_after_index: Option<usize>,
    #[serde(default)]
    pub next_after_message_id: Option<String>,
}

impl HistoryPageMeta {
    pub(crate) fn matches_snapshot(&self, expected: Option<&str>) -> bool {
        expected.is_none_or(|expected| self.snapshot.as_deref() == Some(expected))
    }

    pub(crate) fn extend(&mut self, page: Self, newer: bool) {
        if newer {
            self.has_newer = page.has_newer;
            self.next_after_index = page.next_after_index;
            self.next_after_message_id = page.next_after_message_id;
        } else {
            self.start_index = page.start_index;
            self.has_older = page.has_older;
            self.next_before_index = page.next_before_index;
            self.next_before_message_id = page.next_before_message_id;
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPageResponse {
    #[serde(default)]
    pub history: Vec<Value>,
    #[serde(default)]
    pub history_message_ids: Vec<String>,
    pub history_page: HistoryPageMeta,
}

#[derive(Deserialize)]
pub struct PrReviewDraft {
    pub prompt: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSession {
    #[serde(default)]
    pub history_page: Option<HistoryPageMeta>,
    #[serde(default)]
    pub fork_draft: Option<Value>,
    #[serde(default)]
    pub pr_review: Option<PrReviewDraft>,
    #[serde(default)]
    pub handoff_snapshot: Option<Value>,
    #[serde(default)]
    pub handoff_delivery: Option<Value>,
    #[serde(default)]
    pub passive: Option<bool>,
    #[serde(default)]
    pub saved_history_passive: bool,
    #[serde(default)]
    pub lineage: Option<Value>,
    #[serde(default)]
    pub initialization: Option<Initialization>,
    #[serde(default)]
    pub read_only: bool,
    pub identity_error: Option<SnapshotIdentityError>,
    #[serde(default)]
    pub can_check_saved_codex_modes: bool,
    #[serde(default)]
    pub history_incomplete: bool,
    pub history_warning: Option<String>,
    #[serde(default)]
    pub interactions: Vec<AgentInteraction>,
    pub history_message_ids: Vec<String>,
    pub active_message: Option<ActiveMessage>,
    pub event_sequence: u64,
    #[serde(default)]
    pub busy: bool,
    #[serde(default)]
    pub pending: usize,
    #[serde(default)]
    pub pending_reports: Vec<PendingReport>,
    #[serde(default)]
    pub pending_user_inputs: Vec<PendingUserInput>,
    pub delivery_failure: Option<DeliveryFailure>,
    #[serde(default)]
    pub completed_delivery_ids: Vec<String>,
    #[serde(flatten)]
    pub session: Session,
    #[serde(default)]
    pub history: Vec<Value>,
    #[serde(default)]
    pub models: Vec<Model>,
    pub model: Option<ModelIdentity>,
    pub unavailable_model: Option<ModelIdentity>,
}
#[derive(Clone, Deserialize)]
pub struct SnapshotIdentityError {
    pub code: String,
    pub message: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveMessage {
    pub message_id: String,
    pub message: Value,
}
#[derive(Clone, Deserialize)]
pub struct ModelIdentity {
    pub provider: String,
    pub id: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInteraction {
    pub id: String,
    pub backend: String,
    pub method: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub native_request_id: Value,
    pub state: String,
    pub params: Value,
    pub item: Option<Value>,
}
impl AgentInteraction {
    pub fn has_native_identity(&self) -> bool {
        let present = |id: &Option<String>| id.as_ref().is_some_and(|id| !id.is_empty());
        match self.backend.as_str() {
            "codex" => [&self.thread_id, &self.turn_id, &self.item_id]
                .iter()
                .all(|id| present(id)),
            "acp" => self.method == "session/request_permission" && (self.native_request_id.as_str().is_some_and(|id| !id.is_empty()) || self.native_request_id.is_number()) && self.params["sessionId"].as_str().is_some_and(|id| !id.is_empty()) && self.params["toolCall"]["toolCallId"].as_str().is_some_and(|id| !id.is_empty()),
            "claude" => {
                present(&self.turn_id)
                    && present(&self.item_id)
                    && self
                        .native_request_id
                        .as_str()
                        .is_some_and(|id| !id.is_empty())
            }
            _ => true,
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiEvent {
    #[serde(rename_all = "camelCase")]
    SessionModeChanged { session_id: String, sequence: u64 },
    #[serde(rename_all = "camelCase")]
    SessionNavigation {
        session_id: String,
        sequence: u64,
        archived: bool,
        pinned: bool,
        updated_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    Interaction {
        session_id: String,
        sequence: u64,
        interaction: AgentInteraction,
    },
    #[serde(rename_all = "camelCase")]
    InteractionResolved {
        session_id: String,
        sequence: u64,
        interaction_id: String,
        reason: String,
    },
    #[serde(rename_all = "camelCase")]
    State {
        session_id: String,
        sequence: u64,
        busy: bool,
        pending: usize,
        #[serde(default)]
        pending_reports: Vec<PendingReport>,
        #[serde(default)]
        pending_user_inputs: Vec<PendingUserInput>,
        completed_report_id: Option<String>,
        delivery_failure: Option<DeliveryFailure>,
        handoff_delivery: Option<Value>,
    },
    #[serde(rename_all = "camelCase")]
    AssistantDelta {
        session_id: String,
        sequence: u64,
        message_id: String,
        kind: String,
        content_index: usize,
        delta: String,
    },
    #[serde(rename_all = "camelCase")]
    MessageEnd {
        session_id: String,
        sequence: u64,
        message_id: String,
        message: Value,
        inbox_reply: Option<InboxReplyEvent>,
    },
    #[serde(rename_all = "camelCase")]
    ToolStart {
        session_id: String,
        sequence: u64,
        tool_call_id: String,
        tool_name: String,
        args: Value,
    },
    #[serde(rename_all = "camelCase")]
    ToolUpdate {
        session_id: String,
        sequence: u64,
        tool_call_id: String,
        tool_name: String,
        result: Value,
    },
    #[serde(rename_all = "camelCase")]
    ToolEnd {
        session_id: String,
        sequence: u64,
        tool_call_id: String,
        tool_name: String,
        result: Value,
        is_error: bool,
    },
    #[serde(rename_all = "camelCase")]
    Settled {
        session_id: String,
        sequence: u64,
        aborted: bool,
    },
    #[serde(rename_all = "camelCase")]
    Error {
        session_id: String,
        sequence: u64,
        message: String,
        #[serde(default)]
        diagnostic_write_failed: bool,
    },
}
impl PiEvent {
    pub fn sequence(&self) -> u64 {
        match self {
            Self::SessionModeChanged { sequence, .. }
            | Self::SessionNavigation { sequence, .. }
            | Self::Interaction { sequence, .. }
            | Self::InteractionResolved { sequence, .. }
            | Self::State { sequence, .. }
            | Self::AssistantDelta { sequence, .. }
            | Self::MessageEnd { sequence, .. }
            | Self::ToolStart { sequence, .. }
            | Self::ToolUpdate { sequence, .. }
            | Self::ToolEnd { sequence, .. }
            | Self::Settled { sequence, .. }
            | Self::Error { sequence, .. } => *sequence,
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::SessionModeChanged { session_id, .. }
            | Self::SessionNavigation { session_id, .. }
            | Self::Interaction { session_id, .. }
            | Self::InteractionResolved { session_id, .. }
            | Self::State { session_id, .. }
            | Self::AssistantDelta { session_id, .. }
            | Self::MessageEnd { session_id, .. }
            | Self::ToolStart { session_id, .. }
            | Self::ToolUpdate { session_id, .. }
            | Self::ToolEnd { session_id, .. }
            | Self::Settled { session_id, .. }
            | Self::Error { session_id, .. } => session_id,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalRun {
    pub run_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    pub name: String,
    pub status: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalWireEvent {
    pub attachment_id: String,
    pub workspace_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    pub run_id: String,
    #[serde(flatten)]
    pub frame: TerminalFrame,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalFrame {
    Restore {
        sequence: u64,
        bytes: Vec<u8>,
        cols: usize,
        rows: usize,
    },
    Output {
        sequence: u64,
        bytes: Vec<u8>,
    },
    Exit {
        code: Option<i32>,
    },
    Error {
        message: String,
    },
    Closed {
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extending_focused_history_keeps_the_opposite_cursor() {
        let mut window: HistoryPageMeta = serde_json::from_value(json!({
            "snapshot": "source-1",
            "startIndex": 50, "hasOlder": true, "nextBeforeIndex": 50,
            "nextBeforeMessageId": "m50", "hasNewer": true,
            "nextAfterIndex": 149, "nextAfterMessageId": "m149"
        })).unwrap();
        let older: HistoryPageMeta = serde_json::from_value(json!({
            "snapshot": "source-1",
            "startIndex": 0, "hasOlder": false, "hasNewer": true,
            "nextAfterIndex": 49, "nextAfterMessageId": "m49"
        })).unwrap();
        assert!(older.matches_snapshot(window.snapshot.as_deref()));
        window.extend(older, false);
        assert!(!window.has_older);
        assert_eq!(window.next_before_index, None);
        assert_eq!(window.next_after_message_id.as_deref(), Some("m149"));

        let newer: HistoryPageMeta = serde_json::from_value(json!({
            "snapshot": "source-1",
            "startIndex": 150, "hasOlder": true, "nextBeforeIndex": 150,
            "nextBeforeMessageId": "m150", "hasNewer": false
        })).unwrap();
        window.extend(newer, true);
        assert_eq!(window.start_index, 0);
        assert_eq!(window.next_before_message_id, None);
        assert!(!window.has_newer);
        assert_eq!(window.next_after_index, None);
        assert_eq!(window.snapshot.as_deref(), Some("source-1"));
        assert!(!window.matches_snapshot(Some("source-2")));
        assert!(window.matches_snapshot(None));
    }

    #[test]
    fn initialization_wire_fields_preserve_script_and_recovery_actions() {
        let initialization: Initialization = serde_json::from_value(json!({
            "version": "setup-revision-1",
            "status": "failed",
            "script": {"name": "prepare worktree"},
            "errorCode": "SCRIPT_FAILED",
            "runId": "script-run-1",
            "actions": {"retry": true, "skip": false, "reason": "Review script output"}
        }))
        .unwrap();
        assert_eq!(initialization.version, "setup-revision-1");
        assert_eq!(initialization.status, "failed");
        assert_eq!(initialization.script.unwrap().name, "prepare worktree");
        assert_eq!(initialization.error_code.as_deref(), Some("SCRIPT_FAILED"));
        assert_eq!(initialization.run_id.as_deref(), Some("script-run-1"));
        let actions = initialization.actions.unwrap();
        assert!(actions.retry);
        assert!(!actions.skip);
        assert_eq!(actions.reason.as_deref(), Some("Review script output"));
    }

    #[test]
    fn queued_attachment_and_failure_wire_fields_preserve_original_data() {
        let attachment = json!({"id":"upload-1","workspaceId":"workspace-1","name":"角色模板.txt","mimeType":"text/plain","size":23});
        let snapshot: OpenSession = serde_json::from_value(json!({
            "id":"chat-1","workspaceId":"workspace-1","title":"成员职责","backend":"pi-rpc",
            "historyMessageIds":[],"activeMessage":null,"eventSequence":17,"pending":1,
            "pendingUserInputs":[{"deliveryId":"delivery-1","text":"原始输入","attachments":[attachment.clone()],"processing":false}],
            "deliveryFailure":{"deliveryId":"delivery-1","message":"原生诊断：原文保留"},
            "model":{"provider":"openai","id":"gpt-6.1-terra"},"unavailableModel":null
        })).unwrap();
        assert_eq!(snapshot.session.title, "成员职责");
        assert!(matches!(snapshot.session.backend, Backend::PiRpc));
        assert_eq!(snapshot.event_sequence, 17);
        assert_eq!(snapshot.pending_user_inputs.len(), 1);
        let input = &snapshot.pending_user_inputs[0];
        assert_eq!(input.delivery_id, "delivery-1");
        assert_eq!(input.text, "原始输入");
        assert!(!input.processing);
        assert_eq!(input.attachments.len(), 1);
        assert_eq!(
            serde_json::to_value(&input.attachments[0]).unwrap(),
            attachment
        );
        let failure = snapshot.delivery_failure.unwrap();
        assert_eq!(failure.delivery_id, "delivery-1");
        assert_eq!(failure.message, "原生诊断：原文保留");
        assert!(
            serde_json::from_value::<DeliveryFailure>(
                json!({"delivery_id":"wrong-case","message":"diagnostic"})
            )
            .is_err()
        );
    }
}
