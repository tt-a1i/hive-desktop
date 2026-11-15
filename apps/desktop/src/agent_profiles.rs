use crate::{
    app::HiveApp,
    language::{english, tr},
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputState},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub use crate::protocol::ProfileRef;
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub driver: String,
    pub label: String,
    pub enabled: bool,
    pub current_revision: u64,
    pub revisions: Vec<Revision>,
}
#[derive(Clone, Deserialize)]
pub struct Revision {
    pub revision: u64,
    pub launch: Launch,
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub store_version: u64,
    pub profiles: Vec<Profile>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mutation {
    store_version: u64,
    profile: MutationProfile,
}
#[derive(Deserialize)]
struct MutationProfile {
    #[serde(rename = "ref")]
    reference: ProfileRef,
}
fn localized_profile_label<'a>(id: &str, label: &'a str) -> &'a str {
    match (id, label) {
        ("default-pi", "Built-in Pi") => tr("内置 Pi"),
        ("default-pi-rpc", "External Pi") => tr("外部 Pi"),
        _ => label,
    }
}
fn supported_driver(driver: &str) -> Option<&'static str> {
    match driver {
        "pi" => Some("pi"),
        "codex" => Some("codex"),
        "pi-rpc" => Some("pi-rpc"),
        "claude" => Some("claude"),
        "acp" => Some("acp"),
        _ => None,
    }
}
pub struct Profiles {
    pub snapshot: Option<Snapshot>,
    pub loading: bool,
    pub generation: u64,
    pub error: Option<String>,
    pub saved: bool,
    pub selected: Option<String>,
    pub driver: &'static str,
    pub label: Entity<InputState>,
    pub command: Entity<InputState>,
    pub args: Entity<InputState>,
    pub config: Entity<InputState>,
    pub scroll: ScrollHandle,
}
impl Profiles {
    pub fn new(window: &mut Window, cx: &mut Context<HiveApp>) -> Self {
        Self {
            snapshot: None,
            loading: false,
            generation: 0,
            error: None,
            saved: false,
            selected: None,
            driver: "pi",
            label: cx.new(|cx| InputState::new(window, cx)),
            command: cx.new(|cx| InputState::new(window, cx)),
            args: cx.new(|cx| InputState::new(window, cx).placeholder("[]")),
            config: cx.new(|cx| InputState::new(window, cx)),
            scroll: ScrollHandle::new(),
        }
    }
}
impl HiveApp {
    pub fn profile_label(&self, reference: &ProfileRef) -> String {
        let name = self
            .profiles
            .snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot.profiles.iter().find(|p| {
                    p.id == reference.id
                        && p.revisions.iter().any(|r| r.revision == reference.revision)
                })
            })
            .map(|p| localized_profile_label(&reference.id, p.label.as_str()))
            .unwrap_or(reference.id.as_str());
        format!("{} · r{}", name, reference.revision)
    }
    pub fn load_profiles(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.profiles.loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.profiles.loading = true;
        self.profiles.generation += 1;
        let generation = self.profiles.generation;
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("agent.profiles.list", json!({}))
                .await
                .and_then(|v| serde_json::from_value::<Snapshot>(v).map_err(|e| e.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.profiles.generation != generation
                {
                    return;
                }
                this.profiles.loading = false;
                match result {
                    Ok(snapshot) => {
                        this.profiles.snapshot = Some(snapshot);
                        this.profiles.error = None
                    }
                    Err(e) => { this.profiles.snapshot = None; this.profiles.error = Some(e.to_string()); },
                };
                cx.notify();
            });
        })
        .detach();
    }
    fn edit_profile(
        &mut self,
        id: Option<String>,
        driver: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let profile = id
            .as_ref()
            .and_then(|id| {
                self.profiles
                    .snapshot
                    .as_ref()?
                    .profiles
                    .iter()
                    .find(|p| &p.id == id)
            })
            .cloned();
        let launch = profile
            .as_ref()
            .and_then(|p| {
                p.revisions
                    .iter()
                    .find(|r| r.revision == p.current_revision)
            })
            .map(|r| r.launch.clone())
            .unwrap_or_default();
        self.profiles.selected = id;
        self.profiles.driver = driver;
        self.profiles.error = None;
        self.profiles.saved = false;
        for (input, value) in [
            (
                &self.profiles.label,
                profile.map(|p| p.label).unwrap_or_default(),
            ),
            (&self.profiles.command, launch.command.unwrap_or_default()),
            (
                &self.profiles.args,
                serde_json::to_string(&launch.args).unwrap_or_else(|_| "[]".into()),
            ),
            (&self.profiles.config, launch.config_dir.unwrap_or_default()),
        ] {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        cx.notify();
    }
    fn mutate_profile(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.profiles.loading || !self.connected {
            return;
        }
        let (Some(runtime), Some(snapshot)) =
            (self.runtime.clone(), self.profiles.snapshot.as_ref())
        else {
            return;
        };
        let mut body = json!({"expectedStoreVersion":snapshot.store_version});
        if let Some(id) = &self.profiles.selected {
            body["id"] = json!(id);
        }
        if action == "create" || action == "update" {
            let args = match serde_json::from_str::<Vec<String>>(
                self.profiles.args.read(cx).value().as_str(),
            ) {
                Ok(args) => args,
                Err(_) => {
                    self.profiles.error =
                        Some(tr("参数必须是 JSON 字符串数组，例如 [\"--flag\", \"value\"]").into());
                    cx.notify();
                    return;
                }
            };
            let driver = self.profiles.driver;
            let command = self.profiles.command.read(cx).value().to_string();
            let config = self.profiles.config.read(cx).value().to_string();
            let mut launch = json!({"args":args});
            if !command.is_empty() {
                launch["command"] = json!(command);
            }
            if driver != "acp" && !config.is_empty() {
                launch["configDir"] = json!(config);
            }
            body["label"] = json!(self.profiles.label.read(cx).value().to_string());
            body["launch"] = launch;
            if action == "create" {
                body["driver"] = json!(driver);
            }
        }
        self.profiles.loading = true;
        self.profiles.saved = false;
        self.profiles.generation += 1;
        let generation = self.profiles.generation;
        let connection = self.connection_generation;
        cx.spawn(async move |this,cx| {
            let result=runtime.request(&format!("agent.profiles.{action}"),body).await;
            let _=this.update(cx,|this,cx| {
                if this.connection_generation!=connection || this.profiles.generation!=generation { return; }
                this.profiles.loading=false;
                match result {
                    Ok(value)=>match serde_json::from_value::<Mutation>(value) {
                        Ok(mutation) if !mutation.profile.reference.id.is_empty() && mutation.profile.reference.revision > 0 => {
                            if action=="create" { this.profiles.selected=Some(mutation.profile.reference.id); }
                            if let Some(snapshot)=this.profiles.snapshot.as_mut() { snapshot.store_version=mutation.store_version; }
                            this.profiles.error=None; this.profiles.saved=true; this.load_profiles(cx);
                        }
                        _ => {
                            // A successful create may already be stored. Prevent retry until a fresh list is read.
                            this.profiles.snapshot=None;
                            this.profiles.error=Some(if english() { "The save response was invalid. Refresh profiles before saving again." } else { "保存响应无效，请先刷新配置再保存。" }.into());
                        }
                    },
                    Err(e)=>{ if e.contains("PROFILE_STORE_CONFLICT") { this.profiles.snapshot=None; } this.profiles.error=Some(e.to_string()) }
                }
                cx.notify();
            });
        }).detach();
    }
    pub fn render_profiles(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entries = self
            .profiles
            .snapshot
            .as_ref()
            .map(|s| s.profiles.clone())
            .unwrap_or_default();
        let selected = self
            .profiles
            .selected
            .as_ref()
            .and_then(|id| entries.iter().find(|p| &p.id == id))
            .cloned();
        let driver = self.profiles.driver;
        let busy = self.profiles.loading || !self.connected;
        v_flex()
            .id("profile-settings")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.profiles.scroll)
            .vertical_scrollbar(&self.profiles.scroll)
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_xs()
                    .child(tr("只填写非机密的路径和参数；请勿填写凭据。")),
            )
            .child(
                Button::new("profiles-refresh")
                    .label(tr("刷新配置"))
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| this.load_profiles(cx))),
            )
            .when(self.profiles.saved, |v| {
                let message = if english() { "Saved." } else { "已保存。" };
                v.child(div().id("profile-save-status").role(Role::Status).aria_label(message).child(message))
            })
            .when_some(self.profiles.error.clone(), |v, error| {
                v.child(
                    div()
                        .id("profile-error-status")
                        .role(Role::Status)
                        .aria_label(error.clone())
                        .text_color(crate::palette::Color::Text.get(cx))
                        .child(error),
                )
            })
            .children(entries.into_iter().enumerate().map(|(i, p)| {
                let id = p.id.clone();
                let driver = supported_driver(&p.driver);
                let history = p
                    .revisions
                    .iter()
                    .map(|r| {
                        format!(
                            "{}: {}",
                            r.revision,
                            serde_json::to_string(&r.launch).unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" · ");
                v_flex()
                    .gap_1()
                    .child(
                        Button::new(("profile-edit", i))
                            .label(format!(
                                "{} · {} · r{}{}",
                                localized_profile_label(&p.id, &p.label),
                                p.driver,
                                p.current_revision,
                                if p.enabled { String::new() } else { format!(" · {}", tr("已停用")) }
                            ))
                            .disabled(busy || driver.is_none())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(driver) = driver {
                                    this.edit_profile(Some(id.clone()), driver, window, cx)
                                }
                            })),
                    )
                    .child(div().text_xs().child(history))
                    .when(driver.is_none(), |v| {
                        v.child(div().text_xs().child(tr("不支持此后端，只能查看。")))
                    })
            }))
            .child(div().child(tr("新建配置")))
            .child(
                h_flex().gap_1().children(
                    [
                        ("pi", "Pi"),
                        ("codex", "Codex"),
                        ("pi-rpc", "外部 Pi"),
                        ("claude", "Claude"),
                        ("acp", "ACP"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (driver, label))| {
                        Button::new(("new-profile", i))
                            .label(tr(label))
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_profile(None, driver, window, cx)
                            }))
                    }),
                ),
            )
            .when_some(selected.as_ref(), |view, profile| {
                let backend = supported_driver(&profile.driver);
                let reference = ProfileRef { id: profile.id.clone(), revision: profile.current_revision };
                let usable = !busy && profile.enabled && backend.is_some()
                    && profile.revisions.iter().any(|revision| revision.revision == profile.current_revision)
                    && self.workspace.is_some() && self.workspace_writes_allowed();
                let display_label = localized_profile_label(&profile.id, &profile.label);
                let accessible = if english() {
                    format!("New conversation using saved profile {display_label} · r{}", profile.current_revision)
                } else {
                    format!("按已保存配置新建会话：{display_label} · r{}", profile.current_revision)
                };
                view.child(Button::new("profile-create-session")
                    .label(tr("按已保存配置新建会话"))
                    .accessibility_label(accessible)
                    .disabled(!usable)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let valid = this.connected && !this.profiles.loading && this.workspace.is_some()
                            && this.workspace_writes_allowed()
                            && this.profiles.snapshot.as_ref().is_some_and(|snapshot| snapshot.profiles.iter().any(|p|
                                p.id == reference.id && p.enabled && p.current_revision == reference.revision
                                && supported_driver(&p.driver) == backend && p.revisions.iter().any(|r| r.revision == reference.revision)));
                        if !valid { return; }
                        if let Some(backend) = backend {
                            let modal = this.preferences_modal_open;
                            this.leave_preferences(window, cx);
                            if modal { window.close_dialog(cx); }
                            this.new_session_with_profile(backend, Some(reference.clone()), cx);
                        }
                    })))
                    .child(div().text_xs().child(tr("新会话使用上方已保存的版本，不使用表单中未保存的修改。")))
            })
            .child(div().child(if self.profiles.selected.is_some() {
                tr("编辑配置")
            } else {
                tr("新建配置")
            }))
            .child(div().child(tr("名称")))
            .child(Input::new(&self.profiles.label).aria_label(tr("名称")))
            .when(driver != "pi", |v| {
                v.child(div().child(tr("可执行文件")))
                    .child(Input::new(&self.profiles.command).aria_label(tr("可执行文件")))
            })
            .when(driver != "acp", |v| v.child(div().child(tr("配置目录（绝对路径，可留空）"))).child(Input::new(&self.profiles.config).aria_label(tr("配置目录（绝对路径，可留空）"))))
            .when(driver == "acp", |v| v.child(div().text_xs().child(tr("通过原生命令参数配置。"))))
            .when(driver != "pi" && driver != "claude", |v| {
                v.child(div().child(tr("参数（JSON 字符串数组；每项是一个完整参数）")))
                    .child(
                        Input::new(&self.profiles.args)
                            .aria_label(tr("参数（JSON 字符串数组；每项是一个完整参数）")),
                    )
            })
            .when(driver == "claude", |v| {
                v.child(div().child(tr("Claude 不支持启动参数")))
            })
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("save-profile")
                            .primary()
                            .label(tr("保存"))
                            .disabled(busy || self.profiles.snapshot.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                let action = if this.profiles.selected.is_some() {
                                    "update"
                                } else {
                                    "create"
                                };
                                this.mutate_profile(action, cx)
                            })),
                    )
                    .when_some(selected.as_ref(), |v, p| {
                        let enabled = p.enabled;
                        v.child(
                            Button::new("toggle-profile")
                                .label(if enabled { tr("停用") } else { tr("启用") })
                                .accessibility_label(format!(
                                    "{} · {}",
                                    if enabled { tr("停用") } else { tr("启用") },
                                    localized_profile_label(&p.id, &p.label)
                                ))
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.mutate_profile(
                                        if enabled { "disable" } else { "enable" },
                                        cx,
                                    )
                                })),
                        )
                    }),
            )
    }
}
