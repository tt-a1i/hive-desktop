use crate::{
    app::{HiveApp, SettingsSection},
    focus_scroll::Reveal,
    language::tr,
    palette::Color,
};
use gpui_kit::{
    component::{Theme, button::Button, *},
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
static LIMIT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LimitWindow {
    position: String,
    used_percent: f64,
    window_duration_mins: Option<f64>,
    resets_at: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LimitsResult {
    status: String,
    checked_at: String,
    ordinary_usage_allowed: Option<bool>,
    windows: Vec<LimitWindow>,
    profile: Option<crate::protocol::ProfileRef>,
    #[serde(rename = "source")]
    _source: String,
}
#[derive(Clone, Copy)]
enum LimitsView {
    ExternalProgram,
    AgentProfile,
}
#[derive(Default)]
struct LimitsState {
    scope: Option<super::CheckScope>,
    request_id: u64,
    loading: bool,
    result: Option<Result<LimitsResult, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountResult {
    status: String,
    account_type: Option<String>,
    command: String,
    source: String,
    login_command: Option<String>,
    profile: Option<crate::protocol::ProfileRef>,
}
#[derive(Default)]
pub struct CodexAccountCheck {
    setup_focus_pending: bool,
    scope: Option<super::CheckScope>,
    request_id: u64,
    loading: bool,
    result: Option<Result<AccountResult, String>>,
    limits: LimitsState,
    profile_limits: LimitsState,
}
impl LimitsView {
    fn scope(self, app: &HiveApp, cx: &App) -> Result<super::CheckScope, String> {
        if matches!(self, Self::ExternalProgram) {
            return app.program_check_scope("codex", cx);
        }
        let unavailable = || tr("Agent 配置不可用，请刷新配置后重试。").to_owned();
        if app.profiles.loading || app.profiles.error.is_some() {
            return Err(unavailable());
        }
        let snapshot = app.profiles.snapshot.as_ref().ok_or_else(unavailable)?;
        let selected = app.profiles.selected.as_ref().ok_or_else(unavailable)?;
        let profile = snapshot.profiles.iter().find(|profile| {
            profile.id == *selected && profile.driver == "codex" && profile.enabled
                && profile.revisions.iter().any(|revision| revision.revision == profile.current_revision)
        }).ok_or_else(unavailable)?;
        Ok(super::CheckScope::Profile {
            reference: crate::protocol::ProfileRef {
                id: profile.id.clone(),
                revision: profile.current_revision,
            },
            label: profile.label.clone(),
            store_version: snapshot.store_version,
        })
    }

    fn state(self, check: &CodexAccountCheck) -> &LimitsState {
        match self {
            Self::ExternalProgram => &check.limits,
            Self::AgentProfile => &check.profile_limits,
        }
    }

    fn state_mut(self, check: &mut CodexAccountCheck) -> &mut LimitsState {
        match self {
            Self::ExternalProgram => &mut check.limits,
            Self::AgentProfile => &mut check.profile_limits,
        }
    }
}
impl LimitsResult {
    fn matches_scope(&self, scope: &super::CheckScope) -> bool {
        match scope {
            super::CheckScope::Profile { reference, .. } => self.profile.as_ref() == Some(reference),
            super::CheckScope::Draft { .. } => self.profile.is_none(),
        }
    }
}
impl LimitsState {
    fn accepts(&self, id: u64, connection: u64, current_connection: u64, connected: bool) -> bool {
        connected && connection == current_connection && self.request_id == id
    }
    fn input_changed(&self, current: &Result<super::CheckScope, String>) -> bool {
        current.as_ref().ok() != self.scope.as_ref()
    }
}
impl AccountResult {
    fn matches_scope(&self, scope: &super::CheckScope) -> bool {
        match scope {
            super::CheckScope::Profile { reference, .. } => self.profile.as_ref() == Some(reference),
            super::CheckScope::Draft { .. } => self.profile.is_none(),
        }
    }

    fn message_key(&self) -> &'static str {
        match self.status.as_str() {
            "account_detected" => match self.account_type.as_deref() {
                Some("chatgpt") => "已连接 Codex，检测到 ChatGPT 账号记录。",
                Some("apiKey") => "已连接 Codex，检测到 API Key 账号记录。",
                Some("amazonBedrock") => "已连接 Codex，检测到 Amazon Bedrock 账号记录。",
                Some("gcp") => "已连接 Codex，检测到 Google Cloud 账号记录。",
                _ => "已连接 Codex，账号类型尚无法识别。",
            },
            "signed_out" => "已连接 Codex，未检测到账号；请在对应 CLI 中登录。",
            "not_required" => "已连接 Codex，当前配置不要求 OpenAI 账号。",
            "unknown" => "已连接 Codex，账号类型尚无法识别。",
            "cancelled" => "检查已取消。",
            _ => "无法完成 Codex 连接与账号检查，请核对程序路径并重试。",
        }
    }
}
impl CodexAccountCheck {
    fn input_changed(&self, current: &Result<super::CheckScope, String>) -> bool {
        current.as_ref().ok() != self.scope.as_ref()
    }
    fn message(&self) -> (String, bool) {
        match self.result.as_ref() {
            None => (tr("尚未检查连接与账号").into(), false),
            Some(Err(_)) => (
                tr("无法完成 Codex 连接与账号检查，请核对程序路径并重试。").into(),
                true,
            ),
            Some(Ok(result)) => (tr(result.message_key()).into(), result.status == "failed"),
        }
    }
}
impl HiveApp {
    pub(crate) fn can_open_codex_sign_in_setup(&self, cx: &App) -> bool {
        if let Some(settings) = &self.executable_settings
            && self.codex_command.read(cx).value().trim()
                != settings.codex_command.as_deref().unwrap_or_default()
        {
            return false;
        }
        self.profiles.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.profiles.iter().any(|profile| {
                profile.id == "default-codex"
                    && profile.driver == "codex"
                    && profile.enabled
                    && profile.revisions.iter().any(|revision| {
                        revision.revision == profile.current_revision
                            && revision.launch.config_dir.is_none()
                            && revision.launch.args.is_empty()
                    })
            })
        })
    }

    pub(crate) fn open_codex_sign_in_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.select_settings_section(SettingsSection::Programs, window, cx);
        self.codex_account.setup_focus_pending = true;
        cx.notify();
    }

    pub(super) fn cancel_codex_account_setup_focus(&mut self) {
        self.codex_account.setup_focus_pending = false;
    }

    pub(super) fn finish_codex_account_setup(&mut self, window: &mut Window, cx: &mut App) {
        if !self.codex_account.setup_focus_pending
            || self.settings_loading
            || self.profiles.loading
            || self.codex_account.loading
        {
            return;
        }
        self.cancel_codex_account_setup_focus();
        if self.settings_panel
            && self.settings_section == SettingsSection::Programs
            && self.preferences_focus.is_focused(window)
            && self.program_check_scope("codex", cx).is_ok()
        {
            self.codex_account_focus.focus(window, cx);
        }
    }

    fn check_codex_account(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.codex_account.loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let scope = match self.program_check_scope("codex", cx) { Ok(scope) => scope, Err(error) => { self.settings_message = Some(super::SettingsNotice::Error("读取配置失败", error)); cx.notify(); return; } };
        let params = scope.params();
        let id = LIMIT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        self.codex_account.scope = Some(scope.clone());
        self.codex_account.request_id = id;
        self.codex_account.loading = true;
        self.codex_account.result = None;
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "desktop.settings.codexAccount",
                    params,
                )
                .await
                .and_then(|value| serde_json::from_value::<AccountResult>(value).map_err(|error| error.to_string()))
                .and_then(|result| {
                    if result.matches_scope(&scope) {
                        Ok(result)
                    } else {
                        Err("Codex account response did not match the requested scope".into())
                    }
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection || this.codex_account.request_id != id { return; }
                let current = this.program_check_scope("codex", cx);
                this.codex_account.loading = false;
                if this.connected && current.as_ref().ok() == Some(&scope) { this.codex_account.result = Some(result); }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn check_codex_limits(&mut self, view: LimitsView, cx: &mut Context<Self>) {
        if !self.connected || self.closing {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let Ok(scope) = view.scope(self, cx) else {
            return;
        };
        let state = view.state_mut(&mut self.codex_account);
        if state.loading {
            return;
        }
        let params = scope.params();
        let id = LIMIT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        *state = LimitsState {
            scope: Some(scope.clone()),
            request_id: id,
            loading: true,
            result: None,
        };
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "desktop.settings.codexLimits",
                    params,
                )
                .await
                .and_then(|value| serde_json::from_value::<LimitsResult>(value).map_err(|error| error.to_string()))
                .and_then(|result| {
                    if result.matches_scope(&scope) {
                        Ok(result)
                    } else {
                        Err("Codex quota response did not match the requested executable or profile revision".into())
                    }
                });
            let _ = this.update(cx, |this, cx| {
                if !view.state(&this.codex_account).accepts(
                    id,
                    connection,
                    this.connection_generation,
                    this.connected,
                ) {
                    return;
                }
                let current = view.scope(this, cx);
                let state = view.state_mut(&mut this.codex_account);
                state.loading = false;
                if current.as_ref().ok() == Some(&scope) {
                    state.result = Some(result);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn render_selected_codex_limits(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        self.render_codex_limits(LimitsView::AgentProfile, cx)
    }

    fn render_codex_limits(&self, view: LimitsView, cx: &Context<Self>) -> impl IntoElement + use<> {
        let state = view.state(&self.codex_account);
        let scope = view.scope(self, cx);
        let stale = state.input_changed(&scope);
        let loading = state.loading;
        let profile_view = matches!(view, LimitsView::AgentProfile);
        v_flex()
            .gap_2()
            .pt_2()
            .border_t_1()
            .border_color(Color::Divider.get(cx))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(tr("Codex 账号额度")),
            )
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr(
                "账号额度与当前会话的 token 用量不同。此检查不会启动模型；切换账号后请重新读取。",
            )))
            .when(profile_view, |v| {
                v.child(crate::text_reading::selectable_text(
                    "codex-profile-limits-saved-revision",
                    scope.as_ref().map(super::CheckScope::label).unwrap_or_else(|_| tr("额度读取需要启用的已保存 Codex 配置。").to_owned()),
                ).text_xs())
                .child(div().text_xs().child(tr("额度读取使用已保存的版本。")))
            })
            .child(
                Button::new("check-codex-limits")
                    .outline()
                    .small()
                    .label(if loading {
                        tr("正在读取额度…")
                    } else {
                        tr("读取 Codex 账号额度")
                    })
                    .disabled(
                        !self.connected || self.closing
                            || (!profile_view && self.settings_loading)
                            || scope.is_err()
                            || loading,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.check_codex_limits(view, cx))),
            )
            .when(!self.connected && state.result.is_some(), |v| {
                v.child(
                    div()
                        .id("codex-limits-status")
                        .role(Role::Status)
                        .aria_label(tr("后台连接已断开，请重新读取额度。"))
                        .text_xs()
                        .child(tr("后台连接已断开，请重新读取额度。")),
                )
            })
            .when(self.connected && stale && state.result.is_some(), |v| {
                v.child(
                    div()
                        .id("codex-limits-status")
                        .role(Role::Status)
                        .aria_label(tr("配置或输入已更改，请重新读取额度。"))
                        .text_xs()
                        .child(tr("配置或输入已更改，请重新读取额度。")),
                )
            })
            .when(!profile_view && self.connected && !stale && state.result.is_some(), |v| v.child(div().text_xs().child(state.scope.as_ref().map(|scope| scope.label()).unwrap_or_default())))
            .when(self.connected && !stale, |v| {
                v.when_some(state.result.as_ref(), |v, result| match result {
                    Err(_) => v.child(
                        div()
                            .id("codex-limits-status")
                            .role(Role::Status)
                            .aria_label(tr("无法读取 Codex 账号额度。"))
                            .text_xs()
                            .text_color(Color::Error.get(cx))
                            .child(tr("无法读取 Codex 账号额度。")),
                    ),
                    Ok(result) => {
                        let message = match result.status.as_str() {
                            "ok" => tr("额度读取成功"),
                            "signed_out" => tr("未检测到 Codex 登录。"),
                            "not_applicable" => tr("当前账号不提供 ChatGPT 额度窗口。"),
                            "unavailable" => tr("账号额度暂不可用。"),
                            "cancelled" => tr("检查已取消。"),
                            _ => tr("无法读取 Codex 账号额度。"),
                        };
                        let checked_at = format!("{}: {}", tr("读取时间"), result.checked_at);
                        v.child(
                            div()
                                .id("codex-limits-status")
                                .role(Role::Status)
                                .aria_label(message)
                                .text_xs()
                                .child(message),
                        )
                        .when(
                            result.status == "ok" || result.status == "unavailable",
                            |v| {
                                let text = match result.ordinary_usage_allowed {
                                    Some(true) => tr("账号允许普通使用"),
                                    Some(false) => tr("当前普通使用受限"),
                                    None => tr("普通使用状态未知"),
                                };
                                v.child(
                                    div()
                                        .id("codex-limits-ordinary")
                                        .role(Role::Label)
                                        .aria_value(text)
                                        .text_xs()
                                        .child(text),
                                )
                            },
                        )
                        .child(
                            div()
                                .id("codex-limits-checked-at")
                                .role(Role::Label)
                                .aria_value(checked_at.clone())
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(checked_at),
                        )
                        .children(result.windows.iter().map(|window| {
                            let label = if window.position == "primary" {
                                tr("主要窗口")
                            } else {
                                tr("次要窗口")
                            };
                            let duration = window
                                .window_duration_mins
                                .map(|x| format!("{x} {}", tr("分钟")))
                                .unwrap_or_else(|| tr("时长未知").to_owned());
                            let reset = window.resets_at.as_deref().unwrap_or(tr("重置时间未知"));
                            let text = format!(
                                "{label}: {}% {} · {duration} · {}: {reset}",
                                100. - window.used_percent,
                                tr("剩余"),
                                tr("重置")
                            );
                            div()
                                .id(SharedString::from(format!(
                                    "codex-limit-window-{}",
                                    window.position
                                )))
                                .role(Role::Label)
                                .aria_value(text.clone())
                                .text_xs()
                                .child(text)
                        }))
                    }
                })
            })
    }
    pub(crate) fn codex_account_setup_status(&self, cx: &Context<Self>) -> String {
        let state = &self.codex_account;
        if state.loading {
            return tr("正在检查连接与账号…").into();
        }
        if state.result.is_none() {
            return state.message().0;
        }
        if state.input_changed(&self.program_check_scope("codex", cx)) {
            return tr("配置或输入已更改，请重新检查连接与账号。").into();
        }
        let message = state.message().0;
        state.scope.as_ref().map(|scope| format!("{}: {message}", scope.label())).unwrap_or(message)
    }
    pub(crate) fn render_codex_account_check(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let state = &self.codex_account;
        let changed = state.scope.is_some() && state.input_changed(&self.program_check_scope("codex", cx));
        let (dot_color, pill_text) = if state.loading {
            (super::amber_dot(cx), tr("检查中…"))
        } else if changed {
            (super::amber_dot(cx), tr("需检查"))
        } else {
            match state.result.as_ref() {
                None => (Color::Muted.get(cx), tr("待配置")),
                Some(Err(_)) => (Color::Error.get(cx), tr("需检查")),
                Some(Ok(res)) => match res.status.as_str() {
                    "account_detected" | "not_required" => (Color::Success.get(cx), tr("已连接")),
                    "signed_out" => (super::amber_dot(cx), tr("未登录")),
                    "failed" => (Color::Error.get(cx), tr("需检查")),
                    _ => (Color::Muted.get(cx), tr("状态未知")),
                },
            }
        };
        let check = Button::new("check-codex-account")
            .track_focus(&self.codex_account_focus)
            .outline()
            .small()
            .h(px(28.))
            .px_3()
            .rounded_lg()
            .label(if state.loading {
                tr("检查中…")
            } else {
                tr("检查 Codex 连接与账号")
            })
            .disabled(
                !self.connected
                    || self.settings_loading
                    || self.executable_settings.is_none()
                    || state.loading,
            )
            .on_click(cx.listener(|this, _, _, cx| this.check_codex_account(cx)));
        v_flex()
            .min_w_0()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(Color::Inset.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .flex_wrap()
                    .child(Reveal::new("check-codex-account-reveal", &self.program_scroll, check))
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py(px(2.))
                            .rounded_full()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(div().size(px(6.)).rounded_full().bg(dot_color))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(pill_text),
                            ),
                    ),
            )
            .child(
                crate::text_reading::selectable_text(
                    "codex-account-scope",
                    tr("建立原生连接并只读检查账号，不刷新凭据、不启动模型。检测到账号不代表登录仍有效或模型可用。"),
                )
                .text_xs()
                .text_color(Color::Muted.get(cx)),
            )
            .child(self.render_codex_limits(LimitsView::ExternalProgram, cx))
            .when_some(state.result.as_ref(), |view, result| {
                if changed {
                    return view.child(
                        crate::text_reading::selectable_text(
                            "codex-account-changed",
                            tr("配置或输入已更改，请重新检查。"),
                        )
                        .text_xs()
                        .text_color(Color::Secondary.get(cx)),
                    );
                }
                let (message, error) = state.message();
                view.child(div().text_xs().child(state.scope.as_ref().map(|scope| scope.label()).unwrap_or_default()))
                .child(
                    crate::text_reading::selectable_text("codex-account-status", message)
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if error {
                            Color::Error.get(cx)
                        } else {
                            Color::Text.get(cx)
                        }),
                )
                .when_some(result.as_ref().ok(), |view, result| {
                    view.child(
                        crate::text_reading::selectable_text(
                            "codex-account-source",
                            super::EffectiveCommand {
                                command: result.command.clone(),
                                source: result.source.clone(),
                            }
                            .label(tr("本次检查")),
                        )
                        .text_xs()
                        .text_color(Color::Muted.get(cx)),
                    )
                    .when_some(
                        result
                            .login_command
                            .as_ref()
                            .filter(|_| self.connected && result.status == "signed_out"),
                        |view, command| {
                            let copy = command.clone();
                            let request_id = state.request_id;
                            let copy_button = Button::new("copy-codex-login-command")
                                .outline()
                                .small()
                                .h(px(26.))
                                .px_2p5()
                                .rounded_lg()
                                .label(tr("复制 Codex 登录命令"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let current = this.program_check_scope("codex", cx);
                                    if this.connected
                                        && this.codex_account.request_id == request_id
                                        && !this.codex_account.input_changed(&current)
                                    {
                                        cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                                    }
                                }));
                            view.child(
                                v_flex()
                                    .gap_1p5()
                                    .pt_1()
                                    .child(
                                        crate::text_reading::selectable_text(
                                            "codex-login-instructions",
                                            if cfg!(target_os = "windows") {
                                                tr("在 PowerShell 中执行以下命令完成登录，然后重新检查。")
                                            } else {
                                                tr("在终端执行以下命令完成登录，然后重新检查。")
                                            },
                                        )
                                        .text_xs()
                                        .text_color(Color::Secondary.get(cx)),
                                    )
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .px_2p5()
                                            .py_2()
                                            .rounded_lg()
                                            .bg(Color::Card.get(cx))
                                            .border_1()
                                            .border_color(Color::Divider.get(cx))
                                            .child(
                                                div().flex_1().min_w_0().child(
                                                    crate::text_reading::selectable_text(
                                                        "codex-login-command",
                                                        command.clone(),
                                                    )
                                                    .font_family(
                                                        Theme::global(cx).mono_font_family.clone(),
                                                    )
                                                    .text_xs()
                                                    .text_color(Color::Text.get(cx)),
                                                ),
                                            )
                                            .child(Reveal::new(
                                                "copy-codex-login-command-reveal",
                                                &self.program_scroll,
                                                copy_button,
                                            )),
                                    ),
                            )
                        },
                    )
                })
            })
    }
}

