use crate::{
    app::{ErrorNotice, HiveApp},
    focus_scroll::{Reveal, Viewport},
    language::tr,
    palette::Color,
    protocol::Workspace,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};

#[derive(Deserialize)]
struct Discovery {
    projects: Vec<Project>,
    limited: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    id: String,
    path: String,
    name: String,
    session_count: usize,
    backends: Vec<String>,
    workspace_id: Option<String>,
}

#[derive(Deserialize)]
struct ApplyResponse {
    results: Vec<ApplyResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyResult {
    id: String,
    workspace: Option<Workspace>,
    imported: usize,
    already_imported: usize,
    skipped: usize,
    error: Option<String>,
}

impl ApplyResult {
    fn complete(&self) -> bool {
        self.workspace.is_some() && self.skipped == 0 && self.error.is_none()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Discovering,
    Applying,
}

#[derive(Default)]
pub(super) struct ProjectImport {
    connection: u64,
    pending: Option<Pending>,
    discovery: Option<Discovery>,
    selected: HashSet<String>,
    results: HashMap<String, ApplyResult>,
    scroll: ScrollHandle,
    notice: Option<ErrorNotice>,
}

impl ProjectImport {
    pub(super) fn busy(&self, connection: u64) -> bool {
        self.connection == connection && self.pending.is_some()
    }

    fn completed(&self, id: &str) -> bool {
        self.results.get(id).is_some_and(ApplyResult::complete)
    }

