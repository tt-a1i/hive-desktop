mod backend;
use crate::{
    protocol::*,
    transport::{Runtime, RuntimeEvent},
};
use gpui_kit::component::WindowExt;
use gpui_kit::{
    component::{
        input::{InputEvent, InputState, TextareaState},
        text::TextViewState,
    },
    *,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Appearance,
    Team,
    Notifications,
    Programs,
}

pub struct ToolPresentation {
    pub name: String,
    pub status: &'static str,
    pub expanded: bool,
}
pub struct DeliveryCard {
    pub dispatch_id: Option<String>,
    pub report: bool,
    pub worker_id: Option<String>,
    pub artifacts: Vec<String>,
    pub heading: String,
    pub raw: String,
    pub expanded: bool,
}
pub struct Message {
    pub id: String,
    pub role: String,
    pub text: String,
    pub markdown: Option<Entity<TextViewState>>,
    pub tool: Option<ToolPresentation>,
    pub delivery: Option<DeliveryCard>,
    pub explicit_user: bool,
}
pub struct ConversationPosition {
    pub offset: Option<Point<Pixels>>,
    pub expanded: HashSet<String>,
    pub expanded_tool_groups: HashSet<String>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct NavigationSelection {
    pub workspace: Option<String>,
    pub sessions: HashMap<String, String>,
}

#[derive(Clone, PartialEq)]
pub enum ConversationSource {
    Team,
    Dispatch(String),
    Session(String),
}

pub struct HiveApp {
    pub conversation_find: Option<crate::conversation_find::ConversationFind>,
    pub conversation_sources: Vec<ConversationSource>,
    pub navigation: NavigationSelection,
    pub sidebar_open: bool,
    pub composer_can_submit: bool,
    pub composer_has_text: bool,
    pub connected: bool,
    pub interactions: crate::interactions::Interactions,
    pub terminal: crate::terminal::WorkspaceTerminal,
    pub input: Entity<TextareaState>,
    pub(crate) draft_store: Option<crate::drafts::DraftStore>,
    pub(crate) draft_save_task: Option<Task<()>>,
    pub drafts: HashMap<String, String>,
    pub draft_owner: Option<String>,
    pub draft_target: Option<String>,
    pub input_placeholder: String,
    pub folder: Entity<InputState>,
    pub add_workspace_open: bool,
    pub model_search: Entity<InputState>,
    pub member_name: Entity<InputState>,
    pub member_templates_open: bool,
    pub member_description: Entity<TextareaState>,
    pub role_name: Entity<InputState>,
    pub role_description: Entity<TextareaState>,
    pub roles: crate::roles::Roles,
    pub member_rename: Entity<InputState>,
    pub member_model_search: Entity<InputState>,
    pub member_picker_target: Option<String>,
    pub member_catalog_backend: Option<String>,
    pub member_catalog: Vec<Model>,
    pub member_catalog_loading: bool,
    pub member_catalog_error: Option<String>,
    pub member_catalog_generation: u64,
    pub pi_command: Entity<InputState>,
    pub codex_command: Entity<InputState>,
    pub claude_command: Entity<InputState>,
    pub settings_panel: bool,
    pub preferences_focus: FocusHandle,
    pub preferences_return_focus: Option<FocusHandle>,
    pub settings_section: SettingsSection,
    pub settings_loading: bool,
    pub executable_picker_generation: u64,
    pub settings_message: Option<String>,
    pub executable_settings: Option<crate::settings::ExecutableSettings>,
    pub notification_url: Entity<InputState>,
    pub notification_loaded: bool,
    pub notification_loading: bool,
    pub notification_generation: u64,
    pub notification_message: Option<String>,
    pub team_autostaff: Option<bool>,
    pub team_autostaff_loading: bool,
    pub team_autostaff_message: Option<String>,
    pub collaboration: crate::collaboration::Collaboration,
    pub memory: crate::memory::Memory,
    pub members: Vec<Member>,
    pub member_role: &'static str,
    pub member_backend: &'static str,
    pub member_panel: bool,
    pub member_form_open: bool,
    pub member_loading: bool,
    pub member_generation: u64,
    pub member_stopping: Option<String>,
    pub editing_member: Option<String>,
    pub deleting_member: Option<String>,
    pub runtime: Option<Arc<Runtime>>,
    pub workspaces: Vec<Workspace>,
    pub workspace: Option<String>,
    pub workspace_removing: Option<String>,
    pub removed_sessions: HashSet<String>,
    pub projects_collapsed: bool,
    pub expanded_projects: HashSet<String>,
    pub project_sessions: HashMap<String, Vec<Session>>,
    pub sessions: Vec<Session>,
    pub session: Option<Session>,
    pub session_name_target: Option<String>,
    pub session_name: Entity<InputState>,
    pub session_name_loading: bool,
    pub messages: Vec<Message>,
    pub expanded_tool_groups: HashSet<String>,
    pub tool_inputs: HashMap<String, Value>,
    pub pi_agent_dir: Option<std::path::PathBuf>,
    pub refreshing_models: bool,
    pub models: Vec<Model>,
    pub model: Option<ModelIdentity>,
    pub unavailable_model: Option<ModelIdentity>,
    pub busy: HashSet<String>,
    pub pending_reports: Vec<PendingReport>,
    pub completed_deliveries: HashSet<String>,
    pub pending: HashMap<String, usize>,
    pub unread: HashSet<String>,
    pub failed_drafts: HashMap<String, Vec<String>>,
    pub background_sequences: HashMap<String, u64>,
    pub list_refreshing: bool,
    pub list_dirty: bool,
    pub event_sequence: u64,
    pub open_request: u64,
    pub conversation_ready: bool,
    pub pending_events: Vec<PiEvent>,
    pub choosing_model: bool,
    pub closing: bool,
    pub restarting: bool,
    pub backend_initialized: bool,
    pub connection_generation: u64,
    pub inflight_inputs: HashMap<String, String>,
    pub status: String,
    pub error: Option<String>,
    pub loading: bool,
    pub positions: HashMap<String, ConversationPosition>,
    pub scroll: ScrollHandle,
    pub collaboration_scroll: ScrollHandle,
    events: Task<()>,
    _quit: Subscription,
    _composer_subscription: Subscription,
    _model_search_subscription: Subscription,
    _member_model_search_subscription: Subscription,
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
        let weak = cx.weak_entity();
        let handle = window.window_handle();
        gpui_kit::App::on_action(cx, move |_: &crate::OpenPreferences, cx| {
            let weak = weak.clone();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    if window.has_active_dialog(cx) {
                        return;
                    }
                    let _ = weak.update(cx, |this, cx| {
                        if !this.settings_panel && !this.roles.visible {
                            this.open_settings(window, cx);
                        }
                    });
                });
            });
        });
        let weak = cx.weak_entity();
        let handle = window.window_handle();
        gpui_kit::App::on_action(cx, move |_: &crate::OpenSessionSearch, cx| {
            let weak = weak.clone();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.open_session_search(window, cx));
                });
            });
        });
        let weak = cx.weak_entity();
        let handle = window.window_handle();
        gpui_kit::App::on_action(cx, move |_: &crate::OpenConversationFind, cx| {
            let weak = weak.clone();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.open_conversation_find(window, cx));
                });
            });
        });
        let [pi_command, codex_command, claude_command] = std::array::from_fn(|_| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("留空时使用 PATH"));
            cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if this.executable_paths_changed(cx) {
                        this.settings_message = None;
                    }
                    cx.notify();
                }
            })
            .detach();
            input
        });
        let session_name = cx.new(|cx| InputState::new(window, cx).placeholder("会话名称"));
        cx.subscribe(&session_name, |this, _, event: &InputEvent, cx| {
            if matches!(
                event,
                InputEvent::PressEnter {
                    secondary: false,
                    shift: false
                }
            ) && this.session_name_target.is_some()
            {
                this.save_session_name(cx);
            }
        })
        .detach();
        let model_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索模型、提供商…"));
        let model_search_subscription =
            cx.subscribe_in(&model_search, window, |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
        let member_model_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索提供商或模型"));
        let member_model_search_subscription = cx.subscribe_in(
            &member_model_search,
            window,
            |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(2, 7)
                .placeholder("输入消息…")
        });
        let composer_subscription = cx.subscribe_in(
            &input,
            window,
            move |this: &mut Self, input, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter {
                    secondary: false,
                    shift: false,
                } if this.composer_can_submit => this.send(window, cx),
                InputEvent::Change => {
                    this.schedule_draft_save(cx);
                    let has_text = !input.read(cx).value().trim().is_empty();
                    if this.composer_has_text != has_text {
                        cx.notify();
                    }
                }
                _ => {}
            },
        );
        let mut app = Self {
            conversation_sources: Vec::new(),
            navigation: NavigationSelection::default(),
            sidebar_open: window.viewport_size().width >= px(820.),
            connected: false,
            interactions: crate::interactions::Interactions::default(),
            terminal: crate::terminal::WorkspaceTerminal::new(cx),
            input,
            composer_can_submit: false,
            composer_has_text: false,
            _composer_subscription: composer_subscription,
            draft_store: None,
            draft_save_task: None,
            drafts: HashMap::new(),
            draft_owner: None,
            draft_target: None,
            input_placeholder: "输入消息…".into(),
            add_workspace_open: false,
            folder: cx.new(|cx| InputState::new(window, cx).placeholder("项目文件夹的绝对路径")),
            model_search,
            member_name: cx.new(|cx| InputState::new(window, cx).placeholder("成员名称")),
            member_templates_open: false,
            member_description: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(2, 6)
                    .placeholder("职责说明（可选）")
            }),
            role_name: cx.new(|cx| InputState::new(window, cx).placeholder("模板名称")),
            role_description: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(3, 8)
                    .placeholder("职责说明")
            }),
            roles: crate::roles::Roles::new(),
            member_rename: cx.new(|cx| InputState::new(window, cx).placeholder("新名称")),
            member_model_search,
            member_picker_target: None,
            member_catalog_backend: None,
            member_catalog: vec![],
            member_catalog_loading: false,
            member_catalog_error: None,
            member_catalog_generation: 0,
            pi_command,
            codex_command,
            claude_command,
            conversation_find: None,
            settings_panel: false,
            preferences_focus: cx.focus_handle(),
            preferences_return_focus: None,
            settings_section: SettingsSection::Appearance,
            settings_loading: false,
            executable_picker_generation: 0,
            settings_message: None,
            executable_settings: None,
            notification_url: cx.new(|cx| {
                let mut input = InputState::new(window, cx).placeholder("https://…");
                input.set_masked(true, window, cx);
                input
            }),
            notification_loaded: false,
            notification_loading: false,
            notification_generation: 0,
            notification_message: None,
            team_autostaff: None,
            team_autostaff_loading: false,
            team_autostaff_message: None,
            collaboration: crate::collaboration::Collaboration::new(cx),
            memory: crate::memory::Memory::new(window, cx),
            members: vec![],
            member_role: "coder",
            member_backend: "pi",
            member_panel: false,
            member_form_open: false,
            member_loading: false,
            member_generation: 0,
            member_stopping: None,
            editing_member: None,
            deleting_member: None,
            _model_search_subscription: model_search_subscription,
            _member_model_search_subscription: member_model_search_subscription,
            runtime: None,
            workspaces: vec![],
            workspace: None,
            workspace_removing: None,
            removed_sessions: HashSet::new(),
            projects_collapsed: false,
            expanded_projects: HashSet::new(),
            project_sessions: HashMap::new(),
            sessions: vec![],
            session: None,
            session_name_target: None,
            session_name,
            session_name_loading: false,
            messages: vec![],
            expanded_tool_groups: HashSet::new(),
            tool_inputs: HashMap::new(),
            pi_agent_dir: None,
            refreshing_models: false,
            models: vec![],
            model: None,
            unavailable_model: None,
            busy: HashSet::new(),
            pending_reports: Vec::new(),
            completed_deliveries: HashSet::new(),
            pending: HashMap::new(),
            unread: HashSet::new(),
            failed_drafts: HashMap::new(),
            background_sequences: HashMap::new(),
            list_refreshing: false,
            list_dirty: false,
            event_sequence: 0,
            open_request: 0,
            conversation_ready: false,
            pending_events: vec![],
            choosing_model: false,
            closing: false,
            restarting: false,
            backend_initialized: false,
            connection_generation: 0,
            inflight_inputs: HashMap::new(),
            status: "正在启动 Hive 后台…".into(),
            error: None,
            loading: true,
            positions: HashMap::new(),
            scroll: ScrollHandle::new(),
            collaboration_scroll: ScrollHandle::new(),
            events: Task::ready(()),
            _quit: cx.on_app_quit(|this: &mut Self, cx| {
                if let Err(error) = this.flush_drafts(cx) {
                    eprintln!("无法保存本地草稿：{error}");
                }
                let runtime = this.runtime.take();
                async move {
                    if let Some(runtime) = runtime {
                        runtime.shutdown().await;
                    }
                }
            }),
        };
        app.start_backend(window.window_handle(), cx);
        app
    }
    pub fn exit(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if let Err(error) = self.flush_drafts(cx) {
            self.error = Some(format!("无法保存草稿，请复制内容或重试退出：{error}"));
            cx.notify();
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
                match runtime.shutdown_checked(std::time::Duration::from_secs(3)).await {
                    Ok(_) => { let _ = this.update(cx, |_, cx| cx.quit()); }
                    Err(error) => {
                        let _ = this.update(cx, move |this, cx| {
                            // Keep the owner/identity so another explicit Exit can
                            // retry cleanup. Never create a replacement or resume work.
                            this.runtime = Some(runtime);
                            this.closing = false;
                            this.status = "后台进程清理未确认，已保留窗口".into();
                            this.error = Some(format!("未能确认后台进程已退出：{error}。请重试退出；当前不会启动新的后台。"));
                            cx.notify();
                        });
                    }
                }
            } else {
                let _ = this.update(cx, |_, cx| cx.quit());
            }
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
        let workspace_write = method.starts_with("chat.")
            && !matches!(method, "chat.open" | "chat.list")
            || method == "hive.request"
                && params["method"]
                    .as_str()
                    .is_some_and(|method| method != "GET")
                && params["path"]
                    .as_str()
                    .is_some_and(|path| !path.starts_with("/api/settings/"));
        if workspace_write && !self.workspace_writes_allowed() {
            return;
        }
        if !self.connected {
            self.loading = false;
            self.error = Some("后台连接已断开，当前草稿仍保留。可在窗口顶部重新启动后台。".into());
            cx.notify();
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            self.error = Some("后台尚未连接".into());
            return;
        };
        let open_request = self.open_request;
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                match result {
                    Ok(value) => done(this, value, cx),
                    Err(error) => {
                        if this.open_request == open_request {
                            this.loading = false;
                            this.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
