use crate::app::HiveApp;
use gpui_kit::{
    component::{
        WindowExt,
        input::{InputEvent, InputState},
    },
    *,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
mod draft;
mod view;

#[derive(Clone, Deserialize)]
struct Snapshot {
    revision: String,
    path: String,
    providers: Vec<Provider>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Provider {
    id: String,
    name: Option<String>,
    base_url: Option<String>,
    api: Option<String>,
    models: Vec<Model>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Model {
    id: String,
    name: Option<String>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    reasoning: bool,
    vision: bool,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Target {
    provider: String,
    model: Option<String>,
    is_model: bool,
    creating: bool,
}
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
struct Values {
    id: String,
    name: String,
    url: String,
    first: String,
    context: String,
    output: String,
    api: String,
    reasoning: bool,
    vision: bool,
}

pub struct PiModels {
    draft_root: Option<std::path::PathBuf>,
    form_path: String,
    draft_checked: Option<String>,
    window: AnyWindowHandle,
    snapshot: Option<Snapshot>,
    target: Option<Target>,
    initial: Values,
    revision: String,
    id: Entity<InputState>,
    name: Entity<InputState>,
    url: Entity<InputState>,
    first: Entity<InputState>,
    context: Entity<InputState>,
    output: Entity<InputState>,
    api: String,
    reasoning: bool,
    vision: bool,
    loading: bool,
    generation: u64,
    message: Option<String>,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}
impl PiModels {
    pub fn new(window: &mut Window, cx: &mut Context<HiveApp>) -> Self {
        let mut subscriptions = vec![];
        let mut input = |placeholder: &'static str, cx: &mut Context<HiveApp>| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            subscriptions.push(cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.schedule_draft_save(cx);
                    cx.notify();
                }
            }));
            input
        };
        let id = input("例如 my-provider 或模型 ID", cx);
        let name = input("显示名称，可留空", cx);
        let url = input("https://example.com/v1", cx);
        let first = input("例如 gpt-6.1-terra", cx);
        let context = input("留空使用 Pi 默认值", cx);
        let output = input("留空使用 Pi 默认值", cx);
        Self {
            draft_root: None,
            form_path: String::new(),
            draft_checked: None,
            window: window.window_handle(),
            snapshot: None,
            target: None,
            initial: Values::default(),
            revision: String::new(),
            id,
            name,
            url,
            first,
            context,
            output,
            api: String::new(),
            reasoning: false,
            vision: false,
            loading: false,
            generation: 0,
            message: None,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }
    fn values(&self, cx: &App) -> Values {
        Values {
            id: self.id.read(cx).value().to_string(),
            name: self.name.read(cx).value().trim().to_string(),
            url: self.url.read(cx).value().trim().to_string(),
            first: self.first.read(cx).value().trim().to_string(),
            context: self.context.read(cx).value().trim().to_string(),
            output: self.output.read(cx).value().trim().to_string(),
            api: self.api.clone(),
            reasoning: self.reasoning,
            vision: self.vision,
        }
    }
    fn dirty(&self, cx: &App) -> bool {
        self.target.is_some() && self.values(cx) != self.initial
    }
    pub fn reset_connection(&mut self) {
        self.generation += 1;
        self.loading = false;
        self.snapshot = None;
        self.message = Some("连接已中断，表单已保留。请重新读取列表，核对是否已保存。".into());
    }
}
impl HiveApp {
    pub fn load_pi_models(&mut self, cx: &mut Context<Self>) {
        if self.pi_models.snapshot.is_none() && !self.pi_models.loading {
            self.pi_models_request(None, cx);
        }
    }
    fn pi_models_request(&mut self, change: Option<Value>, cx: &mut Context<Self>) {
        if !self.connected || self.pi_models.loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.pi_models.generation += 1;
        let generation = self.pi_models.generation;
        let connection = self.connection_generation;
        let save = change.is_some();
        let params = change.map_or_else(
            || json!({}),
            |change| json!({"revision":self.pi_models.revision,"change":change}),
        );
        self.pi_models.loading = true;
        self.pi_models.message = None;
        cx.notify();
        let window = self.pi_models.window;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    if save {
                        "pi.models.change"
                    } else {
                        "pi.models.get"
                    },
                    params,
                )
                .await;
            let _ = window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection
                        || this.pi_models.generation != generation
                    {
                        return;
                    }
                    this.pi_models.loading = false;
                    let result = result.and_then(|value| {
                        let conflict = save && value["conflict"] == true;
                        let snapshot: Snapshot = serde_json::from_value(if save {
                            value["snapshot"].clone()
                        } else {
                            value
                        })
                        .map_err(|error| error.to_string())?;
                        this.pi_models.snapshot = Some(snapshot);
                        this.pi_models.message = Some(
                        if conflict {
                            "配置文件已在外部更新，草稿已保留。请重新编辑此项，核对新配置后再保存。"
                        } else if save {
                            "配置已保存。请回到会话刷新模型；新会话将读取更新后的配置。"
                        } else if this.pi_models.target.is_some() {
                            "列表已更新，表单草稿保持不变。重新编辑此项可载入新配置。"
                        } else {
                            "已读取 Pi 模型配置。"
                        }
                        .into(),
                    );
                        if save && !conflict {
                            if let Err(error) = this.pi_models.clear_draft() {
                                this.pi_models.message =
                                    Some(format!("配置已保存，但旧草稿未能清除：{error}"));
                            }
                            this.pi_models.target = None;
                            this.pi_accounts.invalidate_catalog();
                        }
                        Ok(())
                    });
                    let succeeded = result.is_ok();
                    if let Err(error) = result {
                        this.pi_models.message = Some(format!("模型配置操作失败：{error}"));
                    }
                    if succeeded && !save && this.pi_models.snapshot.is_some() {
                        this.restore_pi_model_draft(window, cx);
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }
    fn edit_pi_model_config(
        &mut self,
        target: Option<Target>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pi_models.loading || self.pi_models.snapshot.is_none() {
            return;
        }
        if self.pi_models.dirty(cx) {
            let weak = cx.weak_entity();
            window.open_alert_dialog(cx, move |dialog, _, _| {
                let target = target.clone();
                let weak = weak.clone();
                dialog
                    .title("放弃当前表单修改？")
                    .description("尚未保存的修改会被替换。")
                    .show_cancel(true)
                    .ok_text("放弃修改")
                    .cancel_text("继续编辑")
                    .on_ok(move |_, window, cx| {
                        weak.update(cx, |this, cx| {
                            this.begin_pi_model_form(target.clone(), window, cx)
                        })
                        .is_ok()
                    })
            });
        } else {
            self.begin_pi_model_form(target, window, cx);
        }
    }
    fn begin_pi_model_form(
        &mut self,
        target: Option<Target>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = self.pi_models.snapshot.as_ref() else {
            return;
        };
        let mut value = Values::default();
        if let Some(target) = &target {
            if target.is_model {
                if !target.creating {
                    let Some(model) = snapshot
                        .providers
                        .iter()
                        .find(|p| p.id == target.provider)
                        .and_then(|p| {
                            p.models
                                .iter()
                                .find(|m| Some(&m.id) == target.model.as_ref())
                        })
                    else {
                        return;
                    };
                    value.id = model.id.clone();
                    value.name = model.name.clone().unwrap_or_default();
                    value.context = model
                        .context_window
                        .map(|n| n.to_string())
                        .unwrap_or_default();
                    value.output = model.max_tokens.map(|n| n.to_string()).unwrap_or_default();
                    value.reasoning = model.reasoning;
                    value.vision = model.vision;
                }
            } else if !target.creating {
                let Some(provider) = snapshot.providers.iter().find(|p| p.id == target.provider)
                else {
                    return;
                };
                value.id = provider.id.clone();
                value.name = provider.name.clone().unwrap_or_default();
                value.url = provider.base_url.clone().unwrap_or_default();
                value.api = provider.api.clone().unwrap_or_default();
            } else {
                value.api = "openai-completions".into();
            }
        }
        let revision = snapshot.revision.clone();
        let path = snapshot.path.clone();
        if let Err(error) = self.pi_models.clear_draft() {
            self.pi_models.message = Some(format!("无法丢弃本地草稿，表单已保留：{error}"));
            cx.notify();
            return;
        }
        self.pi_models.form_path = path;
        self.pi_models.revision = revision;
        self.pi_models.target = target;
        self.pi_models.initial = value.clone();
        self.apply_pi_model_values(value, window, cx);
        self.pi_models.message = None;
        self.pi_models.scroll.set_offset(point(px(0.), px(0.)));
        if self.pi_models.target.is_some() {
            let focus = self.pi_models.name.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        cx.notify();
    }
    fn apply_pi_model_values(
        &mut self,
        value: Values,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pi_models.api = value.api;
        self.pi_models.reasoning = value.reasoning;
        self.pi_models.vision = value.vision;
        for (input, text) in [
            (&self.pi_models.id, value.id),
            (&self.pi_models.name, value.name),
            (&self.pi_models.url, value.url),
            (&self.pi_models.first, value.first),
            (&self.pi_models.context, value.context),
            (&self.pi_models.output, value.output),
        ] {
            input.update(cx, |input, cx| input.set_value(text, window, cx));
        }
    }
    fn save_pi_model_form(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.pi_models.target.clone() else {
            return;
        };
        if self
            .pi_models
            .snapshot
            .as_ref()
            .is_none_or(|snapshot| snapshot.path != self.pi_models.form_path)
        {
            self.pi_models.message = Some("配置路径已改变，请重新读取并核对草稿。".into());
            cx.notify();
            return;
        }
        let value = self.pi_models.values(cx);
        let optional = |value: String| if value.is_empty() { None } else { Some(value) };
        let change = if target.is_model {
            let token = |value: &str| {
                if value.is_empty() {
                    Ok(None)
                } else {
                    value
                        .parse::<u64>()
                        .ok()
                        .filter(|v| *v > 0)
                        .map(Some)
                        .ok_or("Token 数量需要正整数，或留空使用默认值")
                }
            };
            let numbers = token(&value.context)
                .and_then(|context| token(&value.output).map(|output| (context, output)));
            let (context, output) = match numbers {
                Ok(value) => value,
                Err(error) => {
                    self.pi_models.message = Some(error.into());
                    cx.notify();
                    return;
                }
            };
            json!({"action":"model","id":target.provider,"modelId":if target.creating {value.id.trim().to_string()} else {target.model.unwrap_or(value.id)},"name":optional(value.name),"contextWindow":context,"maxTokens":output,"reasoning":value.reasoning,"vision":value.vision})
        } else {
            json!({"action":"provider","id":if target.creating {value.id.trim().to_string()} else {target.provider},"name":optional(value.name),"baseUrl":optional(value.url),"api":optional(value.api),"firstModelId":optional(value.first)})
        };
        self.pi_models_request(Some(change), cx);
    }
    fn remove_pi_model_config(
        &mut self,
        provider: String,
        model: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.pi_models.loading || self.pi_models.target.is_some() {
            return;
        }
        let Some(snapshot) = &self.pi_models.snapshot else {
            return;
        };
        let revision = snapshot.revision.clone();
        let weak = cx.weak_entity();
        let connection = self.connection_generation;
        let title = format!(
            "移除 {} 的自定义配置？",
            model.as_deref().unwrap_or(&provider)
        );
        let change = match model {
            Some(model) => json!({"action":"removeModel","id":provider,"modelId":model}),
            None => json!({"action":"removeProvider","id":provider}),
        };
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let revision = revision.clone();
            let change = change.clone();
            dialog
                .title(title.clone())
                .description(
                    "不会删除账号凭据或历史会话。内置项会恢复默认；仅自定义的项将从目录移除。",
                )
                .show_cancel(true)
                .ok_text("移除配置")
                .cancel_text("取消")
                .on_ok(move |_, _, cx| {
                    weak.update(cx, |this, cx| {
                        if this.connection_generation == connection {
                            this.pi_models.revision = revision.clone();
                            this.pi_models_request(Some(change.clone()), cx);
                        }
                    })
                    .is_ok()
                })
        });
    }
}
