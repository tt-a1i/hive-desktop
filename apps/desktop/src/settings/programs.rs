use crate::{app::HiveApp, palette::Color, settings::EffectiveCommand, language::{english,tr}};
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
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramResult {
    status: String,
    version: Option<String>,
    verified_version: String,
    command: String,
    source: String,
    message: String,
}
impl ProgramResult {
    fn message(&self) -> String {
        if let Some(version) = self.version.as_ref().filter(|version| self.status == "available" && self.message == format!("已检测到版本 {version}。")) {
            return if english() { format!("Detected version {version}.") } else { self.message.clone() };
        }
        [
            "检查已取消。",
            "程序没有在 5 秒内返回版本，已停止检查。",
            "程序输出过长，已停止检查。",
            "程序输入通道失败，已停止检查。",
            "找不到程序，请检查路径或安装对应 CLI。",
            "无法运行程序，请检查文件权限和路径。",
            "程序未能正常返回版本，请在终端检查该 CLI。",
            "程序已运行，但没有返回可识别的版本。",
        ].into_iter().find(|key| *key == self.message).map(|key| tr(key).to_owned()).unwrap_or_else(|| self.message.clone())
    }
}
pub struct ProgramCheck {
    input: String,
    loading: bool,
    result: Option<Result<ProgramResult, String>>,
}
impl ProgramCheck {
    fn input_changed(&self, input: &str) -> bool {
        let environment = self.result.as_ref().and_then(|result| result.as_ref().ok())
            .is_some_and(|result| result.source == "env");
        !environment && self.input != input.trim()
    }
}
impl HiveApp {
    pub(crate) fn open_program_setup(&mut self, backend: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.program_setup_focus = Some(backend);
        self.settings_section = crate::app::SettingsSection::Programs;
        self.open_settings(window, cx);
        self.finish_program_setup(window, cx);
    }

    pub(crate) fn finish_program_setup(&mut self, window: &mut Window, cx: &mut App) {
        if self.settings_loading || self.executable_settings.is_none() { return; }
        let Some(backend) = self.program_setup_focus.take() else { return; };
        if !self.settings_panel || self.settings_section != crate::app::SettingsSection::Programs { return; }
        let input = match backend { "codex" => &self.codex_command, "claude" => &self.claude_command, _ => &self.pi_command };
        input.update(cx, |input, cx| { input.focus(window, cx); input.select_all(window, cx); });
    }

    pub(crate) fn program_setup_status(&self, backend: &'static str, cx: &Context<Self>) -> String {
        let input = match backend { "codex" => &self.codex_command, "claude" => &self.claude_command, _ => &self.pi_command };
        let Some(check) = self.program_checks.get(backend) else { return tr("尚未检查程序版本").into(); };
        if check.loading { return tr("检查中…").into(); }
        if check.input_changed(&input.read(cx).value()) { return tr("输入已更改，请重新检查。").into(); }
        let message = match check.result.as_ref() {
            Some(Ok(result)) => result.message(),
            Some(Err(error)) => crate::language::localize_owned_error(error),
            None => tr("尚未检查程序版本").into(),
        };
        let saved = self.executable_settings.as_ref().and_then(|settings| match backend {
            "codex" => settings.codex_command.as_deref(), "claude" => settings.claude_command.as_deref(), _ => settings.pi_command.as_deref(),
        }).unwrap_or_default();
        let environment = check.result.as_ref().and_then(|result| result.as_ref().ok()).is_some_and(|result| result.source == "env");
        if !environment && input.read(cx).value().trim() != saved {
            format!("{}: {message}", tr("未保存的程序路径"))
        } else { message }
    }

    pub(crate) fn program_local_check_state(&self, backend: &'static str, cx: &Context<Self>) -> Option<bool> {
        let input = match backend { "codex" => &self.codex_command, "claude" => &self.claude_command, _ => &self.pi_command };
        let check = self.program_checks.get(backend)?;
        if check.loading || check.input_changed(&input.read(cx).value()) {
            return None;
        }
        match check.result.as_ref()? {
            Ok(result) => Some(result.status == "available"),
            Err(_) => Some(false),
        }
    }

    pub(crate) fn render_program_settings(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let scroll = &self.program_scroll;
        crate::focus_scroll::Viewport::new("programs-viewport", scroll,
            v_flex().id("settings-programs-content").flex_1().min_w_0().min_h_0()
                .overflow_y_scroll().track_scroll(scroll).vertical_scrollbar(scroll).gap_3()
                .child(crate::text_reading::selectable_text("programs-description", tr("路径留空时使用 PATH 查找，环境变量优先。可先检查当前输入，再保存；版本检查只读取程序版本。修改在下次启动对应 Agent 时生效。")).text_sm().text_color(Color::Secondary.get(cx)))
                .child(crate::text_reading::selectable_text("programs-account-guidance", tr("Claude Code、Codex 和外部 Pi 使用各自 CLI 的登录与配置。检测到版本不代表已登录或模型可用；请先在对应 CLI 中完成设置。")).text_sm().text_color(Color::Secondary.get(cx)))
                .when_some(self.settings_message.as_ref().map(|notice| notice.text()), |view, message| view.child(crate::text_reading::selectable_text("programs-save-notice", message).p_3().rounded_lg().bg(Color::Inset.get(cx)).text_sm()))
                .child(self.executable_field("pi", tr("外部 Pi"), &self.pi_command, self.executable_settings.as_ref().map(|s| &s.effective.pi), cx))
                .child(self.executable_field("codex", "Codex", &self.codex_command, self.executable_settings.as_ref().map(|s| &s.effective.codex), cx))
                .child(self.executable_field("claude", "Claude Code", &self.claude_command, self.executable_settings.as_ref().map(|s| &s.effective.claude), cx))
                .child(crate::focus_scroll::Reveal::new("programs-actions-reveal", scroll,
                    h_flex().items_center().flex_wrap().gap_2()
                        .child(Button::new("save-executable-settings").primary().rounded_full().label(if self.settings_loading { tr("处理中…") } else { tr("保存设置") }).disabled(!self.connected || self.settings_loading || !self.executable_paths_changed(cx)).on_click(cx.listener(|this, _, window, cx| this.settings_request(true, window.window_handle(), cx))))
                        .child(Button::new("reload-executable-settings").rounded_full().label(tr("重新读取")).disabled(!self.connected || self.settings_loading).on_click(cx.listener(|this, _, window, cx| this.settings_request(false, window.window_handle(), cx))))
                        .when(!self.settings_loading && self.executable_paths_changed(cx), |row| row.child(crate::text_reading::selectable_text("programs-unsaved", tr("有未保存的修改")).text_xs().text_color(Color::Secondary.get(cx)))))))
    }

