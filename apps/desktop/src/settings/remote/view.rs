use super::*;
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
        "刚刚".into()
    } else if seconds < 3600 {
        format!("{} 分钟前", seconds / 60)
    } else if seconds < 86400 {
        format!("{} 小时前", seconds / 3600)
    } else {
        format!("{} 天前", seconds / 86400)
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
                "disabled" => "已关闭",
                "loggedOut" => "未登录",
                "connecting" => "正在连接",
                "online" => "已连接",
                "reconnecting" => "正在重连",
                "revoked" => "登录已撤销",
                _ => "状态未知",
            })
            .unwrap_or("正在读取");
        v_flex().gap_4().min_w_0()
            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child("复用 Hive 的端到端加密远程接入。网关转发密文；退出桌面应用后，远程连接也会停止。"))
            .child(h_flex().items_center().flex_wrap().gap_3()
                .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("远程接入 · {connection}")))
                .child(Switch::new("remote-enabled").accessibility_label("启用远程接入").checked(enabled).disabled(!usable || !linked)
                    .on_change(cx.listener(|this, value, _, cx| this.remote_action("PUT", "/api/remote/enabled".into(), json!({"enabled":*value}), cx))))
                .child(Button::new("remote-refresh").ghost().compact().label("刷新").disabled(!self.connected || remote.loading).on_click(cx.listener(|this, _, _, cx| this.refresh_remote(cx)))))
            .when(linked && enabled, |view| view.child(Button::new("remote-reconnect").ghost().compact().label("重新连接网关").disabled(!usable).on_click(cx.listener(|this, _, _, cx| this.remote_action("PUT", "/api/remote/enabled".into(), json!({"enabled":true}), cx)))))
            .when_some(status.and_then(|s| s.gateway_url.clone()), |view, url| view.child(div().text_sm().text_color(Color::Secondary.get(cx)).child(format!("网关：{url}"))))
            .when_some(remote.error.clone(), |view, error| view.child(div().p_3().rounded_lg().bg(Color::ErrorSurface.get(cx)).text_sm().child(error)))
            .when_some(remote.message.clone(), |view, message| view.child(div().text_sm().child(message)))
            .when(remote.loading, |view| view.child(div().text_xs().text_color(Color::Muted.get(cx)).child("正在同步远程状态，暂显示上次读取的内容…")))
            .child(v_flex().gap_2().p_3().rounded_lg().bg(Color::Inset.get(cx))
                .child(div().text_sm().child(if linked { "已连接 Hive 账号。退出登录将关闭远程接入；请在终端执行本应用专用命令。" } else { "请复制登录命令并在终端执行，再按提示打开浏览器授权。成功登录会启用远程接入。" }))
                .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(if cfg!(target_os = "windows") { "命令用于 PowerShell，并绑定当前桌面的独立数据目录。" } else { "命令绑定当前桌面的独立数据目录。" }))
                .child(h_flex().flex_wrap().gap_2().children([("login", "复制登录命令"), ("logout", "复制退出登录命令"), ("status", "复制状态命令")].into_iter().map(|(key, label)| {
                    let command = remote.commands[key].as_str().map(str::to_owned);
                    Button::new(format!("remote-command-{key}")).compact().label(label).disabled(command.is_none() || !self.connected)
                        .on_click(move |_, _, cx| { if let Some(command) = &command { cx.write_to_clipboard(ClipboardItem::new_string(command.clone())); } })
                }))))
            .child(h_flex().items_center().gap_3()
                .child(div().flex_1().text_lg().font_weight(FontWeight::SEMIBOLD).child("配对设备"))
                .child(Button::new("remote-new-pairing").label("添加设备").disabled(!usable || !enabled || !linked)
                    .on_click(cx.listener(|this, _, _, cx| this.remote_action("POST", "/api/remote/pairings".into(), json!({}), cx)))))
            .when_some(remote.ticket.clone(), |view, ticket| {
                let remaining = ticket.expires_at.saturating_sub(now()) / 1000;
                view.child(v_flex().gap_2().p_3().rounded_lg().bg(Color::Inset.get(cx))
                    .child(div().text_sm().child("在手机 Hive 配对页面输入此码，然后对照两端的安全短码。"))
                    .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(if remaining == 0 { "配对码已过期".into() } else { ticket.code.clone() }))
                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("剩余 {remaining} 秒 · 每个配对码仅用于本次配对")))
                    .child(h_flex().gap_2()
                        .child(Button::new("remote-copy-pairing").compact().label("复制配对码").disabled(!usable || remaining == 0).on_click(cx.listener(|this, _, _, cx| {
                            if this.remote.valid && let Some(ticket) = &this.remote.ticket && ticket.expires_at > now() { cx.write_to_clipboard(ClipboardItem::new_string(ticket.code.clone())); }
                        })))
                        .child(Button::new("remote-cancel-pairing").ghost().compact().label("取消本次配对").disabled(!usable).on_click(cx.listener(|this, _, _, cx| {
                            if let Some(ticket) = &this.remote.ticket { this.remote_action("POST", format!("/api/remote/pairings/{}/reject", path_id(&ticket.pairing_id)), json!({}), cx); }
                        })))))
            })
            .children(remote.pending.iter().map(|pending| {
                let confirm_id = pending.pairing_id.clone();
                let reject_id = confirm_id.clone();
                v_flex().gap_2().p_3().rounded_lg().bg(Color::Attention.get(cx))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("确认配对：{}", pending.device_name.as_deref().unwrap_or("新设备"))))
                    .child(div().text_sm().child("请确认手机显示的安全短码与下方完全一致。不是你发起的请求或短码不符，请拒绝。"))
                    .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(pending.sas.clone()))
                    .child(div().text_xs().child(format!("剩余 {} 秒", pending.expires_at.saturating_sub(now()) / 1000)))
                    .child(h_flex().gap_2()
                        .child(Button::new(format!("remote-confirm-{confirm_id}")).primary().label("短码一致，确认").disabled(!usable || pending.expires_at <= now()).on_click(cx.listener(move |this, _, _, cx| this.remote_pairing_decision(&confirm_id, true, cx))))
                        .child(Button::new(format!("remote-reject-{reject_id}")).label("拒绝").disabled(!usable || pending.expires_at <= now()).on_click(cx.listener(move |this, _, _, cx| this.remote_pairing_decision(&reject_id, false, cx)))))
            }))
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("已配对设备"))
            .when(remote.details_valid && remote.devices.is_empty(), |view| view.child(div().text_sm().text_color(Color::Muted.get(cx)).child("尚无已配对设备。")))
            .children(remote.devices.iter().map(|device| {
                let id = device.id.clone();
                let revoke_id = id.clone();
                let confirming = remote.revoking.as_ref() == Some(&id);
                v_flex().gap_2().py_3().border_b_1().border_color(Color::Divider.get(cx))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(device.name.clone()))
                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} · 创建于{} · 最近活动{}", device.id, age(device.created_at), device.last_active.map(age).unwrap_or("尚无记录".into()))))
                    .when(confirming, |view| view.child(div().text_sm().child("撤销后将断开此设备，需重新配对才能访问。确认撤销？")))
                    .child(h_flex().gap_2()
                        .child(Button::new(format!("remote-revoke-{id}")).compact().label(if confirming { "确认撤销" } else { "撤销访问" }).disabled(!usable || !remote.details_valid)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if confirming { this.remote_revoke(&revoke_id, cx); }
                                else { this.remote.revoking = Some(revoke_id.clone()); cx.notify(); }
                            })))
                        .when(confirming, |view| view.child(Button::new(format!("remote-revoke-cancel-{id}")).ghost().compact().label("取消").on_click(cx.listener(|this, _, _, cx| { this.remote.revoking = None; cx.notify(); })))))
            }))
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("远程访问记录"))
            .child(div().text_xs().text_color(Color::Secondary.get(cx)).child("最近 100 条记录。只展示现有 Hive 审计数据。"))
            .when(remote.details_valid && remote.audit.is_empty(), |view| view.child(div().text_sm().text_color(Color::Muted.get(cx)).child("尚无远程访问记录。")))
            .children(remote.audit.iter().map(|entry| div().id(format!("remote-audit-{}", entry.id)).py_2().border_b_1().border_color(Color::Divider.get(cx)).text_xs()
                .when(entry.result != "ok", |row| row.text_color(Color::Error.get(cx)))
                .child(format!("{} · {} · {} · {}{}{}", age(entry.ts), entry.device_id.as_deref().unwrap_or("未知设备"), entry.action, entry.result,
                    entry.endpoint.as_ref().map(|p| format!(" · {p}")).unwrap_or_default(),
                    entry.reject_reason.as_ref().map(|p| format!(" · {p}")).unwrap_or_default()))
                .when_some(entry.workspace_id.clone(), |row, workspace| row.child(div().child(format!("工作区：{workspace}"))))
                .when_some(entry.byte_count, |row, count| row.child(div().child(format!("{count} 字节"))))
                .when_some(entry.preview.clone(), |row, preview| row.child(div().child(preview)))))
    }
}
