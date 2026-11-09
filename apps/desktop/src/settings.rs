use crate::language::{english,tr};
use crate::app::{HiveApp, SettingsSection};
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::InputState,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
mod appearance;
mod language;
mod search;
mod notification;
pub mod remote;
pub mod pi_accounts;
pub mod pi_models;
pub mod programs;

pub enum SettingsNotice { Message(&'static str), Error(&'static str, String) }
impl SettingsNotice {
    fn text(&self) -> String {
        match self {
            Self::Message(key) => tr(key).to_owned(),
            Self::Error(key, error) => format!("{}: {error}", tr(key)),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableSettings {
    pub pi_command: Option<String>,
    pub codex_command: Option<String>,
    pub claude_command: Option<String>,
    pub effective: EffectiveCommands,
}
#[derive(Deserialize)]
pub struct EffectiveCommands {
    pub pi: EffectiveCommand,
    pub codex: EffectiveCommand,
    pub claude: EffectiveCommand,
}
#[derive(Deserialize)]
pub struct EffectiveCommand {
    pub command: String,
    pub source: String,
}
impl EffectiveCommand {
    pub fn label(&self, prefix: &str) -> String {
        let source = match self.source.as_str() {
            "env" => tr("环境变量（覆盖应用设置）"),
            "app" => tr("应用设置"),
            "draft" => tr("输入路径"),
            "path" => "PATH",
            _ => tr("未知来源"),
        };
        format!("{prefix}: {} · {}", self.command, source)
    }
}
impl HiveApp {
    fn browse_executable(
        &mut self,
        label: &'static str,
        target: Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some((if english() { format!("Select {label} executable") } else { format!("选择 {label} 可执行文件") }).into()),
        });
        let window = window.window_handle();
        let generation = self.executable_picker_generation;
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = match selected.await {
                Ok(Ok(Some(paths))) => paths
                    .into_iter()
                    .next()
                    .map(|path| {
                        path.into_os_string()
                            .into_string()
                            .map_err(|_| "文件路径无法显示为文本".to_owned())
                    })
                    .transpose(),
                Ok(Ok(None)) => return,
                Ok(Err(error)) => Err(error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            let _ = window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return;
                    }
                    if !this.settings_panel || this.executable_picker_generation != generation {
                        return;
                    }
                    match result {
                        Ok(Some(path)) => {
                            target.update(cx, |input, cx| input.set_value(path, window, cx));
                            this.settings_message = None;
                        }
                        Ok(None) => return,
                        Err(error) => {
                            this.settings_message = Some(SettingsNotice::Error("选择文件失败", error))
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub fn executable_paths_changed(&self, cx: &App) -> bool {
        self.executable_settings.as_ref().is_some_and(|saved| {
            [
                (&self.pi_command, &saved.pi_command),
                (&self.codex_command, &saved.codex_command),
                (&self.claude_command, &saved.claude_command),
            ]
            .iter()
            .any(|(input, path)| {
                input.read(cx).value().trim() != path.as_deref().unwrap_or_default()
            })
        })
    }

    pub fn close_settings(&mut self) {
        if self.settings_panel {
            self.settings_panel = false;
            self.executable_picker_generation += 1;
        }
    }
    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings_panel && !self.roles.visible {
            self.preferences_return_focus = window.focused(cx);
            self.preferences_focus.focus(window, cx);
        }
        self.close_roles();
        self.settings_panel = true;
        self.load_preferences(window.window_handle(), cx);
        cx.notify();
    }
    pub fn load_preferences(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        if !self.connected {
            return;
        }
        // Only initial load and explicit reload replace unsaved executable paths.
        if self.executable_settings.is_none() {
            self.settings_request(false, window, cx);
        }
        self.load_pi_accounts(window, cx);
        if self.settings_section == SettingsSection::Models {
            self.load_pi_models(cx);
        }
        if self.settings_section == SettingsSection::Remote { self.refresh_remote(cx); }
        self.team_autostaff_request(None, cx);
        if !self.notification_loaded {
            self.notification_request(None, window, cx);
        }
    }
    pub fn team_autostaff_request(&mut self, save: Option<bool>, cx: &mut Context<Self>) {
        if !self.connected || self.team_autostaff_loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            self.team_autostaff_message = Some(SettingsNotice::Message("后台尚未连接"));
            cx.notify();
            return;
        };
        self.team_autostaff_loading = true;
        self.team_autostaff_message = None;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let params = match save {
                Some(enabled) => json!({"method":"PUT", "path":"/api/settings/team-autostaff", "body":{"enabled":enabled}}),
                None => json!({"method":"GET", "path":"/api/settings/team-autostaff"}),
            };
            let result = runtime.request("hive.request", params).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection { return; }
                this.team_autostaff_loading = false;
                match result {
                    Ok(value) => match value.get("enabled").and_then(|v| v.as_bool()) {
                        Some(enabled) => {
                            this.team_autostaff = Some(enabled);
                            if save.is_some() {
                                this.team_autostaff_message = Some(SettingsNotice::Message("设置已保存"));
                            }
                        }
                        None => this.team_autostaff_message = Some(SettingsNotice::Message("自动组队设置响应格式错误")),
                    },
                    Err(error) => this.team_autostaff_message = Some(SettingsNotice::Error(if save.is_some() { "保存自动组队设置失败" } else { "读取自动组队设置失败" }, error)),
                }
                cx.notify();
            });
        }).detach();
    }
    pub fn settings_request(
        &mut self,
        save: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        self.request_executable_settings(save, !save, window, cx);
    }
    pub fn refresh_executable_snapshot(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        self.request_executable_settings(false, self.executable_settings.is_none(), window, cx);
    }
    fn request_executable_settings(
        &mut self,
        save: bool,
        replace_inputs: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.settings_loading || !self.connected {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            self.settings_message = Some(SettingsNotice::Message("后台尚未连接"));
            cx.notify();
            return;
        };
        let pi = self.pi_command.read(cx).value().trim().to_string();
        let codex = self.codex_command.read(cx).value().trim().to_string();
        let claude = self.claude_command.read(cx).value().trim().to_string();
        let params = if save {
            json!({"piCommand":if pi.is_empty() { None } else { Some(pi.clone()) }, "codexCommand":if codex.is_empty() { None } else { Some(codex.clone()) }, "claudeCommand":if claude.is_empty() { None } else { Some(claude.clone()) }})
        } else {
            json!({})
        };
        self.executable_picker_generation += 1;
        self.settings_loading = true;
        self.settings_message = None;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    if save {
                        "desktop.settings.update"
                    } else {
                        "desktop.settings.get"
                    },
                    params,
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.settings_loading = false;
                match result {
                    Ok(value) => match serde_json::from_value::<ExecutableSettings>(value) {
                        Ok(settings) => {
                            if replace_inputs {
                                let inputs = [
                                    (this.pi_command.clone(), pi, settings.pi_command.clone().unwrap_or_default()),
                                    (this.codex_command.clone(), codex, settings.codex_command.clone().unwrap_or_default()),
                                    (this.claude_command.clone(), claude, settings.claude_command.clone().unwrap_or_default()),
                                ];
                                let _ = window.update(cx, |_, window, cx| {
                                    // Keep edits made while the asynchronous read was in flight.
                                    for (input, expected, saved) in inputs {
                                        if input.read(cx).value().trim() == expected {
                                            input.update(cx, |input, cx| input.set_value(saved, window, cx));
                                        }
                                    }
                                });
                            }
                            this.executable_settings = Some(settings);
                            if save {
                                this.settings_message =
                                    Some(SettingsNotice::Message("设置已保存。新启动的外部进程将使用更新后的配置。"));
                            }
                        }
                        Err(error) => {
                            this.settings_message = Some(SettingsNotice::Error("设置响应格式错误", error.to_string()))
                        }
                    },
                    Err(error) => {
                        this.settings_message = Some(SettingsNotice::Error(if save { "保存设置失败" } else { "读取设置失败" }, error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl HiveApp {
    pub fn leave_preferences(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_search.update(cx, |input, cx| input.set_value("", window, cx));
        self.close_settings();
        self.close_roles();
        if window.viewport_size().width < px(820.) {
            self.sidebar_open = false;
        }
        if let Some(focus) = self.preferences_return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    pub fn render_preferences(
        &self,
        compact_settings: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let content = if !self.settings_search.read(cx).value().trim().is_empty() {
            self.render_settings_results(cx).into_any_element()
        } else if self.roles.visible {
            self.render_roles(cx).into_any_element()
        } else {
            v_flex().flex_1().w_full().max_w(px(960.)).mx_auto().min_h_0().min_w_0().px_5().py_3().gap_3().when(!compact_settings, |panel| panel.pl_5().pr_6().py_7().gap_5())
                .when(!compact_settings, |panel| panel.child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(match self.settings_section { SettingsSection::Appearance => tr("外观"), SettingsSection::Chat => tr("聊天"), SettingsSection::PiAccounts => tr("内置 Pi"), SettingsSection::Models => tr("模型与接口"), SettingsSection::Team => tr("团队"), SettingsSection::Notifications => tr("通知"), SettingsSection::Remote => tr("远程接入"), SettingsSection::Programs => tr("外部程序") })))
                .child({
                    let content = v_flex().flex_1().min_h_0().gap_3();
                    match self.settings_section {
                        SettingsSection::Remote => content.id("settings-remote-container").child(crate::focus_scroll::Viewport::new("remote-viewport", &self.remote.scroll, v_flex().flex_1().min_h_0().min_w_0().id("settings-remote-content").overflow_y_scroll().track_scroll(&self.remote.scroll).child(self.render_remote_settings(cx)))),
                        SettingsSection::Chat => content.id("settings-chat-content").overflow_y_scroll().child(self.render_chat_settings(cx)),
                        SettingsSection::Models => content.id("settings-model-config-content").child(self.render_pi_models(cx)),
                        SettingsSection::PiAccounts => content.id("settings-pi-accounts-container").child(crate::focus_scroll::Viewport::new("pi-accounts-viewport", &self.pi_accounts.scroll, v_flex().id("settings-pi-accounts").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(&self.pi_accounts.scroll).child(self.render_pi_accounts(cx)))),
                        SettingsSection::Appearance => content.id("settings-appearance-content").overflow_y_scroll().child(self.render_appearance_settings(cx)),
                        SettingsSection::Team => content.id("settings-team-content").overflow_y_scroll()
                    .child(h_flex().gap_3().items_center().pb_3().border_b_1().border_color(Color::Divider.get(cx))
                        .child(v_flex().flex_1().min_w_0().gap_1()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("自动组队引导")))
                            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child(tr("在所有工作区引导主控按任务需要组建团队。"))))
                        .when_some(self.team_autostaff, |row, enabled| row.child(gpui_kit::component::switch::Switch::new("toggle-team-autostaff")
                            .accessibility_label(tr("自动组队引导")).checked(enabled).disabled(!self.connected || self.team_autostaff_loading)
                            .on_change(cx.listener(|this, enabled, _, cx| this.team_autostaff_request(Some(*enabled), cx))))))
                    .when(self.team_autostaff_loading, |view| view.child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("正在同步设置…"))))
                    .when(!self.team_autostaff_loading, |view| view.child(h_flex().child(Button::new("reload-team-autostaff").ghost().compact().label(tr("重新读取")).disabled(!self.connected).on_click(cx.listener(|this, _, _, cx| this.team_autostaff_request(None, cx))))))
                    .when_some(self.team_autostaff_message.as_ref().map(|notice| notice.text()), |view, message| view.child(div().text_sm().child(message))),
                        SettingsSection::Notifications => content.id("settings-notification-content").overflow_y_scroll().child(self.render_notification_settings(cx)),
                        SettingsSection::Programs => content.id("settings-programs-container").child(self.render_program_settings(cx)),
                    }
                }).into_any_element()
        };
        h_flex()
            .key_context("HivePreferences")
            .track_focus(&self.preferences_focus)
            .on_action(
                cx.listener(|this, _: &crate::ClosePreferences, window, cx| {
                    this.leave_preferences(window, cx)
                }),
            )
            .items_stretch()
            .when(compact_settings, |layout| layout.flex_col())
            .flex_1()
            .min_w_0()
            .h_full()
            .child(self.render_settings_tabs(compact_settings, cx))
            .child(v_flex().flex_1().min_w_0().min_h_0().child(self.render_settings_search(cx)).child(content))
    }
}
