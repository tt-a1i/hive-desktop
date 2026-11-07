use crate::app::HiveApp;
use gpui_kit::*;
use serde_json::json;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRemovalOutcome {
    ok: bool,
    removed_session_ids: Option<Vec<String>>,
    error: Option<WorkspaceRemovalFailure>,
}
#[derive(serde::Deserialize)]
struct WorkspaceRemovalFailure {
    message: String,
    details: WorkspaceRemovalDetails,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRemovalDetails {
    workspace_id: String,
    workspace_removed: Option<bool>,
    removed_session_ids: Vec<String>,
}

impl HiveApp {
    pub fn choose_workspace_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择项目文件夹".into()),
        });
        let window = window.window_handle();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.first() {
                    if let Some(path) = path.to_str() {
                        let _ = window.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |this, cx| {
                                if this.connection_generation != connection {
                                    return;
                                }
                                if this.connected {
                                    this.folder
                                        .update(cx, |input, cx| input.set_value(path, window, cx));
                                    this.add_workspace(window, cx);
                                }
                            });
                        });
                    } else {
                        let _ = this.update(cx, |this, cx| {
                            if this.connection_generation != connection {
                                return;
                            }
                            this.error = Some("所选文件夹路径无法显示为文本".into());
                            cx.notify();
                        });
                    }
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(error)) => {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return;
                    }
                    this.error = Some(format!("选择文件夹失败：{error}"));
                    cx.notify();
                });
            }
            Err(error) => {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return;
                    }
                    this.error = Some(format!("文件夹选择器已关闭：{error}"));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn open_workspace_target(
        &mut self,
        workspace: String,
        target: &'static str,
        cx: &mut Context<Self>,
    ) {
        if !self.connected {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime.request("hive.request", json!({"method":"POST", "path":format!("/api/workspaces/{workspace}/open"), "body":{"target_id":target}})).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection { return; }
                if this.workspace.as_ref() == Some(&workspace) {
                    if let Err(error) = result { this.error = Some(format!("无法通过 {target} 打开项目，请确认目标应用已安装：{error}")); }
                    cx.notify();
                }
            });
        }).detach();
    }
}

impl HiveApp {
    pub(crate) fn workspace_writes_allowed(&self) -> bool {
        self.workspace_removing.is_none() || self.workspace_removing != self.workspace
    }

