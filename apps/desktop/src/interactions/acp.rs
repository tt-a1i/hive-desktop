use super::*;
use crate::{palette::Color, protocol::{Backend, ProfileRef}};
use gpui_kit::{component::{button::Button, *}, prelude::FluentBuilder};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Delivery { delivery_id: String, text: String, origin: String, notice: String, dispatch_id: Option<String>, version: Option<String>, disposition: Option<String>, decided_at: Option<String>, #[serde(default)] attachments: Vec<crate::protocol::AttachmentRef> }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Preview { session_id: String, profile: ProfileRef, deliveries: Vec<Delivery>, blocked_reason: Option<String>, native_session_id: Option<String>, #[serde(default)] resolved: Vec<Delivery> }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Discard { session_id: String, delivery_id: String, #[serde(default)] discarded: bool, #[serde(default)] retry_approved: bool, native_outcome: String, receipt: Option<Delivery> }
#[derive(Clone, Copy)]
enum Decision {
    Discard,
    Retry,
}
#[derive(Default)]
pub(super) struct Uncertain {
    session: Option<String>, profile: Option<ProfileRef>, generation: u64, connection: u64, open: u64,
    loading: bool, deliveries: Vec<Delivery>, resolved: Vec<Delivery>, native: Option<String>, blocked: Option<String>, error: Option<String>, confirm: Option<String>, retry: bool, notice: Option<Decision>,
}
impl Uncertain {
    fn cancel_confirmation(&mut self, id: &str) {
        if !self.loading && self.confirm.as_deref() == Some(id) {
            self.confirm = None;
        }
    }
}
impl HiveApp {
    fn acp_scope(&self) -> Option<(String, ProfileRef)> {
        let session=self.session.as_ref()?;
        if !matches!(session.backend,Backend::Acp | Backend::Codex) { return None; }
        Some((session.id.clone(),session.profile.clone()?))
    }
    fn load_uncertain_acp(&mut self,cx:&mut Context<Self>) {
        if !self.connected { return; }
        let (Some(runtime),Some((session,profile)))=(self.runtime.clone(),self.acp_scope()) else { return; };
        if self.interactions.uncertain.loading && self.interactions.uncertain.session.as_ref()==Some(&session) && self.interactions.uncertain.open==self.open_request && self.interactions.uncertain.connection==self.connection_generation { return; }
        let state=&mut self.interactions.uncertain;
        if state.connection!=self.connection_generation || state.open!=self.open_request || state.session.as_ref()!=Some(&session) || state.profile.as_ref()!=Some(&profile) { state.deliveries.clear(); state.resolved.clear(); state.native=None; state.blocked=None; }
        state.session=Some(session.clone()); state.profile=Some(profile.clone());state.notice=None;state.open=self.open_request;state.connection=self.connection_generation;state.loading=true;state.confirm=None;state.error=None;state.generation+=1;
        let generation=state.generation;let connection=self.connection_generation;let open=self.open_request;
        cx.spawn(async move |this,cx| {
            let result=runtime.request("chat.queue.uncertainDeliveries",json!({"sessionId":session})).await.and_then(|v|serde_json::from_value::<Preview>(v).map_err(|e|e.to_string()));
            let _=this.update(cx,|this,cx| {
                if this.interactions.uncertain.generation!=generation { return; }
                this.interactions.uncertain.loading=false;
                if this.connection_generation!=connection || this.open_request!=open || this.acp_scope()!=Some((session.clone(),profile.clone())) { return; }
                let state=&mut this.interactions.uncertain;
                match result { Ok(preview) if preview.session_id==session && preview.profile==profile => {state.deliveries=preview.deliveries;state.resolved=preview.resolved;state.native=preview.native_session_id;state.blocked=preview.blocked_reason;state.error=None;}, Ok(_)=>state.error=Some("Queue preview identity mismatch".into()),Err(error)=>state.error=Some(error) };cx.notify();
            });
        }).detach();cx.notify();
    }
    fn discard_uncertain_acp(&mut self,id:String,text:String,version:Option<String>,retry:bool,cx:&mut Context<Self>) {
        if !self.connected || self.interactions.uncertain.loading {return;}
        let (Some(runtime),Some((session,profile)))=(self.runtime.clone(),self.acp_scope()) else {return;};
        let state=&mut self.interactions.uncertain;
        if state.connection!=self.connection_generation || state.open!=self.open_request || state.session.as_ref()!=Some(&session) || state.profile.as_ref()!=Some(&profile) || state.confirm.as_ref()!=Some(&id) || state.retry!=retry || !state.deliveries.iter().any(|d|d.delivery_id==id && d.text==text && d.version==version) {return;}
        let Some(native)=state.native.clone() else{return;};
        state.loading=true;state.error=None;state.generation+=1;let generation=state.generation;let connection=self.connection_generation;let open=self.open_request;
        cx.spawn(async move |this,cx| {
            let result=runtime.request(if retry {"chat.queue.retryUncertainDelivery"} else {"chat.queue.discardUncertainDelivery"},json!({"sessionId":session,"deliveryId":id,"expectedText":text,"expectedProfile":profile,"expectedNativeSessionId":native,"expectedVersion":version})).await.and_then(|v|serde_json::from_value::<Discard>(v).map_err(|e|e.to_string()));
            let _=this.update(cx,|this,cx| {
                if this.interactions.uncertain.generation!=generation {return;}
                this.interactions.uncertain.loading=false;
                if this.connection_generation!=connection || this.open_request!=open || this.acp_scope()!=Some((session.clone(),profile.clone())) {return;}
                let state=&mut this.interactions.uncertain;
                match result { Ok(result) if result.session_id==session && result.delivery_id==id && (if retry {result.retry_approved} else {result.discarded}) && result.native_outcome=="unknown" => {state.deliveries.retain(|d|d.delivery_id!=id || d.text!=text);if let Some(receipt)=result.receipt {state.resolved.retain(|d|d.delivery_id!=receipt.delivery_id||d.version!=receipt.version);state.resolved.push(receipt);}state.confirm=None;state.notice=Some(if retry {Decision::Retry}else{Decision::Discard});}, Ok(_)=>state.error=Some(if english(){"Queue decision response identity mismatch"}else{"队列处理结果的身份不匹配"}.into()),Err(error)=>state.error=Some(error) };cx.notify();
            });
        }).detach();cx.notify();
    }
    pub(super) fn render_uncertain_acp(&self,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let scope=self.acp_scope();let state=&self.interactions.uncertain;
        let codex=self.session.as_ref().is_some_and(|session|matches!(session.backend,Backend::Codex));
        let decided_notice=state.notice.map(|decision|match (decision,english()){
            (Decision::Retry,true)=>"Resend approved. The earlier outcome is unknown. Select Resume processing to send the original input again.",
            (Decision::Retry,false)=>"已批准重发。之前的执行结果仍未知。点击「继续处理」可重发原消息。",
            (Decision::Discard,true)=>"Queued wait discarded; native outcome remains unknown.",
            (Decision::Discard,false)=>"已放弃等待投递；原生执行结果仍未知。",
        });
        let discard_confirmation_notice=if english(){"The agent may already have acted. This only discards the queued wait; native work and Hive audit remain."}else{"Agent 可能已经执行。此操作只放弃这次等待投递，不会撤销可能发生的原生操作，并保留 Hive 审计。"};
        let confirmation_notice=if state.retry {if english(){"The agent may already have acted. Resending this input and its attachments may repeat work. After approving, select Resume processing to send it."}else{"Agent 可能已经执行。重发原消息和附件可能导致重复执行。批准后，点击「继续处理」才会发送。"}}else{discard_confirmation_notice};
        let empty_notice=match (codex,english()) {
            (true,true)=>"There are no pending deliveries to review.",
            (true,false)=>"没有待核对的投递。",
            (false,true)=>"This preview has no queued wait available to discard. Native outcomes may still be unknown.",
            (false,false)=>"当前预览没有可放弃的等待投递；原生执行结果仍可能未知。",
        };
        let current=state.connection==self.connection_generation && state.open==self.open_request && scope.as_ref().is_some_and(|(s,p)|state.session.as_ref()==Some(s) && state.profile.as_ref()==Some(p));
        v_flex().gap_2().p_3().border_1().border_color(Color::Divider.get(cx))
            .child(Button::new("acp-uncertain-refresh").outline().small().label(if english(){"Review uncertain deliveries"}else if codex{"核对结果未确认的投递"}else{"查看未确认的待投递"}).disabled(!self.connected || (state.loading && current) || scope.is_none()).on_click(cx.listener(|this,_,_,cx|this.load_uncertain_acp(cx))))
            .when(current,|view|view
                .when_some(state.native.clone(),|v,native|v.child(div().text_xs().child(format!("{}: {} · {}",if english(){"Original session"}else{"原生会话"},native,state.profile.as_ref().map(|p|format!("{}@{}",p.id,p.revision)).unwrap_or_default()))))
                .when_some(decided_notice,|v,notice|v.child(div().id("acp-discard-status").role(Role::Status).aria_label(notice).child(notice)))
                .when(!state.loading && state.error.is_none() && state.blocked.is_none() && state.deliveries.is_empty() && state.resolved.is_empty() && state.notice.is_none(),|v|v.child(div().id("acp-uncertain-empty").role(Role::Status).aria_label(empty_notice).child(empty_notice)))
                .when_some(state.blocked.clone(),|v,text|{
                    let text=if codex&&!english()&&text=="Stop and finish the current turn before reviewing uncertain input"{"请先停止当前回复，等它结束后再核对投递。".into()}else{text};
                    v.child(div().id("acp-uncertain-blocked").role(Role::Status).aria_label(text.clone()).child(text))
                })
                .when_some(state.error.clone(),|v,text|v.child(div().id("acp-uncertain-error").role(Role::Status).aria_label(text.clone()).child(text)))
                .children(state.deliveries.iter().enumerate().map(|(index,delivery)| {
                    let id=delivery.delivery_id.clone();let text=delivery.text.clone();let confirm=state.confirm.as_ref()==Some(&id);
                    let notice=if codex {if english(){"The agent may already have received this input. Review native history before choosing how to proceed."}else{"Agent 可能已收到此消息。请先核对原生会话历史，再选择如何处理。"}}else{delivery.notice.as_str()};
                    let origin=if delivery.origin=="user"&&!english(){"用户"}else{delivery.origin.as_str()};
                    let source=format!("{}{} · {}{}",origin,delivery.dispatch_id.as_ref().map(|id|format!(" · {id}")).unwrap_or_default(),notice,if delivery.attachments.is_empty(){String::new()}else{format!(" · {}: {}",if english(){"Attachments"}else{"附件"},delivery.attachments.iter().map(|a|a.name.as_str()).collect::<Vec<_>>().join(", "))});
                    v_flex().gap_1().child(div().id(("acp-uncertain-source",index)).role(Role::Label).aria_label(source.clone()).aria_value(source.clone()).text_xs().child(source)).child(div().id(("acp-uncertain-text",index)).role(Role::Label).aria_label(text.clone()).aria_value(text.clone()).text_sm().child(text.clone()))
                        .when(!confirm,|v| {let id=id.clone();v.child(Button::new(("acp-discard-ask",index)).label(if english(){"Discard this queued wait…"}else{"放弃这次等待投递…"}).disabled(state.loading || state.error.is_some()).on_click(cx.listener(move |this,_,_,cx|{this.interactions.uncertain.retry=false;this.interactions.uncertain.confirm=Some(id.clone());cx.notify();})))})
                        .when(!confirm && codex,|v| {let id=id.clone();v.child(Button::new(("codex-retry-ask",index)).label(if english(){"Review resend…"}else{"核对并选择重发…"}).disabled(state.loading || state.error.is_some()).on_click(cx.listener(move |this,_,_,cx|{this.interactions.uncertain.retry=true;this.interactions.uncertain.confirm=Some(id.clone());cx.notify();})))})
                        .when(confirm,|v| {let id=id.clone();let text=text.clone();let version=delivery.version.clone();let cancel_id=id.clone();let retry=state.retry;v.child(div().id(("acp-uncertain-confirmation",index)).role(Role::Label).aria_label(confirmation_notice).aria_value(confirmation_notice).text_xs().child(confirmation_notice)).child(Button::new(("acp-discard-cancel",index)).label(if retry {if english(){"Cancel resend"}else{"取消重发"}}else{if english(){"Cancel discard"}else{"取消放弃"}}).disabled(state.loading).on_click(cx.listener(move |this,_,_,cx|{this.interactions.uncertain.cancel_confirmation(&cancel_id);cx.notify();}))).child(Button::new(("acp-discard-confirm",index)).label(if retry {if english(){"Approve resend (may duplicate)"}else{"确认允许重发（可能重复）"}}else{if english(){"Confirm discard queued wait"}else{"确认放弃这次等待投递"}}).disabled(state.loading || state.error.is_some() || !self.connected).on_click(cx.listener(move |this,_,_,cx|this.discard_uncertain_acp(id.clone(),text.clone(),version.clone(),retry,cx))))})
                }))
                .children(state.resolved.iter().enumerate().map(|(index,delivery)|{
                    let notice=match (delivery.disposition.as_deref(),delivery.decided_at.as_deref(),english()) {
                        (Some("retry-approved"),Some(at),false)=>format!("已批准重发。之前的执行结果未知。{at} · {}",delivery.delivery_id),
                        (Some("discarded-local-wait"),Some(at),false)=>format!("已放弃等待投递。之前的执行结果未知。{at} · {}",delivery.delivery_id),
                        _=>delivery.notice.clone(),
                    };
                    v_flex().gap_1().child(div().id(("local-wait-receipt",index)).role(Role::Status).aria_label(notice.clone()).text_xs().child(notice)).child(div().text_sm().child(delivery.text.clone()))
                }))
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Preview, Discard, Uncertain, Decision};
    use serde_json::json;
    #[test]
    fn cancel_only_clears_matching_idle_confirmation() {
        let mut state = Uncertain { confirm: Some("delivery-a".into()), notice: Some(Decision::Discard), ..Uncertain::default() };
        state.cancel_confirmation("delivery-b");
        assert_eq!(state.confirm.as_deref(), Some("delivery-a"));
        state.loading = true;
        state.cancel_confirmation("delivery-a");
        assert_eq!(state.confirm.as_deref(), Some("delivery-a"));
        state.loading = false;
        state.cancel_confirmation("delivery-a");
        assert_eq!(state.confirm, None);
        assert!(matches!(state.notice, Some(Decision::Discard)));
    }
    #[test]
    fn preview_and_discard_keep_identity_and_unknown_outcome() {
        let preview: Preview = serde_json::from_value(json!({"sessionId":"s","profile":{"id":"p","revision":2},"deliveries":[{"deliveryId":"d","text":"original","origin":"user","notice":"may have run"}]})).unwrap();
        assert_eq!(preview.profile.revision,2);assert_eq!(preview.deliveries[0].text,"original");
        let reply: Discard=serde_json::from_value(json!({"sessionId":"s","deliveryId":"d","discarded":true,"alreadyDiscarded":false,"nativeOutcome":"unknown"})).unwrap();
        assert_eq!(reply.native_outcome,"unknown");assert_eq!(reply.delivery_id,"d");
        let retry: Discard=serde_json::from_value(json!({"sessionId":"s","deliveryId":"d","retryApproved":true,"alreadyApproved":false,"nativeOutcome":"unknown"})).unwrap();
        assert!(retry.retry_approved);assert!(!retry.discarded);assert_eq!(retry.native_outcome,"unknown");
        let codex:Preview=serde_json::from_value(json!({"sessionId":"s","nativeSessionId":"native-s","profile":{"id":"p","revision":2},"deliveries":[],"resolved":[{"deliveryId":"d","text":"original","origin":"user","notice":"Local wait discarded; native outcome unknown","version":"review-version","attachments":[]}]})).unwrap();
        assert_eq!(codex.native_session_id.as_deref(),Some("native-s"));assert_eq!(codex.resolved[0].delivery_id,"d");assert_eq!(codex.resolved[0].version.as_deref(),Some("review-version"));
    }
}
