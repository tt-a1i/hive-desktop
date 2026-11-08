mod view;
use super::{Catalog, NativeCommand};
use crate::{app::HiveApp, transport::Runtime};
use gpui_kit::{
    component::{
        input::{InputEvent, InputState},
        *,
    },
    *,
};
use serde_json::json;
use std::sync::Arc;

struct Resources {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    session: String,
    connection: u64,
    open_request: u64,
    query: Entity<InputState>,
    filter: &'static str,
    commands: Vec<NativeCommand>,
    loading: bool,
    error: Option<String>,
    scroll: ScrollHandle,
}
impl HiveApp {
    pub(crate) fn open_session_resources(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected
            || !self.conversation_ready
            || !self.pi_commands_available()
            || window.has_active_dialog(cx)
        {
            return;
        }
        let (Some(runtime), Some(session)) = (self.runtime.clone(), self.session.as_ref()) else {
            return;
        };
        let id = session.id.clone();
        let connection = self.connection_generation;
        let open_request = self.open_request;
        let app = cx.weak_entity();
        let commands = self.command_picker.commands.clone();
        self.command_picker.open = false;
        let resources = cx.new(|cx| {
            let query = cx.new(|cx| {
                InputState::new(window, cx).placeholder("搜索名称、完整说明、来源或路径…")
            });
            cx.subscribe(&query, |this: &mut Resources, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }
            })
            .detach();
            let resources = Resources {
                app,
                runtime,
                session: id,
                connection,
                open_request,
                query,
                filter: "all",
                commands,
                loading: false,
                error: None,
                scroll: ScrollHandle::new(),
            };
            // The parent is still updating while this child entity is created.
            cx.defer_in(window, |resources, _, cx| resources.load(cx));
            resources
        });
        let width = (window.viewport_size().width - px(48.)).min(px(880.));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title("会话资源").w(width).child(resources.clone())
        });
        cx.notify();
    }
}
impl Resources {
    fn matches_app(&self, app: &HiveApp) -> bool {
        app.connected
            && app.conversation_ready
            && app.connection_generation == self.connection
            && app.open_request == self.open_request
            && app
                .session
                .as_ref()
                .is_some_and(|session| session.id == self.session)
    }
    fn current(&self, cx: &mut Context<Self>) -> bool {
        self.app
            .update(cx, |app, _| self.matches_app(app))
            .unwrap_or(false)
    }
    fn stale(&mut self, cx: &mut Context<Self>) {
        self.error = Some("会话或后台连接已变化，请关闭后从当前会话重新打开资源。".into());
        cx.notify();
    }
    fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if !self.current(cx) {
            self.stale(cx);
            return;
        }
        self.loading = true;
        let runtime = self.runtime.clone();
        let session = self.session.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.commands", json!({"sessionId":session}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<Catalog>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                if !this.current(cx) {
                    this.stale(cx);
                    return;
                }
                match result {
                    Ok(catalog) if catalog.supported => {
                        this.commands = catalog.commands;
                        this.error = None;
                    }
                    Ok(_) => this.error = Some("此后端未提供原生资源目录。".into()),
                    Err(error) => this.error = Some(format!("目录读取失败，保留上次内容：{error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn use_command(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let used = self
            .app
            .update(cx, |app, cx| {
                if !self.matches_app(app) {
                    return false;
                }
                app.accept_command(name, window, cx);
                true
            })
            .unwrap_or(false);
        if used {
            window.close_dialog(cx);
        } else {
            self.stale(cx);
        }
    }
    fn preview(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let opened = self
            .app
            .update(cx, |app, cx| {
                if !self.matches_app(app) {
                    return false;
                }
                app.open_resource_preview(path, window, cx);
                true
            })
            .unwrap_or(false);
        if !opened {
            self.stale(cx);
        }
    }
}
fn kind(source: &str) -> &str {
    match source {
        "skill" => "技能",
        "prompt" => "提示模板",
        "extension" => "扩展命令",
        _ => source,
    }
}
fn scope(command: &NativeCommand) -> &str {
    match command.source_info["scope"].as_str() {
        Some("user") => "用户",
        Some("project") => "项目",
        Some("temporary") => "临时",
        _ => "范围未提供",
    }
}
fn local_path(command: &NativeCommand) -> Option<String> {
    let path = command.source_info["path"].as_str()?;
    (std::path::Path::new(path).is_absolute()
        && !path.starts_with("//")
        && !path.starts_with(r"\\")
        && !path.chars().any(char::is_control))
    .then(|| path.to_owned())
}
