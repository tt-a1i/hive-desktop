use crate::{
    app::HiveApp,
    protocol::{Member, MemberRun, Model},
};
use gpui_kit::*;
use serde_json::{Value, json};

#[derive(serde::Deserialize)]
struct ModelCatalog {
    models: Vec<Model>,
}
pub fn command_backend(command: Option<&str>) -> Option<&'static str> {
    match command {
        Some("hive:pi") => Some("pi"),
        Some("hive:codex") => Some("codex"),
        Some("hive:pi-rpc") => Some("pi-rpc"),
        Some("hive:claude") => Some("claude"),
        _ => None,
    }
}

impl HiveApp {
    pub fn choose_member_models(
        &mut self,
        id: String,
        command: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !self.workspace_writes_allowed() {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let Some(backend) = command_backend(command.as_deref()) else {
            self.error = Some("该成员没有桌面支持的后端配置".into());
            cx.notify();
            return;
        };
        self.member_catalog_generation += 1;
        let generation = self.member_catalog_generation;
        self.member_picker_target = Some(id.clone());
        self.member_catalog_backend = Some(backend.into());
        self.member_catalog.clear();
        self.member_selected_model = None;
        self.member_catalog_error = None;
        self.member_catalog_loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "agent.models",
                    json!({"workspaceId":workspace,"backend":backend}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() == Some(&workspace)
                    && this.member_catalog_generation == generation
                    && this.member_picker_target.as_ref() == Some(&id)
                    && this.member_catalog_backend.as_deref() == Some(backend)
                {
                    this.member_catalog_loading = false;
                    match result.and_then(|v| {
                        serde_json::from_value::<ModelCatalog>(v).map_err(|e| e.to_string())
                    }) {
                        Ok(catalog) => {
                            this.member_catalog =
                                catalog.models.into_iter().filter(|m| m.available).collect()
                        }
                        Err(error) => {
                            this.member_catalog_error =
                                Some(format!("读取 {backend} 模型失败：{error}"))
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub fn refresh_members(&mut self, cx: &mut Context<Self>) {
        if self.member_loading {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        self.member_loading = true;
        self.member_generation += 1;
        let generation = self.member_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "hive.request",
                    json!({"method":"GET","path":format!("/api/ui/workspaces/{workspace}/team")}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() == Some(&workspace)
                    && this.member_generation == generation
                {
                    this.member_loading = false;
                    match result {
                        Ok(value) => match serde_json::from_value::<Vec<Member>>(value) {
                            Ok(members) => this.members = members,
                            Err(error) => this.error = Some(format!("成员列表格式错误：{error}")),
                        },
                        Err(error) => this.error = Some(format!("读取成员失败：{error}")),
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub fn member_action(
        &mut self,
        method: &'static str,
        path: String,
        body: Option<Value>,
        cx: &mut Context<Self>,
    ) {
        if !self.workspace_writes_allowed() {
            return;
        }
        if !self.connected {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        self.member_loading = true;
        self.member_generation += 1;
        let generation = self.member_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "hive.request",
                    json!({"method":method,"path":path,"body":body}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() == Some(&workspace)
                    && this.member_generation == generation
                {
                    this.member_loading = false;
                    match result {
                        Ok(_) => {
                            this.editing_member = None;
                            this.deleting_member = None;
                            this.refresh_members(cx);
                            this.refresh_sessions(cx);
                        }
                        Err(error) => this.error = Some(format!("成员操作失败：{error}")),
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub fn add_member(&mut self, cx: &mut Context<Self>) {
        if !self.workspace_writes_allowed() {
            return;
        }
        if !self.connected {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let name = self.member_name.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.error = Some("请输入成员名称".into());
            cx.notify();
            return;
        }
        let description = self.member_description.read(cx).value().trim().to_string();
        let command = format!("hive:{}", self.member_backend);
        let mut body = json!({"name":name,"role":self.member_role,"avatar":null,"autostart":false});
        if !description.is_empty() {
            body["description"] = json!(description);
        }
        self.member_loading = true;
        self.member_generation += 1;
        let generation = self.member_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = async {
                let member = runtime.request("hive.request", json!({"method":"POST","path":format!("/api/workspaces/{workspace}/workers"),"body":body})).await?;
                let id = member["id"].as_str().ok_or_else(|| "成员已创建，但响应缺少 ID；请刷新列表".to_string())?;
                runtime.request("hive.request", json!({"method":"POST","path":format!("/api/workspaces/{workspace}/agents/{id}/config"),"body":{"command":command,"command_preset_id":null,"args":[]}})).await.map_err(|e| format!("成员已创建，但后端配置失败：{e}"))?;
                Ok::<(), String>(())
            }.await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() == Some(&workspace) && this.member_generation == generation {
                    this.member_loading = false;
                    if let Err(error) = result { this.error = Some(format!("添加成员：{error}")); }
                    this.refresh_members(cx);
                    this.refresh_sessions(cx);
                    cx.notify();
                }
            });
        }).detach();
    }
    pub fn start_member(&mut self, id: String, command: Option<String>, cx: &mut Context<Self>) {
        if !self.workspace_writes_allowed() {
            return;
        }
        if !self.connected {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let Some(command) = command.filter(|c| {
            matches!(
                c.as_str(),
                "hive:pi" | "hive:codex" | "hive:pi-rpc" | "hive:claude"
            )
        }) else {
            self.error = Some("该成员没有桌面支持的后端配置".into());
            cx.notify();
            return;
        };
        let Some((_, _, selected)) =
            self.member_selected_model
                .as_ref()
                .filter(|(target, backend, _)| {
                    target == &id && Some(backend.as_str()) == command_backend(Some(&command))
                })
        else {
            self.error = Some("请先为此成员选择其后端的可用模型".into());
            cx.notify();
            return;
        };
        let provider = selected.provider.clone();
        let model = selected.id.clone();
        self.member_loading = true;
        self.member_generation += 1;
        let generation = self.member_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = async {
                runtime.request("hive.request", json!({"method":"POST","path":format!("/api/workspaces/{workspace}/agents/{id}/config"),"body":{"command":command,"command_preset_id":null,"args":["--provider",provider,"--model",model]}})).await?;
                runtime.request("hive.request", json!({"method":"POST","path":format!("/api/workspaces/{workspace}/agents/{id}/start")})).await?;
                Ok::<(), String>(())
            }.await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() == Some(&workspace) && this.member_generation == generation {
                    this.member_loading = false;
                    match result { Ok(()) => { this.refresh_members(cx); this.refresh_sessions(cx); }, Err(error) => this.error = Some(format!("启动成员失败：{error}")) }
                    cx.notify();
                }
            });
        }).detach();
    }
    pub fn stop_member(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.workspace_writes_allowed() {
            return;
        }
        if !self.connected {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        self.member_loading = true;
        self.member_generation += 1;
        let generation = self.member_generation;
        self.member_stopping = Some(id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let operation = async {
                let value = runtime.request("hive.request", json!({"method":"GET","path":format!("/api/ui/workspaces/{workspace}/runs")})).await?;
                let runs: Vec<MemberRun> = serde_json::from_value(value).map_err(|e| format!("运行列表格式错误：{e}"))?;
                let run = runs.iter().find(|run| run.agent_id == id && matches!(run.status.as_str(), "idle" | "working" | "running" | "starting"))
                    .ok_or_else(|| "找不到该成员的活动运行，请刷新状态".to_string())?;
                let run_id = run.run_id.clone();
                runtime.request("hive.request", json!({"method":"POST","path":format!("/api/runtime/runs/{run_id}/stop")})).await?;
                loop {
                    smol::Timer::after(std::time::Duration::from_millis(300)).await;
                    let value = runtime.request("hive.request", json!({"method":"GET","path":format!("/api/ui/workspaces/{workspace}/runs")})).await?;
                    let runs: Vec<MemberRun> = serde_json::from_value(value).map_err(|e| format!("运行列表格式错误：{e}"))?;
                    if !runs.iter().any(|run| run.run_id == run_id && matches!(run.status.as_str(), "idle" | "working" | "running" | "starting")) { return Ok::<(), String>(()); }
                }
            };
            let result = smol::future::or(operation, async {
                smol::Timer::after(std::time::Duration::from_secs(10)).await;
                Err("等待停止确认超时；请求可能已送达，请刷新后查看状态".to_string())
            }).await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() == Some(&workspace) && this.member_generation == generation {
                    this.member_loading = false;
                    this.member_stopping = None;
                    if let Err(error) = result { this.error = Some(format!("停止成员未确认：{error}")); }
                    this.refresh_members(cx);
                    this.refresh_sessions(cx);
                    cx.notify();
                }
            });
        }).detach();
    }
}
