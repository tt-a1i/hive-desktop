use crate::language::{english, tr};
use crate::app::{HiveApp, SettingsSection};
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::Button,
        input::InputState,
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
mod appearance;
mod about;
mod language;
mod search;
mod shell;
mod notification;
pub mod remote;
pub mod pi_accounts;
pub mod pi_models;
pub mod programs;
pub mod codex_account;

#[derive(Clone)]
pub enum SettingsNotice { Message(&'static str), Error(&'static str, String) }
impl SettingsNotice {
    fn text(&self) -> String {
        match self {
            Self::Message(key) => tr(key).to_owned(),
            Self::Error(key, error) => format!("{}: {}", tr(key), crate::language::localize_owned_error(error)),
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
            self.program_setup_focus = None;
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
        self.program_save_message = None;
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
                            let _ = window.update(cx, |_, window, cx| this.finish_program_setup(window, cx));
                            if save {
                                this.settings_message =
                                    Some(SettingsNotice::Message("设置已保存。新启动的外部进程将使用更新后的配置。"));
                            }
                        }
                        Err(error) => {
                            this.program_setup_focus = None;
                            this.settings_message = Some(SettingsNotice::Error("设置响应格式错误", error.to_string()))
                        }
                    },
                    Err(error) => {
                        this.program_setup_focus = None;
                        this.settings_message = Some(SettingsNotice::Error(if save { "保存设置失败" } else { "读取设置失败" }, error))
                    }
                }
                if save {
                    this.program_save_message = this.settings_message.clone();
                }
                cx.notify();
            });
        })
        .detach();
    }
}

pub(super) fn settings_card(cx: &App) -> Div {
    v_flex()
        .w_full()
        .min_w_0()
        .rounded_xl()
        .bg(Color::Card.get(cx))
        .border_1()
        .border_color(Color::Divider.get(cx))
        .overflow_hidden()
}

pub(super) fn settings_row(divider_top: bool, cx: &App) -> Div {
    div()
        .w_full()
        .min_w_0()
        .px_4()
        .py_3p5()
        .when(divider_top, |row| {
            row.border_t_1().border_color(Color::Divider.get(cx))
        })
}

pub(super) fn settings_segmented_bar(cx: &App) -> Div {
    h_flex()
        .flex_shrink_0()
        .items_center()
        .gap_1()
        .p_0p5()
        .rounded_lg()
        .bg(Color::Inset.get(cx))
        .border_1()
        .border_color(Color::Divider.get(cx))
}

pub(super) fn settings_notice_box(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    is_error: bool,
    cx: &App,
) -> Stateful<Div> {
    let text = text.into();
    div()
        .id(id.into())
        .role(Role::Status)
        .aria_label(text.clone())
        .w_full()
        .min_w_0()
        .px_3p5()
        .py_2p5()
        .rounded_xl()
        .border_1()
        .text_size(px(13.))
        .when(is_error, |el| {
            el.bg(Color::ErrorSurface.get(cx))
                .border_color(Color::Error.get(cx).opacity(0.24))
                .text_color(Color::Error.get(cx))
        })
        .when(!is_error, |el| {
            el.bg(Color::Inset.get(cx).opacity(0.75))
                .border_color(Color::Divider.get(cx))
                .text_color(Color::Secondary.get(cx))
        })
        .child(text)
}

pub(super) fn status_pill(
    dot_color: Rgba,
    label: impl Into<SharedString>,
    cx: &App,
) -> Div {
    h_flex()
        .flex_shrink_0()
        .items_center()
        .gap_1p5()
        .px_2()
        .py(px(2.))
        .rounded_full()
        .bg(Color::Inset.get(cx))
        .child(div().size(px(6.)).rounded_full().bg(dot_color))
        .child(
            div()
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Color::Secondary.get(cx))
                .child(label.into()),
        )
}

pub(super) fn amber_dot(cx: &App) -> Rgba {
    rgb(if Theme::global(cx).is_dark() {
        0xfbbf24
    } else {
        0xd97706
    })
}