    fn check_program(
        &mut self,
        backend: &'static str,
        input: Entity<InputState>,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self
                .program_checks
                .get(backend)
                .is_some_and(|check| check.loading)
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let value = input.read(cx).value().trim().to_owned();
        self.program_checks.insert(
            backend,
            ProgramCheck {
                input: value.clone(),
                loading: true,
                result: None,
            },
        );
        let connection = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "desktop.settings.probe",
                    json!({"backend":backend,"command":if value.is_empty(){None}else{Some(value)}}),
                )
                .await
                .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                if let Some(check) = this.program_checks.get_mut(backend) {
                    check.loading = false;
                    check.result = Some(result);
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn executable_field(
        &self,
        backend: &'static str,
        label: &'static str,
        input: &Entity<InputState>,
        effective: Option<&EffectiveCommand>,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let disabled =
            !self.connected || self.settings_loading || self.executable_settings.is_none();
        let target = input.clone();
        let probe_input = input.clone();
        let check = self.program_checks.get(backend);
        let loading = check.is_some_and(|check| check.loading);
        let changed = check.is_some_and(|check| check.input_changed(&input.read(cx).value()));
        let readout = |suffix: &str, text: String| crate::text_reading::selectable_text(
            SharedString::from(format!("program-{backend}-{suffix}")), text);
        let field = v_flex().min_w_0().gap_2().py_3().border_b_1().border_color(Color::Divider.get(cx))
            .child(h_flex().items_center().gap_2()
                .child(div().id(SharedString::from(format!("program-{backend}-name"))).role(Role::Label).aria_label(label).flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(label))
                .child(Button::new(SharedString::from(format!("probe-{backend}"))).ghost().compact()
                    .label(if loading {tr("检查中…")} else {tr("检查程序")}).accessibility_label(if english() { format!("Check {label} version") } else { format!("检查 {label} 程序版本") })
                    .disabled(disabled || loading).on_click(cx.listener(move |this,_,_,cx|this.check_program(backend,probe_input.clone(),cx)))))
            .child(h_flex().w_full().gap_2()
                .child(div().flex_1().min_w_0().child(Input::new(input).rounded_full().aria_label(if english() { format!("{label} executable path") } else { format!("{label} 可执行文件路径") }).disabled(disabled)))
                .child(Button::new(SharedString::from(format!("browse-{backend}"))).rounded_full().label(tr("浏览…")).accessibility_label(if english() { format!("Select {label} executable") } else { format!("选择 {label} 可执行文件") }).disabled(disabled)
                    .on_click(cx.listener(move |this,_,window,cx|this.browse_executable(label,target.clone(),window,cx)))))
            .when_some(effective,|row,value|row.child(readout("effective",value.label(tr("当前使用"))).text_xs().text_color(Color::Muted.get(cx))))
            .when(changed, |row|row.child(readout("changed",tr("输入已更改，请重新检查。").into()).text_xs().text_color(Color::Secondary.get(cx))))
            .when_some(check.filter(|_|!changed).and_then(|check|check.result.as_ref()),|row,result|match result {
                Err(error)=>row.child(readout("error",crate::language::localize_owned_error(error)).text_sm().text_color(Color::Error.get(cx))),
                Ok(result)=>row.child(v_flex().min_w_0().gap_1()
                    .child(readout("result",result.message()).text_sm().text_color(if result.status=="available" {Color::Text.get(cx)} else {Color::Error.get(cx)}))
                    .child(readout("checked",EffectiveCommand{command:result.command.clone(),source:result.source.clone()}.label(tr("本次检查"))).text_xs().text_color(Color::Muted.get(cx)))
                    .when(result.version.as_ref().is_some_and(|version|version!=&result.verified_version),|row|row.child(readout("compatibility",if english() { format!("Client-verified version: {}. The detected version differs; compatibility has not been verified.",result.verified_version) } else { format!("客户端已验证版本：{}。检测版本不同，尚未验证兼容性。",result.verified_version) }).text_xs().text_color(Color::Secondary.get(cx)))))
            });
        let field = field.when(backend == "codex", |field| field.child(self.render_codex_account_check(cx)));
        crate::focus_scroll::Reveal::new(SharedString::from(format!("program-{backend}-reveal")), &self.program_scroll, field)
    }
}
