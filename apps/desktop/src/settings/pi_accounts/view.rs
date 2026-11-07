use super::*;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        *,
    },
    prelude::FluentBuilder,
};

fn http_url(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

impl HiveApp {
    pub fn render_pi_accounts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let accounts = &self.pi_accounts;
        let disabled = !self.connected || accounts.loading;
        v_flex().gap_4()
            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child("连接内置 Pi 使用的账号，与 Pi CLI 共用登录信息。外部 Agent 使用各自的账号配置。"))
            .child(h_flex().gap_2().flex_wrap()
                .child(Button::new("reload-pi-accounts").ghost().compact().label(if accounts.loading { "处理中…" } else { "刷新账号状态" }).disabled(disabled)
                    .on_click(cx.listener(|this, _, window, cx| this.pi_auth_request("pi.auth.providers", json!({}), window.window_handle(), cx))))
                .when_some(accounts.agent_dir.clone(), |row, dir| row.child(Button::new("open-pi-config-folder").ghost().compact().label("打开 Pi 配置目录").tooltip(dir.clone())
                    .on_click(move |_, _, cx| cx.reveal_path(std::path::Path::new(&dir))))))
            .when_some(accounts.message.clone(), |view, message| view.child(div().p_3().rounded_lg().bg(Color::Inset.get(cx)).text_sm().child(message)))
            .when_some(accounts.login.clone(), |view, login| {
                let running = login.state == "running";
                let provider = accounts.providers.as_ref().and_then(|providers| providers.iter().find(|provider| provider.id == login.provider_id)).map(|provider| provider.name.clone()).unwrap_or(login.provider_id.clone());
                let login_id = login.id.clone();
                view.child(v_flex().p_4().gap_3().rounded_lg().border_1().border_color(Color::Divider.get(cx))
                    .child(h_flex().gap_2().items_center().flex_wrap()
                        .child(div().flex_1().min_w_0().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("{} · {}", provider, match login.state.as_str() { "running" => "正在登录", "succeeded" => "已连接", "cancelled" => "已取消", _ => "登录失败" })))
                        .when(running, |row| row.child(Button::new("cancel-pi-login").ghost().compact().label("取消登录").disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| this.pi_auth_request("pi.auth.cancel", json!({"loginId":login_id}), window.window_handle(), cx))))))
                    .when(login.state == "succeeded" && self.session.as_ref().is_some_and(|session| matches!(session.backend, crate::protocol::Backend::Pi)), |view| view.child(Button::new("return-after-pi-login").rounded_full().label("返回会话并刷新模型").disabled(!self.connected || self.session.as_ref().is_some_and(|session| self.busy.contains(&session.id))).on_click(cx.listener(|this, _, window, cx| { this.leave_preferences(window, cx); this.refresh_pi_models(cx); }))))
                    .when_some(login.message, |view, message| view.child(div().text_sm().child(message)))
                    .when_some(login.authorization.as_ref().or(login.notice.as_ref()), |view, notice| {
                        let kind = notice.get("type").and_then(Value::as_str).unwrap_or("");
                        let uri = notice.get(if kind == "device_code" { "verificationUri" } else { "url" }).and_then(Value::as_str).unwrap_or("").to_string();
                        let message = notice.get("instructions").or_else(|| notice.get("message")).and_then(Value::as_str).unwrap_or("").to_string();
                        view.child(v_flex().gap_2().when(!message.is_empty(), |view| view.child(div().text_sm().child(message)))
                            .when_some(notice.get("userCode").and_then(Value::as_str).map(String::from), |view, code| view.child(h_flex().items_center().gap_3()
                                .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(code.clone()))
                                .child(Button::new("copy-pi-device-code").ghost().compact().label("复制设备码").on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(code.clone()))))))
                            .when(http_url(&uri), |view| view.child(h_flex().flex_wrap().gap_2()
                                .child(Button::new("open-pi-login-url").primary().rounded_full().label("在浏览器中授权").on_click({ let uri = uri.clone(); move |_, _, cx| cx.open_url(&uri) }))
                                .child(Button::new("copy-pi-login-url").ghost().compact().label("复制授权链接").on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(uri.clone())))))))
                    })
                    .when_some(login.notice.as_ref().filter(|_| login.authorization.is_some()).and_then(|notice| notice.get("message")).and_then(Value::as_str).map(String::from), |view, message| view.child(div().text_sm().text_color(Color::Secondary.get(cx)).child(message)))
                    .when_some(login.prompt.filter(|_| running), |view, prompt| {
                        view.child(v_flex().gap_2().child(div().text_sm().child(prompt.message.clone()))
                            .when(prompt.kind != "select", |view| view
                                .child(Input::new(&accounts.input).aria_label(prompt.message).disabled(disabled).mask_toggle())
                                .child(h_flex().child(Button::new("submit-pi-login").primary().rounded_full().label("继续登录").disabled(disabled || accounts.input.read(cx).value().trim().is_empty())
                                    .on_click(cx.listener(|this, _, window, cx| this.submit_pi_login(None, window, cx))))))
                            .when(prompt.kind == "select", |view| view.children(prompt.options.into_iter().map(|choice| {
                                let id = choice.id;
                                v_flex().gap_1().child(Button::new(SharedString::from(format!("pi-login-choice-{id}"))).label(choice.label).disabled(disabled)
                                    .on_click(cx.listener(move |this, _, window, cx| this.submit_pi_login(Some(id.clone()), window, cx))))
                                    .when_some(choice.description, |view, description| view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(description)))
                            }))))
                    }))
            })
            .when(accounts.providers.is_none(), |view| view.child(div().text_sm().text_color(Color::Secondary.get(cx)).child(if accounts.loading { "正在读取 Pi 账号…" } else { "点击刷新读取可连接的账号。" })))
            .children(accounts.providers.iter().flatten().map(|provider| {
                let id = provider.id.clone();
                h_flex().items_center().gap_3().py_3().border_b_1().border_color(Color::Divider.get(cx))
                    .child(v_flex().flex_1().min_w_0().gap_1()
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(provider.name.clone()))
                        .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(provider.label.clone()))
                        .child(div().text_xs().text_color(Color::Muted.get(cx)).child(match provider.stored_type.as_deref() { Some("oauth") => "已保存订阅账号", Some("api_key") => "已保存 API 配置 · 登录将替换该提供商的凭据", _ => "尚未保存账号" })))
                    .child(Button::new(SharedString::from(format!("connect-pi-{id}"))).rounded_full().label(if provider.stored_type.is_some() { "重新登录" } else { "连接" }).accessibility_label(format!("{} {} 账号", if provider.stored_type.is_some() { "重新登录" } else { "连接" }, provider.name)).disabled(disabled || accounts.running())
                        .on_click(cx.listener(move |this, _, window, cx| this.pi_auth_request("pi.auth.start", json!({"providerId":id}), window.window_handle(), cx))))
            }))
            .child(div().pt_2().text_xs().text_color(Color::Muted.get(cx)).child("API 提供商和自定义模型沿用 Pi 的环境变量与 models.json 配置。此处的“已保存”表示本机有凭据，不代表服务端已验证账号仍有效。"))
    }
}