fn section_meta(section: SettingsSection) -> (&'static str, &'static str) {
    match section {
        SettingsSection::Appearance => ("外观", "主题、界面字号、语言与阅读预览"),
        SettingsSection::Chat => ("聊天", "发送快捷键、工具活动与思考展示"),
        SettingsSection::PiAccounts => ("内置 Pi", "连接提供商、订阅登录与 API Key"),
        SettingsSection::Models => ("模型与接口", "自定义接口、模型覆盖、上下文与能力"),
        SettingsSection::Team => ("团队", "自动组队引导"),
        SettingsSection::Notifications => ("通知", "本机系统提醒与 Webhook 通知地址"),
        SettingsSection::Remote => ("远程接入", "账号连接、设备配对与访问记录"),
        SettingsSection::Programs => ("外部程序", "外部 Pi、Codex 与 Claude Code 的路径和版本检查"),
        SettingsSection::About => ("关于与诊断", "版本、构建与后台连接信息"),
    }
}

impl HiveApp {
    pub fn leave_preferences(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_search.update(cx, |input, cx| input.set_value("", window, cx));
        self.close_settings();
        self.close_roles();
        self.preferences_modal_open = false;
        self.preferences_search_open = false;
        if let Some(focus) = self.preferences_return_focus.take() {
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        cx.notify();
    }

    pub fn render_preferences(
        &self,
        compact_settings: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (title_key, subtitle_key) = section_meta(self.settings_section);
        let content = if self.roles.visible {
            self.render_roles(cx).into_any_element()
        } else {
            v_flex()
                .flex_1()
                .w_full()
                .max_w(px(720.))
                .mx_auto()
                .min_h_0()
                .min_w_0()
                .px(if compact_settings { px(16.) } else { px(24.) })
                .py_5()
                .gap_4()
                .map(|panel| {
                    panel.child(
                        v_flex()
                            .flex_shrink_0()
                            .gap_1()
                            .pb_0p5()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr(title_key)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr(subtitle_key)),
                            ),
                    )
                })
                .child({
                    let content = v_flex().flex_1().min_h_0().gap_3();
                    match self.settings_section {
                        SettingsSection::Remote => content.id("settings-remote-container").child(crate::focus_scroll::Viewport::new("remote-viewport", &self.remote.scroll, v_flex().flex_1().min_h_0().min_w_0().id("settings-remote-content").overflow_y_scroll().track_scroll(&self.remote.scroll).vertical_scrollbar(&self.remote.scroll).pr_1().child(self.render_remote_settings(cx)))),
                        SettingsSection::Chat => content.id("settings-chat-container")
                            .child(crate::focus_scroll::Viewport::new(
                                "chat-settings-viewport", &self.chat_settings_scroll,
                                v_flex().id("settings-chat-content").flex_1().min_h_0().min_w_0()
                                    .overflow_y_scroll().track_scroll(&self.chat_settings_scroll)
                                    .vertical_scrollbar(&self.chat_settings_scroll).pr_1()
                                    .child(self.render_chat_settings(cx)),
                            ))
                            .children(self.render_chat_settings_notice(cx)),
                        SettingsSection::Models => content.id("settings-model-config-content").child(self.render_pi_models(cx)),
                        SettingsSection::PiAccounts => content.id("settings-pi-accounts-container").child(crate::focus_scroll::Viewport::new("pi-accounts-viewport", &self.pi_accounts.scroll, v_flex().id("settings-pi-accounts").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(&self.pi_accounts.scroll).vertical_scrollbar(&self.pi_accounts.scroll).pr_1().child(self.render_pi_accounts(cx)))),
                        SettingsSection::Appearance => content.id("settings-appearance-container").child(crate::focus_scroll::Viewport::new("appearance-settings-viewport", &self.appearance_scroll, v_flex().id("settings-appearance-content").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(&self.appearance_scroll).vertical_scrollbar(&self.appearance_scroll).pr_1().child(self.render_appearance_settings(cx)))),
                        SettingsSection::Team => {
                            let is_error = matches!(self.team_autostaff_message, Some(SettingsNotice::Error(..)));
                            content.id("settings-team-container").child(crate::focus_scroll::Viewport::new(
                                "team-settings-viewport",
                                &self.team_settings_scroll,
                                v_flex()
                                    .id("settings-team-content")
                                    .flex_1()
                                    .min_h_0()
                                    .min_w_0()
                                    .gap_3()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.team_settings_scroll)
                                    .vertical_scrollbar(&self.team_settings_scroll)
                                    .pr_1()
                                    .child(
                                        settings_card(cx)
                                            .child(settings_row(false, cx).child(
                                                crate::focus_scroll::Reveal::new(
                                                    "team-autostaff-toggle-reveal",
                                                    &self.team_settings_scroll,
                                                    h_flex()
                                                        .w_full()
                                                        .min_w_0()
                                                        .gap_4()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            v_flex()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .gap_0p5()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .text_color(Color::Text.get(cx))
                                                                        .child(tr("自动组队引导")),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(Color::Muted.get(cx))
                                                                        .child(tr("在所有工作区引导主控按任务需要组建团队。")),
                                                                ),
                                                        )
                                                        .when_some(self.team_autostaff, |row, enabled| {
                                                            row.child(
                                                                gpui_kit::component::switch::Switch::new("toggle-team-autostaff")
                                                                    .accessibility_label(tr("自动组队引导"))
                                                                    .checked(enabled)
                                                                    .disabled(!self.connected || self.team_autostaff_loading)
                                                                    .on_change(cx.listener(|this, enabled, _, cx| {
                                                                        this.team_autostaff_request(Some(*enabled), cx)
                                                                    })),
                                                            )
                                                        }),
                                                ),
                                            ))
                                            .child(
                                                settings_row(true, cx)
                                                    .bg(Color::Inset.get(cx).opacity(0.6))
                                                    .py_2p5()
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .min_w_0()
                                                            .items_center()
                                                            .justify_between()
                                                            .gap_3()
                                                            .child(
                                                                div()
                                                                    .flex_1()
                                                                    .min_w_0()
                                                                    .text_xs()
                                                                    .text_color(if is_error {
                                                                        Color::Error.get(cx)
                                                                    } else {
                                                                        Color::Muted.get(cx)
                                                                    })
                                                                    .when(self.team_autostaff_loading, |el| {
                                                                        el.child(tr("正在同步设置…"))
                                                                    })
                                                                    .when(!self.team_autostaff_loading, |el| {
                                                                        el.when_some(
                                                                            self.team_autostaff_message.as_ref().map(|notice| notice.text()),
                                                                            |el, message| el.child(message),
                                                                        )
                                                                    }),
                                                            )
                                                            .when(!self.team_autostaff_loading, |row| {
                                                                row.child(crate::focus_scroll::Reveal::new(
                                                                    "team-autostaff-reload-reveal",
                                                                    &self.team_settings_scroll,
                                                                    Button::new("reload-team-autostaff")
                                                                        .outline()
                                                                        .small()
                                                                        .h(px(28.))
                                                                        .px_3()
                                                                        .rounded_lg()
                                                                        .label(tr("重新读取"))
                                                                        .disabled(!self.connected)
                                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                                            this.team_autostaff_request(None, cx)
                                                                        })),
                                                                ))
                                                            }),
                                                    ),
                                            ),
                                    ),
                            ))
                        }
                        SettingsSection::Notifications => content.id("settings-notification-container").child(crate::focus_scroll::Viewport::new("notifications-viewport", &self.notification_scroll, v_flex().id("settings-notification-content").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(&self.notification_scroll).vertical_scrollbar(&self.notification_scroll).pr_1().child(self.render_notification_settings(cx)))),
                        SettingsSection::Programs => content.id("settings-programs-container").child(self.render_program_settings(cx)),
                        SettingsSection::About => content.id("settings-about-container").child(crate::focus_scroll::Viewport::new(
                            "about-viewport", &self.about_scroll,
                            v_flex().id("settings-about-content").flex_1().min_h_0().min_w_0()
                                .overflow_y_scroll().track_scroll(&self.about_scroll).vertical_scrollbar(&self.about_scroll).pr_1()
                                .child(self.render_about_settings(cx)))),
                    }
                }).into_any_element()
        };
        self.render_settings_shell(content, compact_settings, window, cx)
    }
}
