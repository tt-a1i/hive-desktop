use serde::Deserialize;
use serde_json::Value;

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
    pub name: String,
    pub role: String,
    pub status: String,
    #[serde(default)]
    pub configured_command: Option<String>,
    pub desktop_model: Option<ModelIdentity>,
}
impl Member {
    pub fn role_label(&self) -> &str {
        match self.role.as_str() {
            "coder" => "开发",
            "reviewer" => "审查",
            "tester" => "测试",
            "custom" => "自定义",
            other => other,
        }
    }
    pub fn status_label(&self) -> &str {
        match self.status.as_str() {
            "idle" => "待命",
            "working" => "工作中",
            "stopped" => "已停止",
            other => other,
        }
    }
    pub fn backend_label(&self) -> &str {
        match self.configured_command.as_deref() {
            Some("hive:pi") => "Pi",
            Some("hive:codex") => "Codex",
            Some("hive:pi-rpc") => "外部 Pi",
            Some("hive:claude") => "Claude",
            Some(_) => "其他后端",
            None => "未配置后端",
        }
    }
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
        completed_report_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    AssistantDelta {
        session_id: String,
        sequence: u64,
        message_id: String,
        kind: String,
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
    },
}
impl PiEvent {
    pub fn sequence(&self) -> u64 {
        match self {
            Self::Interaction { sequence, .. }
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
            Self::Interaction { session_id, .. }
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
