use super::*;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
struct Asset {
    sha256: String,
}

#[derive(Clone, Deserialize)]
struct Candidate {
    version: String,
    asset: Asset,
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
    checked: Option<Check>,
    downloaded: bool,
    error: Option<String>,
}

impl HiveApp {
    fn check_desktop_update(&mut self, cx: &mut Context<Self>) {
        if self.desktop_updates.checking || self.desktop_updates.downloading {
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
                    Ok(checked) => state.checked = Some(checked),
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
        if state.checking || state.downloading || state.downloaded {
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

    pub(super) fn render_desktop_updates(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.desktop_updates;
        let busy = state.checking || state.downloading;
        let disabled = busy || !self.connected || self.closing;
        let status = if state.checking {
            tr("正在检查更新…").to_owned()
        } else if state.downloading {
            tr("正在下载并校验更新…").to_owned()
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
    }
}
