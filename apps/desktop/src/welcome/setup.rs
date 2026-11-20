use crate::{
    app::{ErrorNotice, HiveApp, SettingsSection},
    language::tr,
    palette::Color,
    protocol::Workspace,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
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
    preferred_workspace: Option<String>,
    suggestions: Option<Vec<PathBuf>>,
    discovering: bool,
    pub(crate) adding: bool,
    notice: Option<ErrorNotice>,
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
        self.welcome_setup.notice = None;
        self.welcome_scroll.set_offset(point(px(0.), px(0.)));
        self.showing_welcome_guide = true;
        self.refresh_readiness(cx);
        cx.notify();
    }

    fn finish_welcome_setup(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.welcome_setup.adding {
            return;
        }
        if let Some(root) = &self.data_dir
            && let Err(error) = save_completed(root)
        {
            self.welcome_setup.notice = Some(ErrorNotice::Detail(
                "无法保存欢迎引导状态",
                error.to_string(),
            ));
            cx.notify();
            return;
        }
        self.showing_welcome_guide = false;
        self.welcome_setup.notice = None;
        if let Some(id) = self.welcome_setup.preferred_workspace.take() {
            self.select_workspace(id, cx);
        }
        if window.viewport_size().width < px(820.) {
            self.sidebar_open = false;
        }
        self.focus_project_input = true;
        cx.notify();
    }

    fn discover_welcome_projects(&mut self, cx: &mut Context<Self>) {
        if self.welcome_setup.discovering {
            return;
        }
        self.welcome_setup.discovering = true;
        cx.spawn(async move |this, cx| {
            let projects = smol::unblock(super::projects::discover_recent_projects).await;
            let _ = this.update(cx, |this, cx| {
                this.welcome_setup.discovering = false;
                this.welcome_setup.suggestions = Some(projects);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn add_welcome_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.welcome_setup.adding || !self.connected || self.closing {
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
        let ready = ["pi", "codex", "pi-rpc"].into_iter().any(|backend| {
            self.backend_readiness(backend)
                .is_some_and(|entry| entry.level == "ready" && entry.credential_state == "present")
        });
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
            .child(
                v_flex()
                    .id("welcome-setup")
                    .track_scroll(&self.welcome_scroll)
                    .overflow_y_scroll()
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
                                    .child(
                                        Button::new("welcome-settings")
                                            .ghost()
                                            .small()
                                            .icon(Glyph::Settings)
                                            .label(tr("设置"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.open_settings(window, cx)
                                            })),
                                    ),
                            )
                            .child(self.render_welcome_guide_panel(cx))
                            .child(self.render_welcome_projects(cx))
                            .when_some(notice, |panel, notice| {
                                panel.child(
                                    div()
                                        .text_sm()
                                        .text_color(Color::Error.get(cx))
                                        .child(notice),
                                )
                            }),
                    ),
            )
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
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.select_settings_section(
                                            SettingsSection::Remote,
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("finish-welcome-setup")
                                    .primary()
                                    .small()
                                    .disabled(self.welcome_setup.adding || self.closing)
                                    .label(if ready {
                                        tr("进入工作区")
                                    } else {
                                        tr("先进入工作区，稍后配置 Agent")
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.finish_welcome_setup(window, cx)
                                    })),
                            ),
                    ),
            )
    }

    fn render_welcome_projects(&self, cx: &Context<Self>) -> Div {
        let can_add = self.connected && !self.welcome_setup.adding && !self.closing;
        let suggestions = self.welcome_setup.suggestions.as_ref();
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
                    ),
            )
            .when_some(
                self.welcome_setup
                    .preferred_workspace
                    .as_ref()
                    .and_then(|id| self.workspaces.iter().find(|workspace| &workspace.id == id)),
                |panel, item| {
                    panel.child(
                        div()
                            .text_sm()
                            .text_color(Color::Success.get(cx))
                            .child(format!("{} · {}", tr("将打开此项目"), item.name)),
                    )
                },
            )
            .when(
                suggestions.is_none() || suggestions.is_some_and(Vec::is_empty),
                |panel| {
                    panel.child(div().text_xs().text_color(Color::Muted.get(cx)).child(
                        if suggestions.is_some() {
                            tr("没有找到最近使用的项目文件夹。")
                        } else {
                            tr("也可以查找在本机 Agent 中用过的文件夹。")
                        },
                    ))
                },
            )
            .when_some(suggestions, |panel, paths| {
                panel.children(paths.iter().filter_map(|path| {
                    let display_path = path.to_str()?;
                    if self
                        .workspaces
                        .iter()
                        .any(|workspace| workspace.path == display_path)
                    {
                        return None;
                    }
                    let path = path.clone();
                    Some(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .justify_between()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .child(display_path.to_owned()),
                            )
                            .child(
                                Button::new(format!("welcome-recent-folder-{display_path}"))
                                    .ghost()
                                    .small()
                                    .disabled(!can_add)
                                    .label(tr("添加项目"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.add_welcome_project(path.clone(), cx)
                                    })),
                            ),
                    )
                }))
            })
            .child(
                h_flex().child(
                    Button::new("welcome-find-recent-projects")
                        .ghost()
                        .small()
                        .disabled(self.welcome_setup.discovering)
                        .label(if self.welcome_setup.discovering {
                            tr("正在查找…")
                        } else {
                            tr("查找最近项目")
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.discover_welcome_projects(cx))),
                ),
            )
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
