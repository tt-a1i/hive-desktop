use super::*;
use crate::{palette::Color, protocol::{Backend, ProfileRef}};
use gpui_kit::{component::{button::Button, *}, prelude::FluentBuilder};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Delivery { delivery_id: String, text: String, origin: String, notice: String, dispatch_id: Option<String> }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Preview { session_id: String, profile: ProfileRef, deliveries: Vec<Delivery>, blocked_reason: Option<String> }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Discard { session_id: String, delivery_id: String, discarded: bool, native_outcome: String }
#[derive(Default)]
pub(super) struct Uncertain {
    session: Option<String>, profile: Option<ProfileRef>, generation: u64, connection: u64, open: u64,
    loading: bool, deliveries: Vec<Delivery>, blocked: Option<String>, error: Option<String>, confirm: Option<String>, notice: bool,
}
impl HiveApp {
    fn acp_scope(&self) -> Option<(String, ProfileRef)> {
        let session=self.session.as_ref()?;
        if !matches!(session.backend,Backend::Acp) { return None; }
        Some((session.id.clone(),session.profile.clone()?))
    }
    fn load_uncertain_acp(&mut self,cx:&mut Context<Self>) {
        if !self.connected { return; }
        let (Some(runtime),Some((session,profile)))=(self.runtime.clone(),self.acp_scope()) else { return; };
        if self.interactions.uncertain.loading && self.interactions.uncertain.session.as_ref()==Some(&session) && self.interactions.uncertain.open==self.open_request && self.interactions.uncertain.connection==self.connection_generation { return; }
        let state=&mut self.interactions.uncertain;
        if state.connection!=self.connection_generation || state.open!=self.open_request || state.session.as_ref()!=Some(&session) || state.profile.as_ref()!=Some(&profile) { state.deliveries.clear(); state.blocked=None; }
        state.session=Some(session.clone()); state.profile=Some(profile.clone());state.notice=false;state.open=self.open_request;state.connection=self.connection_generation;state.loading=true;state.confirm=None;state.error=None;state.generation+=1;
        let generation=state.generation;let connection=self.connection_generation;let open=self.open_request;
        cx.spawn(async move |this,cx| {
            let result=runtime.request("chat.acp.uncertainDeliveries",json!({"sessionId":session})).await.and_then(|v|serde_json::from_value::<Preview>(v).map_err(|e|e.to_string()));
            let _=this.update(cx,|this,cx| {
                if this.interactions.uncertain.generation!=generation { return; }
                this.interactions.uncertain.loading=false;
                if this.connection_generation!=connection || this.open_request!=open || this.acp_scope()!=Some((session.clone(),profile.clone())) { return; }
                let state=&mut this.interactions.uncertain;
                match result { Ok(preview) if preview.session_id==session && preview.profile==profile => {state.deliveries=preview.deliveries;state.blocked=preview.blocked_reason;state.error=None;}, Ok(_)=>state.error=Some("ACP preview identity mismatch".into()),Err(error)=>state.error=Some(error) };cx.notify();
            });
        }).detach();cx.notify();
    }
    fn discard_uncertain_acp(&mut self,id:String,text:String,cx:&mut Context<Self>) {
        if !self.connected || self.interactions.uncertain.loading {return;}
        let (Some(runtime),Some((session,profile)))=(self.runtime.clone(),self.acp_scope()) else {return;};
        let state=&mut self.interactions.uncertain;
        if state.connection!=self.connection_generation || state.open!=self.open_request || state.session.as_ref()!=Some(&session) || state.profile.as_ref()!=Some(&profile) || state.confirm.as_ref()!=Some(&id) || !state.deliveries.iter().any(|d|d.delivery_id==id && d.text==text) {return;}
        state.loading=true;state.error=None;state.generation+=1;let generation=state.generation;let connection=self.connection_generation;let open=self.open_request;
        cx.spawn(async move |this,cx| {
            let result=runtime.request("chat.acp.discardUncertainDelivery",json!({"sessionId":session,"deliveryId":id,"expectedText":text})).await.and_then(|v|serde_json::from_value::<Discard>(v).map_err(|e|e.to_string()));
            let _=this.update(cx,|this,cx| {
                if this.interactions.uncertain.generation!=generation {return;}
                this.interactions.uncertain.loading=false;
                if this.connection_generation!=connection || this.open_request!=open || this.acp_scope()!=Some((session.clone(),profile.clone())) {return;}
                let state=&mut this.interactions.uncertain;
                match result { Ok(result) if result.session_id==session && result.delivery_id==id && result.discarded && result.native_outcome=="unknown" => {state.deliveries.retain(|d|d.delivery_id!=id || d.text!=text);state.confirm=None;state.notice=true;}, Ok(_)=>state.error=Some("ACP discard response identity mismatch".into()),Err(error)=>state.error=Some(error) };cx.notify();
            });
        }).detach();cx.notify();
    }
    pub(super) fn render_uncertain_acp(&self,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let scope=self.acp_scope();let state=&self.interactions.uncertain;
        let discarded_notice=if english(){"Queued wait discarded; native outcome remains unknown."}else{"已放弃等待投递；原生执行结果仍未知。"};
        let confirmation_notice=if english(){"The agent may already have acted. This only discards the queued wait; native work and Hive audit remain."}else{"Agent 可能已经执行。此操作只放弃这次等待投递，不会撤销可能发生的原生操作，并保留 Hive 审计。"};
        let empty_notice=if english(){"This preview has no queued wait available to discard. Native outcomes may still be unknown."}else{"当前预览没有可放弃的等待投递；原生执行结果仍可能未知。"};
        let current=state.connection==self.connection_generation && state.open==self.open_request && scope.as_ref().is_some_and(|(s,p)|state.session.as_ref()==Some(s) && state.profile.as_ref()==Some(p));
        v_flex().gap_2().p_3().border_1().border_color(Color::Divider.get(cx))
            .child(Button::new("acp-uncertain-refresh").outline().small().label(if english(){"Review uncertain deliveries"}else{"查看未确认的待投递"}).disabled(!self.connected || (state.loading && current) || scope.is_none()).on_click(cx.listener(|this,_,_,cx|this.load_uncertain_acp(cx))))
            .when(current,|view|view
                .when(state.notice,|v|v.child(div().id("acp-discard-status").role(Role::Status).aria_label(discarded_notice).child(discarded_notice)))
                .when(!state.loading && state.error.is_none() && state.blocked.is_none() && state.deliveries.is_empty() && !state.notice,|v|v.child(div().id("acp-uncertain-empty").role(Role::Status).aria_label(empty_notice).child(empty_notice)))
                .when_some(state.blocked.clone(),|v,text|v.child(div().id("acp-uncertain-blocked").role(Role::Status).aria_label(text.clone()).child(text)))
                .when_some(state.error.clone(),|v,text|v.child(div().id("acp-uncertain-error").role(Role::Status).aria_label(text.clone()).child(text)))
                .children(state.deliveries.iter().enumerate().map(|(index,delivery)| {
                    let id=delivery.delivery_id.clone();let text=delivery.text.clone();let confirm=state.confirm.as_ref()==Some(&id);
                    let source=format!("{}{} · {}",delivery.origin,delivery.dispatch_id.as_ref().map(|id|format!(" · {id}")).unwrap_or_default(),delivery.notice);
                    v_flex().gap_1().child(div().id(("acp-uncertain-source",index)).role(Role::Label).aria_label(source.clone()).text_xs().child(source)).child(div().id(("acp-uncertain-text",index)).role(Role::Label).aria_label(text.clone()).text_sm().child(text.clone()))
                        .when(!confirm,|v| {let id=id.clone();v.child(Button::new(("acp-discard-ask",index)).label(if english(){"Discard this queued wait…"}else{"放弃这次等待投递…"}).disabled(state.loading || state.error.is_some()).on_click(cx.listener(move |this,_,_,cx|{this.interactions.uncertain.confirm=Some(id.clone());cx.notify();})))})
                        .when(confirm,|v| {let id=id.clone();let text=text.clone();v.child(div().id(("acp-uncertain-confirmation",index)).role(Role::Label).aria_label(confirmation_notice).text_xs().child(confirmation_notice)).child(Button::new(("acp-discard-confirm",index)).label(if english(){"Confirm discard queued wait"}else{"确认放弃这次等待投递"}).disabled(state.loading || state.error.is_some() || !self.connected).on_click(cx.listener(move |this,_,_,cx|this.discard_uncertain_acp(id.clone(),text.clone(),cx))))})
                }))
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Preview, Discard};
    use serde_json::json;
    #[test]
    fn preview_and_discard_keep_identity_and_unknown_outcome() {
        let preview: Preview = serde_json::from_value(json!({"sessionId":"s","profile":{"id":"p","revision":2},"deliveries":[{"deliveryId":"d","text":"original","origin":"user","notice":"may have run"}]})).unwrap();
        assert_eq!(preview.profile.revision,2);assert_eq!(preview.deliveries[0].text,"original");
        let reply: Discard=serde_json::from_value(json!({"sessionId":"s","deliveryId":"d","discarded":true,"alreadyDiscarded":false,"nativeOutcome":"unknown"})).unwrap();
        assert_eq!(reply.native_outcome,"unknown");assert_eq!(reply.delivery_id,"d");
    }
}
