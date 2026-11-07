use super::*;
use gpui_kit::component::WindowExt;

impl HiveApp {
    pub fn confirm_pi_disconnect(
        &mut self,
        provider_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.pi_accounts.loading || self.pi_accounts.running() {
            return;
        }
        let Some(provider) = self
            .pi_accounts
            .providers
            .as_ref()
            .and_then(|providers| providers.iter().find(|provider| provider.id == provider_id))
        else {
            return;
        };
        if provider.stored_type.is_none() {
            return;
        }
        let name = provider.name.clone();
        let connection = self.connection_generation;
        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let id = provider_id.clone();
            dialog.title(format!("移除 {name} 的登录信息？"))
                .description("Hive 和共用配置的 Pi 将停止使用这份已保存的登录信息。会话记录保留；环境变量等其他登录来源不受影响。")
                .show_cancel(true).ok_text("移除登录信息").cancel_text("取消")
                .on_ok(move |_, window, cx| {
                    weak.update(cx, |this, cx| {
                        if this.connection_generation == connection {
                            this.pi_auth_request("pi.auth.logout", json!({"providerId":id}), window.window_handle(), cx);
                        }
                    }).is_ok()
                })
        });
    }
}
