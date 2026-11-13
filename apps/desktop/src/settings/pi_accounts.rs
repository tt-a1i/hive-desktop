use crate::{app::HiveApp, language::{english,tr}};
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
    registered: bool,
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
    message: Option<AccountNotice>,
    input: Entity<InputState>,
    search: Entity<InputState>,
    generation: u64,
    poll: Option<Task<()>>,
    _input_subscription: Subscription,
    _search_subscription: Subscription,
}
fn runtime_message(message: &str) -> String {
    [
        "不支持的认证方式",
        "该提供商不支持此配置方式",
        "配置超时，请重新开始",
        "登录超时，请重新开始",
        "配置已保存。可返回会话刷新模型列表并选择模型。",
        "凭据已保存，但模型列表更新失败。请返回会话刷新模型；若仍失败，请检查 Pi 配置。",
        "登录已取消",
        "登录步骤已结束，请刷新后继续",
        "请输入登录信息",
        "无效的登录选项",
        "登录已结束",
        "配置已取消",
        "请先完成或取消当前账号操作",
        "提供商不存在",
        "已移除本机保存的凭据。环境变量等其他登录来源不受影响。",
        "凭据已移除，但模型列表更新失败。请返回会话刷新模型。",
        "后台已关闭，登录已取消",
        "读取本机登录信息失败，请检查 Pi 的 auth.json 文件后重新读取。",
        "登录失败",
    ].into_iter().find(|key| *key == message).map(|key| tr(key).to_owned()).unwrap_or_else(|| crate::language::localize_owned_error(message))
}
enum AccountNotice { Disconnected, Runtime(String), Failed(String) }
impl AccountNotice {
    fn text(&self) -> String {
        match self {
            Self::Disconnected => tr("后台连接已断开，请重新开始登录。").to_owned(),
            Self::Runtime(message) => runtime_message(message),
            Self::Failed(error) => format!("{}: {}", tr("账号操作失败"), runtime_message(error)),
        }
    }
}
fn input_placeholder(prompt: Option<&Prompt>) -> String {
    prompt.and_then(|prompt| prompt.placeholder.clone()).unwrap_or_else(|| tr(match prompt.map(|prompt| prompt.kind.as_str()) {
        Some("secret") => "输入或粘贴凭据",
        Some("manual_code") => "粘贴授权码或回调地址",
        _ => "按上方提示输入",
    }).to_owned())
}
impl PiAccounts {
    pub(crate) fn setup_status(&self) -> String {
        if self.loading { return tr("正在读取 Pi 账号…").into(); }
        if self.running() { return tr("账号配置进行中…").into(); }
        if matches!(self.message, Some(AccountNotice::Failed(_))) {
            return tr("本机登录信息未能更新，请在设置中重新读取。").into();
        }
        let Some(providers) = &self.providers else { return tr("连接 Pi 账号或配置模型提供商。").into(); };
        let count = providers.iter().filter(|provider| provider.stored_type.is_some()).count();
        if count == 0 { return tr("本机没有保存登录信息；环境变量等配置仍可生效。").into(); }
        if english() {
            format!("Login information saved locally for {count} {}. Choose a model after opening a project.", if count == 1 { "provider" } else { "providers" })
        } else { format!("本机已保存 {count} 个提供商的登录信息；打开项目后选择模型。") }
    }

    pub(crate) fn local_check_state(&self) -> Option<bool> {
        if self.loading || self.running() {
            return None;
        }
        if matches!(self.message, Some(AccountNotice::Failed(_))) {
            return Some(false);
        }
        let providers = self.providers.as_ref()?;
        providers.iter().any(|provider| provider.stored_type.is_some()).then_some(true)
    }

    pub(crate) fn localize_placeholders(&self, window: &mut Window, cx: &mut App) {
        self.search.update(cx, |input,cx| input.set_placeholder(tr("搜索提供商，如 OpenAI、Anthropic…"),window,cx));
        self.input.update(cx, |input,cx| input.set_placeholder(input_placeholder(self.login.as_ref().and_then(|login| login.prompt.as_ref())),window,cx));
    }
    pub fn new(window: &mut Window, cx: &mut Context<HiveApp>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(input_placeholder(None)));
        let search = cx
            .new(|cx| InputState::new(window, cx).placeholder(tr("搜索提供商，如 OpenAI、Anthropic…")));
        let search_subscription = cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.pi_accounts.scroll.set_offset(point(px(0.), px(0.)));
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
    pub fn invalidate_catalog(&mut self) {
        self.providers = None;
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
            self.message = Some(AccountNotice::Disconnected);
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
                        this.pi_accounts.message = catalog.message.map(AccountNotice::Runtime);
                    } else {
                        this.pi_accounts.login =
                            serde_json::from_value(value).map_err(|error| error.to_string())?;
                    }
                    Ok(())
                });
                if let Err(error) = result {
                    this.pi_accounts.message = Some(AccountNotice::Failed(error));
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
                    let placeholder = input_placeholder(prompt);
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
