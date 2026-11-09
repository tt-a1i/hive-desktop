use super::*;
use crate::language::{english, tr};
use crate::palette::Color;
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
        v_flex().gap_4().min_w_0()
            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child(tr("复用 Hive 的端到端加密远程接入。网关转发密文；退出桌面应用后，远程连接也会停止。")))
            .child(h_flex().items_center().flex_wrap().gap_3()
                .child(div().id("remote-connection-status").role(Role::Status).aria_label(status_label.clone()).flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(status_label))
                .child(Switch::new("remote-enabled").accessibility_label(tr("启用远程接入")).checked(enabled).disabled(!usable || !linked)
                    .on_change(cx.listener(|this, value, _, cx| this.remote_action("PUT", "/api/remote/enabled".into(), json!({"enabled":*value}), cx))))
                .child(Button::new("remote-refresh").ghost().compact().label(tr("刷新")).disabled(!self.connected || remote.loading).on_click(cx.listener(|this, _, _, cx| this.refresh_remote(cx)))))
            .when(linked && enabled, |view| view.child(Button::new("remote-reconnect").ghost().compact().label(tr("重新连接网关")).disabled(!usable).on_click(cx.listener(|this, _, _, cx| this.remote_action("PUT", "/api/remote/enabled".into(), json!({"enabled":true}), cx)))))
            .when_some(status.and_then(|s| s.gateway_url.clone()), |view, url| view.child(div().text_sm().text_color(Color::Secondary.get(cx)).child(format!("{}: {url}", tr("网关")))))
            .when_some(remote.error.as_ref().map(|error| format!("{}: {error}. {}", tr("远程接入读取失败"), tr("显示上次读取的内容，操作已暂停。"))), |view, error| view.child(div().id("remote-read-error").role(Role::Status).aria_label(error.clone()).p_3().rounded_lg().bg(Color::ErrorSurface.get(cx)).text_sm().child(error)))
            .when_some(remote.message.as_ref().map(|message| message.text()), |view, message| view.child(div().id("remote-action-notice").role(Role::Status).aria_label(message.clone()).text_sm().child(message)))
            .when(remote.loading, |view| view.child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("正在同步远程状态，暂显示上次读取的内容…"))))
            .child(v_flex().gap_2().p_3().rounded_lg().bg(Color::Inset.get(cx))
                .child(div().text_sm().child(if linked { tr("已连接 Hive 账号。退出登录将关闭远程接入；请在终端执行本应用专用命令。") } else { tr("请复制登录命令并在终端执行，再按提示打开浏览器授权。成功登录会启用远程接入。") }))
                .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(if cfg!(target_os = "windows") { tr("命令用于 PowerShell，并绑定当前桌面的独立数据目录。") } else { tr("命令绑定当前桌面的独立数据目录。") }))
                .child(h_flex().flex_wrap().gap_2().children([("login", tr("复制登录命令")), ("logout", tr("复制退出登录命令")), ("status", tr("复制状态命令"))].into_iter().map(|(key, label)| {
                    let command = remote.commands[key].as_str().map(str::to_owned);
                    Button::new(format!("remote-command-{key}")).compact().label(tr(label)).disabled(command.is_none() || !self.connected)
                        .on_click(move |_, _, cx| { if let Some(command) = &command { cx.write_to_clipboard(ClipboardItem::new_string(command.clone())); } })
                }))))
            .child(h_flex().items_center().gap_3()
                .child(div().flex_1().text_lg().font_weight(FontWeight::SEMIBOLD).child(tr("配对设备")))
                .child(Button::new("remote-new-pairing").label(tr("添加设备")).disabled(!usable || !enabled || !linked)
                    .on_click(cx.listener(|this, _, _, cx| this.remote_action("POST", "/api/remote/pairings".into(), json!({}), cx)))))
            .when_some(remote.ticket.clone(), |view, ticket| {
                let remaining = ticket.expires_at.saturating_sub(now()).div_ceil(1000);
                let code = if remaining == 0 { tr("配对码已过期").into() } else { ticket.code.clone() };
                view.child(v_flex().gap_2().p_3().rounded_lg().bg(Color::Inset.get(cx))
                    .child(div().text_sm().child(tr("在手机 Hive 配对页面输入此码，然后对照两端的安全短码。")))
                    .child(div().id("remote-pairing-code").role(Role::Label).aria_label(if english() { format!("Pairing code: {code}") } else { format!("配对码：{code}") }).text_2xl().font_weight(FontWeight::SEMIBOLD).child(code))
                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(if english() { format!("Expires in {remaining}s · Use this code only for this pairing") } else { format!("剩余 {remaining} 秒 · 每个配对码仅用于本次配对") }))
                    .child(h_flex().gap_2()
                        .child(Button::new("remote-copy-pairing").compact().label(tr("复制配对码")).disabled(!usable || remaining == 0).on_click(cx.listener(|this, _, _, cx| {
                            if this.remote.valid && let Some(ticket) = &this.remote.ticket && ticket.expires_at > now() { cx.write_to_clipboard(ClipboardItem::new_string(ticket.code.clone())); }
                        })))
                        .child(Button::new("remote-cancel-pairing").ghost().compact().label(tr("取消本次配对")).disabled(!usable).on_click(cx.listener(|this, _, _, cx| {
                            if let Some(ticket) = &this.remote.ticket { this.remote_action("POST", format!("/api/remote/pairings/{}/reject", path_id(&ticket.pairing_id)), json!({}), cx); }
                        })))))
            })
            .children(remote.pending.iter().map(|pending| {
                let confirm_id = pending.pairing_id.clone();
                let reject_id = confirm_id.clone();
                let device_name = pending.device_name.as_deref().unwrap_or(tr("新设备"));
                let title = format!("{}: {device_name}", tr("确认配对"));
                let guidance = tr("请确认手机显示的安全短码与下方完全一致。不是你发起的请求或短码不符，请拒绝。");
                v_flex().gap_2().p_3().rounded_lg().bg(Color::Attention.get(cx))
                    .child(div().id(format!("remote-pairing-name-{confirm_id}")).role(Role::Label).aria_label(title.clone()).text_sm().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(div().id(format!("remote-pairing-guidance-{confirm_id}")).role(Role::Label).aria_label(guidance).text_sm().child(guidance))
                    .child(div().id(format!("remote-pairing-sas-{confirm_id}")).role(Role::Label).aria_label(if english() { format!("Security code: {}", pending.sas) } else { format!("安全短码：{}", pending.sas) }).text_2xl().font_weight(FontWeight::SEMIBOLD).child(pending.sas.clone()))
                    .child(div().text_xs().child(if english() { format!("Expires in {}s", pending.expires_at.saturating_sub(now()).div_ceil(1000)) } else { format!("剩余 {} 秒", pending.expires_at.saturating_sub(now()).div_ceil(1000)) }))
                    .child(h_flex().gap_2()
                        .child(Button::new(format!("remote-confirm-{confirm_id}")).primary().label(tr("短码一致，确认")).accessibility_label(format!("{} · {device_name} · {}", tr("短码一致，确认"), pending.sas)).disabled(!usable || pending.expires_at <= now()).on_click(cx.listener(move |this, _, _, cx| this.remote_pairing_decision(&confirm_id, true, cx))))
                        .child(Button::new(format!("remote-reject-{reject_id}")).label(tr("拒绝")).accessibility_label(format!("{} · {device_name}", tr("拒绝"))).disabled(!usable || pending.expires_at <= now()).on_click(cx.listener(move |this, _, _, cx| this.remote_pairing_decision(&reject_id, false, cx)))))
            }))
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(tr("已配对设备")))
            .when(remote.details_valid && remote.devices.is_empty(), |view| view.child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr("尚无已配对设备。"))))
            .children(remote.devices.iter().map(|device| {
                let id = device.id.clone();
                let revoke_id = id.clone();
                let confirming = remote.revoking.as_ref() == Some(&id);
                v_flex().gap_2().py_3().border_b_1().border_color(Color::Divider.get(cx))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(device.name.clone()))
                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(if english() { format!("{} · Created {} · Last active {}", device.id, age(device.created_at), device.last_active.map(age).unwrap_or(tr("尚无记录").into())) } else { format!("{} · 创建于{} · 最近活动{}", device.id, age(device.created_at), device.last_active.map(age).unwrap_or(tr("尚无记录").into())) }))
                    .when(confirming, |view| view.child(div().text_sm().child(tr("撤销后将断开此设备，需重新配对才能访问。确认撤销？"))))
                    .child(h_flex().gap_2()
                        .child(Button::new(format!("remote-revoke-{id}")).compact().label(if confirming { tr("确认撤销") } else { tr("撤销访问") }).disabled(!usable || !remote.details_valid)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if confirming { this.remote_revoke(&revoke_id, cx); }
                                else { this.remote.revoking = Some(revoke_id.clone()); cx.notify(); }
                            })))
                        .when(confirming, |view| view.child(Button::new(format!("remote-revoke-cancel-{id}")).ghost().compact().label(tr("取消")).on_click(cx.listener(|this, _, _, cx| { this.remote.revoking = None; cx.notify(); })))))
            }))
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(tr("远程访问记录")))
            .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(tr("最近 100 条记录。只展示现有 Hive 审计数据。")))
            .when(remote.details_valid && remote.audit.is_empty(), |view| view.child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr("尚无远程访问记录。"))))
            .children(remote.audit.iter().map(|entry| div().id(format!("remote-audit-{}", entry.id)).py_2().border_b_1().border_color(Color::Divider.get(cx)).text_xs()
                .when(entry.result != "ok", |row| row.text_color(Color::Error.get(cx)))
                .child(format!("{} · {} · {} · {}{}{}", age(entry.ts), entry.device_id.as_deref().unwrap_or(tr("未知设备")), entry.action, outcome(&entry.result),
                    entry.endpoint.as_ref().map(|p| format!(" · {p}")).unwrap_or_default(),
                    entry.reject_reason.as_ref().map(|p| format!(" · {p}")).unwrap_or_default()))
                .when_some(entry.workspace_id.clone(), |row, workspace| row.child(div().child(format!("{} {workspace}", tr("工作区：")))))
                .when_some(entry.byte_count, |row, count| row.child(div().child(if english() { format!("{count} {}", if count == 1 { "byte" } else { "bytes" }) } else { format!("{count} 字节") })))
                .when_some(entry.preview.clone(), |row, preview| row.child(div().child(preview)))))
    }
}
