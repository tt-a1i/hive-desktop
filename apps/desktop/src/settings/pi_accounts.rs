use crate::app::HiveApp;
use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
use serde::Deserialize;
use serde_json::{Value, json};
mod disconnect;
mod view;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Provider {
    id: String,
    name: String,
    methods: Vec<AuthMethod>,
    stored_type: Option<String>,
}
#[derive(Clone, Deserialize)]
struct AuthMethod {
    #[serde(rename = "type")]
    kind: String,
    label: String,
}
#[derive(Clone, Deserialize)]
struct Login {
    id: String,
    #[serde(rename = "providerId")]
    provider_id: String,
    #[serde(rename = "authType")]
    auth_type: String,
    state: String,
    message: Option<String>,
    notice: Option<Value>,
    authorization: Option<Value>,
    prompt: Option<Prompt>,
}
#[derive(Clone, Deserialize)]
struct Prompt {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    message: String,
    placeholder: Option<String>,
    #[serde(default)]
    options: Vec<Choice>,
}
#[derive(Clone, Deserialize)]
struct Choice {
    id: String,
    label: String,
    description: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    agent_dir: String,
    providers: Vec<Provider>,
    login: Option<Login>,
    message: Option<String>,
}

pub struct PiAccounts {
    pub(super) scroll: ScrollHandle,
    providers: Option<Vec<Provider>>,
    agent_dir: Option<String>,
    login: Option<Login>,
    loading: bool,
    message: Option<String>,
    input: Entity<InputState>,
    search: Entity<InputState>,
    generation: u64,
    poll: Option<Task<()>>,
    _input_subscription: Subscription,
    _search_subscription: Subscription,
}
impl PiAccounts {
    pub fn new(window: &mut Window, cx: &mut Context<HiveApp>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("粘贴授权码或回调地址"));
        let search = cx
            .new(|cx| InputState::new(window, cx).placeholder("搜索提供商，如 OpenAI、Anthropic…"));
        let search_subscription = cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let subscription = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
            if matches!(
                event,
                InputEvent::PressEnter {
                    secondary: false,
                    ..
                }
            ) {
                this.submit_pi_login(None, window, cx);
            }
        });
        Self {
            scroll: ScrollHandle::new(),
            providers: None,
            agent_dir: None,
            login: None,
            loading: false,
            message: None,
            input,
            search,
            generation: 0,
            poll: None,
            _input_subscription: subscription,
            _search_subscription: search_subscription,
        }
    }
    pub fn reset_connection(&mut self) {
        self.generation += 1;
        self.poll = None;
        self.providers = None;
        self.loading = false;
        if self
            .login
            .as_ref()
            .is_some_and(|login| login.state == "running")
        {
            self.message = Some("后台连接已断开，请重新开始登录。".into());
        }
        self.login = None;
    }
    fn running(&self) -> bool {
        self.login
            .as_ref()
            .is_some_and(|login| login.state == "running")
    }
}