    pub fn remove_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected || self.workspace_removing.is_some() {
            return;
        }
        let Some(workspace) = self
            .workspaces
            .iter()
            .find(|workspace| Some(&workspace.id) == self.workspace.as_ref())
            .cloned()
        else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let detail = format!(
            "{}\n{}\n\n将关闭团队与会话，并删除 Hive 中的协作记录和上传文件。项目文件夹与 Agent 原生会话记录保留。此操作无法撤销。",
            workspace.name, workspace.path
        );
        let answer = window.prompt(
            PromptLevel::Warning,
            "移除此工作区？",
            Some(&detail),
            &[
                PromptButton::cancel("取消"),
                PromptButton::new("移除工作区"),
            ],
            cx,
        );
        self.workspace_removing = Some(workspace.id.clone());
        let mut known_sessions: Vec<String> = self
            .sessions
            .iter()
            .map(|session| session.id.clone())
            .collect();
        if let Some(session) = &self.session {
            if !known_sessions.contains(&session.id) {
                known_sessions.push(session.id.clone());
            }
        }
        let window = window.window_handle();
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            if answer.await != Ok(1) {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return;
                    }
                    this.workspace_removing = None;
                    cx.notify();
                });
                return;
            }
            let id = workspace.id;
            let result = runtime
                .request("workspace.remove", json!({"workspaceId":id}))
                .await;
            let outcome = result.and_then(|value| {
                serde_json::from_value::<WorkspaceRemovalOutcome>(value)
                    .map_err(|_| "移除响应格式错误，需重新确认工作区状态".to_string())
            });
            let (reported_removed, mut session_ids, operation_error) = match outcome {
                Ok(outcome) if outcome.ok => match outcome.removed_session_ids {
                    Some(ids) => (Some(true), ids, None),
                    None => (
                        None,
                        vec![],
                        Some("移除成功响应缺少会话身份，需重新确认工作区状态".into()),
                    ),
                },
                Ok(outcome) => match outcome.error {
                    Some(error) if error.details.workspace_id == id => (
                        error.details.workspace_removed,
                        error.details.removed_session_ids,
                        Some(error.message),
                    ),
                    _ => (
                        None,
                        vec![],
                        Some("移除失败响应中的工作区身份或详情不完整".into()),
                    ),
                },
                Err(error) => (None, vec![], Some(error)),
            };
            for session in known_sessions {
                if !session_ids.contains(&session) {
                    session_ids.push(session);
                }
            }
            let refreshed = runtime
                .request(
                    "hive.request",
                    json!({"method":"GET", "path":"/api/workspaces"}),
                )
                .await
                .and_then(|value| {
                    serde_json::from_value::<Vec<crate::protocol::Workspace>>(value)
                        .map_err(|error| format!("工作区列表格式错误：{error}"))
                });
            let _ = window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return;
                    }
                    this.workspace_removing = None;
                    let absent = refreshed.as_ref().is_ok_and(|workspaces| {
                        !workspaces.iter().any(|workspace| workspace.id == id)
                    });
                    let removed = reported_removed == Some(true) || absent;
                    let error = match operation_error {
                        Some(error) => Some(format!(
                            "移除工作区未完整完成：{error}。{}",
                            if removed {
                                "已确认工作区记录不再存在；本地视图已更新。"
                            } else if refreshed.is_err() {
                                "无法确认工作区当前状态，请重新读取后再操作。"
                            } else {
                                "工作区仍存在，草稿已保留。"
                            }
                        )),
                        None if refreshed.is_err() => Some(
                            "工作区已移除，但刷新其他工作区失败。请重新启动后确认列表。".into(),
                        ),
                        _ => None,
                    };
                    if let Ok(workspaces) = refreshed {
                        this.workspaces = workspaces;
                    }
                    if removed {
                        this.forget_removed_workspace(&id, &session_ids, window, cx);
                        this.status = "工作区已移除".into();
                    }
                    this.error = error;
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn forget_removed_workspace(
        &mut self,
        workspace: &str,
        sessions: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.workspaces.retain(|item| item.id != workspace);
        self.navigation.sessions.remove(workspace);
        if self.navigation.workspace.as_deref() == Some(workspace) {
            self.navigation.workspace = None;
        }
        self.save_navigation(cx);
        let workspace_key = format!("workspace:{workspace}");
        let removed_key = |key: &str| {
            key == workspace_key || sessions.iter().any(|id| key == format!("session:{id}"))
        };
        self.drafts.retain(|key, _| !removed_key(key));
        if self.draft_owner.as_deref().is_some_and(removed_key) {
            self.draft_owner = None;
            if self.draft_target.as_deref().is_some_and(removed_key) {
                self.draft_target = None;
            }
            self.input
                .update(cx, |input, cx| input.set_value("", window, cx));
        } else if self.draft_target.as_deref().is_some_and(removed_key) {
            self.draft_target = None;
        }
        for id in sessions {
            self.positions.remove(id);
            self.removed_sessions.insert(id.clone());
            self.interactions.sessions.remove(id);
            self.busy.remove(id);
            self.pending.remove(id);
            self.unread.remove(id);
            self.failed_drafts.remove(id);
            self.background_sequences.remove(id);
        }
        self.schedule_draft_save(cx);
        self.expanded_projects.remove(workspace);
        self.project_sessions.remove(workspace);
        self.collaboration.task_drafts.remove(workspace);
        self.forget_workspace_memory(workspace);
        if self.workspace.as_deref() == Some(workspace) {
            self.close_memory();
            self.close_roles();
            self.close_collaboration();
            self.member_panel = false;
            self.workspace = None;
            self.reset_workspace_view();
            self.loading = false;
            if let Some(first) = self.workspaces.first() {
                self.select_workspace(first.id.clone(), cx);
            }
        }
    }
}