#[cfg(test)]
mod limits_tests {
    use super::{LimitsResult, LimitsState};
    #[test]
    fn controlled_limit_response_decodes_without_losing_windows() {
        let result: LimitsResult = serde_json::from_value(serde_json::json!({
            "status": "ok", "checkedAt": "2026-11-15T00:00:00.000Z", "ordinaryUsageAllowed": false,
            "windows": [
                {"position":"primary", "usedPercent":22, "windowDurationMins":300, "resetsAt":null},
                {"position":"secondary", "usedPercent":95, "windowDurationMins":null, "resetsAt":null}
            ], "source":"app"
        })).unwrap();
        assert_eq!(result.status, "ok");
        assert_eq!(result.ordinary_usage_allowed, Some(false));
        assert_eq!(result.windows.len(), 2);
        assert_eq!(result.windows[0].used_percent, 22.);
        assert_eq!(result.windows[1].used_percent, 95.);
    }
    #[test]
    fn old_requests_and_changed_executables_do_not_show_as_current() {
        let state = LimitsState {
            scope: Some(super::super::CheckScope::Draft { command: "codex-a".into(), reference: crate::protocol::ProfileRef { id: "default-codex".into(), revision: 1 }, store_version: 1 }),
            request_id: 9,
            loading: false,
            result: None,
        };
        assert!(state.accepts(9, 2, 2, true));
        assert!(!state.accepts(8, 2, 2, true));
        assert!(!state.accepts(9, 2, 3, true));
        assert!(!state.accepts(9, 2, 2, false));
        assert!(state.input_changed(&Ok(super::super::CheckScope::Draft { command: "codex-b".into(), reference: crate::protocol::ProfileRef { id: "default-codex".into(), revision: 1 }, store_version: 1 })));
        assert!(!state.input_changed(&Ok(super::super::CheckScope::Draft { command: "codex-a".into(), reference: crate::protocol::ProfileRef { id: "default-codex".into(), revision: 1 }, store_version: 1 })));
    }

    #[test]
    fn quota_response_matches_only_the_requested_saved_profile_revision() {
        use super::super::CheckScope;
        use crate::protocol::ProfileRef;

        let requested = ProfileRef { id: "custom-codex".into(), revision: 2 };
        let scope = CheckScope::Profile {
            reference: requested.clone(),
            label: "Custom Codex".into(),
            store_version: 4,
        };
        let mut result: LimitsResult = serde_json::from_value(serde_json::json!({
            "status": "ok", "checkedAt": "2026-11-19T00:00:00.000Z",
            "ordinaryUsageAllowed": null, "windows": [], "source": "profile"
        })).unwrap();
        assert!(!result.matches_scope(&scope));
        result.profile = Some(ProfileRef { id: requested.id.clone(), revision: 1 });
        assert!(!result.matches_scope(&scope));
        result.profile = Some(ProfileRef { id: "other-codex".into(), revision: 2 });
        assert!(!result.matches_scope(&scope));
        result.profile = Some(requested);
        assert!(result.matches_scope(&scope));
    }
}
