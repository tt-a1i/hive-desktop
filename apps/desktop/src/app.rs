use crate::{
    protocol::*,
    transport::{Runtime, RuntimeEvent},
};
use gpui_kit::{
    component::{
        input::{InputEvent, InputState, TextareaState},
        text::TextViewState,
    },
    *,
};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Arc};

pub struct Message {
    pub id: String,
    pub role: String,
    pub text: String,
    pub markdown: Entity<TextViewState>,
}
pub struct HiveApp {
    pub input: Entity<TextareaState>,
    pub folder: Entity<InputState>,
    pub model_search: Entity<InputState>,
    pub runtime: Option<Arc<Runtime>>,
    pub workspaces: Vec<Workspace>,
    pub workspace: Option<String>,
    pub sessions: Vec<Session>,
    pub session: Option<Session>,
    pub messages: Vec<Message>,
    pub models: Vec<Model>,
    pub model: Option<ModelIdentity>,
    pub busy: HashSet<String>,
    pub event_sequence: u64,
    pub open_request: u64,
    pub pending_events: Vec<PiEvent>,
    pub choosing_model: bool,
    pub closing: bool,
    pub status: String,
    pub error: Option<String>,
    pub loading: bool,
    pub scroll: ScrollHandle,
    events: Task<()>,
    _quit: Subscription,
    _model_search_subscription: Subscription,
}
impl HiveApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = weak.update(cx, |this, cx| this.exit(cx));
            false
        });
        let weak = cx.weak_entity();
        gpui_kit::App::on_action(cx, move |_: &crate::Quit, cx| {
            let _ = weak.update(cx, |this, cx| this.exit(cx));
        });
        let model_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索模型、提供商…"));
        let model_search_subscription =
            cx.subscribe_in(&model_search, window, |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
        let mut app = Self {
            input: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(2, 7)
                    .placeholder("向 Pi 发送消息…")
            }),
            folder: cx.new(|cx| InputState::new(window, cx).placeholder("项目文件夹的绝对路径")),
            model_search,
            _model_search_subscription: model_search_subscription,
            runtime: None,
            workspaces: vec![],
            workspace: None,
            sessions: vec![],
            session: None,
            messages: vec![],
            models: vec![],
            model: None,
            busy: HashSet::new(),
            event_sequence: 0,
            open_request: 0,
            pending_events: vec![],
            choosing_model: false,
            closing: false,
            status: "正在启动 Hive 后台…".into(),
            error: None,
            loading: true,
            scroll: ScrollHandle::new(),
            events: Task::ready(()),
            _quit: cx.on_app_quit(|this: &mut Self, _| {
                let runtime = this.runtime.take();
                async move {
                    if let Some(runtime) = runtime {
                        runtime.shutdown().await;
                    }
                }
            }),
        };
        app.events = cx.spawn(async move |this, cx| {
            let runtime = match smol::unblock(Runtime::launch).await {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.loading = false;
                        this.error = Some(error);
                        if this.closing {
                            cx.quit();
                        }
                        cx.notify();
                    });
                    return;
                }
            };
            let events = runtime.events.clone();
            let result = runtime.request("initialize", json!({})).await;
            let _ = this.update(cx, |this, cx| {
                this.runtime = Some(runtime);
                this.loading = false;
                if this.closing {
                    this.shutdown_and_quit(cx);
                    return;
                }
                match result {
                    Ok(_) => {
                        this.status = "已连接 · Pi 就绪".into();
                        this.load_workspaces(cx);
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
            // This task holds no Runtime Arc while awaiting notifications, allowing app drop to shut it down.
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |this, cx| {
                        match event {
                            RuntimeEvent::Failed(error) => {
                                this.status = "后台连接已断开".into();
                                this.error = Some(error);
                                this.busy.clear();
                            }
                            RuntimeEvent::Agent(event) => this.agent_event(event, cx),
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
        app
    }
    pub fn exit(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.status = "正在保存会话并退出…".into();
        if self.runtime.is_some() || !self.loading {
            self.shutdown_and_quit(cx);
        }
        cx.notify();
    }
    fn shutdown_and_quit(&mut self, cx: &mut Context<Self>) {
        let runtime = self.runtime.take();
        cx.spawn(async move |this, cx| {
            if let Some(runtime) = runtime {
                runtime
                    .shutdown_with_grace(std::time::Duration::from_secs(3))
                    .await;
            }
            let _ = this.update(cx, |_, cx| cx.quit());
        })
        .detach();
    }
    pub fn request(
        &mut self,
        method: &'static str,
        params: Value,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Value, &mut Context<Self>) + 'static,
    ) {
        let Some(runtime) = self.runtime.clone() else {
            self.error = Some("后台尚未连接".into());
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(value) => done(this, value, cx),
                    Err(error) => {
                        this.loading = false;
                        this.error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
