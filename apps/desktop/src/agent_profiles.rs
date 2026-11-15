use crate::{app::HiveApp, language::tr};
use gpui_kit::{component::{button::{Button, ButtonVariants}, input::{Input, InputState}, scroll::ScrollableElement, *}, prelude::FluentBuilder, *};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProfileRef { pub id: String, pub revision: u64 }
#[derive(Clone, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Profile { pub id: String, pub driver: String, pub label: String, pub enabled: bool, pub current_revision: u64, pub revisions: Vec<Revision> }
#[derive(Clone, Deserialize)]
pub struct Revision { pub revision: u64, pub launch: Launch }
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all="camelCase")]
pub struct Launch { #[serde(skip_serializing_if="Option::is_none")] pub command: Option<String>, pub args: Vec<String>, #[serde(skip_serializing_if="Option::is_none")] pub config_dir: Option<String> }
#[derive(Clone, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Snapshot { pub store_version: u64, pub profiles: Vec<Profile> }
pub struct Profiles {
    pub snapshot: Option<Snapshot>, pub loading: bool, pub generation: u64, pub error: Option<String>,
    pub selected: Option<String>, pub driver: &'static str, pub label: Entity<InputState>, pub command: Entity<InputState>, pub args: Entity<InputState>, pub config: Entity<InputState>, pub scroll: ScrollHandle,
}
impl Profiles {
    pub fn new(window: &mut Window, cx: &mut Context<HiveApp>) -> Self {
        Self { snapshot: None, loading:false, generation:0, error:None, selected:None, driver:"pi", label:cx.new(|cx| InputState::new(window,cx)), command:cx.new(|cx| InputState::new(window,cx)), args:cx.new(|cx| InputState::new(window,cx).placeholder("[]")), config:cx.new(|cx| InputState::new(window,cx)), scroll:ScrollHandle::new() }
    }
}
impl HiveApp {
    pub fn profile_label(&self, reference: &ProfileRef) -> String {
        let name = self.profiles.snapshot.as_ref().and_then(|snapshot| snapshot.profiles.iter().find(|p| p.id == reference.id && p.revisions.iter().any(|r| r.revision == reference.revision))).map(|p| p.label.as_str()).unwrap_or(reference.id.as_str());
        format!("{} · r{}", name, reference.revision)
    }
    pub fn load_profiles(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.profiles.loading { return; }
        let Some(runtime)=self.runtime.clone() else { return; };
        self.profiles.loading=true; self.profiles.generation+=1;
        let generation=self.profiles.generation; let connection=self.connection_generation;
        cx.spawn(async move |this,cx| {
            let result=runtime.request("agent.profiles.list",json!({})).await.and_then(|v| serde_json::from_value::<Snapshot>(v).map_err(|e| e.to_string()));
            let _=this.update(cx,|this,cx| { if this.connection_generation!=connection || this.profiles.generation!=generation { return; }
                this.profiles.loading=false; match result { Ok(snapshot)=>{this.profiles.snapshot=Some(snapshot);this.profiles.error=None},Err(e)=>this.profiles.error=Some(e.to_string()) }; cx.notify(); });
        }).detach();
    }
    fn edit_profile(&mut self, id: Option<String>, driver: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let profile=id.as_ref().and_then(|id| self.profiles.snapshot.as_ref()?.profiles.iter().find(|p| &p.id==id)).cloned();
        let launch=profile.as_ref().and_then(|p| p.revisions.iter().find(|r| r.revision==p.current_revision)).map(|r|r.launch.clone()).unwrap_or_default();
        self.profiles.selected=id; self.profiles.driver=driver; self.profiles.error=None;
        for (input,value) in [(&self.profiles.label,profile.map(|p|p.label).unwrap_or_default()),(&self.profiles.command,launch.command.unwrap_or_default()),(&self.profiles.args,serde_json::to_string(&launch.args).unwrap_or_else(|_|"[]".into())),(&self.profiles.config,launch.config_dir.unwrap_or_default())] { input.update(cx,|input,cx|input.set_value(value,window,cx)); }
        cx.notify();
    }
    fn mutate_profile(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.profiles.loading || !self.connected { return; }
        let (Some(runtime),Some(snapshot))=(self.runtime.clone(),self.profiles.snapshot.as_ref()) else { return; };
        let mut body=json!({"expectedStoreVersion":snapshot.store_version});
        if let Some(id)=&self.profiles.selected { body["id"]=json!(id); }
        if action=="create" || action=="update" {
            let args=match serde_json::from_str::<Vec<String>>(self.profiles.args.read(cx).value().as_str()) { Ok(args)=>args, Err(_)=>{self.profiles.error=Some(tr("参数必须是 JSON 字符串数组，例如 [\"--flag\", \"value\"]").into());cx.notify();return;} };
            let driver=self.profiles.driver;
            let command=self.profiles.command.read(cx).value().to_string(); let config=self.profiles.config.read(cx).value().to_string();
            let mut launch=json!({"args":args});
            if !command.is_empty() { launch["command"]=json!(command); } if !config.is_empty() { launch["configDir"]=json!(config); }
            body["label"]=json!(self.profiles.label.read(cx).value().to_string());body["launch"]=launch;
            if action=="create" { body["driver"]=json!(driver); }
        }
        self.profiles.loading=true; self.profiles.generation+=1; let generation=self.profiles.generation; let connection=self.connection_generation;
        cx.spawn(async move |this,cx| { let result=runtime.request(&format!("agent.profiles.{action}"),body).await; let _=this.update(cx,|this,cx| { if this.connection_generation!=connection || this.profiles.generation!=generation { return; } this.profiles.loading=false; match result { Ok(_)=>{this.profiles.error=None; this.load_profiles(cx)},Err(e)=>this.profiles.error=Some(e.to_string()) } cx.notify(); }); }).detach();
    }
    pub fn render_profiles(&self,cx:&mut Context<Self>) -> impl IntoElement {
        let entries=self.profiles.snapshot.as_ref().map(|s|s.profiles.clone()).unwrap_or_default();
        let selected=self.profiles.selected.as_ref().and_then(|id|entries.iter().find(|p|&p.id==id)).cloned();
        let driver=self.profiles.driver; let busy=self.profiles.loading || !self.connected;
        v_flex().id("profile-settings").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.profiles.scroll).vertical_scrollbar(&self.profiles.scroll).gap_3().p_4()
            .child(div().text_xs().child(tr("只填写非机密的路径和参数；请勿填写凭据。")))
            .child(Button::new("profiles-refresh").label(tr("刷新配置")).disabled(busy).on_click(cx.listener(|this,_,_,cx|this.load_profiles(cx))))
            .when_some(self.profiles.error.clone(),|v,error|v.child(div().text_color(crate::palette::Color::Text.get(cx)).child(error)))
            .children(entries.into_iter().enumerate().map(|(i,p)| { let id=p.id.clone();let driver=match p.driver.as_str(){"codex"=>"codex","pi-rpc"=>"pi-rpc","claude"=>"claude",_=>"pi"};
                let history=p.revisions.iter().map(|r|format!("{}: {}",r.revision,serde_json::to_string(&r.launch).unwrap_or_default())).collect::<Vec<_>>().join(" · ");
                v_flex().gap_1().child(Button::new(("profile-edit",i)).label(format!("{} · {} · r{}{}",p.label,p.driver,p.current_revision,if p.enabled {""} else {" · disabled"})).on_click(cx.listener(move |this,_,window,cx|this.edit_profile(Some(id.clone()),driver,window,cx)))).child(div().text_xs().child(history))
            }))
            .child(div().child(tr("新建配置")))
            .child(h_flex().gap_1().children([("pi","Pi"),("codex","Codex"),("pi-rpc","外部 Pi"),("claude","Claude")].into_iter().enumerate().map(|(i,(driver,label))|Button::new(("new-profile",i)).label(label).on_click(cx.listener(move |this,_,window,cx|this.edit_profile(None,driver,window,cx))))))
            .child(div().child(if selected.is_some(){tr("编辑配置")}else{tr("新建配置")}))
            .child(div().child(tr("名称"))).child(Input::new(&self.profiles.label))
            .when(driver!="pi",|v|v.child(div().child(tr("可执行文件"))).child(Input::new(&self.profiles.command)))
            .child(div().child(tr("配置目录（绝对路径，可留空）"))).child(Input::new(&self.profiles.config))
            .when(driver!="pi" && driver!="claude",|v|v.child(div().child(tr("参数（JSON 字符串数组；每项是一个完整参数）"))).child(Input::new(&self.profiles.args)))
            .when(driver=="claude",|v|v.child(div().child(tr("Claude 不支持启动参数"))))
            .child(h_flex().gap_2().child(Button::new("save-profile").primary().label(tr("保存")).disabled(busy || self.profiles.snapshot.is_none()).on_click(cx.listener(|this,_,_,cx|{let action=if this.profiles.selected.is_some(){"update"}else{"create"};this.mutate_profile(action,cx)})))
            .when_some(selected.as_ref(),|v,p|{let enabled=p.enabled;v.child(Button::new("toggle-profile").label(if enabled {tr("停用")}else{tr("启用")}).disabled(busy).on_click(cx.listener(move |this,_,_,cx|this.mutate_profile(if enabled {"disable"}else{"enable"},cx)))) }))
    }
}
