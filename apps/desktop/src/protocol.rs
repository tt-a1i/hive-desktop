use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentRef {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
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
pub struct Session {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub custom_title: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub updated_at: i64,
    pub workspace_id: String,
    pub agent_id: Option<String>,
    #[serde(default)]
    pub backend: Backend,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHit {
    pub session: Session,
    pub workspace_name: String,
    pub title: String,
}
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Pi,
    Codex,
    #[serde(rename = "pi-rpc")]
    PiRpc,
    Claude,
}
impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pi => "Pi",
            Self::Codex => "Codex",
            Self::PiRpc => "外部 Pi",
            Self::Claude => "Claude",
        }
    }
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
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSession {
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
            Self::SessionNavigation { sequence, .. }
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
            Self::SessionNavigation { session_id, .. }
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
    pub name: String,
    pub status: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalWireEvent {
    pub attachment_id: String,
    pub workspace_id: String,
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
        assert_eq!(serde_json::to_value(&input.attachments[0]).unwrap(), attachment);
        let failure = snapshot.delivery_failure.unwrap();
        assert_eq!(failure.delivery_id, "delivery-1");
        assert_eq!(failure.message, "原生诊断：原文保留");
        assert!(serde_json::from_value::<DeliveryFailure>(json!({"delivery_id":"wrong-case","message":"diagnostic"})).is_err());
    }
}