    fn selected_projects(&self) -> impl Iterator<Item = &Project> {
        self.discovery
            .iter()
            .flat_map(|discovery| &discovery.projects)
            .filter(|project| self.selected.contains(&project.id) && !self.completed(&project.id))
    }
}

fn backend_label(backend: &str) -> &str {
    match backend {
        "codex" => "Codex",
        "claude" => "Claude Code",
        "pi-rpc" => tr("外部 Pi"),
        other => other,
    }
}

impl HiveApp {
    fn discover_welcome_projects(&mut self, cx: &mut Context<Self>) {
        if !self.showing_welcome_guide || !self.connected || self.welcome_busy() {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        self.welcome_setup.project_import = ProjectImport {
            connection,
            pending: Some(Pending::Discovering),
            ..Default::default()
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("desktop.import.discover", json!({}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<Discovery>(value).map_err(|e| e.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                let import = &mut this.welcome_setup.project_import;
                import.pending = None;
                match result {
                    Ok(discovery) => import.discovery = Some(discovery),
                    Err(error) => {
                        import.notice = Some(ErrorNotice::Detail("查找最近项目失败", error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_welcome_projects(&mut self, cx: &mut Context<Self>) {
        if !self.showing_welcome_guide
            || !self.connected
            || self.welcome_busy()
            || self.workspace_removing.is_some()
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        let import = &mut self.welcome_setup.project_import;
        if import.connection != connection {
            return;
        }
        let ids: Vec<_> = import
            .selected_projects()
            .map(|project| project.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }
        import.pending = Some(Pending::Applying);
        import.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("desktop.import.apply", json!({"projectIds": ids}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<ApplyResponse>(value).map_err(|e| e.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                let import = &mut this.welcome_setup.project_import;
                import.pending = None;
                let response = match result {
                    Ok(response) => response,
                    Err(error) => {
                        import.notice = Some(ErrorNotice::Detail(
                            "导入结果未确认，请重试或重新查找。",
                            error,
                        ));
                        cx.notify();
                        return;
                    }
                };
                let mut first_workspace = None;
                let mut affected_workspaces = HashSet::new();
                let mut missing = ids.iter().collect::<HashSet<_>>();
                for result in response.results {
                    if !missing.remove(&result.id) {
                        continue;
                    }
                    if result.complete() {
                        import.selected.remove(&result.id);
                    }
                    if let Some(workspace) = &result.workspace {
                        affected_workspaces.insert(workspace.id.clone());
                        first_workspace.get_or_insert_with(|| workspace.id.clone());
                        if let Some(existing) = this
                            .workspaces
                            .iter_mut()
                            .find(|old| old.id == workspace.id)
                        {
                            *existing = workspace.clone();
                        } else {
                            this.workspaces.push(workspace.clone());
                        }
                    }
                    import.results.insert(result.id.clone(), result);
                }
                if !missing.is_empty() {
                    import.notice = Some("有项目未返回导入结果。请重试或重新查找。".into());
                }
                if this.showing_welcome_guide && this.welcome_projects_target().is_none() {
                    this.welcome_setup.preferred_workspace = first_workspace;
                }
                for workspace in affected_workspaces {
                    this.refresh_project_sessions(workspace, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_imported_welcome_project(
        &mut self,
        workspace_id: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !self.showing_welcome_guide
            || self.welcome_busy()
            || self.workspace_removing.is_some()
            || !self.workspaces.iter().any(|item| item.id == workspace_id)
        {
            return;
        }
        self.welcome_setup.preferred_workspace = Some(workspace_id.to_owned());
        self.finish_welcome_setup(window, cx);
    }

    pub(super) fn render_welcome_import(&self, cx: &Context<Self>) -> Div {
        let empty = ProjectImport::default();
        let import = &self.welcome_setup.project_import;
        let import = if import.connection == self.connection_generation {
            import
        } else {
            &empty
        };
        let enabled = self.connected && !self.welcome_busy() && self.workspace_removing.is_none();
        v_flex()
            .w_full()
            .p_3p5()
            .rounded_xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Card.get(cx))
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(tr("导入本机项目与 Agent 历史")),
                    )
                    .child(Reveal::new(
                        "welcome-find-recent-projects-reveal",
                        &self.welcome_scroll,
                        Button::new("welcome-find-recent-projects")
                            .outline()
                            .small()
                            .disabled(!enabled)
                            .label(match import.pending {
                                Some(Pending::Discovering) => tr("正在查找…"),
                                _ if import.discovery.is_some() => tr("重新查找"),
                                _ => tr("查找最近项目"),
                            })
                            .on_click(
                                cx.listener(|this, _, _, cx| this.discover_welcome_projects(cx)),
                            ),
                    )),
            )
            .when(import.discovery.is_none(), |panel| {
                panel.child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr(
                    "查找本机 Codex、Claude Code 和 Pi 用过的文件夹，再选择要导入的项目。",
                )))
            })
            .when_some(import.discovery.as_ref(), |panel, discovery| {
                panel.child(self.render_discovered_projects(import, discovery, enabled, cx))
            })
            .when_some(import.notice.as_ref(), |panel, notice| {
                panel.child(
                    div()
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(notice.text()),
                )
            })
    }

    fn render_discovered_projects(
        &self,
        import: &ProjectImport,
        discovery: &Discovery,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Div {
        let count = import.selected_projects().count();
        let finished = import
            .results
            .values()
            .filter(|result| result.complete())
            .count();
        let incomplete = import.results.len() - finished;
        let available = discovery
            .projects
            .iter()
            .filter(|project| !import.completed(&project.id))
            .count();
        v_flex()
            .gap_2()
            .when(discovery.limited, |panel| {
                panel.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("查找已达到扫描上限，仅显示找到的部分项目或会话。")),
                )
            })
            .when(!import.results.is_empty(), |panel| {
                panel.child(
                    div()
                        .id("welcome-import-summary")
                        .role(Role::Status)
                        .text_xs()
                        .text_color(if incomplete == 0 {
                            Color::Success.get(cx)
                        } else {
                            Color::Error.get(cx)
                        })
                        .child(format!(
                            "{} {finished} · {} {incomplete}",
                            tr("已完成项目"),
                            tr("未完成项目")
                        )),
                )
            })
            .when(discovery.projects.is_empty(), |panel| {
                panel.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("没有找到最近使用的项目文件夹。")),
                )
            })
            .when(!discovery.projects.is_empty(), |panel| {
                panel
                    .child(Reveal::new(
                        "welcome-import-selection-reveal",
                        &self.welcome_scroll,
                        h_flex()
                            .items_center()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                Button::new("welcome-import-select-all")
                                    .ghost()
                                    .small()
                                    .label(tr("全选项目"))
                                    .disabled(!enabled || count == available)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let import = &mut this.welcome_setup.project_import;
                                        if let Some(discovery) = &import.discovery {
                                            import.selected = discovery
                                                .projects
                                                .iter()
                                                .filter(|project| !import.completed(&project.id))
                                                .map(|project| project.id.clone())
                                                .collect();
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("welcome-import-clear")
                                    .ghost()
                                    .small()
                                    .label(tr("清除选择"))
                                    .disabled(!enabled || count == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.welcome_setup.project_import.selected.clear();
                                        cx.notify();
                                    })),
                            ),
                    ))
                    .child(Reveal::new(
                        "welcome-import-projects-reveal",
                        &self.welcome_scroll,
                        Viewport::new(
                            "welcome-import-viewport",
                            &import.scroll,
                            v_flex()
                                .id("welcome-import-list")
                                .role(Role::List)
                                .aria_label(tr("最近项目"))
                                .w_full()
                                .max_h(px(300.))
                                .overflow_y_scroll()
                                .track_scroll(&import.scroll)
                                .vertical_scrollbar(&import.scroll)
                                .children(discovery.projects.iter().map(|project| {
                                    Reveal::new(
                                        format!("welcome-import-reveal-{}", project.id),
                                        &import.scroll,
                                        self.render_import_project(import, project, enabled, cx),
                                    )
                                })),
                        ),
                    ))
                    .child(Reveal::new(
                        "welcome-import-apply-reveal",
                        &self.welcome_scroll,
                        h_flex()
                            .gap_2()
                            .justify_between()
                            .items_center()
                            .flex_wrap()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(format!("{} {count}", tr("已选项目"))),
                            )
                            .child(
                                Button::new("welcome-import-apply")
                                    .primary()
                                    .small()
                                    .disabled(!enabled || count == 0)
                                    .label(if import.pending == Some(Pending::Applying) {
                                        tr("正在导入…")
                                    } else if import
                                        .selected_projects()
                                        .any(|project| import.results.contains_key(&project.id))
                                    {
                                        tr("重试导入所选项目")
                                    } else {
                                        tr("导入所选项目与历史")
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.apply_welcome_projects(cx)
                                    })),
                            ),
                    ))
            })
    }

    fn render_import_project(
        &self,
        import: &ProjectImport,
        project: &Project,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Div {
        let selected = import.selected.contains(&project.id);
        let result = import.results.get(&project.id);
        let id = project.id.clone();
        let mut details = format!("{} {}", tr("可接续会话"), project.session_count);
        for backend in &project.backends {
            details.push_str(" · ");
            details.push_str(backend_label(backend));
        }
        if project.workspace_id.is_some() {
            details.push_str(" · ");
            details.push_str(tr("已在 Hive 中"));
        }
        v_flex()
            .min_w_0()
            .flex_shrink_0()
            .py_2()
            .border_t_1()
            .border_color(Color::Divider.get(cx))
            .gap_1()
            .child(
                Checkbox::new(format!("welcome-import-project-{id}"))
                    .label(project.name.clone())
                    .tooltip(project.path.clone())
                    .checked(selected || import.completed(&id))
                    .disabled(!enabled || import.completed(&id))
                    .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                        let import = &mut this.welcome_setup.project_import;
                        if *checked {
                            import.selected.insert(id.clone());
                        } else {
                            import.selected.remove(&id);
                        }
                        cx.notify();
                    })),
            )
            .child(
                v_flex()
                    .pl_6()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .min_w_0()
                            .truncate()
                            .text_color(Color::Muted.get(cx))
                            .child(project.path.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(details),
                    )
                    .when_some(result, |row, result| {
                        let complete = result.complete();
                        let workspace = result.workspace.as_ref().filter(|_| complete);
                        row.child(
                            h_flex()
                                .items_center()
                                .flex_wrap()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if complete {
                                            Color::Success.get(cx)
                                        } else {
                                            Color::Error.get(cx)
                                        })
                                        .child(format!(
                                            "{} {} · {} {} · {} {}",
                                            tr("本次导入"),
                                            result.imported,
                                            tr("已导入"),
                                            result.already_imported,
                                            tr("未导入"),
                                            result.skipped,
                                        )),
                                )
                                .when_some(workspace, |details, workspace| {
                                    let workspace_id = workspace.id.clone();
                                    details.child(
                                        Button::new(format!(
                                            "welcome-open-imported-{}",
                                            project.id
                                        ))
                                        .outline()
                                        .small()
                                        .label(tr("打开项目"))
                                        .accessibility_label(format!(
                                            "{} · {}",
                                            tr("打开项目"),
                                            project.name
                                        ))
                                        .disabled(!enabled)
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.open_imported_welcome_project(
                                                    &workspace_id,
                                                    window,
                                                    cx,
                                                );
                                            }),
                                        ),
                                    )
                                }),
                        )
                        .when_some(result.error.as_ref(), |row, error| {
                            row.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Error.get(cx))
                                    .child(error.clone()),
                            )
                        })
                    }),
            )
    }
}
