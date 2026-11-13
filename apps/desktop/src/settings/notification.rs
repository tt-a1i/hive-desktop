use crate::app::HiveApp;
use crate::focus_scroll::Reveal;
use crate::language::tr;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::json;

const PATH: &str = "/api/settings/app-state/notifications.webhook-url";

fn valid_url(value: &str) -> bool {
    value.is_empty()
        || url::Url::parse(value).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

impl HiveApp {
    // None reads, Some(false) saves the draft, Some(true) clears it.
    pub fn notification_request(
        &mut self,
        action: Option<bool>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.notification_loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let value = if action == Some(true) {
            String::new()
        } else {
            self.notification_url.read(cx).value().trim().to_string()
        };
        if action == Some(false) && !valid_url(&value) {
            self.notification_message =
                Some("请输入有效的 HTTP 或 HTTPS 地址，或留空清除。".into());
            cx.notify();
            return;
        }
        self.notification_generation += 1;
        let generation = self.notification_generation;
        self.notification_loading = true;
        self.notification_message = None;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let params = if action.is_some() {
                json!({"method":"PUT", "path":PATH, "body":{"value":value}})
            } else {
                json!({"method":"GET", "path":PATH})
            };
            let result = runtime.request("hive.request", params).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                if this.notification_generation != generation {
                    return;
                }
                this.notification_loading = false;
                match result {
                    Ok(response) => {
                        let next = if action.is_some() {
                            Some(value)
                        } else {
                            match response.get("value") {
                                Some(serde_json::Value::Null) => Some(String::new()),
                                Some(serde_json::Value::String(value)) => Some(value.clone()),
                                _ => None,
                            }
                        };
                        if let Some(value) = next {
                            let input = this.notification_url.clone();
                            let _ = window.update(cx, |_, window, cx| {
                                input.update(cx, |input, cx| input.set_value(value, window, cx))
                            });
                            this.notification_loaded = true;
                            if action.is_some() {
                                this.notification_message = Some(
                                    if action == Some(true) {
                                        "通知地址已清除"
                                    } else {
                                        "通知地址已保存"
                                    }
                                    .into(),
                                );
                            }
                        } else {
                            this.notification_message = Some("通知设置响应格式错误".into());
                        }
                    }
                    Err(_) => {
                        this.notification_message = Some(
                            if action.is_some() {
                                "保存通知地址失败，请重试。"
                            } else {
                                "读取通知地址失败，请重试。"
                            }
                            .into(),
                        )
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn render_notification_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = !self.connected || self.notification_loading || !self.notification_loaded;
        let webhook_notice = self.notification_message.map(|message| {
            let is_error = matches!(
                message,
                "通知设置响应格式错误"
                    | "保存通知地址失败，请重试。"
                    | "读取通知地址失败，请重试。"
                    | "后台尚未连接"
            );
            (message, is_error)
        });
        super::settings_card(cx)
            .p_4()
            .gap_4()
            .child(self.render_local_notification_settings(cx))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("完成通知 Webhook")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr("全局设置。成员汇报或工作流结束时，Hive 会尽力向此地址发送通知；留空可关闭。地址可能含密钥，请妥善保管。")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("通知内容包括工作区标识、成员名及汇报或工作流摘要。")),
                            ),
                    )
                    .child(Reveal::new(
                        "notification-webhook-input-reveal",
                        &self.notification_scroll,
                        Input::new(&self.notification_url)
                            .aria_label(tr("完成通知 Webhook 地址"))
                            .mask_toggle()
                            .disabled(disabled),
                    ))
                    .child(Reveal::new(
                        "notification-webhook-actions-reveal",
                        &self.notification_scroll,
                        h_flex()
                            .gap_2()
                            .items_center()
                            .flex_wrap()
                            .child(
                                Button::new("save-notification-url")
                                    .primary()
                                    .h(px(30.))
                                    .px_3p5()
                                    .rounded_lg()
                                    .label(if self.notification_loading {
                                        tr("处理中…")
                                    } else {
                                        tr("保存地址")
                                    })
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.notification_request(
                                            Some(false),
                                            window.window_handle(),
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new("clear-notification-url")
                                    .outline()
                                    .h(px(30.))
                                    .px_3p5()
                                    .rounded_lg()
                                    .label(tr("清除地址"))
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.notification_request(
                                            Some(true),
                                            window.window_handle(),
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new("reload-notification-url")
                                    .ghost()
                                    .h(px(30.))
                                    .px_3()
                                    .rounded_lg()
                                    .label(tr("重新读取"))
                                    .disabled(!self.connected || self.notification_loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.notification_request(None, window.window_handle(), cx)
                                    })),
                            ),
                    ))
                    .when_some(webhook_notice, |view, (message, is_error)| {
                        view.child(super::settings_notice_box(
                            "notification-webhook-notice",
                            tr(message),
                            is_error,
                            cx,
                        ))
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::valid_url;

    #[test]
    fn accepts_empty_and_http_urls_only() {
        assert!(valid_url(""));
        assert!(valid_url("http://localhost:1234/hook"));
        assert!(valid_url("https://example.test/hook?token=x"));
        assert!(!valid_url("ftp://example.test/hook"));
        assert!(!valid_url("https://"));
        assert!(!valid_url("not a url"));
    }
}
