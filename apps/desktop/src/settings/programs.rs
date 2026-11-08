use crate::{app::HiveApp, palette::Color, settings::EffectiveCommand};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputState},
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
pub struct ProgramCheck {
    input: String,
    loading: bool,
    result: Option<Result<ProgramResult, String>>,
}
impl HiveApp {
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
        let changed = check.is_some_and(|check| {
            let environment = check
                .result
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .is_some_and(|result| result.source == "env");
            !environment && check.input != input.read(cx).value().trim()
        });
        v_flex().min_w_0().gap_2().py_3().border_b_1().border_color(Color::Divider.get(cx))
            .child(h_flex().items_center().gap_2()
                .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(label))
                .child(Button::new(SharedString::from(format!("probe-{backend}"))).ghost().compact()
                    .label(if loading {"检查中…"} else {"检查程序"}).accessibility_label(format!("检查 {label} 程序版本"))
                    .disabled(disabled || loading).on_click(cx.listener(move |this,_,_,cx|this.check_program(backend,probe_input.clone(),cx)))))
            .child(h_flex().w_full().gap_2()
                .child(div().flex_1().min_w_0().child(Input::new(input).rounded_full().aria_label(format!("{label} 可执行文件路径")).disabled(disabled)))
                .child(Button::new(SharedString::from(format!("browse-{label}"))).rounded_full().label("浏览…").accessibility_label(format!("选择 {label} 可执行文件")).disabled(disabled)
                    .on_click(cx.listener(move |this,_,window,cx|this.browse_executable(label,target.clone(),window,cx)))))
            .when_some(effective,|row,value|row.child(div().text_xs().text_color(Color::Muted.get(cx)).child(value.label("当前使用"))))
            .when(changed, |row|row.child(div().text_xs().text_color(Color::Secondary.get(cx)).child("输入已更改，请重新检查。")))
            .when_some(check.filter(|_|!changed).and_then(|check|check.result.as_ref()),|row,result|match result {
                Err(error)=>row.child(div().text_sm().text_color(Color::Error.get(cx)).child(error.clone())),
                Ok(result)=>row.child(v_flex().min_w_0().gap_1()
                    .child(div().text_sm().text_color(if result.status=="available" {Color::Text.get(cx)} else {Color::Error.get(cx)}).child(result.message.clone()))
                    .child(div().text_xs().text_color(Color::Muted.get(cx)).child(EffectiveCommand{command:result.command.clone(),source:result.source.clone()}.label("本次检查")))
                    .when(result.version.as_ref().is_some_and(|version|version!=&result.verified_version),|row|row.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("客户端已验证版本：{}。检测版本不同，尚未验证兼容性。",result.verified_version)))))
            })
    }
}
