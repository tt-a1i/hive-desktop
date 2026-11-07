mod tasks;

use crate::app::HiveApp;
use gpui_kit::{component::text::TextViewState, *};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

const PAGE: usize = 20;
#[derive(Clone, Deserialize)]
pub struct Dispatch {
    pub id: String,
    pub state: String,
    pub text: String,
    pub to_agent_id: String,
    pub from_agent_id: Option<String>,
    pub report_text: Option<String>,
    pub artifacts: Vec<String>,
}
#[derive(Clone, Deserialize)]
pub struct DispatchMessage {
    pub id: String,
    pub dispatch_id: String,
    pub reply_to: Option<String>,
    pub from_agent_id: String,
    pub recipient_agent_id: String,
    pub kind: String,
    pub text: String,
    pub delivery_state: String,
    pub delivery_error: Option<String>,
}
#[derive(Deserialize)]
pub struct Responsibility {
    pub id: String,
    pub to_agent_id: String,
    pub owner_name: String,
    pub text: String,
    pub state: String,
}
#[derive(Deserialize)]
struct MessagePage {
    messages: Vec<DispatchMessage>,
    related_dispatches: Vec<Responsibility>,
    next_after_message_id: Option<String>,
}
#[derive(Deserialize)]
pub(crate) struct Tasks {
    pub content: String,
}
pub struct Collaboration {
    pub visible: bool,
    pub tasks: Entity<TextViewState>,
    pub task_text: String,
    pub task_loading: bool,
    pub task_error: Option<String>,
    pub task_generation: u64,
    pub task_drafts: HashMap<String, tasks::TaskDraft>,
    pub filter: &'static str,
    pub offset: usize,
    pub dispatches: Vec<Dispatch>,
    pub list_loading: bool,
    pub list_error: Option<String>,
    pub list_generation: u64,
    pub selected: Option<Dispatch>,
    pub chat_origin: Option<String>,
    pub detail_scroll: ScrollHandle,
    pub detail_error: Option<String>,
    detail_generation: u64,
    pub messages: Vec<DispatchMessage>,
    pub responsibilities: Vec<Responsibility>,
    pub next_message: Option<String>,
    pub message_loading: bool,
    pub requested_after: Option<String>,
    pub message_error: Option<String>,
    pub message_generation: u64,
}
impl Collaboration {
    pub fn new(cx: &mut Context<HiveApp>) -> Self {
        Self {
            visible: false,
            tasks: cx.new(|cx| TextViewState::markdown("", cx)),
            task_text: String::new(),
            task_loading: false,
            task_error: None,
            task_generation: 0,
            task_drafts: HashMap::new(),
            filter: "all",
            offset: 0,
            dispatches: vec![],
            list_loading: false,
            list_error: None,
            list_generation: 0,
            selected: None,
            chat_origin: None,
            detail_scroll: ScrollHandle::new(),
            detail_error: None,
            detail_generation: 0,
            messages: vec![],
            responsibilities: vec![],
            next_message: None,
            message_loading: false,
            requested_after: None,
            message_error: None,
            message_generation: 0,
        }
    }
}
impl HiveApp {
    pub fn close_collaboration(&mut self) {
        self.collaboration.visible = false;
        self.collaboration.task_generation += 1;
        self.collaboration.task_loading = false;
        self.collaboration.list_generation += 1;
        self.collaboration.message_generation += 1;
    }
    pub fn open_collaboration(&mut self, cx: &mut Context<Self>) {
        self.close_memory();
        self.close_roles();
        self.close_settings();
        self.member_panel = false;
        self.collaboration.visible = true;
        self.reload_collaboration(cx);
    }
    pub fn reload_collaboration(&mut self, cx: &mut Context<Self>) {
        self.collaboration.chat_origin = None;
        self.collaboration.offset = 0;
        self.collaboration.selected = None;
        self.collaboration.messages.clear();
        self.collaboration.responsibilities.clear();
        self.collaboration.next_message = None;
        self.collaboration.message_generation += 1;
        self.load_tasks(cx);
        self.load_dispatches(cx);
    }
    pub(crate) fn load_tasks(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let c = &mut self.collaboration;
        c.task_generation += 1;
        if c.task_drafts.contains_key(&workspace) {
            c.task_loading = false;
            cx.notify();
            return;
        }
        let generation = c.task_generation;
        c.task_loading = true;
        c.task_error = None;
        c.task_text.clear();
        c.tasks.update(cx, |state, cx| state.set_text("", cx));
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "hive.request",
                    json!({"method":"GET","path":format!("/api/workspaces/{workspace}/tasks")}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                let c = &mut this.collaboration;
                if c.visible
                    && this.workspace.as_ref() == Some(&workspace)
                    && c.task_generation == generation
                {
                    c.task_loading = false;
                    match result
                        .and_then(|v| serde_json::from_value::<Tasks>(v).map_err(|e| e.to_string()))
                    {
                        Ok(tasks) => {
                            c.task_text = tasks.content;
                            c.tasks
                                .update(cx, |state, cx| state.set_text(&c.task_text, cx));
                        }
                        Err(e) => c.task_error = Some(format!("读取任务失败：{e}")),
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub fn load_dispatches(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let c = &mut self.collaboration;
        c.list_generation += 1;
        let generation = c.list_generation;
        let offset = c.offset;
        let filter = c.filter;
        c.list_loading = true;
        c.list_error = None;
        c.dispatches.clear();
        c.selected = None;
        c.messages.clear();
        c.responsibilities.clear();
        c.next_message = None;
        c.message_generation += 1;
        c.message_loading = false;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let state = if filter == "all" { String::new() } else { format!("&state={filter}") };
            let result = runtime.request("hive.request", json!({"method":"GET","path":format!("/api/ui/workspaces/{workspace}/dispatches?limit={PAGE}&offset={offset}{state}")})).await;
            let _ = this.update(cx, |this, cx| {
                let c = &mut this.collaboration;
                if c.visible && this.workspace.as_ref() == Some(&workspace) && c.list_generation == generation {
                    c.list_loading = false;
                    match result.and_then(|v| serde_json::from_value::<Vec<Dispatch>>(v).map_err(|e| e.to_string())) { Ok(items) => c.dispatches = items, Err(e) => c.list_error = Some(format!("读取派单失败：{e}")) }
                    cx.notify();
                }
            });
        }).detach();
    }
    pub fn open_dispatch_from_chat(&mut self, id: String, cx: &mut Context<Self>) {
        let (Some(workspace), Some(session), Some(runtime)) = (
            self.workspace.clone(),
            self.session.as_ref(),
            self.runtime.clone(),
        ) else {
            return;
        };
        let origin = session.id.clone();
        let open_request = self.open_request;
        self.collaboration.detail_generation += 1;
        let generation = self.collaboration.detail_generation;
        cx.spawn(async move |this, cx| {
            let result = read_dispatch(&runtime, &workspace, &id).await;
            let _ = this.update(cx, |this, cx| {
                if this.collaboration.detail_generation != generation
                    || this.open_request != open_request
                    || !this.viewing_conversation()
                    || this.workspace.as_ref() != Some(&workspace)
                {
                    return;
                }
                match result {
                    Ok(record) => {
                        let c = &mut this.collaboration;
                        c.chat_origin = Some(origin);
                        c.visible = true;
                        this.show_dispatch(record, cx);
                    }
                    Err(error) => this.error = Some(format!("无法打开派单：{error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn select_dispatch(&mut self, item: Dispatch, cx: &mut Context<Self>) {
        self.show_dispatch(item, cx);
        self.load_dispatch_record(cx);
    }
    fn show_dispatch(&mut self, item: Dispatch, cx: &mut Context<Self>) {
        let c = &mut self.collaboration;
        c.detail_scroll = ScrollHandle::new();
        c.detail_error = None;
        c.selected = Some(item);
        c.messages.clear();
        c.responsibilities.clear();
        c.next_message = None;
        self.load_dispatch_messages(None, cx);
    }
    pub fn refresh_dispatch(&mut self, cx: &mut Context<Self>) {
        self.load_dispatch_record(cx);
        self.load_dispatch_messages(None, cx);
    }
    pub fn load_dispatch_record(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime), Some(id)) = (
            self.workspace.clone(),
            self.runtime.clone(),
            self.collaboration
                .selected
                .as_ref()
                .map(|item| item.id.clone()),
        ) else {
            return;
        };
        self.collaboration.detail_generation += 1;
        let generation = self.collaboration.detail_generation;
        self.collaboration.detail_error = None;
        cx.spawn(async move |this, cx| {
            let result = read_dispatch(&runtime, &workspace, &id).await;
            let _ = this.update(cx, |this, cx| {
                let c = &mut this.collaboration;
                if this.workspace.as_ref() != Some(&workspace)
                    || c.detail_generation != generation
                    || !c.selected.as_ref().is_some_and(|d| d.id == id)
                {
                    return;
                }
                match result {
                    Ok(record) => {
                        if let Some(row) = c.dispatches.iter_mut().find(|row| row.id == id) {
                            row.clone_from(&record);
                        }
                        c.selected = Some(record);
                    }
                    Err(error) => c.detail_error = Some(format!("未能更新派单详情：{error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn load_dispatch_messages(&mut self, after: Option<String>, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime), Some(dispatch)) = (
            self.workspace.clone(),
            self.runtime.clone(),
            self.collaboration.selected.as_ref().map(|d| d.id.clone()),
        ) else {
            return;
        };
        let c = &mut self.collaboration;
        c.message_generation += 1;
        let generation = c.message_generation;
        c.message_loading = true;
        c.requested_after = after.clone();
        c.message_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mut query = url::form_urlencoded::Serializer::new(String::new());
            query.append_pair("scope", "collaboration");
            if let Some(cursor) = &after {
                query.append_pair("after_message_id", cursor);
            }
            let path = format!(
                "/api/ui/workspaces/{workspace}/dispatches/{dispatch}/messages?{}",
                query.finish()
            );
            let result = runtime
                .request("hive.request", json!({"method":"GET", "path":path}))
                .await;
            let _ = this.update(cx, |this, cx| {
                let c = &mut this.collaboration;
                if c.visible
                    && this.workspace.as_ref() == Some(&workspace)
                    && c.message_generation == generation
                    && c.selected.as_ref().is_some_and(|d| d.id == dispatch)
                {
                    c.message_loading = false;
                    match result.and_then(|v| {
                        serde_json::from_value::<MessagePage>(v).map_err(|e| e.to_string())
                    }) {
                        Ok(page) => {
                            if after.is_none() {
                                c.messages.clear();
                            }
                            c.messages.extend(page.messages);
                            c.responsibilities = page.related_dispatches;
                            c.next_message = page.next_after_message_id;
                        }
                        Err(e) => c.message_error = Some(format!("读取消息失败：{e}")),
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
async fn read_dispatch(
    runtime: &crate::transport::Runtime,
    workspace: &str,
    id: &str,
) -> Result<Dispatch, String> {
    let path = format!("/api/ui/workspaces/{workspace}/dispatches/{id}");
    let value = runtime
        .request("hive.request", json!({"method":"GET","path":path}))
        .await?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}
pub fn dispatch_state(value: &str) -> &str {
    match value {
        "queued" => "排队中",
        "submitted" => "已提交",
        "reported" => "已汇报",
        "cancelled" => "已取消",
        _ => value,
    }
}
pub fn message_kind(value: &str) -> &str {
    match value {
        "note" => "说明",
        "question" => "问题",
        "answer" => "回答",
        "progress" => "进展",
        _ => value,
    }
}
pub fn delivery_state(value: &str) -> &str {
    match value {
        "recorded" => "已记录",
        "queued" => "待投递",
        "delivering" => "投递中",
        "delivered" => "已投递",
        "cancelled" => "投递取消",
        _ => value,
    }
}
