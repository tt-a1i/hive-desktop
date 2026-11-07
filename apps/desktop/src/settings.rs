use crate::app::HiveApp;
use gpui_kit::*;
use serde::Deserialize;
use serde_json::json;

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
    pub fn close_settings(&mut self) {
        if self.settings_panel {
            self.settings_panel = false;
            self.settings_generation += 1;
            self.settings_loading = false;
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
        cx.notify();
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
