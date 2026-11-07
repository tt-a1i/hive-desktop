mod view;

use crate::app::HiveApp;
use gpui_kit::{component::input::InputState, *};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Deserialize)]
pub struct MemoryEntry {
    id: String,
    body: String,
    kind: String,
    scope: String,
    status: String,
    tags: Vec<String>,
    pinned: bool,
    disabled: bool,
    updated_at: u64,
    procedure_ref: Option<ProcedureRef>,
    sources: Vec<MemorySource>,
}
#[derive(Clone, Deserialize)]
struct ProcedureRef {
    id: String,
    title: Option<String>,
    #[serde(rename = "type")]
    ref_type: String,
}
#[derive(Clone, Deserialize)]
struct MemorySource {
    source_type: String,
    source_id: Option<String>,
    source_sequence: Option<u64>,
    actor_name_snapshot: Option<String>,
    excerpt: Option<String>,
}
#[derive(Deserialize)]
struct MemoryList {
    memories: Vec<MemoryEntry>,
}
#[derive(Deserialize)]
struct MemorySettings {
    enabled: bool,
}
#[derive(Deserialize)]
struct MemoryMutation {
    memory: MemoryEntry,
}

pub struct Memory {
    pub visible: bool,
    query: Entity<InputState>,
    status: &'static str,
    scope: &'static str,
    workspace: Option<String>,
    entries: Vec<MemoryEntry>,
    selected: Option<String>,
    enabled: Option<bool>,
    loading: bool,
    acting: bool,
    error: Option<String>,
    settings_error: Option<String>,
    notice: Option<String>,
    generation: u64,
    pending_actions: HashSet<String>,
    action_results: HashMap<String, Result<(), String>>,
}
impl Memory {
    pub fn new(window: &mut Window, cx: &mut Context<HiveApp>) -> Self {
        Self {
            visible: false,
            query: cx.new(|cx| InputState::new(window, cx).placeholder("搜索记忆内容…")),
            status: "active",
            scope: "workspace",
            workspace: None,
            entries: vec![],
            selected: None,
            enabled: None,
            loading: false,
            acting: false,
            error: None,
            settings_error: None,
            notice: None,
            generation: 0,
            pending_actions: HashSet::new(),
            action_results: HashMap::new(),
        }
    }
}

fn encoded(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}
fn status_label(status: &str) -> &str {
    match status {
        "active" => "有效",
        "candidate" => "待审核",
        "rejected" => "已拒绝",
        "archived" => "已归档",
        _ => status,
    }
}
fn kind_label(kind: &str) -> &str {
    match kind {
        "fact" => "事实",
        "preference" => "偏好",
        "decision" => "决策",
        "pitfall" => "注意事项",
        "procedure_ref" => "流程引用",
        _ => kind,
    }
}

fn updated_label(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let age = now.saturating_sub(timestamp / 1000);
    if age < 60 {
        "刚刚".into()
    } else if age < 3600 {
        format!("{} 分钟前", age / 60)
    } else if age < 86400 {
        format!("{} 小时前", age / 3600)
    } else {
        format!("{} 天前", age / 86400)
    }
}
fn source_label(value: &str) -> &str {
    match value {
        "manual" => "手动记录",
        "message" => "消息",
        "dispatch" => "派单",
        "report" => "汇报",
        "dream" => "后台整理",
        _ => value,
    }
}

impl HiveApp {
    pub fn close_memory(&mut self) {
        self.memory.visible = false;
        self.memory.generation += 1;
        self.memory.loading = false;
        self.memory.acting = false;
    }
    pub fn open_memory(&mut self, cx: &mut Context<Self>) {
        self.close_roles();
        self.close_settings();
        self.close_collaboration();
        self.member_panel = false;
        self.memory.visible = true;
        self.load_memory(cx);
    }
    pub fn memory_workspace_changed(&mut self, cx: &mut Context<Self>) {
        self.load_memory(cx);
    }
    pub fn load_memory(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let m = &mut self.memory;
        if m.workspace.as_ref() != Some(&workspace) {
            m.selected = None;
        }
        m.workspace = Some(workspace.clone());
        m.generation += 1;
        let generation = m.generation;
        let path = format!(
            "/api/ui/workspaces/{workspace}/memory?status={}&scope={}&query={}&limit=50",
            m.status,
            m.scope,
            encoded(m.query.read(cx).value().trim())
        );
        m.loading = true;
        m.acting = m.pending_actions.contains(&workspace);
        m.error = m
            .action_results
            .get(&workspace)
            .and_then(|result| result.as_ref().err())
            .cloned();
        m.notice = m
            .action_results
            .get(&workspace)
            .filter(|result| result.is_ok())
            .map(|_| "已保存".into());
        m.settings_error = None;
        m.entries.clear();
        m.enabled = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let list = runtime.request("hive.request", json!({"method":"GET","path":path})).await.and_then(|value| serde_json::from_value::<MemoryList>(value).map_err(|error| error.to_string()));
            let settings = runtime.request("hive.request", json!({"method":"GET","path":format!("/api/ui/workspaces/{workspace}/memory/settings")})).await.and_then(|value| serde_json::from_value::<MemorySettings>(value).map_err(|error| error.to_string()));
            let _ = this.update(cx, |this, cx| {
                let m = &mut this.memory;
                if !m.visible || this.workspace.as_ref() != Some(&workspace) || m.generation != generation { return; }
                m.loading = false;
                match list { Ok(list) => m.entries = list.memories, Err(error) => m.error = Some(format!("读取记忆失败：{error}")) }
                match settings { Ok(settings) => m.enabled = Some(settings.enabled), Err(error) => m.settings_error = Some(format!("读取注入设置失败：{error}")) }
                cx.notify();
            });
        }).detach();
    }
    fn memory_action(
        &mut self,
        method: &'static str,
        suffix: String,
        body: Option<Value>,
        cx: &mut Context<Self>,
    ) {
        if !self.connected {
            return;
        }
        if self.memory.loading || self.memory.acting {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        if !self.memory.pending_actions.insert(workspace.clone()) {
            return;
        }
        self.memory.action_results.remove(&workspace);
        let settings = suffix == "/settings";
        self.memory.generation += 1;
        self.memory.acting = true;
        self.memory.error = None;
        self.memory.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request("hive.request", json!({"method":method,"path":format!("/api/ui/workspaces/{workspace}/memory{suffix}"),"body":body})).await;
            let _ = this.update(cx, |this, cx| {
                let outcome = if settings {
                    result.and_then(|value| serde_json::from_value::<MemorySettings>(value).map_err(|error| error.to_string())).map(|_| ())
                } else {
                    result.and_then(|value| serde_json::from_value::<MemoryMutation>(value).map_err(|error| error.to_string())).map(|response| { let _ = response.memory; })
                }.map_err(|error| format!("保存记忆变更失败：{error}"));
                this.memory.pending_actions.remove(&workspace);
                this.memory.action_results.insert(workspace.clone(), outcome);
                if this.memory.visible && this.workspace.as_ref() == Some(&workspace) {
                    this.load_memory(cx);
                }
                cx.notify();
            });
        }).detach();
    }
}
