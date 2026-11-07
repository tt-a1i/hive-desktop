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
        let query = accounts.search.read(cx).value().trim().to_lowercase();
        let matching = accounts
            .providers
            .iter()
            .flatten()
            .filter(|provider| provider.matches(&query))
            .collect::<Vec<_>>();
        v_flex().gap_4()
            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child("连接内置 Pi 使用的账号，与 Pi CLI 共用登录信息。外部 Agent 使用各自的账号配置。"))
            .child(h_flex().gap_2().flex_wrap()
                .child(Button::new("reload-pi-accounts").ghost().compact().label(if accounts.loading { "处理中…" } else { "刷新账号状态" }).disabled(disabled)
                    .on_click(cx.listener(|this, _, window, cx| this.pi_auth_request("pi.auth.providers", json!({}), window.window_handle(), cx))))
                .when_some(accounts.agent_dir.clone(), |row, dir| row.child(Button::new("open-pi-config-folder").ghost().compact().label("打开 Pi 配置目录").tooltip(dir.clone())
                    .on_click(move |_, _, cx| cx.reveal_path(std::path::Path::new(&dir))))))
            .child(Input::new(&accounts.search).aria_label("搜索 Pi 提供商").disabled(accounts.providers.is_none()))
            .when(accounts.providers.is_some(), |view| view.child(h_flex().items_center().justify_between().gap_2()
                .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} 个提供商", matching.len())))
                .when(!query.is_empty(), |row| row.child(Button::new("clear-pi-provider-search").ghost().compact().label("清除搜索").on_click(cx.listener(|this, _, window, cx| this.pi_accounts.search.update(cx, |input, cx| { input.set_value("", window, cx); input.focus_handle(cx).focus(window, cx); })))))))
            .when_some(accounts.message.clone(), |view, message| view.child(div().p_3().rounded_lg().bg(Color::Inset.get(cx)).text_sm().child(message)))
            .when_some(accounts.login.clone(), |view, login| {
                let running = login.state == "running";
                let api = login.auth_type == "api_key";
                let provider = accounts.providers.as_ref().and_then(|providers| providers.iter().find(|provider| provider.id == login.provider_id)).map(|provider| provider.name.clone()).unwrap_or(login.provider_id.clone());
                let login_id = login.id.clone();
                view.child(v_flex().p_4().gap_3().rounded_lg().border_1().border_color(Color::Divider.get(cx))
                    .child(h_flex().gap_2().items_center().flex_wrap()
                        .child(div().flex_1().min_w_0().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("{} · {}", provider, match login.state.as_str() { "running" => if api { "正在配置" } else { "正在登录" }, "succeeded" => "已保存", "cancelled" => "已取消", _ => "登录失败" })))
                        .when(running, |row| row.child(Button::new("cancel-pi-login").ghost().compact().label(if api { "取消配置" } else { "取消登录" }).disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| this.pi_auth_request("pi.auth.cancel", json!({"loginId":login_id}), window.window_handle(), cx))))))
                    .when(login.state == "succeeded" && self.session.as_ref().is_some_and(|session| matches!(session.backend, crate::protocol::Backend::Pi)), |view| view.child(Button::new("return-after-pi-login").rounded_full().label("返回会话并刷新模型").disabled(!self.connected || self.session.as_ref().is_some_and(|session| self.busy.contains(&session.id))).on_click(cx.listener(|this, _, window, cx| { this.leave_preferences(window, cx); this.refresh_pi_models(cx); }))))
                    .when(running && accounts.providers.as_ref().and_then(|providers| providers.iter().find(|provider| provider.id == login.provider_id)).is_some_and(|provider| provider.stored_type.as_ref().is_some_and(|kind| kind != &login.auth_type)), |view| view.child(div().text_sm().text_color(Color::Secondary.get(cx)).child("完成后将替换此提供商已保存的登录方式；中途取消会保留旧配置。")))
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
                                .child(Input::new(&accounts.input).aria_label(prompt.message).disabled(disabled).when(matches!(prompt.kind.as_str(), "secret" | "manual_code"), |input| input.mask_toggle()))
                                .child(h_flex().child(Button::new("submit-pi-login").primary().rounded_full().label(if api { "继续" } else { "继续登录" }).disabled(disabled || (prompt.kind != "text" && accounts.input.read(cx).value().trim().is_empty()))
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
            .when(accounts.providers.is_some() && matching.is_empty(), |view| view.child(div().py_6().text_sm().text_color(Color::Secondary.get(cx)).child("没有匹配的提供商。可以换个名称搜索，或清除搜索查看全部。")))
            .children(matching.into_iter().map(|provider| self.render_pi_provider(provider, cx)))
            .child(div().pt_2().text_xs().text_color(Color::Muted.get(cx)).child("本机保存的配置与 Pi 共用；环境变量等配置仍可生效。保存状态不代表已连接服务端。自定义模型可在配置目录中编辑。"))
    }
}

impl Provider {
    fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || format!(
                "{} {} {}",
                self.id,
                self.name,
                self.methods
                    .iter()
                    .map(|method| method.label.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .to_lowercase()
            .contains(query)
    }
}
impl HiveApp {
    fn render_pi_provider(
        &self,
        provider: &Provider,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = provider.id.clone();
        let disabled = !self.connected || self.pi_accounts.loading || self.pi_accounts.running();
        v_flex()
            .gap_2()
            .py_3()
            .border_b_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .flex_wrap()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(provider.name.clone()),
                    )
                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(
                        match provider.stored_type.as_deref() {
                            Some("oauth") => "已保存订阅账号",
                            Some("api_key") => "已保存 API 配置",
                            _ => "未保存本机凭据",
                        },
                    )),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .children(provider.methods.iter().map(|method| {
                        let kind = method.kind.clone();
                        let id = provider.id.clone();
                        let label = match (kind.as_str(), provider.stored_type.as_deref()) {
                            ("oauth", Some("oauth")) => "重新登录",
                            ("oauth", _) => "订阅登录",
                            (_, Some("api_key")) => "更新 API 配置",
                            _ => "API 配置",
                        };
                        Button::new(SharedString::from(format!("connect-pi-{id}-{kind}")))
                            .rounded_full()
                            .compact()
                            .label(label)
                            .tooltip(method.label.clone())
                            .accessibility_label(format!(
                                "{} · {}",
                                if kind == "oauth" {
                                    "订阅登录"
                                } else {
                                    "API 配置"
                                },
                                provider.name
                            ))
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.pi_auth_request(
                                    "pi.auth.start",
                                    json!({"providerId":id,"authType":kind}),
                                    window.window_handle(),
                                    cx,
                                )
                            }))
                    }))
                    .when(provider.methods.is_empty(), |row| {
                        row.child(
                            div()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child("沿用 Pi 的现有配置"),
                        )
                    })
                    .when(provider.stored_type.is_some(), |row| {
                        row.child(
                            Button::new(SharedString::from(format!("disconnect-pi-{id}")))
                                .ghost()
                                .compact()
                                .label("移除登录信息")
                                .accessibility_label(format!("移除 {} 登录信息", provider.name))
                                .disabled(disabled)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.confirm_pi_disconnect(id.clone(), window, cx)
                                })),
                        )
                    }),
            )
    }
}
