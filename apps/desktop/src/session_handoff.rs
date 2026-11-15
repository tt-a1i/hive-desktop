use crate::{
    app::HiveApp,
    language::english,
    palette::Color,
    protocol::{Backend, ProfileRef, Session},
    transport::Runtime,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::{Value, json};
use std::sync::Arc;

pub(crate) fn historical_item_id(message: &Value) -> Option<String> {
    if !matches!(message["role"].as_str(), Some("user" | "assistant")) {
        return None;
    }
    message["handoffItemId"]
        .as_str()
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}
fn text(zh: &'static str, en: &'static str) -> &'static str {
    if english() { en } else { zh }
}
struct HandoffDialog {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    workspace: String,
    session: String,
    connection: u64,
    open_request: u64,
    through: String,
    starts: Vec<(String, String)>,
    from: Option<String>,
    profiles: Vec<crate::agent_profiles::Profile>,
    target: Option<(String, ProfileRef)>,
    checkpoint: Option<String>,
    checkpoints: Vec<Value>,
    eligibility: Option<Value>,
    preview: Option<Value>,
    operation: Option<Value>,
    operations: Vec<Value>,
    exclusions: Vec<String>,
    request_id: Option<String>,
    busy: bool,
    generation: u64,
    error: Option<String>,
}
impl HiveApp {
    pub(crate) fn open_session_handoff(
        &mut self,
        through: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.closing || window.has_active_dialog(cx) {
            return;
        }
        let (Some(runtime), Some(session)) = (self.runtime.clone(), self.session.as_ref()) else {
            return;
        };
        let starts = self
            .messages
            .iter()
            .filter_map(|m| {
                m.handoff_item_id
                    .as_ref()
                    .map(|id| (id.clone(), m.text.chars().take(100).collect::<String>()))
            })
            .take_while_inclusive(|(id, _)| id != &through);
        if !starts.iter().any(|(id, _)| id == &through) {
            return;
        }
        let profiles = self
            .profiles
            .snapshot
            .as_ref()
            .map(|s| {
                s.profiles
                    .iter()
                    .filter(|p| {
                        p.enabled
                            && matches!(
                                p.driver.as_str(),
                                "pi" | "pi-rpc" | "codex" | "claude" | "acp"
                            )
                            && p.revisions.iter().any(|r| r.revision == p.current_revision)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let workspace = session.workspace_id.clone();
        let session = session.id.clone();
        let connection = self.connection_generation;
        let open_request = self.open_request;
        let app = cx.weak_entity();
        let editor = cx.new(|cx: &mut Context<HandoffDialog>| {
            let editor = HandoffDialog {
                app,
                runtime,
                workspace,
                session,
                connection,
                open_request,
                through,
                starts,
                from: None,
                profiles,
                target: None,
                checkpoint: None,
                checkpoints: vec![],
                eligibility: None,
                preview: None,
                operation: None,
                operations: vec![],
                exclusions: vec![],
                request_id: None,
                busy: false,
                generation: 0,
                error: None,
            };
            let weak = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = weak.update(cx, |this, cx| this.load(cx));
            });
            editor
        });
        let cancel = editor.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let busy = editor.read(cx).busy;
            let cancel = cancel.clone();
            dialog
                .title(text("跨后端文本接续", "Continue with another agent"))
                .w(px(680.))
                .overlay_closable(false)
                .keyboard(!busy)
                .close_button(!busy)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !cancel.read(cx).busy)
                .child(editor.clone())
        });
    }
}
// Include the terminating item without assuming that user-visible rows map to native positions.
trait TakeThrough: Iterator + Sized {
    fn take_while_inclusive<P>(self, predicate: P) -> Vec<Self::Item>
    where
        P: Fn(&Self::Item) -> bool,
    {
        let mut out = Vec::new();
        for item in self {
            let more = predicate(&item);
            out.push(item);
            if !more {
                break;
            }
        }
        out
    }
}
impl<I: Iterator> TakeThrough for I {}
impl HandoffDialog {
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|app| {
            let a = app.read(cx);
            a.connected
                && !a.closing
                && a.connection_generation == self.connection
                && a.open_request == self.open_request
                && a.workspace.as_deref() == Some(&self.workspace)
                && a.session.as_ref().is_some_and(|s| s.id == self.session)
                && a.runtime
                    .as_ref()
                    .is_some_and(|r| Arc::ptr_eq(r, &self.runtime))
        })
    }
    fn load(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.current(cx) {
            return;
        }
        self.busy = true;
        self.generation += 1;
        let generation = self.generation;
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let caps = runtime
                .request(
                    "chat.handoff.capabilities",
                    json!({"workspaceId":workspace,"sourceSessionId":session}),
                )
                .await;
            let checkpoints = runtime
                .request(
                    "checkpoint.list",
                    json!({"workspaceId":workspace,"sessionId":session}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                if this.current(cx) {
                    match caps {
                        Ok(v) => this.eligibility = Some(v),
                        Err(e) => this.error = Some(e),
                    };
                    match checkpoints {
                        Ok(v) => {
                            this.checkpoints =
                                v["checkpoints"].as_array().cloned().unwrap_or_default()
                        }
                        Err(e) => this.error = Some(e),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn call(&mut self, method: &'static str, params: Value, cx: &mut Context<Self>) {
        if self.busy || !self.current(cx) {
            return;
        }
        self.busy = true;
        self.error = None;
        self.generation += 1;
        let generation = self.generation;
        let runtime = self.runtime.clone();
        let requested = params.clone();
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                if this.current(cx) {
                    match result {
                        Err(e) => this.error = Some(e),
                        Ok(v) => match method {
                            "chat.handoff.preview" => {
                                if v["input"] == requested
                                    && v["snapshot"]["workspaceId"] == this.workspace
                                    && v["snapshot"]["sourceSessionId"] == this.session
                                    && v["snapshot"]["sha256"] == v["snapshotHash"]
                                    && v["snapshot"]["history"]["omissionsHash"]
                                        == v["omissionsHash"]
                                {
                                    this.preview = Some(v)
                                } else {
                                    this.error = Some("Preview identity mismatch".into());
                                }
                            }
                            "chat.handoff.apply" | "chat.handoff.get" | "chat.handoff.recover" => {
                                let valid = v["workspaceId"] == this.workspace
                                    && v["sourceSessionId"] == this.session
                                    && if method == "chat.handoff.apply" {
                                        v["requestId"] == requested["requestId"]
                                            && v["snapshotHash"] == requested["snapshotHash"]
                                            && this.target.as_ref().is_some_and(
                                                |(driver, profile)| {
                                                    v["target"]["driver"] == *driver
                                                        && v["target"]["profile"] == json!(profile)
                                                },
                                            )
                                    } else {
                                        v["operationId"] == requested["operationId"]
                                    };
                                if valid {
                                    this.operation = Some(v);
                                } else {
                                    this.error = Some("Handoff operation identity mismatch".into());
                                }
                            }
                            "chat.handoff.list" => {
                                this.operations =
                                    v["operations"].as_array().cloned().unwrap_or_default()
                            }
                            _ => {}
                        },
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn preview(&mut self, cx: &mut Context<Self>) {
        let Some((driver, profile)) = self.target.clone() else {
            return;
        };
        let mut selection = json!({"throughItemId":self.through});
        if let Some(from) = &self.from {
            selection["fromItemId"] = json!(from);
        }
        if !self.exclusions.is_empty() {
            selection["excludeItemIds"] = json!(self.exclusions);
        }
        let code = self
            .checkpoint
            .as_ref()
            .map(|id| json!({"kind":"checkpoint","checkpointId":id}))
            .unwrap_or(json!({"kind":"current"}));
        self.preview = None;
        self.operation = None;
        self.request_id = None;
        self.call("chat.handoff.preview",json!({"workspaceId":self.workspace,"sourceSessionId":self.session,"target":{"driver":driver,"profile":profile},"selection":selection,"code":code,"imagePolicy":"references-only"}),cx);
    }
    fn apply(&mut self, cx: &mut Context<Self>) {
        let Some(preview) = self.preview.as_ref().filter(|p| p["eligible"] == true) else {
            return;
        };
        let (Some(id), Some(hash), Some(omissions)) = (
            preview["previewId"].as_str(),
            preview["snapshotHash"].as_str(),
            preview["omissionsHash"].as_str(),
        ) else {
            return;
        };
        let params = json!({"workspaceId":self.workspace,"previewId":id,"snapshotHash":hash,"acknowledgedOmissionsHash":omissions,"requestId":self.request_id.get_or_insert_with(||uuid::Uuid::new_v4().to_string())});
        self.call("chat.handoff.apply", params, cx);
    }
    fn open_child(&mut self, op: Value, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy
            || !self.current(cx)
            || op["state"] != "ready"
            || op["workspaceId"] != self.workspace
            || op["sourceSessionId"] != self.session
        {
            return;
        }
        let (Some(id), Some(driver), Some(origin), Ok(profile)) = (
            op["childSessionId"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            op["target"]["driver"].as_str().map(str::to_owned),
            op["originAgentId"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            serde_json::from_value::<ProfileRef>(op["target"]["profile"].clone()),
        ) else {
            return;
        };
        self.busy = true;
        self.generation += 1;
        let generation = self.generation;
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.list", json!({"workspaceId":workspace}))
                .await
                .and_then(|v| serde_json::from_value::<Vec<Value>>(v).map_err(|e| e.to_string()));
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                if !this.current(cx) {
                    return;
                }
                match result {
                    Ok(sessions) => {
                        let session = sessions
                            .into_iter()
                            .filter(|v| v["backend"].as_str() == Some(driver.as_str()))
                            .filter_map(|v| serde_json::from_value::<Session>(v).ok())
                            .find(|s| {
                                s.id == id
                                    && s.workspace_id == workspace
                                    && s.profile.as_ref() == Some(&profile)
                                    && s.origin_agent_id.as_deref() == Some(origin.as_str())
                                    && s.identity_state.as_deref() == Some("known")
                                    && backend_key(s.backend) == driver
                            });
                        if let Some(session) = session {
                            let _ = handle.update(cx, |_, window, cx| {
                                let _ = this.app.update(cx, |app, cx| {
                                    app.sessions.retain(|s| s.id != id);
                                    app.sessions.insert(0, session);
                                    app.show_session(id.clone(), cx);
                                    window.close_dialog(cx);
                                });
                            });
                        } else {
                            this.error = Some(
                                text("目标会话身份不匹配。", "Target session identity mismatch.")
                                    .into(),
                            );
                        }
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
fn backend_key(b: Backend) -> &'static str {
    match b {
        Backend::Pi => "pi",
        Backend::PiRpc => "pi-rpc",
        Backend::Codex => "codex",
        Backend::Claude => "claude",
        Backend::Acp => "acp",
    }
}
fn strings(v: &Value) -> String {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" · ")
}
fn blockers(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["message"].as_str().map(str::to_owned))
        .collect()
}
impl Render for HandoffDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current(cx);
        let locked = !current || self.busy || self.operation.is_some() || self.request_id.is_some();
        let profiles = self.profiles.clone();
        let starts = self.starts.clone();
        let checkpoints = self.checkpoints.clone();
        v_flex().id("handoff-dialog").gap_3().min_w_0().max_h(window.viewport_size().height*0.78).overflow_y_scroll()
            .child(div().text_sm().child(text("从选中的历史区间提取可见文本，创建独立的新会话和代码目录。不会迁移厂商原生历史或复制账号设置；创建和查看不会启动 Agent。","Create an independent conversation and code directory from visible historical text. Native history and account settings are not migrated. Creating or viewing it does not start an agent.")))
            .child(div().text_sm().child(text("首次普通文本发送才会激活目标会话；模型未指定，首次启动使用目标后端默认配置，当前未探测。","The target starts only on the first ordinary text input. No model is specified or probed; the target backend uses its own default at startup.")))
            .when(!current,|v|v.child(div().id("handoff-context-changed").role(Role::Alert).child(text("项目、会话或连接已变化，请关闭后重新打开。","The project, conversation, or connection changed. Close and reopen."))))
            .when(self.profiles.is_empty(),|v|v.child(div().text_sm().child(text("没有可用的已保存目标配置，请先刷新或启用配置。","No saved target profile is available. Refresh or enable a profile first."))))
            .when_some(self.eligibility.as_ref(),|v,c|v.children(blockers(&c["blockers"]).into_iter().map(|b|div().text_sm().text_color(Color::Error.get(cx)).child(b))))
            .child(div().text_sm().child(text("来源区间：从历史开头，或选择一个已保存的用户或助手消息作为起点；终点为刚才选中的消息。","Source range: begin at the start, or choose a saved user or assistant message; the end is the message selected earlier.")))
            .child(h_flex().gap_1().flex_wrap().child(Button::new("handoff-from-start").outline().small().label(text("历史开头","Start of history")).selected(self.from.is_none()).disabled(locked).on_click(cx.listener(|this,_,_,cx|{this.from=None;this.preview=None;cx.notify();})))
                .children(starts.into_iter().enumerate().map(|(i,(id,label))|{let selected=self.from.as_ref()==Some(&id);Button::new(("handoff-from",i)).outline().small().label(if label.is_empty(){id.clone()}else{label}).selected(selected).disabled(locked).on_click(cx.listener(move |this,_,_,cx|{this.from=Some(id.clone());this.preview=None;cx.notify();}))})))
            .child(div().text_sm().child(text("目标配置（必须选择已启用的保存版本）","Target profile (select an enabled saved revision)")))
            .child(h_flex().gap_1().flex_wrap().children(profiles.into_iter().enumerate().map(|(i,p)|{let reference=ProfileRef{id:p.id.clone(),revision:p.current_revision};let target=(p.driver.clone(),reference);let selected=self.target.as_ref()==Some(&target);Button::new(("handoff-target",i)).outline().small().label(format!("{} · {} · r{}",p.label,p.driver,p.current_revision)).selected(selected).disabled(locked).on_click(cx.listener(move |this,_,_,cx|{this.target=Some(target.clone());this.preview=None;cx.notify();}))})))
            .child(div().text_sm().child(text("代码关系：当前代码是现在的工作区状态，不代表历史消息发生时的代码。","Code: current code is the workspace now; it does not represent code at the historical message.")))
            .child(h_flex().gap_1().flex_wrap().child(Button::new("handoff-code-current").outline().small().label(text("当前代码","Current code")).selected(self.checkpoint.is_none()).disabled(locked).on_click(cx.listener(|this,_,_,cx|{this.checkpoint=None;this.preview=None;cx.notify();})))
                .children(checkpoints.into_iter().enumerate().filter_map(|(i,c)|{let id=c["checkpointId"].as_str()?.to_owned();let selected=self.checkpoint.as_ref()==Some(&id);Some(Button::new(("handoff-code-checkpoint",i)).outline().small().label(format!("{} {id}",text("检查点","Checkpoint"))).selected(selected).disabled(locked).on_click(cx.listener(move |this,_,_,cx|{this.checkpoint=Some(id.clone());this.preview=None;cx.notify();})))})))
            .when(!self.exclusions.is_empty(), |v| v.child(Button::new("handoff-clear-exclusions").outline().small().label(text("清除手动排除","Clear exclusions")).disabled(locked).on_click(cx.listener(|this,_,_,cx|{this.exclusions.clear();this.preview=None;cx.notify();}))))
            .child(Button::new("handoff-preview").outline().small().label(text("预览接续","Preview handoff")).disabled(locked || self.target.is_none() || !self.eligibility.as_ref().is_some_and(|c|c["eligible"]==true)).on_click(cx.listener(|this,_,_,cx|this.preview(cx))))
            .when_some(self.preview.clone(),|v,p| {
                let history=&p["snapshot"]["history"];let facts=history["facts"].as_array().cloned().unwrap_or_default();let omissions=history["coverage"]["omitted"].as_array().cloned().unwrap_or_default();let required=history["requiresExplicitExclusionItemIds"].as_array().cloned().unwrap_or_default();
                let code=&p["snapshot"]["code"];
                v.child(div().text_xs().child(format!("Snapshot SHA-256: {} · Omissions SHA-256: {}",p["snapshotHash"].as_str().unwrap_or(""),p["omissionsHash"].as_str().unwrap_or(""))))
                    .child(div().text_sm().child(format!("{}: {} · {}",text("来源","Source"),p["snapshot"]["sourceSessionId"].as_str().unwrap_or(""),p["snapshot"]["sourceOriginAgentId"].as_str().unwrap_or(""))))
                    .child(div().text_sm().child(format!("{}: {} · {}",text("历史覆盖","History coverage"),history["coverage"]["completeness"].as_str().unwrap_or("unknown"),history["coverage"]["nativeFreshness"].as_str().unwrap_or("unknown"))))
                    .child(div().text_sm().child(format!("{}: {} · HEAD {}",text("代码来源","Code source"),code["sourceExecutionPath"].as_str().unwrap_or(""),code["headOid"].as_str().unwrap_or(""))))
                    .child(div().text_sm().child(format!("{}: {}",text("排除文件","Excluded files"),strings(&code["exclusions"]))))
                    .child(div().text_sm().child(format!("{}: {}",text("代码覆盖","Code coverage"),code["coverage"])))
                    .children(blockers(&p["blockers"]).into_iter().map(|b|div().text_sm().text_color(Color::Error.get(cx)).child(b)))
                    .children(history["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|w|div().text_sm().child(w.to_owned())))
                    .children(omissions.into_iter().map(|o|div().text_sm().child(format!("{} · {} · {}",text("遗漏","Omitted"),o["itemId"].as_str().unwrap_or(""),format!("{} · {} · {}",o["reason"].as_str().unwrap_or(""),o["bytes"],o["sha256"].as_str().unwrap_or(""))))))
                    .children(facts.into_iter().enumerate().map(|(i,f)|{let source=&f["source"];v_flex().id(("handoff-fact",i)).gap_1().p_2().border_1().border_color(Color::Divider.get(cx)).child(div().text_xs().child(format!("{} · {} · {} · {} · {}",f["kind"].as_str().unwrap_or(""),source["sourceSessionId"].as_str().unwrap_or(""),source["sourceDriver"].as_str().unwrap_or(""),source["nativeItemId"].as_str().unwrap_or(""),source["executionPath"].as_str().unwrap_or("")))).child(div().text_xs().child(format!("{} · {} · {}",source["sourceProfile"],source["contentSha256"],f["excerpt"]))).child(div().id(("handoff-fact-text",i)).role(Role::Label).aria_value(f["text"].as_str().unwrap_or("").to_owned()).text_sm().child(f["text"].as_str().unwrap_or("").to_owned())).children(f["attachments"].as_array().into_iter().flatten().map(|a|div().text_xs().child(format!("{} · {} · {}",a["name"].as_str().unwrap_or(""),a["policy"].as_str().unwrap_or(""),a["availability"].as_str().unwrap_or("")))))}))
                    .children(required.into_iter().enumerate().filter_map(|(i,id)|{let id=id.as_str()?.to_owned();let selected=self.exclusions.contains(&id);Some(Button::new(("handoff-exclude",i)).outline().small().selected(selected).disabled(locked).label(format!("{} {id}",text("明确排除此项并重新预览","Explicitly exclude and preview again"))).on_click(cx.listener(move |this,_,_,cx|{if !this.exclusions.contains(&id){this.exclusions.push(id.clone());}this.preview=None;cx.notify();})))}))
                    .child(Button::new("handoff-apply").primary().small().label(text("确认已查看快照与遗漏，创建接续","Confirm snapshot and omissions; create handoff")).disabled(!current || self.busy || p["eligible"]!=true || p["previewId"].as_str().is_none() || p["snapshotHash"].as_str().is_none() || p["omissionsHash"].as_str().is_none()).on_click(cx.listener(|this,_,_,cx|this.apply(cx))))
            })
            .when_some(self.operation.clone(),|v,op|{let id=op["operationId"].as_str().unwrap_or("").to_owned();let refresh=id.clone();let recover=id.clone();let state=op["state"].as_str().unwrap_or("unknown").to_owned();
                v.child(div().id("handoff-operation-status").role(Role::Status).child(format!("{} · {} · {} · {}",state,id,op["target"]["driver"].as_str().unwrap_or(""),op["target"]["profile"]))).when_some(op["error"]["message"].as_str().map(str::to_owned),|v,e|v.child(div().text_sm().child(e)))
                .when(state=="ready",|v|v.child(Button::new("handoff-open-child").outline().small().label(text("打开接续会话","Open handoff conversation")).disabled(!current || self.busy).on_click(cx.listener(move |this,_,window,cx|this.open_child(op.clone(),window,cx)))))
                .when(matches!(state.as_str(),"blocked" | "prepared" | "codeReady" | "targetPublished"),|v|v.child(Button::new("handoff-recover").outline().small().label(text("核验并恢复创建","Verify and recover creation")).disabled(!current || self.busy || recover.is_empty()).on_click(cx.listener(move |this,_,_,cx|this.call("chat.handoff.recover",json!({"workspaceId":this.workspace,"operationId":recover}),cx)))))
                .child(Button::new("handoff-refresh-operation").outline().small().label(text("刷新状态","Refresh status")).disabled(!current || self.busy || refresh.is_empty()).on_click(cx.listener(move |this,_,_,cx|this.call("chat.handoff.get",json!({"workspaceId":this.workspace,"operationId":refresh}),cx))))
            })
            .child(Button::new("handoff-list-operations").outline().small().label(text("查看接续操作","View handoff operations")).disabled(!current || self.busy).on_click(cx.listener(|this,_,_,cx|this.call("chat.handoff.list",json!({"workspaceId":this.workspace}),cx))))
            .children(self.operations.clone().into_iter().enumerate().filter_map(|(i,op)|{if op["sourceSessionId"]!=self.session{return None;}let id=op["operationId"].as_str()?.to_owned();Some(Button::new(("handoff-operation",i)).outline().small().label(format!("{} · {}",id,op["state"].as_str().unwrap_or("unknown"))).disabled(!current || self.busy).on_click(cx.listener(move |this,_,_,cx|this.call("chat.handoff.get",json!({"workspaceId":this.workspace,"operationId":id}),cx))))}))
            .when_some(self.error.clone(),|v,e|v.child(div().id("handoff-error").role(Role::Alert).text_color(Color::Error.get(cx)).child(e)))
    }
}
pub(crate) fn handoff_origin(
    snapshot: &Value,
    delivery: Option<&Value>,
    cx: &mut Context<HiveApp>,
) -> impl IntoElement + use<> {
    let history = &snapshot["history"];
    let state = delivery
        .and_then(|v| v["status"].as_str())
        .unwrap_or("unknown");
    let heading = format!(
        "{}: {} · {}: {}",
        text("文本接续来源", "Text handoff source"),
        snapshot["sourceSessionId"].as_str().unwrap_or(""),
        text("交付状态", "Delivery status"),
        state
    );
    v_flex().id("handoff-origin").gap_1().p_2().border_1().border_color(Color::Divider.get(cx))
        .child(div().id("handoff-origin-heading").role(Role::Label).aria_value(heading.clone()).child(heading))
        .child(div().text_xs().child(text("这是冻结的历史文本，不是目标厂商的原生历史；来源会话保持原样。","This is frozen historical text, not the target provider's native history. The source is unchanged.")))
        .when(state == "pending", |v| v.child(div().text_xs().child(text("首次普通文本发送才激活接续。","The first ordinary text input activates the handoff."))))
        .when(state != "consumed",|v|v.child(Button::new("handoff-refresh-target").outline().small().label(text("刷新目标状态与模型","Refresh target status and models")).on_click(cx.listener(|this,_,_,cx|{if let Some(id)=this.session.as_ref().map(|s|s.id.clone()){this.show_session(id,cx);}}))))
        .when(matches!(state,"attempted"|"uncertain"|"unknown"),|v|v.child(div().id("handoff-delivery-uncertain").role(Role::Alert).child(text("历史文本可能已经交付；请核验目标，不会自动重试。","The historical text may have been delivered. Inspect the target; it will not be retried automatically."))))
        .child(div().text_xs().child(format!("{}: {} · {}",text("覆盖","Coverage"),history["coverage"]["completeness"].as_str().unwrap_or("unknown"),history["coverage"]["nativeFreshness"].as_str().unwrap_or("unknown"))))
        .children(history["coverage"]["omitted"].as_array().into_iter().flatten().map(|o|div().text_xs().child(format!("{}: {}",text("遗漏","Omitted"),o["reason"].as_str().unwrap_or("")))))
        .children(history["facts"].as_array().into_iter().flatten().enumerate().map(|(index,fact)|{let source=&fact["source"];v_flex().gap_1().child(div().text_xs().child(format!("{} · {} · {} · {}",source["sourceSessionId"].as_str().unwrap_or(""),source["sourceDriver"].as_str().unwrap_or(""),source["nativeItemId"].as_str().unwrap_or(""),source["executionPath"].as_str().unwrap_or("")))).child(div().text_xs().child(format!("{} · {}", source["sourceProfile"], source["contentSha256"]))).child(div().id(("handoff-origin-fact",index)).role(Role::Label).aria_value(fact["text"].as_str().unwrap_or("").to_owned()).text_sm().child(fact["text"].as_str().unwrap_or("").to_owned()))}))
}

#[cfg(test)]
mod tests {
    use super::historical_item_id;
    use serde_json::json;
    #[test]
    fn history_identity_requires_backend_supplied_id() {
        assert_eq!(
            historical_item_id(&json!({"role":"user","handoffItemId":"native-id"})).as_deref(),
            Some("native-id")
        );
        assert_eq!(
            historical_item_id(&json!({"role":"user","source":{"entryId":"cannot-infer"}})),
            None
        );
        assert_eq!(
            historical_item_id(&json!({"role":"assistant","handoffItemId":"native-id"})).as_deref(),
            Some("native-id")
        );
        assert_eq!(
            historical_item_id(&json!({"role":"toolResult","handoffItemId":"native-id"})),
            None
        );
    }
}
