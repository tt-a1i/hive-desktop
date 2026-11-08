use super::*;
use crate::palette::Color;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        switch::Switch,
        *,
    },
    prelude::FluentBuilder,
};

impl HiveApp {
    pub fn render_pi_models(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.pi_models;
        let disabled = !self.connected || state.loading;
        v_flex().flex_1().min_h_0().min_w_0().id("pi-model-config-scroll").overflow_y_scroll().track_scroll(&state.scroll).gap_4()
            .child(div().text_sm().text_color(Color::Secondary.get(cx)).child(tr("管理自定义接口、模型和内置配置覆盖。保存后在“内置 Pi”设置 API Key；免认证服务可使用占位值。")))
            .child(h_flex().gap_2().flex_wrap()
                .child(Button::new("add-pi-provider").primary().rounded_full().label(tr("添加提供商")).disabled(disabled || state.snapshot.is_none()).on_click(cx.listener(|this,_,window,cx|this.edit_pi_model_config(Some(Target{provider:String::new(),model:None,is_model:false,creating:true}),window,cx))))
                .child(Button::new("reload-pi-model-config").ghost().compact().label(if state.loading {tr("处理中…")} else {tr("重新读取列表")}).disabled(disabled).on_click(cx.listener(|this,_,_,cx|this.pi_models_request(None,cx))))
                .when_some(state.snapshot.as_ref().map(|s|std::path::PathBuf::from(&s.path)).or_else(||self.pi_agent_dir.clone()),|row,path|{row.child(Button::new("locate-pi-model-config").ghost().compact().label(tr("定位配置文件")).on_click(move|_,_,cx|cx.reveal_path(&path)))}))
            .when_some(state.message.as_ref().map(|notice|notice.text()),|view,message|view.child(div().p_3().rounded_lg().bg(Color::Inset.get(cx)).text_sm().child(message)))
            .when_some(state.target.clone(),|view,target|view.child(self.render_pi_model_form(target,cx)))
            .when(state.snapshot.as_ref().is_some_and(|snapshot|snapshot.providers.is_empty()),|view|view.child(div().py_6().text_sm().text_color(Color::Secondary.get(cx)).child(tr("尚未添加自定义配置。内置模型仍可在会话的模型选择器中使用。"))))
            .children(state.snapshot.iter().flat_map(|snapshot|&snapshot.providers).map(|provider|{
                let id=provider.id.clone();let remove=id.clone();let add=id.clone();
                v_flex().gap_2().py_3().border_b_1().border_color(Color::Divider.get(cx))
                    .child(h_flex().gap_2().items_center().flex_wrap()
                        .child(div().flex_1().min_w_0().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("{} · {}",provider.name.as_deref().unwrap_or(&provider.id),provider.id)))
                        .child(Button::new(SharedString::from(format!("edit-pi-provider-{id}"))).ghost().compact().label(tr("编辑接口")).accessibility_label(format!("{}: {}",tr("编辑接口"),provider.id)).disabled(disabled).on_click(cx.listener(move|this,_,window,cx|this.edit_pi_model_config(Some(Target{provider:id.clone(),model:None,is_model:false,creating:false}),window,cx))))
                        .child(Button::new(SharedString::from(format!("remove-pi-provider-{remove}"))).ghost().compact().label(tr("移除")).accessibility_label(format!("{}: {}",tr("移除接口配置"),provider.id)).disabled(disabled || state.target.is_some()).on_click(cx.listener(move|this,_,window,cx|this.remove_pi_model_config(remove.clone(),None,window,cx)))))
                    .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(provider.base_url.clone().unwrap_or_else(||tr("使用内置接口地址").into())))
                    .children(provider.models.iter().map(|model|{
                        let provider_id=provider.id.clone();let model_id=model.id.clone();let remove_provider=provider.id.clone();let remove_model=model.id.clone();
                        h_flex().gap_2().items_center().flex_wrap().pl_3()
                            .child(v_flex().flex_1().min_w_0().gap_1().child(div().text_sm().child(model.name.clone().unwrap_or(model.id.clone()))).child(div().text_xs().text_color(Color::Muted.get(cx)).child(model.id.clone())).when(model.override_only,|view|view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(tr("模型覆盖")))))
                            .child(Button::new(SharedString::from(format!("edit-pi-model-{provider_id}-{model_id}"))).ghost().compact().label(tr("模型设置")).accessibility_label(format!("{}: {provider_id}/{model_id}",tr("编辑模型"))).disabled(disabled).on_click(cx.listener(move|this,_,window,cx|this.edit_pi_model_config(Some(Target{provider:provider_id.clone(),model:Some(model_id.clone()),is_model:true,creating:false}),window,cx))))
                            .child(Button::new(SharedString::from(format!("remove-pi-model-{remove_provider}-{remove_model}"))).ghost().compact().label(if model.override_only {tr("移除覆盖")} else {tr("移除模型")}).accessibility_label(format!("{}: {remove_provider}/{remove_model}",tr("移除模型配置"))).disabled(disabled || state.target.is_some()).on_click(cx.listener(move|this,_,window,cx|this.remove_pi_model_config(remove_provider.clone(),Some(remove_model.clone()),window,cx))))
                    }))
                    .child(h_flex().child(Button::new(SharedString::from(format!("add-pi-model-{add}"))).ghost().compact().label(tr("添加模型")).accessibility_label(format!("{}: {add}",tr("添加模型"))).disabled(disabled).on_click(cx.listener(move|this,_,window,cx|this.edit_pi_model_config(Some(Target{provider:add.clone(),model:None,is_model:true,creating:true}),window,cx)))))
            }))
    }
    fn render_pi_model_form(&self, target: Target, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.pi_models;
        let disabled = !self.connected || state.loading;
        let field = |label: &'static str, input: &Entity<InputState>, locked: bool| {
            v_flex().gap_1().child(div().text_sm().child(label)).child(
                Input::new(input)
                    .aria_label(label)
                    .disabled(disabled || locked),
            )
        };
        v_flex().p_4().gap_3().rounded_lg().border_1().border_color(Color::Divider.get(cx))
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(if target.is_model {format!("{} · {}",target.provider,if target.creating {tr("添加模型")} else {tr("编辑模型")})} else if target.creating {tr("添加提供商").into()} else {format!("{} · {}",tr("编辑接口"),target.provider)}))
            .child(field(if target.is_model {tr("模型 ID")} else {tr("提供商 ID")},&state.id,!target.creating))
            .child(field(tr("显示名称（可留空）"),&state.name,false))
            .when(!target.is_model,|form|form.child(field(tr("接口地址（Base URL）"),&state.url,false))
                .child(div().text_sm().child(tr("接口类型")))
                .child(h_flex().flex_wrap().gap_2().children([
                    ("",tr("继承内置")),("openai-completions","Chat Completions"),("openai-responses","Responses"),("anthropic-messages","Anthropic"),("google-generative-ai","Gemini")
                ].into_iter().map(|(api,label)|Button::new(SharedString::from(format!("pi-config-api-{api}"))).rounded_full().compact().label(label).accessibility_label(format!("{}: {label}",tr("模型接口类型"))).selected(state.api==api).toggled(state.api==api).disabled(disabled).on_click(cx.listener(move|this,_,_,cx|{this.pi_models.api=api.into();this.schedule_draft_save(cx);cx.notify();})))))
                .when(!state.api.is_empty(),|form|form.child(div().text_xs().text_color(Color::Muted.get(cx)).child(state.api.clone())))
                .when(target.creating,|form|form.child(field(tr("首个模型 ID（新接口必填，内置覆盖可留空）"),&state.first,false))))
            .when(target.is_model,|form|form
                .when(state.override_only,|form|form.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(tr("仅修改模型覆盖；继承内置表示不固定此能力。模型是否可用以会话原生目录为准。"))))
                .child(field(tr("上下文 Token 数（可留空）"),&state.context,false))
                .child(field(tr("最大输出 Token 数（可留空）"),&state.output,false))
                .when(!state.override_only,|form|form.child(h_flex().gap_3().items_center().child(div().flex_1().text_sm().child(tr("支持思考")))
                    .child(Switch::new("pi-model-reasoning").accessibility_label(tr("模型支持思考")).checked(state.reasoning.unwrap_or(false)).disabled(disabled).on_change(cx.listener(|this,value,_,cx|{this.pi_models.reasoning=Some(*value);this.schedule_draft_save(cx);cx.notify();}))))
                .child(h_flex().gap_3().items_center().child(div().flex_1().text_sm().child(tr("支持图片输入")))
                    .child(Switch::new("pi-model-vision").accessibility_label(tr("模型支持图片输入")).checked(state.vision.unwrap_or(false)).disabled(disabled).on_change(cx.listener(|this,value,_,cx|{this.pi_models.vision=Some(*value);this.schedule_draft_save(cx);cx.notify();})))))
                .when(state.override_only,|form|form.child(self.override_capability(tr("思考能力"),true,state.reasoning,disabled,cx)).child(self.override_capability(tr("图片输入"),false,state.vision,disabled,cx))))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("保存到 Pi 的 models.json，高级字段和其他提供商保持原样。仅保存配置，不验证服务端认证。")))
            .child(h_flex().gap_2().items_center().flex_wrap()
                .child(Button::new("save-pi-model-form").primary().rounded_full().label(tr("保存配置")).disabled(disabled || !state.dirty(cx)).on_click(cx.listener(|this,_,_,cx|this.save_pi_model_form(cx))))
                .child(Button::new("cancel-pi-model-form").rounded_full().label(tr("关闭表单")).disabled(state.loading).on_click(cx.listener(|this,_,window,cx|this.edit_pi_model_config(None,window,cx))))
                .when(state.dirty(cx),|row|row.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(tr("有未保存的修改")))))
    }
    fn override_capability(
        &self,
        label: &'static str,
        reasoning: bool,
        selected: Option<bool>,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .gap_2()
            .items_center()
            .flex_wrap()
            .child(div().flex_1().text_sm().child(label))
            .children(
                [
                    (None, tr("继承内置")),
                    (Some(true), tr("支持")),
                    (Some(false), tr("不支持")),
                ]
                .into_iter()
                .map(|(value, text)| {
                    Button::new(SharedString::from(format!(
                        "pi-override-{reasoning}-{}", match value { None=>"inherit",Some(true)=>"enabled",Some(false)=>"disabled" }
                    )))
                    .rounded_full()
                    .compact()
                    .label(text)
                    .accessibility_label(format!("{label}: {text}"))
                    .selected(selected == value)
                    .toggled(selected == value)
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if reasoning {
                            this.pi_models.reasoning = value;
                        } else {
                            this.pi_models.vision = value;
                        }
                        this.schedule_draft_save(cx);
                        cx.notify();
                    }))
                }),
            )
    }
}
