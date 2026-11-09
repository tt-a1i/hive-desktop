use crate::language::{english, tr};
mod description;
mod view;
use crate::{
    app::HiveApp,
    protocol::{Member, MemberRun, Model},
};
use gpui_kit::{
    assets::IconName as Glyph,
    prelude::FluentBuilder,
    component::{
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenuItem},
        input::Input,
        *,
    },
    *,
};
use serde_json::{Value, json};

impl Member {
    pub fn role_label(&self) -> &str {
        crate::roles::role_label(&self.role)
    }
    pub fn status_label(&self) -> &str {
        match self.status.as_str() {
            "idle" => tr("待命"),
            "working" => tr("工作中"),
            "stopped" => tr("已停止"),
            other => other,
        }
    }
    pub fn workload_label(&self) -> String {
        if self.pending_task_count == 0 { return self.status_label().to_owned(); }
        let count = self.pending_task_count;
        if english() {
            format!("{} · {count} open {}", self.status_label(), if count == 1 { "dispatch" } else { "dispatches" })
        } else {
            format!("{} · {count} 项未结派单", self.status_label())
        }
    }
    pub fn backend_label(&self) -> &str {
        match self.configured_command.as_deref() {
            Some("hive:pi") => "Pi",
            Some("hive:codex") => "Codex",
            Some("hive:pi-rpc") => tr("外部 Pi"),
            Some("hive:claude") => "Claude",
            Some(_) => tr("其他后端"),
            None => tr("未配置后端"),
        }
    }
}

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
    pub(crate) fn member_menu(
        &self,
        member: &Member,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = member.id.clone();
        let name = member.name.clone();
        let weak = cx.weak_entity();
        Button::new(SharedString::from(format!("member-menu-{id}")))
            .ghost()
            .compact()
            .icon(Glyph::Ellipsis)
            .tooltip(tr("成员操作"))
            .accessibility_label(if english() { format!("Member actions: {}", member.name) } else { format!("{} 的成员操作", member.name) })
            .disabled(!self.connected || self.member_loading)
            .dropdown_menu(move |menu, _, _| {
                let rename = weak.clone();
                let rename_id = id.clone();
                let current_name = name.clone();
                let delete = weak.clone();
                let delete_id = id.clone();
                menu.item(
                    PopupMenuItem::new(tr("修改名称…")).on_click(move |_, window, cx| {
                        let _ = rename.update(cx, |this, cx| {
                            this.deleting_member = None;
                            this.editing_member = Some(rename_id.clone());
                            this.member_rename.update(cx, |input, cx| {
                                input.set_value(current_name.clone(), window, cx);
                            });
                            let focus = this.member_rename.read(cx).focus_handle(cx);
                            window.on_next_frame(move |window, cx| focus.focus(window, cx));
                            cx.notify();
                        });
                    }),
                )
                .separator()
                .item(PopupMenuItem::new(tr("删除成员…")).on_click(move |_, _, cx| {
                    let _ = delete.update(cx, |this, cx| {
                        this.editing_member = None;
                        this.deleting_member = Some(delete_id.clone());
                        cx.notify();
                    });
                }))
            })
    }

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
        self.member_catalog_error = None;
        self.member_catalog_loading = true;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "agent.models",
                    json!({"workspaceId":workspace,"backend":backend}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
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
                                Some((backend.to_owned(), error))
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
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("team.members", json!({"workspaceId":workspace}))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                if this.workspace.as_ref() == Some(&workspace)
                    && this.member_generation == generation
                {
                    this.member_loading = false;
                    match result {
                        Ok(value) => match serde_json::from_value::<Vec<Member>>(value) {
                            Ok(members) => {
                                this.members = members;
                                this.member_error = None;
                                this.sync_pinned_project();
                            }
                            Err(error) => {
                                this.member_error = Some(error.to_string())
                            }
                        },
                        Err(error) => {
                            this.member_error = Some(error.to_string())
                        }
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
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "hive.request",
                    json!({"method":method,"path":path,"body":body}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                if this.workspace.as_ref() == Some(&workspace)
                    && this.member_generation == generation
                {
                    this.member_loading = false;
                    match result {
                        Ok(_) => {
                            this.member_focus_search = this.editing_member.is_some() || this.deleting_member.is_some();
                            this.editing_member = None;
                            this.deleting_member = None;
                            this.refresh_members(cx);
                            this.refresh_sessions(cx);
                        }
                        Err(error) => this.error = Some(crate::app::ErrorNotice::Detail("成员操作失败",error.to_string())),
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
        self.error = None;
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
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = async {
                let member = runtime.request("hive.request", json!({"method":"POST","path":format!("/api/workspaces/{workspace}/workers"),"body":body})).await?;
                let id = member["id"].as_str().ok_or_else(|| "成员已创建，但响应缺少 ID；请刷新列表".to_string())?;
                runtime.request("hive.request", json!({"method":"POST","path":format!("/api/workspaces/{workspace}/agents/{id}/config"),"body":{"command":command,"command_preset_id":null,"args":[]}})).await.map_err(|e| format!("成员已创建，但后端配置失败：{e}"))?;
                Ok::<(), String>(())
            }.await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection { return; }
                if this.workspace.as_ref() == Some(&workspace) && this.member_generation == generation {
                    this.member_loading = false;
                    if let Err(error) = result { this.error = Some(crate::app::ErrorNotice::Detail("添加成员",error.to_string())); } else { this.member_form_open = false; this.member_focus_search = true; }
                    this.refresh_members(cx);
                    this.refresh_sessions(cx);
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
        let connection = self.connection_generation;
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
                if this.connection_generation != connection { return; }
                if this.workspace.as_ref() == Some(&workspace) && this.member_generation == generation {
                    this.member_loading = false;
                    this.member_stopping = None;
                    if let Err(error) = result { this.error = Some(crate::app::ErrorNotice::Detail("停止成员未确认",error.to_string())); }
                    this.refresh_members(cx);
                    this.refresh_sessions(cx);
                    cx.notify();
                }
            });
        }).detach();
    }
}

impl HiveApp {
    pub(crate) fn render_member_filters(
        &self,
        count: usize,
        filtering: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .gap_2()
            .child(Input::new(&self.member_search).aria_label(tr("搜索团队成员")))
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .children(
                        [
                            ("all", tr("全部")),
                            ("working", tr("工作中")),
                            ("idle", tr("待命")),
                            ("stopped", tr("已停止")),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (status, label))| {
                            let count = self
                                .members
                                .iter()
                                .filter(|member| status == "all" || member.status == status)
                                .count();
                            Button::new(("member-status-filter", i))
                                .ghost()
                                .compact()
                                .label(format!("{label} {count}"))
                                .accessibility_label(if english() { format!("Filter members: {label}") } else { format!("筛选成员：{label}") })
                                .selected(self.member_status_filter == status)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.member_status_filter = status;
                                    cx.notify();
                                }))
                        }),
                    )
                    .when(filtering, |row| {
                        row.child(
                            Button::new("clear-member-filters")
                                .ghost()
                                .compact()
                                .label(tr("清除筛选"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.member_status_filter = "all";
                                    this.member_search
                                        .update(cx, |input, cx| input.set_value("", window, cx));
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(crate::palette::Color::Muted.get(cx))
                    .child(tr("工作中表示有未完成派单；待命表示当前没有派单。")),
            )
            .when(filtering, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(crate::palette::Color::Muted.get(cx))
                        .child(if english() { format!("Showing {count} of {} members", self.members.len()) } else { format!("显示 {count} / {} 位成员", self.members.len()) }),
                )
            })
    }
}
