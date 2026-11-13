mod view;

use crate::{app::HiveApp, language::{english, tr}};
use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
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
    pub(super) scroll: ScrollHandle,
    query: Entity<InputState>,
    status: &'static str,
    scope: &'static str,
    workspace: Option<String>,
    entries: Vec<MemoryEntry>,
    selected: Option<String>,
    enabled: Option<bool>,
    loading: bool,
    acting: bool,
    error: Option<MemoryError>,
    settings_error: Option<String>,
    notice: Option<&'static str>,
    generation: u64,
    pending_actions: HashSet<String>,
    action_results: HashMap<String, Result<(), MemoryError>>,
    _query_subscription: Subscription,
}
#[derive(Clone)]
enum MemoryError { Load(String), Save(String) }
impl MemoryError {
    fn text(&self) -> String {
        let (key, error) = match self {
            Self::Load(error) => ("读取记忆失败", error),
            Self::Save(error) => ("保存记忆变更失败", error),
        };
        format!("{}: {}", tr(key), crate::language::localize_owned_error(error))
    }
}
impl Memory {
    pub(crate) fn localize_placeholder(&self, window: &mut Window, cx: &mut App) {
        self.query.update(cx, |input, cx| input.set_placeholder(tr("搜索记忆内容…"), window, cx));
    }
    pub fn new(window: &mut Window, cx: &mut Context<HiveApp>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索记忆内容…")));
        let subscription = cx.subscribe(&query, |this: &mut HiveApp, _, event: &InputEvent, cx| {
            if matches!(
                event,
                InputEvent::PressEnter {
                    secondary: false,
                    shift: false,
                }
            ) && this.connected
                && !this.memory.loading
                && !this.memory.acting
            {
                this.memory.selected = None;
                this.memory.scroll.set_offset(point(px(0.), px(0.)));
                this.load_memory(cx);
            }
        });
        Self {
            visible: false,
            scroll: ScrollHandle::new(),
            query,
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
            _query_subscription: subscription,
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
        "active" => tr("有效"),
        "candidate" => tr("待审核"),
        "rejected" => tr("已拒绝"),
        "archived" => tr("已归档"),
        _ => status,
    }
}
fn kind_label(kind: &str) -> &str {
    match kind {
        "fact" => tr("事实"),
        "preference" => tr("偏好"),
        "decision" => tr("决策"),
        "pitfall" => tr("注意事项"),
        "procedure_ref" => tr("流程引用"),
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
        tr("刚刚").into()
    } else if age < 3600 {
        if english() { format!("{}m ago", age / 60) } else { format!("{} 分钟前", age / 60) }
    } else if age < 86400 {
        if english() { format!("{}h ago", age / 3600) } else { format!("{} 小时前", age / 3600) }
    } else {
        if english() { format!("{}d ago", age / 86400) } else { format!("{} 天前", age / 86400) }
    }
}
fn source_label(value: &str) -> &str {
    match value {
        "manual" => tr("手动记录"),
        "message" => tr("消息"),
        "dispatch" => tr("派单"),
        "report" => tr("汇报"),
        "dream" => tr("后台整理"),
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
        self.close_workflows();
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
            m.scroll.set_offset(point(px(0.), px(0.)));
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
            .map(|_| "已保存");
        m.settings_error = None;
        m.entries.clear();
        m.enabled = None;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let list = runtime.request("hive.request", json!({"method":"GET","path":path})).await.and_then(|value| serde_json::from_value::<MemoryList>(value).map_err(|error| error.to_string()));
            let settings = runtime.request("hive.request", json!({"method":"GET","path":format!("/api/ui/workspaces/{workspace}/memory/settings")})).await.and_then(|value| serde_json::from_value::<MemorySettings>(value).map_err(|error| error.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection { return; }
                let m = &mut this.memory;
                if !m.visible || this.workspace.as_ref() != Some(&workspace) || m.generation != generation { return; }
                m.loading = false;
                match list { Ok(list) => m.entries = list.memories, Err(error) => m.error = Some(MemoryError::Load(error)) }
                match settings { Ok(settings) => m.enabled = Some(settings.enabled), Err(error) => m.settings_error = Some(error) }
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
        if !self.connected || !self.workspace_writes_allowed() {
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
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime.request("hive.request", json!({"method":method,"path":format!("/api/ui/workspaces/{workspace}/memory{suffix}"),"body":body})).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection { return; }
                let outcome = if settings {
                    result.and_then(|value| serde_json::from_value::<MemorySettings>(value).map_err(|error| error.to_string())).map(|_| ())
                } else {
                    result.and_then(|value| serde_json::from_value::<MemoryMutation>(value).map_err(|error| error.to_string())).map(|response| { let _ = response.memory; })
                }.map_err(MemoryError::Save);
                if !this.memory.pending_actions.remove(&workspace) { return; }
                this.memory.action_results.insert(workspace.clone(), outcome);
                if this.memory.visible && this.workspace.as_ref() == Some(&workspace) {
                    this.load_memory(cx);
                }
                cx.notify();
            });
        }).detach();
    }
}

impl HiveApp {
    pub(crate) fn forget_workspace_memory(&mut self, workspace: &str) {
        self.memory.pending_actions.remove(workspace);
        self.memory.action_results.remove(workspace);
        if self.memory.workspace.as_deref() == Some(workspace) {
            self.close_memory();
            self.memory.workspace = None;
            self.memory.entries.clear();
            self.memory.selected = None;
        }
    }
}

impl HiveApp {
    pub fn reset_memory_connection(&mut self) {
        self.memory.generation += 1;
        self.memory.loading = false;
        self.memory.acting = false;
        self.memory.pending_actions.clear();
    }
}
