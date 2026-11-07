use crate::app::HiveApp;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::Button,
        input::{Input, InputState},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
mod notification;

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
        let generation = self.settings_generation;
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
                    if !this.settings_panel || this.settings_generation != generation {
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

    pub fn close_settings(&mut self) {
        if self.settings_panel {
            self.settings_panel = false;
            self.settings_generation += 1;
            self.settings_loading = false;
            self.team_autostaff_generation += 1;
            self.team_autostaff_loading = false;
        }
    }
    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_memory();
        self.close_roles();
        self.close_collaboration();
        self.member_panel = false;
        self.settings_panel = true;
        // Reopening a tab must retain edits; only initial load or explicit reload
        // replaces the input values with persisted settings.
        if self.executable_settings.is_none() {
            self.settings_request(false, window, cx);
        }
        self.team_autostaff_request(None, cx);
        if !self.notification_loaded {
            self.notification_request(None, window.window_handle(), cx);
        }
        cx.notify();
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
        self.team_autostaff_generation += 1;
        let generation = self.team_autostaff_generation;
        self.team_autostaff_loading = true;
        self.team_autostaff_message = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let params = match save {
                Some(enabled) => json!({"method":"PUT", "path":"/api/settings/team-autostaff", "body":{"enabled":enabled}}),
                None => json!({"method":"GET", "path":"/api/settings/team-autostaff"}),
            };
            let result = runtime.request("hive.request", params).await;
            let _ = this.update(cx, |this, cx| {
                if !this.settings_panel || this.team_autostaff_generation != generation {
                    return;
                }
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
    pub fn settings_request(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        if save && !self.connected {
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
        self.settings_generation += 1;
        let generation = self.settings_generation;
        self.settings_loading = true;
        self.settings_message = None;
        let window = window.window_handle();
        cx.notify();
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
                if !this.settings_panel || this.settings_generation != generation {
                    return;
                }
                this.settings_loading = false;
                match result {
                    Ok(value) => match serde_json::from_value::<ExecutableSettings>(value) {
                        Ok(settings) => {
                            if !save {
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
