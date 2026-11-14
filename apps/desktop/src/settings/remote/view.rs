use super::*;
use crate::focus_scroll::Reveal;
use crate::language::{english, tr};
use crate::palette::Color;
use crate::text_reading::selectable_text;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        switch::Switch,
        *,
    },
    prelude::FluentBuilder,
};

fn age(time: u64) -> String {
    let seconds = now().saturating_sub(time) / 1000;
    if seconds < 60 {
        tr("刚刚").into()
    } else if seconds < 3600 {
        if english() { format!("{}m ago", seconds / 60) } else { format!("{} 分钟前", seconds / 60) }
    } else if seconds < 86400 {
        if english() { format!("{}h ago", seconds / 3600) } else { format!("{} 小时前", seconds / 3600) }
    } else {
        if english() { format!("{}d ago", seconds / 86400) } else { format!("{} 天前", seconds / 86400) }
    }
}
fn outcome(result: &str) -> &str {
    match result { "ok" => tr("成功"), "rejected" => tr("已拒绝"), "error" => tr("错误"), other => other }
}
fn action_name(action: &str) -> &str {
    match action {
        "http" => tr("接口请求"),
        "ws_open" => tr("打开实时连接"),
        "ws_input" => tr("实时连接输入"),
        "session_open" => tr("建立远程连接"),
        "session_close" => tr("关闭远程连接"),
        "revoke" => tr("撤销设备访问"),
        "reject" => tr("拒绝远程请求"),
        other => other,
    }
}
impl HiveApp {
    pub fn render_remote_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let remote = &self.remote;
        let usable = self.connected && remote.valid && !remote.loading;
        let status = remote.status.as_ref();
        let linked = status.is_some_and(|s| s.logged_in);
        let enabled = status.is_some_and(|s| s.enabled);
        let connection = status
            .map(|s| match s.connection.as_str() {
                "disabled" => tr("已关闭"),
                "loggedOut" => tr("未登录"),
                "connecting" => tr("正在连接"),
                "online" => tr("已连接"),
                "reconnecting" => tr("正在重连"),
                "revoked" => tr("登录已撤销"),
                _ => tr("状态未知"),
            })
            .unwrap_or(tr("正在读取"));
        let status_label = format!("{} · {connection}", tr("远程接入"));
        let mono_font = Theme::global(cx).mono_font_family.clone();
        let status_dot = match status.map(|s| s.connection.as_str()) {
            Some("online") => Color::Success.get(cx),
            Some("connecting" | "reconnecting") => super::super::amber_dot(cx),
            Some("revoked") => Color::Error.get(cx),
            _ => Color::Muted.get(cx),
        };
        v_flex()
            .flex_shrink_0()
            .gap_5()
            .min_w_0()
            .child(
                v_flex()
                    .gap_2p5()
                    .child(
                        div()
                            .text_sm()
                            .text_color(Color::Secondary.get(cx))
                            .child(tr("复用 Hive 的端到端加密远程接入。网关转发密文；退出桌面应用后，远程连接也会停止。")),
                    )
                    .child(
                        super::super::settings_card(cx)
                            .child(
                                super::super::settings_row(false, cx).child(
                                    v_flex()
                                        .gap_2()
                                        .child(Reveal::new(
                                            "remote-status-reveal",
                                            &remote.scroll,
                                            h_flex()
                                                .w_full()
                                                .min_w_0()
                                                .items_center()
                                                .flex_wrap()
                                                .gap_3()
                                                .child(
                                                    h_flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .size(px(6.))
                                                                .rounded_full()
                                                                .bg(status_dot),
                                                        )
                                                        .child(
                                                            div()
                                                                .id("remote-connection-status")
                                                                .role(Role::Status)
                                                                .aria_label(status_label.clone())
                                                                .text_sm()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(Color::Text.get(cx))
                                                                .child(status_label),
                                                        ),
                                                )
                                                .child(
                                                    Switch::new("remote-enabled")
                                                        .accessibility_label(tr("启用远程接入"))
                                                        .checked(enabled)
                                                        .disabled(!usable || !linked)
                                                        .on_change(cx.listener(|this, value, _, cx| {
                                                            this.remote_action(
                                                                "PUT",
                                                                "/api/remote/enabled".into(),
                                                                json!({"enabled":*value}),
                                                                cx,
                                                            )
                                                        })),
                                                )
                                                .child(
                                                    Button::new("remote-refresh")
                                                        .outline()
                                                        .small()
                                                        .h(px(28.))
                                                        .px_2p5()
                                                        .rounded_lg()
                                                        .label(tr("刷新"))
                                                        .disabled(!self.connected || remote.loading)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.refresh_remote(cx)
                                                        })),
                                                )
                                                .when(linked && enabled, |row| {
                                                    row.child(Reveal::new(
                                                        "remote-reconnect-reveal",
                                                        &remote.scroll,
                                                        Button::new("remote-reconnect")
                                                            .outline()
                                                            .small()
                                                            .h(px(28.))
                                                            .px_2p5()
                                                            .rounded_lg()
                                                            .label(tr("重新连接网关"))
                                                            .disabled(!usable)
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.remote_action(
                                                                    "PUT",
                                                                    "/api/remote/enabled".into(),
                                                                    json!({"enabled":true}),
                                                                    cx,
                                                                )
                                                            })),
                                                    ))
                                                }),
                                        ))
                                        .when_some(
                                            status.and_then(|s| s.gateway_url.clone()),
                                            |col, url| {
                                                col.child(
                                                    selectable_text(
                                                        "remote-gateway-address",
                                                        format!("{}: {url}", tr("网关")),
                                                    )
                                                    .text_xs()
                                                    .text_color(Color::Secondary.get(cx)),
                                                )
                                            },
                                        ),
                                ),
                            )
                            .child(
                                super::super::settings_row(true, cx).child(Reveal::new(
                                    "remote-diagnostics-reveal",
                                    &remote.scroll,
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .flex_wrap()
                                        .child(
                                            selectable_text(
                                                "remote-diagnostics-description",
                                                tr("包含项目与成员名称、版本和运行状态，不含消息正文、令牌或完整项目路径；仅复制到本机剪贴板。"),
                                            )
                                            .flex_1()
                                            .min_w_0()
                                            .text_xs()
                                            .text_color(Color::Secondary.get(cx)),
                                        )
                                        .child(
                                            Button::new("remote-copy-diagnostics")
                                                .outline()
                                                .small()
                                                .h(px(28.))
                                                .px_3()
                                                .rounded_lg()
                                                .label(if remote.diagnostics_loading {
                                                    tr("正在读取诊断信息…")
                                                } else {
                                                    tr("复制诊断信息")
                                                })
                                                .disabled(
                                                    !self.connected || remote.diagnostics_loading,
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.copy_remote_diagnostics(cx)
                                                })),
                                        ),
                                )),
                            )
                            .child(
                                super::super::settings_row(true, cx)
                                    .bg(Color::Inset.get(cx))
                                    .child(Reveal::new(
                                        "remote-account-reveal",
                                        &remote.scroll,
                                        v_flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(Color::Text.get(cx))
                                                    .child(if linked {
                                                        tr("已连接 Hive 账号。退出登录将关闭远程接入；请在终端执行本应用专用命令。")
                                                    } else {
                                                        tr("请复制登录命令并在终端执行，再按提示打开浏览器授权。成功登录会启用远程接入。")
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Color::Secondary.get(cx))
                                                    .child(if cfg!(target_os = "windows") {
                                                        tr("命令用于 PowerShell，并绑定当前桌面的独立数据目录。")
                                                    } else {
                                                        tr("命令绑定当前桌面的独立数据目录。")
                                                    }),
                                            )
                                            .child(
                                                h_flex().flex_wrap().gap_2().children(
                                                    [
                                                        ("login", tr("复制登录命令")),
                                                        ("logout", tr("复制退出登录命令")),
                                                        ("status", tr("复制状态命令")),
                                                    ]
                                                    .into_iter()
                                                    .map(|(key, label)| {
                                                        let command =
                                                            remote.commands[key].as_str().map(str::to_owned);
                                                        Button::new(format!("remote-command-{key}"))
                                                            .outline()
                                                            .small()
                                                            .h(px(28.))
                                                            .px_2p5()
                                                            .rounded_lg()
                                                            .label(tr(label))
                                                            .disabled(command.is_none() || !self.connected)
                                                            .on_click(move |_, _, cx| {
                                                                if let Some(command) = &command {
                                                                    cx.write_to_clipboard(
                                                                        ClipboardItem::new_string(
                                                                            command.clone(),
                                                                        ),
                                                                    );
                                                                }
                                                            })
                                                    }),
                                                ),
                                            ),
                                    )),
                            ),
                    ),
            )
            .when_some(
                remote.error.as_ref().map(|error| {
                    format!(
                        "{}: {}. {}",
                        tr("远程接入读取失败"),
                        crate::language::localize_owned_error(error),
                        tr("显示上次读取的内容，操作已暂停。")
                    )
                }),
                |view, error| {
                    view.child(super::super::settings_notice_box(
                        "remote-read-error",
                        error,
                        true,
                        cx,
                    ))
                },
            )
            .when_some(
                remote.message.as_ref().map(|message| (message.text(), remote.message_is_error())),
                |view, (message, is_error)| {
                    view.child(super::super::settings_notice_box(
                        "remote-action-notice",
                        message,
                        is_error,
                        cx,
                    ))
                },
            )
            .when(remote.loading, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("正在同步远程状态，暂显示上次读取的内容…")),
                )
            })
            .child(
                v_flex()
                    .gap_2p5()
                    .child(Reveal::new(
                        "remote-pairing-add-reveal",
                        &remote.scroll,
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("配对设备")),
                            )
                            .child(
                                Button::new("remote-new-pairing")
                                    .primary()
                                    .h(px(30.))
                                    .px_3p5()
                                    .rounded_lg()
                                    .label(tr("添加设备"))
                                    .disabled(!usable || !enabled || !linked)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.remote_action(
                                            "POST",
                                            "/api/remote/pairings".into(),
                                            json!({}),
                                            cx,
                                        )
                                    })),
                            ),
                    ))
                    .when_some(remote.ticket.clone(), |view, ticket| {
                        let remaining = ticket.expires_at.saturating_sub(now()).div_ceil(1000);
                        let code = if remaining == 0 {
                            tr("配对码已过期").into()
                        } else {
                            ticket.code.clone()
                        };
                        view.child(
                            super::super::settings_card(cx).p_4().child(Reveal::new(
                                "remote-ticket-reveal",
                                &remote.scroll,
                                v_flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(Color::Secondary.get(cx))
                                            .child(tr("在手机 Hive 配对页面输入此码，然后对照两端的安全短码。")),
                                    )
                                    .child(
                                        div()
                                            .id("remote-pairing-code")
                                            .role(Role::Label)
                                            .aria_value(if english() {
                                                format!("Pairing code: {code}")
                                            } else {
                                                format!("配对码：{code}")
                                            })
                                            .px_3p5()
                                            .py_2p5()
                                            .rounded_lg()
                                            .bg(Color::Inset.get(cx))
                                            .border_1()
                                            .border_color(Color::Divider.get(cx))
                                            .font_family(mono_font.clone())
                                            .text_2xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(Color::Text.get(cx))
                                            .child(code),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Color::Secondary.get(cx))
                                            .child(if english() {
                                                format!(
                                                    "Expires in {remaining}s · Use this code only for this pairing"
                                                )
                                            } else {
                                                format!(
                                                    "剩余 {remaining} 秒 · 每个配对码仅用于本次配对"
                                                )
                                            }),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("remote-copy-pairing")
                                                    .outline()
                                                    .small()
                                                    .h(px(28.))
                                                    .px_3()
                                                    .rounded_lg()
                                                    .label(tr("复制配对码"))
                                                    .disabled(!usable || remaining == 0)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if this.remote.valid
                                                            && let Some(ticket) =
                                                                &this.remote.ticket
                                                            && ticket.expires_at > now()
                                                        {
                                                            cx.write_to_clipboard(
                                                                ClipboardItem::new_string(
                                                                    ticket.code.clone(),
                                                                ),
                                                            );
                                                        }
                                                    })),
                                            )
                                            .child(
                                                Button::new("remote-cancel-pairing")
                                                    .ghost()
                                                    .small()
                                                    .h(px(28.))
                                                    .px_3()
                                                    .rounded_lg()
                                                    .label(tr("取消本次配对"))
                                                    .disabled(!usable)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Some(ticket) = &this.remote.ticket {
                                                            this.remote_action(
                                                                "POST",
                                                                format!(
                                                                    "/api/remote/pairings/{}/reject",
                                                                    path_id(&ticket.pairing_id)
                                                                ),
                                                                json!({}),
                                                                cx,
                                                            );
                                                        }
                                                    })),
                                            ),
                                    ),
                            )),
                        )
                    })
                    .children(remote.pending.iter().map(|pending| {
                        let confirm_id = pending.pairing_id.clone();
                        let reject_id = confirm_id.clone();
                        let device_name = pending.device_name.as_deref().unwrap_or(tr("新设备"));
                        let title = format!("{}: {device_name}", tr("确认配对"));
                        let guidance =
                            tr("请确认手机显示的安全短码与下方完全一致。不是你发起的请求或短码不符，请拒绝。");
                        let remaining_secs = pending.expires_at.saturating_sub(now()).div_ceil(1000);
                        super::super::settings_card(cx).child(Reveal::new(
                            format!("remote-pending-reveal-{confirm_id}"),
                            &remote.scroll,
                            v_flex()
                                .child(
                                    super::super::settings_row(false, cx)
                                        .bg(Color::Inset.get(cx))
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .justify_between()
                                                .gap_2()
                                                .flex_wrap()
                                                .child(
                                                    div()
                                                        .id(format!(
                                                            "remote-pairing-name-{confirm_id}"
                                                        ))
                                                        .role(Role::Label)
                                                        .aria_value(title.clone())
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(Color::Text.get(cx))
                                                        .child(title),
                                                )
                                                .child(super::super::status_pill(
                                                    super::super::amber_dot(cx),
                                                    if english() {
                                                        format!("Expires in {remaining_secs}s")
                                                    } else {
                                                        format!("剩余 {remaining_secs} 秒")
                                                    },
                                                    cx,
                                                )),
                                        ),
                                )
                                .child(
                                    super::super::settings_row(true, cx).child(
                                        v_flex()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .id(format!(
                                                        "remote-pairing-guidance-{confirm_id}"
                                                    ))
                                                    .role(Role::Label)
                                                    .aria_value(guidance)
                                                    .text_sm()
                                                    .text_color(Color::Secondary.get(cx))
                                                    .child(guidance),
                                            )
                                            .child(
                                                div()
                                                    .id(format!("remote-pairing-sas-{confirm_id}"))
                                                    .role(Role::Label)
                                                    .aria_value(if english() {
                                                        format!("Security code: {}", pending.sas)
                                                    } else {
                                                        format!("安全短码：{}", pending.sas)
                                                    })
                                                    .px_3p5()
                                                    .py_2p5()
                                                    .rounded_lg()
                                                    .bg(Color::Inset.get(cx))
                                                    .border_1()
                                                    .border_color(Color::Divider.get(cx))
                                                    .font_family(mono_font.clone())
                                                    .text_2xl()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(pending.sas.clone()),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(
                                                        Button::new(format!(
                                                            "remote-confirm-{confirm_id}"
                                                        ))
                                                        .primary()
                                                        .h(px(30.))
                                                        .px_3p5()
                                                        .rounded_lg()
                                                        .label(tr("短码一致，确认"))
                                                        .accessibility_label(format!(
                                                            "{} · {device_name} · {}",
                                                            tr("短码一致，确认"),
                                                            pending.sas
                                                        ))
                                                        .disabled(
                                                            !usable || pending.expires_at <= now(),
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.remote_pairing_decision(
                                                                    &confirm_id,
                                                                    true,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                    )
                                                    .child(
                                                        Button::new(format!(
                                                            "remote-reject-{reject_id}"
                                                        ))
                                                        .outline()
                                                        .h(px(30.))
                                                        .px_3p5()
                                                        .rounded_lg()
                                                        .label(tr("拒绝"))
                                                        .accessibility_label(format!(
                                                            "{} · {device_name}",
                                                            tr("拒绝")
                                                        ))
                                                        .disabled(
                                                            !usable || pending.expires_at <= now(),
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.remote_pairing_decision(
                                                                    &reject_id,
                                                                    false,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                    ),
                                            ),
                                    ),
                                ),
                        ))
                    })),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(tr("已配对设备")),
                    )
                    .child(
                        super::super::settings_card(cx)
                            .when(remote.details_valid && remote.devices.is_empty(), |card| {
                                card.child(super::super::settings_row(false, cx).child(
                                    selectable_text("remote-no-devices", tr("尚无已配对设备。"))
                                        .text_sm()
                                        .text_color(Color::Muted.get(cx)),
                                ))
                            })
                            .children(remote.devices.iter().enumerate().map(|(idx, device)| {
                                let id = device.id.clone();
                                let revoke_id = id.clone();
                                let confirming = remote.revoking.as_ref() == Some(&id);
                                super::super::settings_row(idx > 0, cx).child(Reveal::new(
                                    format!("remote-device-reveal-{id}"),
                                    &remote.scroll,
                                    v_flex()
                                        .gap_2()
                                        .child(
                                            h_flex()
                                                .w_full()
                                                .min_w_0()
                                                .items_center()
                                                .justify_between()
                                                .gap_3()
                                                .flex_wrap()
                                                .child(
                                                    v_flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .gap_0p5()
                                                        .child(
                                                            selectable_text(
                                                                format!("remote-device-name-{id}"),
                                                                device.name.clone(),
                                                            )
                                                            .text_sm()
                                                            .font_weight(FontWeight::SEMIBOLD),
                                                        )
                                                        .child(
                                                            selectable_text(
                                                                format!(
                                                                    "remote-device-details-{id}"
                                                                ),
                                                                if english() {
                                                                    format!(
                                                                        "{} · Created {} · Last active {}",
                                                                        device.id,
                                                                        age(device.created_at),
                                                                        device
                                                                            .last_active
                                                                            .map(age)
                                                                            .unwrap_or(
                                                                                tr("尚无记录").into()
                                                                            )
                                                                    )
                                                                } else {
                                                                    format!(
                                                                        "{} · 创建于{} · 最近活动{}",
                                                                        device.id,
                                                                        age(device.created_at),
                                                                        device
                                                                            .last_active
                                                                            .map(age)
                                                                            .unwrap_or(
                                                                                tr("尚无记录").into()
                                                                            )
                                                                    )
                                                                },
                                                            )
                                                            .text_xs()
                                                            .text_color(Color::Secondary.get(cx)),
                                                        ),
                                                )
                                                .child(
                                                    h_flex()
                                                        .flex_shrink_0()
                                                        .items_center()
                                                        .gap_1p5()
                                                        .child(
                                                            Button::new(format!(
                                                                "remote-revoke-{id}"
                                                            ))
                                                            .outline()
                                                            .small()
                                                            .h(px(28.))
                                                            .px_2p5()
                                                            .rounded_lg()
                                                            .label(if confirming {
                                                                tr("确认撤销")
                                                            } else {
                                                                tr("撤销访问")
                                                            })
                                                            .accessibility_label(format!(
                                                                "{} · {} · {id}",
                                                                if confirming {
                                                                    tr("确认撤销")
                                                                } else {
                                                                    tr("撤销访问")
                                                                },
                                                                device.name
                                                            ))
                                                            .disabled(!usable || !remote.details_valid)
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    if confirming {
                                                                        this.remote_revoke(
                                                                            &revoke_id,
                                                                            cx,
                                                                        );
                                                                    } else {
                                                                        this.remote.revoking =
                                                                            Some(revoke_id.clone());
                                                                        cx.notify();
                                                                    }
                                                                },
                                                            )),
                                                        )
                                                        .when(confirming, |row| {
                                                            row.child(
                                                                Button::new(format!(
                                                                    "remote-revoke-cancel-{id}"
                                                                ))
                                                                .ghost()
                                                                .small()
                                                                .h(px(28.))
                                                                .px_2p5()
                                                                .rounded_lg()
                                                                .label(tr("取消"))
                                                                .accessibility_label(format!(
                                                                    "{} · {} · {id}",
                                                                    tr("取消"),
                                                                    device.name
                                                                ))
                                                                .on_click(cx.listener(
                                                                    |this, _, _, cx| {
                                                                        this.remote.revoking = None;
                                                                        cx.notify();
                                                                    },
                                                                )),
                                                            )
                                                        }),
                                                ),
                                        )
                                        .when(confirming, |col| {
                                            col.child(
                                                div()
                                                    .px_3()
                                                    .py_2()
                                                    .rounded_lg()
                                                    .bg(Color::ErrorSurface.get(cx))
                                                    .border_1()
                                                    .border_color(Color::Error.get(cx).opacity(0.24))
                                                    .child(
                                                        selectable_text(
                                                            format!(
                                                                "remote-device-revoke-guidance-{id}"
                                                            ),
                                                            tr("撤销后将断开此设备，需重新配对才能访问。确认撤销？"),
                                                        )
                                                        .text_xs()
                                                        .text_color(Color::Error.get(cx)),
                                                    ),
                                            )
                                        }),
                                ))
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_baseline()
                            .justify_between()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("远程访问记录")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("最近 100 条记录。只展示现有 Hive 审计数据。")),
                            ),
                    )
                    .child(
                        super::super::settings_card(cx)
                            .when(remote.details_valid && remote.audit.is_empty(), |card| {
                                card.child(super::super::settings_row(false, cx).child(
                                    div()
                                        .text_sm()
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("尚无远程访问记录。")),
                                ))
                            })
                            .children(remote.audit.iter().enumerate().map(|(idx, entry)| {
                                let mut text = format!(
                                    "{} · {} · {} · {}{}{}",
                                    age(entry.ts),
                                    entry.device_id.as_deref().unwrap_or(tr("未关联设备")),
                                    action_name(&entry.action),
                                    outcome(&entry.result),
                                    entry
                                        .endpoint
                                        .as_ref()
                                        .map(|p| format!(" · {p}"))
                                        .unwrap_or_default(),
                                    entry
                                        .reject_reason
                                        .as_ref()
                                        .map(|p| format!(" · {p}"))
                                        .unwrap_or_default()
                                );
                                if let Some(workspace) = &entry.workspace_id {
                                    text.push_str(&format!("\n{} {workspace}", tr("工作区：")));
                                }
                                if let Some(count) = entry.byte_count {
                                    text.push_str(&if english() {
                                        format!(
                                            "\n{count} {}",
                                            if count == 1 { "byte" } else { "bytes" }
                                        )
                                    } else {
                                        format!("\n{count} 字节")
                                    });
                                }
                                if let Some(preview) = &entry.preview {
                                    text.push('\n');
                                    text.push_str(preview);
                                }
                                let dot = match entry.result.as_str() {
                                    "ok" => Color::Success.get(cx),
                                    "rejected" => super::super::amber_dot(cx),
                                    _ => Color::Error.get(cx),
                                };
                                super::super::settings_row(idx > 0, cx)
                                    .py_2p5()
                                    .child(
                                        h_flex()
                                            .id(format!("remote-audit-{}", entry.id))
                                            .items_start()
                                            .gap_2p5()
                                            .child(
                                                div()
                                                    .mt(px(5.))
                                                    .size(px(6.))
                                                    .flex_shrink_0()
                                                    .rounded_full()
                                                    .bg(dot),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_xs()
                                                    .when(entry.result != "ok", |row| {
                                                        row.text_color(Color::Error.get(cx))
                                                    })
                                                    .child(selectable_text(
                                                        format!("remote-audit-text-{}", entry.id),
                                                        text,
                                                    )),
                                            ),
                                    )
                            })),
                    ),
            )
    }
}
