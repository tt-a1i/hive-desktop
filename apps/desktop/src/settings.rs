use crate::app::{HiveApp, SettingsSection};
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputState},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
mod appearance;
mod notification;
pub mod pi_accounts;

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
    pub fn label(&self) -> String {
        let source = match self.source.as_str() {
            "env" => "环境变量（覆盖应用设置）",
            "app" => "应用设置",
            "path" => "PATH",
            _ => "未知来源",
        };
        format!("当前使用：{} · {}", self.command, source)
    }
}
impl HiveApp {
    pub(crate) fn executable_field(
        &self,
        label: &'static str,
        input: &Entity<InputState>,
        effective: Option<&EffectiveCommand>,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let disabled =
            !self.connected || self.settings_loading || self.executable_settings.is_none();
        let target = input.clone();
        v_flex()
            .min_w_0()
            .gap_2()
            .py_3()
            .border_b_1()
            .border_color(Color::Divider.get(cx))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(input)
                                .rounded_full()
                                .aria_label(format!("{label} 可执行文件路径"))
                                .disabled(disabled),
                        ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("browse-{label}")))
                            .rounded_full()
                            .label("浏览…")
                            .accessibility_label(format!("选择 {label} 可执行文件"))
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.browse_executable(label, target.clone(), window, cx)
                            })),
                    ),
            )
            .when_some(effective, |row, value| {
                row.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(value.label()),
                )
            })
    }

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
            prompt: Some(format!("选择 {label} 可执行文件").into()),
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
                            this.settings_message = Some(format!("选择文件失败：{error}"))
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
            self.team_autostaff_message = Some("后台尚未连接".into());
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
                                this.team_autostaff_message = Some("设置已保存".into());
                            }
                        }
                        None => this.team_autostaff_message = Some("自动组队设置响应格式错误".into()),
                    },
                    Err(error) => this.team_autostaff_message = Some(format!("{}自动组队设置失败：{error}", if save.is_some() { "保存" } else { "读取" })),
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
            self.settings_message = Some("后台尚未连接".into());
            cx.notify();
            return;
        };
        let pi = self.pi_command.read(cx).value().trim().to_string();
        let codex = self.codex_command.read(cx).value().trim().to_string();
        let claude = self.claude_command.read(cx).value().trim().to_string();
        let params = if save {
            json!({"piCommand":if pi.is_empty() { None } else { Some(pi) }, "codexCommand":if codex.is_empty() { None } else { Some(codex) }, "claudeCommand":if claude.is_empty() { None } else { Some(claude) }})
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
                                let pi_input = this.pi_command.clone();
                                let codex_input = this.codex_command.clone();
                                let claude_input = this.claude_command.clone();
                                let pi = settings.pi_command.clone().unwrap_or_default();
                                let codex = settings.codex_command.clone().unwrap_or_default();
                                let claude = settings.claude_command.clone().unwrap_or_default();
                                let _ = window.update(cx, |_, window, cx| {
                                    pi_input
                                        .update(cx, |input, cx| input.set_value(pi, window, cx));
                                    codex_input
                                        .update(cx, |input, cx| input.set_value(codex, window, cx));
                                    claude_input.update(cx, |input, cx| {
                                        input.set_value(claude, window, cx)
                                    });
                                });
                            }
                            this.executable_settings = Some(settings);
                            if save {
                                this.settings_message =
                                    Some("设置已保存。新启动的外部进程将使用更新后的配置。".into());
                            }
                        }
                        Err(error) => {
                            this.settings_message = Some(format!("设置响应格式错误：{error}"))
                        }
                    },
                    Err(error) => {
                        this.settings_message = Some(format!(
                            "{}设置失败：{error}",
                            if save { "保存" } else { "读取" }
                        ))
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
        self.close_settings();
        self.close_roles();
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
        let content = if self.roles.visible {
            self.render_roles(cx).into_any_element()
        } else {
            v_flex().flex_1().w_full().max_w(px(960.)).mx_auto().min_h_0().min_w_0().px_5().py_6().when(!compact_settings, |panel| panel.pl_5().pr_6().py_7()).gap_5()
                .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(match self.settings_section { SettingsSection::Appearance => "外观", SettingsSection::PiAccounts => "内置 Pi", SettingsSection::Team => "团队", SettingsSection::Notifications => "通知", SettingsSection::Programs => "外部程序" }))
                .child({
                    let content = v_flex().flex_1().min_h_0().gap_3();
                    match self.settings_section {
                        SettingsSection::PiAccounts => content.id("settings-pi-accounts").overflow_y_scroll().child(self.render_pi_accounts(cx)),
                        SettingsSection::Appearance => content.id("settings-appearance-content").overflow_y_scroll().child(self.render_appearance_settings(cx)),
                        SettingsSection::Team => content.id("settings-team-content").overflow_y_scroll()
                    .child(h_flex().gap_3().items_center().pb_3().border_b_1().border_color(Color::Divider.get(cx))
                        .child(v_flex().flex_1().min_w_0().gap_1()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("自动组队引导"))
                            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child("在所有工作区引导主控按任务需要组建团队。")))
                        .when_some(self.team_autostaff, |row, enabled| row.child(gpui_kit::component::switch::Switch::new("toggle-team-autostaff")
                            .accessibility_label("自动组队引导").checked(enabled).disabled(!self.connected || self.team_autostaff_loading)
                            .on_change(cx.listener(|this, enabled, _, cx| this.team_autostaff_request(Some(*enabled), cx))))))
                    .when(self.team_autostaff_loading, |view| view.child(div().text_xs().text_color(Color::Muted.get(cx)).child("正在同步设置…")))
                    .when(!self.team_autostaff_loading, |view| view.child(h_flex().child(Button::new("reload-team-autostaff").ghost().compact().label("重新读取").disabled(!self.connected).on_click(cx.listener(|this, _, _, cx| this.team_autostaff_request(None, cx))))))
                    .when_some(self.team_autostaff_message.clone(), |view, message| view.child(div().text_sm().child(message))),
                        SettingsSection::Notifications => content.id("settings-notification-content").overflow_y_scroll().child(self.render_notification_settings(cx)),
                        SettingsSection::Programs => content.id("settings-programs-content").overflow_y_scroll()
                    .child(div().text_sm().text_color(Color::Secondary.get(cx)).child("路径留空时使用 PATH 查找。环境变量优先，修改在下次启动对应 Agent 时生效。"))
                    .when_some(self.settings_message.clone(), |view, message| view.child(div().p_3().rounded_lg().bg(Color::Inset.get(cx)).text_sm().child(message)))
                    .child(self.executable_field("外部 Pi", &self.pi_command, self.executable_settings.as_ref().map(|s| &s.effective.pi), cx))
                    .child(self.executable_field("Codex", &self.codex_command, self.executable_settings.as_ref().map(|s| &s.effective.codex), cx))
                    .child(self.executable_field("Claude Code", &self.claude_command, self.executable_settings.as_ref().map(|s| &s.effective.claude), cx))
                    .child(h_flex().items_center().gap_2()
                        .child(Button::new("save-executable-settings").primary().rounded_full().label(if self.settings_loading { "处理中…" } else { "保存设置" }).disabled(!self.connected || self.settings_loading || !self.executable_paths_changed(cx)).on_click(cx.listener(|this, _, window, cx| this.settings_request(true, window.window_handle(), cx))))
                        .child(Button::new("reload-executable-settings").rounded_full().label("重新读取").disabled(!self.connected || self.settings_loading).on_click(cx.listener(|this, _, window, cx| this.settings_request(false, window.window_handle(), cx))))
                        .when(!self.settings_loading && self.executable_paths_changed(cx), |row| row.child(div().text_xs().text_color(Color::Secondary.get(cx)).child("有未保存的修改")))),
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
            .child(content)
    }
}