impl HiveApp {
    pub fn reset_pi_accounts(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        self.pi_accounts.reset_connection();
        let input = self.pi_accounts.input.clone();
        let fallback = self.preferences_focus.clone();
        let _ = window.update(cx, |_, window, cx| {
            input.update(cx, |input, cx| {
                let focused = window.focused(cx).as_ref() == Some(&input.focus_handle(cx));
                input.set_value("", window, cx);
                if focused {
                    fallback.focus(window, cx);
                }
            })
        });
    }
    pub fn load_pi_accounts(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        if self.pi_accounts.providers.is_none() && !self.pi_accounts.loading {
            self.pi_auth_request("pi.auth.providers", json!({}), window, cx);
        }
    }
    pub fn submit_pi_login(
        &mut self,
        choice: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pi_accounts.loading {
            return;
        }
        let Some(login) = self
            .pi_accounts
            .login
            .as_ref()
            .filter(|login| login.state == "running")
        else {
            return;
        };
        let Some(prompt) = &login.prompt else {
            return;
        };
        let value = choice.unwrap_or_else(|| self.pi_accounts.input.read(cx).value().to_string());
        if prompt.kind != "text" && value.trim().is_empty() {
            return;
        }
        self.pi_auth_request(
            "pi.auth.respond",
            json!({"loginId":login.id, "promptId":prompt.id, "value":value}),
            window.window_handle(),
            cx,
        );
    }
    fn pi_auth_request(
        &mut self,
        method: &'static str,
        params: Value,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.pi_accounts.loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        if matches!(method, "pi.auth.start" | "pi.auth.logout") {
            self.pi_accounts.scroll.set_offset(point(px(0.), px(0.)));
        }
        self.pi_accounts.poll = None;
        self.pi_accounts.generation += 1;
        let generation = self.pi_accounts.generation;
        let connection = self.connection_generation;
        self.pi_accounts.loading = method != "pi.auth.status";
        self.pi_accounts.message = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.pi_accounts.generation != generation
                {
                    return;
                }
                this.pi_accounts.loading = false;
                let previous_prompt = this
                    .pi_accounts
                    .login
                    .as_ref()
                    .and_then(|login| login.prompt.as_ref())
                    .map(|prompt| prompt.id.clone());
                let previous_state = this
                    .pi_accounts
                    .login
                    .as_ref()
                    .map(|login| login.state.clone());
                let result = result.and_then(|value| {
                    if matches!(method, "pi.auth.providers" | "pi.auth.logout") {
                        let catalog: Catalog =
                            serde_json::from_value(value).map_err(|error| error.to_string())?;
                        this.pi_accounts.providers = Some(catalog.providers);
                        this.pi_accounts.agent_dir = Some(catalog.agent_dir);
                        this.pi_accounts.login = catalog.login;
                        this.pi_accounts.message = catalog.message;
                    } else {
                        this.pi_accounts.login =
                            serde_json::from_value(value).map_err(|error| error.to_string())?;
                    }
                    Ok(())
                });
                if let Err(error) = result {
                    this.pi_accounts.message = Some(format!("账号操作失败：{error}"));
                }
                let prompt = this
                    .pi_accounts
                    .login
                    .as_ref()
                    .and_then(|login| login.prompt.as_ref());
                if previous_prompt.as_deref() != prompt.map(|prompt| prompt.id.as_str())
                    || method == "pi.auth.respond"
                {
                    let masked = prompt.is_some_and(|prompt| {
                        matches!(prompt.kind.as_str(), "secret" | "manual_code")
                    });
                    let placeholder = prompt
                        .and_then(|prompt| prompt.placeholder.clone())
                        .unwrap_or_else(|| {
                            match prompt.map(|prompt| prompt.kind.as_str()) {
                                Some("secret") => "输入或粘贴凭据",
                                Some("manual_code") => "粘贴授权码或回调地址",
                                _ => "按上方提示输入",
                            }
                            .into()
                        });
                    let input = this.pi_accounts.input.clone();
                    let fallback = this.preferences_focus.clone();
                    let focus = this.settings_panel
                        && this.settings_section == crate::app::SettingsSection::PiAccounts
                        && prompt.is_some_and(|prompt| prompt.kind != "select");
                    let _ = window.update(cx, |_, window, cx| {
                        input.update(cx, |input, cx| {
                            let was_focused =
                                window.focused(cx).as_ref() == Some(&input.focus_handle(cx));
                            input.set_value("", window, cx);
                            input.set_masked(masked, window, cx);
                            input.set_placeholder(placeholder, window, cx);
                            if focus {
                                input.focus_handle(cx).focus(window, cx);
                            } else if was_focused {
                                fallback.focus(window, cx);
                            }
                        })
                    });
                }
                if this.pi_accounts.message.is_none() && this.pi_accounts.running() {
                    this.pi_accounts.poll = Some(cx.spawn(async move |this, cx| {
                        smol::Timer::after(std::time::Duration::from_millis(500)).await;
                        let _ = this.update(cx, |this, cx| {
                            if this.connection_generation == connection
                                && this.pi_accounts.generation == generation
                            {
                                this.pi_auth_request("pi.auth.status", json!({}), window, cx);
                            }
                        });
                    }));
                } else if previous_state.as_deref() == Some("running")
                    && this
                        .pi_accounts
                        .login
                        .as_ref()
                        .is_some_and(|login| login.state == "succeeded")
                {
                    // Read the saved credential metadata again; model selection remains per session.
                    this.pi_auth_request("pi.auth.providers", json!({}), window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}
