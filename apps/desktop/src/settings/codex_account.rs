use crate::{app::HiveApp, language::tr, palette::Color};
use gpui_kit::{component::{button::Button, *}, prelude::FluentBuilder, *};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountResult { status: String, account_type: Option<String>, command: String, source: String }
#[derive(Default)]
pub struct CodexAccountCheck {
    input: String,
    loading: bool,
    result: Option<Result<AccountResult, String>>,
}
impl HiveApp {
    fn check_codex_account(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.codex_account.loading { return; }
        let Some(runtime) = self.runtime.clone() else { return; };
        let input = self.codex_command.read(cx).value().trim().to_owned();
        self.codex_account = CodexAccountCheck { input: input.clone(), loading: true, result: None };
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
    pub(crate) fn render_codex_account_check(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let state = &self.codex_account;
        let environment = state.result.as_ref().and_then(|value| value.as_ref().ok()).is_some_and(|value| value.source == "env");
        let changed = !environment && self.codex_command.read(cx).value().trim() != state.input;
        v_flex().min_w_0().gap_2()
            .child(h_flex().child(Button::new("check-codex-account").label(if state.loading { tr("检查中…") } else { tr("检查 Codex 连接与账号") })
                .disabled(!self.connected || self.settings_loading || self.executable_settings.is_none() || state.loading)
                .on_click(cx.listener(|this, _, _, cx| this.check_codex_account(cx)))))
            .child(crate::text_reading::selectable_text("codex-account-scope", tr("建立原生连接并只读检查账号，不刷新凭据、不启动模型。检测到账号不代表登录仍有效或模型可用。"))
                .text_xs().text_color(Color::Muted.get(cx)))
            .when_some(state.result.as_ref(), |view, result| {
                if changed { return view.child(crate::text_reading::selectable_text("codex-account-changed", tr("输入已更改，请重新检查。"))); }
                let (message, error) = match result {
                    Err(_) => (tr("无法完成 Codex 连接与账号检查，请核对程序路径并重试。").to_owned(), true),
                    Ok(result) => {
                        let key = match result.status.as_str() {
                            "account_detected" => match result.account_type.as_deref() {
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
                        };
                        (tr(key).to_owned(), result.status == "failed")
                    }
                };
                view.child(crate::text_reading::selectable_text("codex-account-status", message).text_sm().text_color(if error { Color::Error.get(cx) } else { Color::Text.get(cx) }))
                    .when_some(result.as_ref().ok(), |view, result| view.child(crate::text_reading::selectable_text("codex-account-source",
                        super::EffectiveCommand { command:result.command.clone(), source:result.source.clone() }.label(tr("本次检查"))).text_xs().text_color(Color::Muted.get(cx))))
            })
    }
}
