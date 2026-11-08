use super::{Dispatch, MessagePage, PAGE, Tasks};
use crate::app::HiveApp;
use gpui_kit::*;
use serde_json::json;
use std::time::Duration;

impl HiveApp {
    pub(crate) fn start_workspace_updates(&mut self, cx: &mut Context<Self>) {
        self.workspace_updates = cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(Duration::from_secs(3)).await;
                if this
                    .update(cx, |this, cx| {
                        this.refresh_remote(cx);
                        if !this.connected
                            || this.loading
                            || this.closing
                            || this.restarting
                            || !this.workspace_writes_allowed()
                            || this.settings_panel
                            || this.roles.visible
                            || this.memory.visible
                        {
                            return;
                        }
                        if this.workflows.visible { this.refresh_workflows(cx); }
                        if !this.member_loading
                            && !this.member_form_open
                            && this.editing_member.is_none()
                            && this.deleting_member.is_none()
                            && this.member_picker_target.is_none()
                            && (this.member_panel || this.collaboration.visible)
                        {
                            this.refresh_members(cx);
                        }
                        if this.collaboration.visible {
                            if this.collaboration.selected.is_some() {
                                this.refresh_dispatch(cx);
                            } else {
                                this.refresh_dispatches(cx);
                                this.refresh_task_document(cx);
                            }
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }
    fn refresh_dispatches(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let c = &mut self.collaboration;
        if c.list_loading || c.list_refreshing {
            return;
        }
        c.list_refreshing = true;
        c.list_generation += 1;
        let generation = c.list_generation;
        let offset = c.offset;
        let filter = c.filter;
        let connection = self.connection_generation;
        cx.spawn(async move|this,cx|{
            let state=if filter=="all"{String::new()}else{format!("&state={filter}")};
            let result=runtime.request("hive.request",json!({"method":"GET","path":format!("/api/ui/workspaces/{workspace}/dispatches?limit={PAGE}&offset={offset}{state}")})).await;
            let _=this.update(cx,|this,cx|{
                if this.connection_generation!=connection{return;}
                let c=&mut this.collaboration;
                if !c.visible || this.workspace.as_ref()!=Some(&workspace) || c.list_generation!=generation{return;}
                c.list_refreshing=false;
                match result.and_then(|v|serde_json::from_value::<Vec<Dispatch>>(v).map_err(|e|e.to_string())){
                    Ok(items)=>{c.dispatches=items;c.list_error=None;},
                    Err(error)=>c.list_error=Some(("派单列表未能更新，保留上次结果", error)),
                }
                cx.notify();
            });
        }).detach();
    }
    fn refresh_task_document(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let c = &mut self.collaboration;
        if c.task_loading
            || c.task_refreshing
            || c.task_drafts.contains_key(&workspace)
            || c.recovered_task_drafts.contains_key(&workspace)
        {
            return;
        }
        c.task_refreshing = true;
        c.task_generation += 1;
        let generation = c.task_generation;
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "hive.request",
                    json!({"method":"GET","path":format!("/api/workspaces/{workspace}/tasks")}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                let c = &mut this.collaboration;
                if !c.visible
                    || this.workspace.as_ref() != Some(&workspace)
                    || c.task_generation != generation
                {
                    return;
                }
                c.task_refreshing = false;
                match result
                    .and_then(|v| serde_json::from_value::<Tasks>(v).map_err(|e| e.to_string()))
                {
                    Ok(tasks) => {
                        if c.task_text != tasks.content {
                            c.task_text = tasks.content;
                            c.tasks
                                .update(cx, |state, cx| state.set_text(&c.task_text, cx));
                        }
                        c.task_error = None;
                    }
                    Err(error) => {
                        c.task_error = Some(("任务文档未能更新，保留上次结果", error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn refresh_dispatch_messages(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime), Some(dispatch)) = (
            self.workspace.clone(),
            self.runtime.clone(),
            self.collaboration.selected.as_ref().map(|d| d.id.clone()),
        ) else {
            return;
        };
        let c = &mut self.collaboration;
        if c.message_loading || c.message_refreshing {
            return;
        }
        // Refresh exactly the loaded range, rather than collapsing an expanded
        // history back to page one or fetching an unbounded entire collaboration.
        let pages = c.messages.len().max(1).div_ceil(100);
        c.message_refreshing = true;
        c.message_generation += 1;
        let generation = c.message_generation;
        let connection = self.connection_generation;
        cx.spawn(async move|this,cx|{
            let result=async{
                let mut messages=Vec::new();let mut cursor:Option<String>=None;let mut responsibilities=Vec::new();
                for _ in 0..pages{
                    let mut query=url::form_urlencoded::Serializer::new(String::new());query.append_pair("scope","collaboration");if let Some(after)=&cursor {query.append_pair("after_message_id",after);}
                    let value=runtime.request("hive.request",json!({"method":"GET","path":format!("/api/ui/workspaces/{workspace}/dispatches/{dispatch}/messages?{}",query.finish())})).await?;
                    let page:MessagePage=serde_json::from_value(value).map_err(|e|e.to_string())?;
                    messages.extend(page.messages);responsibilities=page.related_dispatches;cursor=page.next_after_message_id;if cursor.is_none(){break;}
                }
                Ok::<_,String>((messages,responsibilities,cursor))
            }.await;
            let _=this.update(cx,|this,cx|{
                if this.connection_generation!=connection{return;}
                let c=&mut this.collaboration;
                if !c.visible || this.workspace.as_ref()!=Some(&workspace) || c.message_generation!=generation || !c.selected.as_ref().is_some_and(|d|d.id==dispatch){return;}
                c.message_refreshing=false;
                match result{
                    Ok((messages,responsibilities,cursor))=>{c.messages=messages;c.responsibilities=responsibilities;c.next_message=cursor;c.message_error=None;c.message_refresh_failed=false;},
                    Err(error)=>{c.message_refresh_failed=true;c.message_error=Some(("协作消息未能更新，保留上次结果", error));},
                }
                cx.notify();
            });
        }).detach();
    }
}
