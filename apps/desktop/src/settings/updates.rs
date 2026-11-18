use super::*;
use crate::desktop_update::{InstallReceipt, Installer};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
struct Asset {
    sha256: String,
}

#[derive(Clone, Deserialize)]
struct Candidate {
    version: String,
    asset: Asset,
    #[serde(default)]
    staged: bool,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
enum Check {
    Disabled,
    Current,
    Unsupported { version: String },
    Available(Candidate),
}

#[derive(Default)]
pub(crate) struct State {
    checking: bool,
    downloading: bool,
    preparing: bool,
    checked: Option<Check>,
    downloaded: bool,
    error: Option<String>,
    receipt: Option<InstallReceipt>,
    pub(crate) installer: Option<Installer>,
}

impl HiveApp {
    fn check_desktop_update(&mut self, cx: &mut Context<Self>) {
        if self.desktop_updates.checking
            || self.desktop_updates.downloading
            || self.desktop_updates.preparing
        {
            return;
        }
        let Some(runtime) = self
            .runtime
            .clone()
            .filter(|_| self.connected && !self.closing)
        else {
            return;
        };
        let generation = self.connection_generation;
        self.desktop_updates = State {
            checking: true,
            receipt: self.desktop_updates.receipt.take(),
            ..State::default()
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("desktop.updates.check", json!({}))
                .await
                .and_then(|value| {
                    if value
                        .get("installedVersion")
                        .is_some_and(|version| version != env!("CARGO_PKG_VERSION"))
                    {
                        return Err(tr("更新渠道与当前桌面版本不匹配。").to_owned());
                    }
                    serde_json::from_value::<Check>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if generation != this.connection_generation {
                    return;
                }
                let state = &mut this.desktop_updates;
                state.checking = false;
                match result {
                    Ok(checked) => {
                        state.downloaded =
                            matches!(&checked, Check::Available(candidate) if candidate.staged);
                        state.checked = Some(checked);
                    }
                    Err(error) => state.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn download_desktop_update(&mut self, cx: &mut Context<Self>) {
        let Some(runtime) = self
            .runtime
            .clone()
            .filter(|_| self.connected && !self.closing)
        else {
            return;
        };
        let state = &mut self.desktop_updates;
        if state.checking || state.downloading || state.preparing || state.downloaded {
            return;
        }
        let Some(Check::Available(candidate)) = state.checked.clone() else {
            return;
        };
        let generation = self.connection_generation;
        state.downloading = true;
        state.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "desktop.updates.stage",
                    json!({"version":candidate.version,"sha256":candidate.asset.sha256}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.connection_generation {
                    return;
                }
                let state = &mut this.desktop_updates;
                state.downloading = false;
                match result {
                    Ok(value)
                        if value["version"] == candidate.version
                            && value["sha256"] == candidate.asset.sha256 =>
                    {
                        state.downloaded = true
                    }
                    Ok(_) => state.error = Some(tr("更新下载收据不匹配。").to_owned()),
                    Err(error) => state.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn load_update_receipt(&mut self, cx: &mut Context<Self>) {
        let Some(directory) = self.data_dir.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let receipt =
                smol::unblock(move || crate::desktop_update::read_receipt(&directory)).await;
            let _ = this.update(cx, |this, cx| {
                this.desktop_updates.receipt = receipt;
                cx.notify();
            });
        })
        .detach();
    }

    fn confirm_desktop_install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        gpui_kit::component::WindowExt::open_alert_dialog(window, cx, move |dialog, _, _| {
            let weak = weak.clone();
            dialog
                .title(tr("安装更新并重启？"))
                .description(tr(
                    "Hive 将保存草稿、关闭后台和终端，再启动更新后的桌面应用。",
                ))
                .show_cancel(true)
                .cancel_text(tr("取消"))
                .ok_text(tr("安装并重启"))
                .on_ok(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.prepare_desktop_install(cx));
                    true
                })
        });
    }

    fn prepare_desktop_install(&mut self, cx: &mut Context<Self>) {
        if self.has_busy_work() {
            self.desktop_updates.error =
                Some(tr("请等待正在运行的会话结束后再安装更新。").to_owned());
            cx.notify();
            return;
        }
        let Some(runtime) = self
            .runtime
            .clone()
            .filter(|_| self.connected && !self.closing)
        else {
            return;
        };
        let state = &mut self.desktop_updates;
        if !state.downloaded || state.preparing || state.checking || state.downloading {
            return;
        }
        let Some(Check::Available(candidate)) = state.checked.clone() else {
            return;
        };
        state.preparing = true;
        state.error = None;
        let generation = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "desktop.updates.prepareInstaller",
                    json!({"version":candidate.version,"sha256":candidate.asset.sha256}),
                )
                .await
                .and_then(|value| {
                    serde_json::from_value::<Installer>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if generation != this.connection_generation || this.closing {
                    return;
                }
                this.desktop_updates.preparing = false;
                match result {
                    Ok(_) if this.has_busy_work() => {
                        this.desktop_updates.error =
                            Some(tr("请等待正在运行的会话结束后再安装更新。").to_owned());
                    }
                    Ok(installer)
                        if installer.version == candidate.version
                            && installer.sha256 == candidate.asset.sha256 =>
                    {
                        this.desktop_updates.installer = Some(installer);
                        this.exit(cx);
                        if !this.closing {
                            this.desktop_updates.installer.take();
                        }
                    }
                    Ok(_) => {
                        this.desktop_updates.error = Some(tr("更新下载收据不匹配。").to_owned())
                    }
                    Err(error) => this.desktop_updates.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_desktop_updates(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.desktop_updates;
        let busy = state.checking || state.downloading || state.preparing;
        let disabled = busy || !self.connected || self.closing;
        let status = if state.checking {
            tr("正在检查更新…").to_owned()
        } else if state.downloading {
            tr("正在下载并校验更新…").to_owned()
        } else if state.preparing {
            tr("正在准备安装…").to_owned()
        } else {
            match &state.checked {
                Some(Check::Disabled) => tr("此开发包未配置更新渠道。").to_owned(),
                Some(Check::Current) => tr("当前版本已是最新版本。").to_owned(),
                Some(Check::Unsupported { version }) => {
                    format!("{} · {version}", tr("新版本暂不支持此平台"))
                }
                Some(Check::Available(candidate)) => format!(
                    "{} · {}",
                    tr(if state.downloaded {
                        "已下载并校验"
                    } else {
                        "有可用更新"
                    }),
                    candidate.version
                ),
                None => String::new(),
            }
        };
        let can_download = matches!(state.checked, Some(Check::Available(_))) && !state.downloaded;
        super::settings_card(cx)
            .child(
                super::settings_row(false, cx).child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .flex_wrap()
                        .child(div().flex_1().text_sm().child(tr("桌面更新")))
                        .child(
                            Button::new("desktop-update-check")
                                .outline()
                                .small()
                                .label(tr("检查更新"))
                                .disabled(disabled)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.check_desktop_update(cx)),
                                ),
                        )
                        .when(can_download, |row| {
                            row.child(
                                Button::new("desktop-update-download")
                                    .outline()
                                    .small()
                                    .label(tr("下载更新"))
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.download_desktop_update(cx)
                                    })),
                            )
                        })
                        .when(state.downloaded, |row| {
                            row.child(
                                Button::new("desktop-update-install")
                                    .outline()
                                    .small()
                                    .label(tr("安装并重启"))
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_desktop_install(window, cx)
                                    })),
                            )
                        }),
                ),
            )
            .when(!status.is_empty(), |card| {
                card.child(
                    super::settings_row(true, cx).child(super::settings_notice_box(
                        "desktop-update-status",
                        status,
                        false,
                        cx,
                    )),
                )
            })
            .when_some(state.error.clone(), |card, error| {
                card.child(super::settings_notice_box(
                    "desktop-update-error",
                    error,
                    true,
                    cx,
                ))
            })
            .when_some(state.receipt.as_ref(), |card, receipt| {
                let (label, version) = match receipt.state.as_str() {
                    "installed" => (tr("已安装桌面更新"), receipt.version.as_str()),
                    "rolledBack" => (tr("安装失败，已恢复旧版本"), receipt.version.as_str()),
                    _ => (tr("桌面更新失败"), receipt.requested_version.as_str()),
                };
                let mut status = format!("{label} · {version}");
                if let Some(error) = &receipt.error {
                    status.push_str(&format!("\n{error}"));
                }
                card.child(super::settings_notice_box(
                    "desktop-update-install-result",
                    status,
                    receipt.state != "installed",
                    cx,
                ))
            })
    }
}
