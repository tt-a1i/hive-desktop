use crate::{
    agent_profiles::{ProfileRef, supported_driver},
    app::{ErrorNotice, HiveApp, SettingsSection},
    focus_scroll::Viewport,
    language::tr,
    palette::Color,
    protocol::{Session, Workspace},
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenuItem},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const COMPLETION_FILE: &str = "desktop-welcome.complete";

#[derive(Default)]
pub(crate) struct Setup {
    startup_checked: bool,
    pub(super) preferred_workspace: Option<String>,
    preferred_agent: Option<ProfileRef>,
    pub(super) project_import: super::projects::ProjectImport,
    pub(crate) adding: bool,
    pub(crate) starting: bool,
    notice: Option<ErrorNotice>,
}

struct AgentChoice {
    reference: ProfileRef,
    backend: &'static str,
    name: String,
    detected: bool,
}

fn completed(root: &Path) -> io::Result<bool> {
    match fs::metadata(root.join(COMPLETION_FILE)) {
        Ok(info) if info.is_file() => Ok(true),
        Ok(_) => Err(io::Error::other("Expected a welcome completion file")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn save_completed(root: &Path) -> io::Result<()> {
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(COMPLETION_FILE))
    {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && completed(root)? => Ok(()),
        Err(error) => Err(error),
    }
}

impl HiveApp {
    pub(super) fn welcome_busy(&self) -> bool {
        self.welcome_setup.adding
            || self.welcome_setup.starting
            || self
                .welcome_setup
                .project_import
                .busy(self.connection_generation)
            || self.closing
    }

    pub(crate) fn maybe_open_first_run_welcome(&mut self, cx: &mut Context<Self>) {
        if self.welcome_setup.startup_checked {
            return;
        }
        let Some(root) = &self.data_dir else { return };
        self.welcome_setup.startup_checked = true;
        if self.workspaces.is_empty() && self.navigation.workspace.is_none() {
            match completed(root) {
                Ok(true) => (),
                Ok(false) => self.open_welcome_setup(cx),
                Err(error) => {
                    self.open_welcome_setup(cx);
                    self.welcome_setup.notice = Some(ErrorNotice::Detail(
                        "无法读取欢迎引导状态",
                        error.to_string(),
                    ));
                }
            }
        }
    }

    pub(crate) fn open_welcome_setup(&mut self, cx: &mut Context<Self>) {
        self.welcome_setup.startup_checked = true;
        self.welcome_setup.preferred_workspace = None;
        self.welcome_setup.preferred_agent = None;
        self.welcome_setup.notice = None;
        if !self
            .welcome_setup
            .project_import
            .busy(self.connection_generation)
        {
            self.welcome_setup.project_import = Default::default();
        }
        self.welcome_scroll.set_offset(point(px(0.), px(0.)));
        self.showing_welcome_guide = true;
        self.refresh_readiness(cx);
        if self.profiles.snapshot.is_none() {
            self.load_profiles(cx);
        }
        cx.notify();
    }

    fn save_welcome_completion(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(root) = &self.data_dir
            && let Err(error) = save_completed(root)
        {
            self.welcome_setup.notice = Some(ErrorNotice::Detail(
                "无法保存欢迎引导状态",
                error.to_string(),
            ));
            cx.notify();
            return false;
        }
        true
    }

    pub(super) fn finish_welcome_setup(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.welcome_busy() || !self.save_welcome_completion(cx) {
            return;
        }
        self.showing_welcome_guide = false;
        self.welcome_setup.notice = None;
        if let Some(id) = self.welcome_setup.preferred_workspace.take()
            && self.workspace.as_ref() != Some(&id)
            && self.workspaces.iter().any(|item| item.id == id)
        {
            self.select_workspace(id, cx);
        }
        if window.viewport_size().width < px(820.) {
            self.sidebar_open = false;
        }
        self.focus_project_input = true;
        cx.notify();
    }

    pub(super) fn welcome_projects_target(&self) -> Option<&Workspace> {
        let selected = self
            .welcome_setup
            .preferred_workspace
            .as_ref()
            .or(self.workspace.as_ref())?;
        self.workspaces
            .iter()
            .find(|project| &project.id == selected)
    }

    fn welcome_agents(&self) -> Vec<AgentChoice> {
        let mut choices = Vec::new();
        let Some(snapshot) = self.profiles.snapshot.as_ref() else {
            return choices;
        };
        for profile in snapshot.profiles.iter().filter(|item| item.enabled) {
            let Some(backend) = supported_driver(&profile.driver) else {
                continue;
            };
            let Some(revision) = profile
                .revisions
                .iter()
                .find(|item| item.revision == profile.current_revision)
            else {
                continue;
            };
            let reference = ProfileRef {
                id: profile.id.clone(),
                revision: profile.current_revision,
            };
            let detected = self.readiness_error.is_none()
                && profile.id == format!("default-{backend}")
                && revision.launch.config_dir.is_none()
                && revision.launch.args.is_empty()
                && self.backend_readiness(backend).is_some_and(|entry| {
                    entry.level == "ready"
                        && entry.credential_state == "present"
                        && revision
                            .launch
                            .command
                            .as_ref()
                            .is_none_or(|command| entry.executable.as_ref() == Some(command))
                });
            choices.push(AgentChoice {
                name: self.profile_label(&reference),
                reference,
                backend,
                detected,
            });
        }
        choices.sort_by_key(|choice| !choice.detected);
        choices
    }

    fn selected_welcome_agent<'a>(&self, choices: &'a [AgentChoice]) -> Option<&'a AgentChoice> {
        match &self.welcome_setup.preferred_agent {
            Some(selected) => choices.iter().find(|choice| &choice.reference == selected),
            None => choices.iter().find(|choice| choice.detected),
        }
    }

    fn start_welcome_conversation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.welcome_busy()
            || !self.connected
            || self.profiles.loading
            || !self.showing_welcome_guide
        {
            return;
        }
        let Some(project) = self.welcome_projects_target().map(|item| item.id.clone()) else {
            return;
        };
        if self.workspace_removing.as_ref() == Some(&project) {
            return;
        }
        let choices = self.welcome_agents();
        let Some(choice) = self.selected_welcome_agent(&choices) else {
            return;
        };
        let reference = choice.reference.clone();
        let backend = choice.backend;
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.welcome_setup.starting = true;
        self.welcome_setup.notice = None;
        cx.notify();
        let window = window.window_handle();
        let connection = self.connection_generation;
        let open_request = self.open_request;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "chat.createRecord",
                    serde_json::json!({
                        "workspaceId": project, "backend": backend, "profile": reference,
                    }),
                )
                .await
                .and_then(|value| {
                    serde_json::from_value::<Session>(value).map_err(|error| error.to_string())
                });
            let _ = window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return;
                    }
                    this.welcome_setup.starting = false;
                    if !this.showing_welcome_guide
                        || this.open_request != open_request
                        || this.settings_panel
                    {
                        this.refresh_sessions(cx);
                        cx.notify();
                        return;
                    }
                    match result {
                        Ok(session)
                            if session.workspace_id == project
                                && session.backend.wire() == backend
                                && session.profile.as_ref() == Some(&reference) =>
                        {
                            if this.navigate_to_session(session, window, cx) {
                                this.focus_project_input = true;
                                if !this.save_welcome_completion(cx) {
                                    this.error = this.welcome_setup.notice.take();
                                }
                            } else {
                                this.welcome_setup.notice = Some("无法打开刚创建的会话".into());
                            }
                        }
                        Ok(_) => {
                            this.welcome_setup.notice = Some("创建会话返回的配置引用不匹配".into())
                        }
                        Err(error) => {
                            this.welcome_setup.notice =
                                Some(ErrorNotice::Detail("创建会话失败", error))
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub(crate) fn add_welcome_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.welcome_busy() || !self.connected {
            return;
        }
        let (Some(path_text), Some(runtime)) = (path.to_str(), self.runtime.clone()) else {
            return;
        };
        if let Some(workspace) = self
            .workspaces
            .iter()
            .find(|workspace| workspace.path == path_text)
        {
            self.welcome_setup.preferred_workspace = Some(workspace.id.clone());
            cx.notify();
            return;
        }
        let params = crate::workspace_actions::workspace_add_params(path_text);
        self.welcome_setup.adding = true;
        self.welcome_setup.notice = None;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let workspace = runtime
                .request("hive.request", params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<Workspace>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.welcome_setup.adding = false;
                match workspace {
                    Ok(workspace) => {
                        this.welcome_setup.preferred_workspace = Some(workspace.id.clone());
                        if !this.workspaces.iter().any(|old| old.id == workspace.id) {
                            this.workspaces.push(workspace);
                        }
                    }
                    Err(error) => {
                        this.welcome_setup.notice =
                            Some(ErrorNotice::Detail("添加项目失败", error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn render_welcome_setup(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let agents = self.welcome_agents();
        let selected = self
            .selected_welcome_agent(&agents)
            .map(|choice| choice.reference.clone());
        let busy = self.welcome_busy();
        let can_start = selected.is_some()
            && self.welcome_projects_target().is_some()
            && self.connected
            && !self.profiles.loading
            && !busy;
        let agent_picker = self.render_welcome_agent_picker(agents, selected.as_ref(), busy, cx);
        let notice = self
            .welcome_setup
            .notice
            .as_ref()
            .or(self.error.as_ref())
            .map(ErrorNotice::text);
        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(crate::chrome::drag_bar("setup-titlebar", window, cx))
            .child(Viewport::new(
                "welcome-setup-viewport",
                &self.welcome_scroll,
                v_flex()
                    .id("welcome-setup")
                    .track_scroll(&self.welcome_scroll)
                    .overflow_y_scroll()
                    .vertical_scrollbar(&self.welcome_scroll)
                    .flex_1()
                    .min_h_0()
                    .px_5()
                    .pb_6()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(632.))
                            .mx_auto()
                            .gap_4()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_start()
                                    .gap_3()
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(px(27.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(tr("欢迎使用 Hive Desktop")),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(tr(
                                                        "检查本机 Agent，再选择要打开的项目。",
                                                    )),
                                            ),
                                    )
                                    .child(self.global_inbox_button("welcome-global-inbox", cx))
                                    .child(
                                        Button::new("welcome-settings")
                                            .ghost()
                                            .small()
                                            .icon(Glyph::Settings)
                                            .label(tr("设置"))
                                            .disabled(busy)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.open_settings(window, cx)
                                            })),
                                    ),
                            )
                            .child(self.render_welcome_guide_panel(cx))
                            .child(agent_picker)
                            .child(self.render_welcome_projects(cx))
                            .child(self.render_welcome_import(cx))
                            .when_some(notice, |panel, notice| {
                                panel.child(
                                    div()
                                        .text_sm()
                                        .text_color(Color::Error.get(cx))
                                        .child(notice),
                                )
                            }),
                    ),
            ))
            .child(
                h_flex()
                    .flex_shrink_0()
                    .justify_center()
                    .border_t_1()
                    .border_color(Color::Divider.get(cx))
                    .px_5()
                    .py_3()
                    .child(
                        h_flex()
                            .w_full()
                            .max_w(px(632.))
                            .justify_between()
                            .items_center()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new("welcome-sharing")
                                    .ghost()
                                    .small()
                                    .label(tr("连接其他设备"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.select_settings_section(
                                            SettingsSection::Remote,
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .flex_wrap()
                                    .child(
                                        Button::new("finish-welcome-setup")
                                            .outline()
                                            .small()
                                            .disabled(busy)
                                            .label(tr("进入工作区"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.finish_welcome_setup(window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("welcome-start-conversation")
                                            .primary()
                                            .small()
                                            .label(if self.welcome_setup.starting {
                                                tr("正在创建会话…")
                                            } else {
                                                tr("开始聊天")
                                            })
                                            .disabled(!can_start)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.start_welcome_conversation(window, cx)
                                            })),
                                    ),
                            ),
                    ),
            )
    }

    fn render_welcome_agent_picker(
        &self,
        agents: Vec<AgentChoice>,
        selected: Option<&ProfileRef>,
        busy: bool,
        cx: &Context<Self>,
    ) -> Div {
        let title = agents
            .iter()
            .find(|choice| Some(&choice.reference) == selected)
            .map(|choice| choice.name.clone())
            .unwrap_or_else(|| tr("选择 Agent").to_owned());
        let selected = selected.cloned();
        let owner = cx.weak_entity();
        h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .flex_wrap()
            .child(
                Button::new("welcome-agent-choice")
                    .outline()
                    .small()
                    .label(title)
                    .icon(Glyph::ChevronDown)
                    .disabled(busy || !self.connected || self.profiles.loading || agents.is_empty())
                    .accessibility_label(tr("为新会话选择 Agent"))
                    .dropdown_menu(move |mut menu, _, _| {
                        for choice in &agents {
                            let reference = choice.reference.clone();
                            let owner = owner.clone();
                            let name = if choice.detected {
                                choice.name.clone()
                            } else {
                                format!("{} · {}", choice.name, tr("配置未验证"))
                            };
                            menu = menu.item(
                                PopupMenuItem::new(name)
                                    .checked(selected.as_ref() == Some(&reference))
                                    .on_click(move |_, _, cx| {
                                        let _ = owner.update(cx, |app, cx| {
                                            if app.showing_welcome_guide && !app.welcome_busy() {
                                                app.welcome_setup.preferred_agent =
                                                    Some(reference.clone());
                                                cx.notify();
                                            }
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .when(
                !self.profiles.loading && self.profiles.error.is_some(),
                |row| {
                    row.child(
                        Button::new("retry-welcome-profiles")
                            .ghost()
                            .small()
                            .label(tr("重新读取 Agent 配置"))
                            .disabled(busy || !self.connected)
                            .on_click(cx.listener(|this, _, _, cx| this.load_profiles(cx))),
                    )
                },
            )
    }

    fn render_welcome_project_picker(
        &self,
        enabled: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let owner = cx.weak_entity();
        let projects: Vec<_> = self
            .workspaces
            .iter()
            .map(|project| (project.id.clone(), project.name.clone()))
            .collect();
        let selected_id = self
            .welcome_projects_target()
            .map(|project| project.id.clone());
        Button::new("welcome-existing-projects")
            .ghost()
            .small()
            .label(tr("选择已有项目"))
            .icon(Glyph::ChevronDown)
            .disabled(!enabled)
            .dropdown_menu(move |mut menu, _, _| {
                for (id, name) in &projects {
                    let id = id.clone();
                    let owner = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::new(name.clone())
                            .checked(selected_id.as_ref() == Some(&id))
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |app, cx| {
                                    if app.showing_welcome_guide && !app.welcome_busy() {
                                        app.welcome_setup.preferred_workspace = Some(id.clone());
                                        cx.notify();
                                    }
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn render_welcome_projects(&self, cx: &Context<Self>) -> Div {
        let can_add = self.connected && !self.welcome_busy();
        v_flex()
            .w_full()
            .rounded_xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Card.get(cx))
            .p_3p5()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(tr("项目文件夹")),
                    )
                    .child(
                        Button::new("welcome-add-folder")
                            .outline()
                            .small()
                            .icon(Glyph::Folder)
                            .label(if self.welcome_setup.adding {
                                tr("正在添加项目…")
                            } else {
                                tr("选择文件夹")
                            })
                            .disabled(!can_add)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.choose_workspace_folder(window, cx)
                            })),
                    )
                    .when(!self.workspaces.is_empty(), |row| {
                        row.child(self.render_welcome_project_picker(can_add, cx))
                    }),
            )
            .when_some(self.welcome_projects_target(), |panel, item| {
                panel.child(
                    div()
                        .text_sm()
                        .text_color(Color::Success.get(cx))
                        .child(format!("{} · {}", tr("将打开此项目"), item.name)),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{COMPLETION_FILE, completed, save_completed};
    use std::fs;

    #[test]
    fn welcome_completion_survives_restarts_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!completed(dir.path()).unwrap());
        save_completed(dir.path()).unwrap();
        assert!(completed(dir.path()).unwrap());
        save_completed(dir.path()).unwrap();
    }

    #[test]
    fn invalid_welcome_completion_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(COMPLETION_FILE)).unwrap();
        assert!(completed(dir.path()).is_err());
        assert!(save_completed(dir.path()).is_err());
        assert!(dir.path().join(COMPLETION_FILE).is_dir());
    }
}
