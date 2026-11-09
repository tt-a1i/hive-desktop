use crate::app::{HiveApp, SettingsSection};
use gpui_kit::*;
use serde::Deserialize;
use serde_json::{Value, json};
mod view;

#[derive(Clone, Deserialize)]
pub struct Status {
    pub enabled: bool,
    pub logged_in: bool,
    pub gateway_url: Option<String>,
    pub connection: String,
}
#[derive(Clone, Deserialize)]
pub struct Pending {
    pub pairing_id: String,
    pub device_name: Option<String>,
    pub sas: String,
    pub expires_at: u64,
}
#[derive(Clone, Deserialize)]
pub struct Ticket {
    pub pairing_id: String,
    pub code: String,
    pub expires_at: u64,
}
#[derive(Clone, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub last_active: Option<u64>,
    pub created_at: u64,
}
#[derive(Clone, Deserialize)]
pub struct Audit {
    pub id: u64,
    pub ts: u64,
    pub device_id: Option<String>,
    pub action: String,
    pub endpoint: Option<String>,
    pub result: String,
    pub reject_reason: Option<String>,
    pub byte_count: Option<u64>,
    pub preview: Option<String>,
    pub workspace_id: Option<String>,
}
#[derive(Default)]
pub struct Remote {
    pub commands: Value,
    pub status: Option<Status>,
    pub pending: Vec<Pending>,
    pub devices: Vec<Device>,
    pub audit: Vec<Audit>,
    pub ticket: Option<Ticket>,
    pub revoking: Option<String>,
    pub loading: bool,
    pub valid: bool,
    pub details_valid: bool,
    pub error: Option<String>,
    message: Option<Notice>,
    pub scroll: ScrollHandle,
}
enum Notice {
    TicketError(String),
    Submitted,
    ActionError(String),
}
impl Notice {
    fn text(&self) -> String {
        use crate::language::tr;
        match self {
            Self::TicketError(error) => format!("{}: {error}", tr("配对信息无法读取")),
            Self::Submitted => tr("操作已提交，正在重新读取实际状态。").to_owned(),
            Self::ActionError(error) => format!("{}: {error}", tr("操作失败")),
        }
    }
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn path_id(id: &str) -> String {
    url::form_urlencoded::byte_serialize(id.as_bytes()).collect()
}
impl HiveApp {
    pub fn refresh_remote(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.closing || self.restarting || self.remote.loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let details = self.settings_panel && self.settings_section == SettingsSection::Remote;
        let connection = self.connection_generation;
        self.remote.loading = true;
        cx.spawn(async move |this, cx| {
            let result = async {
                let status: Status = serde_json::from_value(
                    runtime
                        .request(
                            "hive.request",
                            json!({"method":"GET","path":"/api/remote/status"}),
                        )
                        .await?,
                )
                .map_err(|e| e.to_string())?;
                let pending: Vec<Pending> = if status.enabled && status.logged_in {
                    serde_json::from_value(
                        runtime
                            .request(
                                "hive.request",
                                json!({"method":"GET","path":"/api/remote/pairings/pending"}),
                            )
                            .await?,
                    )
                    .map_err(|e| e.to_string())?
                } else {
                    Vec::new()
                };
                let devices: Vec<Device> = if details {
                    serde_json::from_value(
                        runtime
                            .request(
                                "hive.request",
                                json!({"method":"GET","path":"/api/remote/devices"}),
                            )
                            .await?,
                    )
                    .map_err(|e| e.to_string())?
                } else {
                    Vec::new()
                };
                let audit: Vec<Audit> = if details {
                    serde_json::from_value(
                        runtime
                            .request(
                                "hive.request",
                                json!({"method":"GET","path":"/api/remote/audit?limit=100"}),
                            )
                            .await?,
                    )
                    .map_err(|e| e.to_string())?
                } else {
                    Vec::new()
                };
                Ok::<_, String>((status, pending, devices, audit))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                let remote = &mut this.remote;
                remote.loading = false;
                match result {
                    Ok((status, pending, devices, audit)) => {
                        if !status.enabled || !status.logged_in {
                            remote.ticket = None;
                        }
                        remote.status = Some(status);
                        remote.pending = pending;
                        remote.valid = true;
                        if details {
                            remote.devices = devices;
                            remote.audit = audit;
                            remote.details_valid = true;
                            if remote
                                .revoking
                                .as_ref()
                                .is_some_and(|id| !remote.devices.iter().any(|d| &d.id == id))
                            {
                                remote.revoking = None;
                            }
                        }
                        remote.error = None;
                        if matches!(remote.message, Some(Notice::Submitted)) {
                            remote.message = None;
                        }
                    }
                    Err(error) => {
                        remote.valid = false;
                        remote.details_valid = false;
                        remote.error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn remote_action(
        &mut self,
        method: &'static str,
        path: String,
        body: Value,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self.closing
            || self.restarting
            || self.remote.loading
            || !self.remote.valid
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        self.remote.loading = true;
        self.remote.message = None;
        let pairing = path == "/api/remote/pairings";
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "hive.request",
                    json!({"method":method,"path":path,"body":body}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.remote.loading = false;
                match result {
                    Ok(value) => {
                        if pairing {
                            match serde_json::from_value(value) {
                                Ok(ticket) => this.remote.ticket = Some(ticket),
                                Err(error) => {
                                    this.remote.message = Some(Notice::TicketError(error.to_string()))
                                }
                            }
                        } else {
                            this.remote.ticket = None;
                            this.remote.revoking = None;
                            this.remote.message = Some(Notice::Submitted);
                        }
                    }
                    Err(error) => {
                        this.remote.message = Some(Notice::ActionError(error));
                    }
                }
                this.remote.valid = false;
                this.remote.details_valid = false;
                this.refresh_remote(cx);
                cx.notify();
            });
        })
        .detach();
    }
    pub fn remote_pairing_decision(&mut self, id: &str, confirm: bool, cx: &mut Context<Self>) {
        if !self
            .remote
            .pending
            .iter()
            .any(|p| p.pairing_id == id && p.expires_at > now())
        {
            return;
        }
        let action = if confirm { "confirm" } else { "reject" };
        self.remote_action(
            "POST",
            format!("/api/remote/pairings/{}/{action}", path_id(id)),
            json!({}),
            cx,
        );
    }
    pub fn remote_revoke(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.remote.details_valid || self.remote.revoking.as_deref() != Some(id) {
            return;
        }
        self.remote_action(
            "POST",
            format!("/api/remote/devices/{}/revoke", path_id(id)),
            json!({}),
            cx,
        );
    }
}
