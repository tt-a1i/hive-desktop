use crate::{app::HiveApp, language::tr, palette::Color};
use gpui_kit::{
    component::{
        button::Button,
        Theme,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
static LIMIT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LimitWindow { position: String, used_percent: f64, window_duration_mins: Option<f64>, resets_at: Option<String> }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LimitsResult { status: String, checked_at: String, windows: Vec<LimitWindow>, source: String }
#[derive(Default)]
struct LimitsState { input: String, request_id: u64, loading: bool, result: Option<Result<LimitsResult, String>> }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountResult { status: String, account_type: Option<String>, command: String, source: String, login_command: Option<String> }
#[derive(Default)]
pub struct CodexAccountCheck {
    input: String,
    loading: bool,
    result: Option<Result<AccountResult, String>>,
    limits: LimitsState,
}
impl LimitsState {
    fn accepts(&self, id: u64, connection: u64, current_connection: u64, connected: bool) -> bool {
        connected && connection == current_connection && self.request_id == id
    }
    fn input_changed(&self, input: &str) -> bool {
        let uses_env = self.result.as_ref().and_then(|r| r.as_ref().ok()).is_some_and(|r| r.source == "env");
        !uses_env && input.trim() != self.input
    }
}
impl AccountResult {
    fn message_key(&self) -> &'static str {
        match self.status.as_str() {
            "account_detected" => match self.account_type.as_deref() {
                Some("chatgpt") => "已连接 Codex，检测到 ChatGPT 账号记录。",
                Some("apiKey") => "已连接 Codex，检测到 API Key 账号记录。",
                Some("amazonBedrock") => "已连接 Codex，检测到 Amazon Bedrock 账号记录。",
                Some("gcp") => "已连接 Codex，检测到 Google Cloud 账号记录。",
                _ => "已连接 Codex，账号类型尚无法识别。",
            },
            "signed_out" => "已连接 Codex，未检测到账号；请在对应 CLI 中登录。",
            "not_required" => "已连接 Codex，当前配置不要求 OpenAI 账号。",
            "unknown" => "已连接 Codex，账号类型尚无法识别。",
            "cancelled" => "检查已取消。",
            _ => "无法完成 Codex 连接与账号检查，请核对程序路径并重试。",
        }
    }
}
impl CodexAccountCheck {
    fn uses_environment(&self) -> bool {
        self.result.as_ref().and_then(|value| value.as_ref().ok()).is_some_and(|value| value.source == "env")
    }
    fn input_changed(&self, input: &str) -> bool { !self.uses_environment() && input.trim() != self.input }
    fn message(&self) -> (String, bool) {
        match self.result.as_ref() {
            None => (tr("尚未检查连接与账号").into(), false),
            Some(Err(_)) => (tr("无法完成 Codex 连接与账号检查，请核对程序路径并重试。").into(), true),
            Some(Ok(result)) => (tr(result.message_key()).into(), result.status == "failed"),
        }
    }
}
impl HiveApp {
    fn check_codex_account(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.codex_account.loading { return; }
        let Some(runtime) = self.runtime.clone() else { return; };
        let input = self.codex_command.read(cx).value().trim().to_owned();
        self.codex_account = CodexAccountCheck { input: input.clone(), loading: true, result: None, limits: LimitsState::default() };
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime.request("desktop.settings.codexAccount", serde_json::json!({"command":if input.is_empty(){None}else{Some(input)}})).await
                .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection { return; }
                this.codex_account.loading = false;
                this.codex_account.result = Some(result);
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn check_codex_limits(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.codex_account.limits.loading { return; }
        let Some(runtime) = self.runtime.clone() else { return; };
        let input = self.codex_command.read(cx).value().trim().to_owned();
        let id = LIMIT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        self.codex_account.limits = LimitsState { input: input.clone(), request_id: id, loading: true, result: None };
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime.request("desktop.settings.codexLimits", serde_json::json!({"command":if input.is_empty(){None}else{Some(input)}})).await
                .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()));
            let _ = this.update(cx, |this, cx| {
                if !this.codex_account.limits.accepts(id, connection, this.connection_generation, this.connected) { return; }
                this.codex_account.limits.loading = false;
                this.codex_account.limits.result = Some(result);
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn render_codex_limits(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let state = &self.codex_account.limits;
        let input = self.codex_command.read(cx).value();
        let stale = state.input_changed(&input);
        v_flex().gap_2().pt_2().border_t_1().border_color(Color::Divider.get(cx))
            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(tr("Codex 账号额度")))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("账号额度与当前会话的 token 用量不同。此检查不会启动模型；切换账号后请重新读取。")))
            .child(Button::new("check-codex-limits").outline().small().label(if state.loading { tr("正在读取额度…") } else { tr("读取 Codex 账号额度") })
                .disabled(!self.connected || self.settings_loading || self.executable_settings.is_none() || state.loading)
                .on_click(cx.listener(|this, _, _, cx| this.check_codex_limits(cx))))
            .when(!self.connected && state.result.is_some(), |v| v.child(div().text_xs().child(tr("后台连接已断开，请重新读取额度。"))))
            .when(self.connected && stale && state.result.is_some(), |v| v.child(div().text_xs().child(tr("程序路径已更改，请重新读取额度。"))))
            .when(self.connected && !stale, |v| v.when_some(state.result.as_ref(), |v, result| match result {
                Err(_) => v.child(div().text_xs().text_color(Color::Error.get(cx)).child(tr("无法读取 Codex 账号额度。"))),
                Ok(result) => {
                    let message = match result.status.as_str() {
                        "ok" => tr("额度读取成功"), "signed_out" => tr("未检测到 Codex 登录。"),
                        "not_applicable" => tr("当前账号不提供 ChatGPT 额度窗口。"),
                        "unavailable" => tr("账号额度暂不可用。"), "cancelled" => tr("检查已取消。"),
                        _ => tr("无法读取 Codex 账号额度。"),
                    };
                    v.child(div().text_xs().child(message))
                        .child(div().text_xs().text_color(Color::Muted.get(cx)).child(format!("{}: {}", tr("读取时间"), result.checked_at)))
                        .children(result.windows.iter().map(|window| {
                            let label = if window.position == "primary" { tr("主要窗口") } else { tr("次要窗口") };
                            let duration = window.window_duration_mins.map(|x| format!("{x} {}", tr("分钟"))).unwrap_or_else(|| tr("时长未知").to_owned());
                            let reset = window.resets_at.as_deref().unwrap_or(tr("重置时间未知"));
                            div().text_xs().child(format!("{label}: {}% {} · {duration} · {}: {reset}", 100. - window.used_percent, tr("剩余"), tr("重置")))
                        }))
                }
            }))
    }
    pub(crate) fn codex_account_setup_status(&self, cx: &Context<Self>) -> String {
        let state = &self.codex_account;
        if state.loading { return tr("正在检查连接与账号…").into(); }
        if state.result.is_none() { return state.message().0; }
        let input = self.codex_command.read(cx).value();
        if state.input_changed(&input) { return tr("输入已更改，请重新检查连接与账号。").into(); }
        let message = state.message().0;
        let saved = self.executable_settings.as_ref().and_then(|settings| settings.codex_command.as_deref()).unwrap_or_default();
        if !state.uses_environment() && input.trim() != saved { format!("{}: {message}", tr("未保存的程序路径")) } else { message }
    }
    pub(crate) fn render_codex_account_check(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let state = &self.codex_account;
        let changed = state.input_changed(&self.codex_command.read(cx).value());
        let (dot_color, pill_text) = if state.loading {
            (super::amber_dot(cx), tr("检查中…"))
        } else if changed {
            (super::amber_dot(cx), tr("需检查"))
        } else {
            match state.result.as_ref() {
                None => (Color::Muted.get(cx), tr("待配置")),
                Some(Err(_)) => (Color::Error.get(cx), tr("需检查")),
                Some(Ok(res)) => match res.status.as_str() {
                    "account_detected" | "not_required" => (Color::Success.get(cx), tr("已连接")),
                    "signed_out" => (super::amber_dot(cx), tr("未登录")),
                    "failed" => (Color::Error.get(cx), tr("需检查")),
                    _ => (Color::Muted.get(cx), tr("状态未知")),
                },
            }
        };
        v_flex()
            .min_w_0()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(Color::Inset.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("check-codex-account")
                            .outline()
                            .small()
                            .h(px(28.))
                            .px_3()
                            .rounded_lg()
                            .label(if state.loading {
                                tr("检查中…")
                            } else {
                                tr("检查 Codex 连接与账号")
                            })
                            .disabled(
                                !self.connected
                                    || self.settings_loading
                                    || self.executable_settings.is_none()
                                    || state.loading,
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.check_codex_account(cx))),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py(px(2.))
                            .rounded_full()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(div().size(px(6.)).rounded_full().bg(dot_color))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(pill_text),
                            ),
                    ),
            )
            .child(
                crate::text_reading::selectable_text(
                    "codex-account-scope",
                    tr("建立原生连接并只读检查账号，不刷新凭据、不启动模型。检测到账号不代表登录仍有效或模型可用。"),
                )
                .text_xs()
                .text_color(Color::Muted.get(cx)),
            )
            .child(self.render_codex_limits(cx))
            .when_some(state.result.as_ref(), |view, result| {
                if changed {
                    return view.child(
                        crate::text_reading::selectable_text(
                            "codex-account-changed",
                            tr("输入已更改，请重新检查。"),
                        )
                        .text_xs()
                        .text_color(Color::Secondary.get(cx)),
                    );
                }
                let (message, error) = state.message();
                view.child(
                    crate::text_reading::selectable_text("codex-account-status", message)
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if error {
                            Color::Error.get(cx)
                        } else {
                            Color::Text.get(cx)
                        }),
                )
                .when_some(result.as_ref().ok(), |view, result| {
                    view.child(
                        crate::text_reading::selectable_text(
                            "codex-account-source",
                            super::EffectiveCommand {
                                command: result.command.clone(),
                                source: result.source.clone(),
                            }
                            .label(tr("本次检查")),
                        )
                        .text_xs()
                        .text_color(Color::Muted.get(cx)),
                    )
                    .when_some(
                        result
                            .login_command
                            .as_ref()
                            .filter(|_| result.status == "signed_out"),
                        |view, command| {
                            let copy = command.clone();
                            view.child(
                                v_flex()
                                    .gap_1p5()
                                    .pt_1()
                                    .child(
                                        crate::text_reading::selectable_text(
                                            "codex-login-instructions",
                                            if cfg!(target_os = "windows") {
                                                tr("在 PowerShell 中执行以下命令完成登录，然后重新检查。")
                                            } else {
                                                tr("在终端执行以下命令完成登录，然后重新检查。")
                                            },
                                        )
                                        .text_xs()
                                        .text_color(Color::Secondary.get(cx)),
                                    )
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .px_2p5()
                                            .py_2()
                                            .rounded_lg()
                                            .bg(Color::Card.get(cx))
                                            .border_1()
                                            .border_color(Color::Divider.get(cx))
                                            .child(
                                                div().flex_1().min_w_0().child(
                                                    crate::text_reading::selectable_text(
                                                        "codex-login-command",
                                                        command.clone(),
                                                    )
                                                    .font_family(
                                                        Theme::global(cx).mono_font_family.clone(),
                                                    )
                                                    .text_xs()
                                                    .text_color(Color::Text.get(cx)),
                                                ),
                                            )
                                            .child(
                                                Button::new("copy-codex-login-command")
                                                    .outline()
                                                    .small()
                                                    .h(px(26.))
                                                    .px_2p5()
                                                    .rounded_lg()
                                                    .label(tr("复制 Codex 登录命令"))
                                                    .on_click(move |_, _, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(copy.clone()),
                                                        )
                                                    }),
                                            ),
                                    ),
                            )
                        },
                    )
                })
            })
    }
}

#[cfg(test)]
mod limits_tests {
    use super::LimitsState;
    #[test]
    fn old_requests_and_changed_executables_do_not_show_as_current() {
        let state = LimitsState { input: "codex-a".into(), request_id: 9, loading: false, result: None };
        assert!(state.accepts(9, 2, 2, true));
        assert!(!state.accepts(8, 2, 2, true));
        assert!(!state.accepts(9, 2, 3, true));
        assert!(!state.accepts(9, 2, 2, false));
        assert!(state.input_changed("codex-b"));
        assert!(!state.input_changed("codex-a"));
    }
}
