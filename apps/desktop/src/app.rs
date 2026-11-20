mod backend;
mod status;
pub(crate) use status::{Status,ErrorNotice};
use crate::{
    protocol::*,
    transport::{Runtime, RuntimeEvent},
};
use gpui_kit::component::WindowExt;
use gpui_kit::{
    component::input::{InputEvent, InputState, TextareaState},
    *,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    General,
    Appearance,
    Chat,
    Keybindings,
    Diffs,
    ProjectScripts,
    PiAccounts,
    Models,
    Team,
    Notifications,
    Remote,
    Programs,
    Profiles,
    Archive,
    About,
}

#[derive(Clone, Debug)]
pub struct SplitPeekMessage {
    pub role: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct SplitPeekState {
    pub session_id: String,
    pub title: String,
    pub backend_label: String,
    pub branch: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
    pub messages: Vec<SplitPeekMessage>,
    pub scroll: ScrollHandle,
}

pub struct ProjectMcpForm {
    pub editing_name: Option<String>,
    pub transport: &'static str,
    pub name: Entity<InputState>,
    pub command: Entity<InputState>,
    pub args: Entity<InputState>,
    pub url: Entity<InputState>,
    pub env: Entity<InputState>,
    pub saving: bool,
}

#[derive(Debug)]
pub struct ToolOutputWindow {
    pub history_message_id: String,
    pub block_index: usize,
    pub display_range: std::ops::Range<usize>,
    pub generation: Arc<()>,
    pub window_start_bytes: u64,
    pub loaded_bytes: u64,
    pub total_bytes: u64,
    pub has_more: bool,
    pub loading: bool,
    pub error: Option<String>,
}

pub struct ToolPresentation {
    pub patch: Option<String>,
    pub name: String,
    pub status: &'static str,
    pub expanded: bool,
    pub details_scroll: ScrollHandle,
    pub output_windows: Vec<ToolOutputWindow>,
}
pub struct DeliveryCard {
    pub dispatch_id: Option<String>,
    pub report: bool,
    pub worker_id: Option<String>,
    pub artifacts: Vec<String>,
    pub heading: &'static str,
    pub worker_name: Option<String>,
    pub raw: String,
    pub expanded: bool,
    pub missing_provenance: bool,
}
impl DeliveryCard {
    pub(crate) fn heading_text(&self) -> String {
        let label = crate::language::tr(self.heading);
        match &self.worker_name { Some(name)=>format!("{name} · {label}"),None=>label.to_owned() }
    }
}
pub struct Message {
    pub layout_revision: u64,
    pub id: String,
    pub role: String,
    pub native_user_item_id: Option<String>,
    pub handoff_item_id: Option<String>,
    pub source_session_id: Option<String>,
    pub text: String,
    pub(crate) markdown_focus: Option<FocusHandle>,
    pub tool: Option<ToolPresentation>,
    pub thinking: Vec<crate::thinking::ThinkingBlock>,
    pub delivery: Option<DeliveryCard>,
    pub explicit_user: bool,
    pub failure: Option<crate::failures::Diagnostic>,
    pub files: Vec<crate::attachments::AttachmentRef>,
    pub(crate) history_images: Option<crate::attachments::HistoryImages>,
    pub images: Vec<std::sync::Arc<RenderImage>>,
    pub image_sources: Vec<Arc<Image>>,
    pub images_loading: bool,
    pub images_error: Option<String>,
    pub(crate) image_task: Option<Task<()>>,
}
impl Message {
    pub(crate) fn display_body(&self) -> &str {
        if self.delivery.as_ref().is_some_and(|card| card.missing_provenance) {
            crate::language::tr("此消息缺少有效的来源记录。展开查看原文。")
        } else if self.delivery.as_ref().is_some_and(|card| card.heading == "会话恢复")
            && self.text == "已恢复会话，继续处理未完成的工作。"
        {
            crate::language::tr("已恢复会话，继续处理未完成的工作。")
        } else {
            &self.text
        }
    }
}
pub struct ConversationPosition {
    pub expanded: HashSet<String>,
    pub expanded_tool_groups: HashSet<String>,
    pub thinking: crate::thinking::ThinkingPosition,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct NavigationSelection {
    pub workspace: Option<String>,
    pub sessions: HashMap<String, String>,
}

#[derive(Clone, PartialEq)]
pub enum ConversationSource {
    Team,
    Workflow,
    Dispatch(String),
    Session(String),
}

pub struct HiveApp {
    pub conversation_find: Option<crate::conversation_find::ConversationFind>,
    pub(crate) assistant_quote_navigation: Option<Task<()>>,
    pub conversation_sources: Vec<ConversationSource>,
    pub navigation: NavigationSelection,
    pub sidebar_open: bool,
    pub(crate) sidebar: crate::navigation::SidebarPreferences,
    pub(crate) sidebar_workspace_filter: Option<String>,
    pub(crate) sidebar_search: Entity<crate::navigation::SidebarSearch>,
    pub composer_can_submit: bool,
    pub composer_submit_on_enter: bool,
    pub composer_has_text: bool,
    pub connected: bool,
    pub interactions: crate::interactions::Interactions,
    pub terminal: crate::terminal::WorkspaceTerminal,
    pub(crate) history_navigation: crate::history_navigation::HistoryNavigation,
    pub nav_back: Vec<(Option<String>, Option<String>)>,
    pub nav_forward: Vec<(Option<String>, Option<String>)>,
    pub nav_suppress_record: bool,
    pub git_drawer_open: bool,
    pub(crate) project_file_dock: Option<Entity<crate::project_files::FileBrowser>>,
    pub(crate) project_file_dock_width: Option<Pixels>,
    pub(crate) project_file_tree_expansions: Entity<crate::project_files::FileTreeExpansions>,
    pub git_drawer_width: f32,
    pub git_drawer_drag: Option<(f32, f32)>,
    pub terminal_drawer_height: f32,
    pub terminal_drawer_drag: Option<(f32, f32)>,
    pub(crate) workspace_focus: crate::workspace_focus::WorkspaceFocus,
    pub(crate) git_workspace: Option<Entity<crate::git_workspace::GitWorkspace>>,
    pub input: Entity<TextareaState>,
    pub(crate) prompt_history: crate::prompt_history::PromptHistory,
    pub(crate) draft_store: Option<crate::drafts::DraftStore>,
    pub(crate) draft_copies: crate::draft_copy::DraftCopies,
    pub(crate) draft_save_task: Option<Task<()>>,
    pub attachments: crate::attachments::Attachments,
    pub session_archive_loading: HashSet<String>,
    pub(crate) worktree_release_loading: HashSet<String>,
    pub drafts: HashMap<String, String>,
    pub draft_owner: Option<String>,
    pub draft_target: Option<String>,
    pub input_placeholder: String,
    pub interface_locale: Option<crate::language::Locale>,
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
    pub member_model_scroll: ScrollHandle,
    pub member_search: Entity<InputState>,
    pub member_status_filter: &'static str,
    pub member_picker_target: Option<String>,
    pub member_catalog_backend: Option<String>,
    pub member_catalog: Vec<Model>,
    pub member_catalog_loading: bool,
    pub member_catalog_error: Option<(String, String)>,
    pub member_catalog_generation: u64,
    pub pi_command: Entity<InputState>,
    pub codex_command: Entity<InputState>,
    pub claude_command: Entity<InputState>,
    pub settings_panel: bool,
    pub preferences_focus: FocusHandle,
    pub preferences_modal_open: bool,
    pub preferences_search_open: bool,
    pub preferences_return_focus: Option<FocusHandle>,
    pub repair_accounts_focus: FocusHandle,
    pub repair_models_focus: FocusHandle,
    pub repair_program_focus: FocusHandle,
    pub preferences_nav_scroll: ScrollHandle,
    pub settings_section: SettingsSection,
    pub(crate) project_scripts: Entity<crate::project_scripts::Page>,
    pub(crate) keybinding_settings: Entity<crate::keybindings::Editor>,
    pub settings_search: Entity<InputState>,
    pub profiles: crate::agent_profiles::Profiles,
    pub pi_accounts: crate::settings::pi_accounts::PiAccounts,
    pub pi_models: crate::settings::pi_models::PiModels,
    pub settings_loading: bool,
    pub executable_picker_generation: u64,
    pub settings_message: Option<crate::settings::SettingsNotice>,
    pub program_save_message: Option<crate::settings::SettingsNotice>,
    pub executable_settings: Option<crate::settings::ExecutableSettings>,
    pub codex_account: crate::settings::codex_account::CodexAccountCheck,
    pub codex_account_focus: FocusHandle,
    pub program_checks: HashMap<&'static str, crate::settings::programs::ProgramCheck>,
    pub program_setup_focus: Option<&'static str>,
    pub local_alerts: crate::local_notifications::Alerts,
    pub notification_scroll: ScrollHandle,
    pub notification_url: Entity<InputState>,
    pub notification_loaded: bool,
    pub notification_loading: bool,
    pub notification_generation: u64,
    pub notification_message: Option<&'static str>,
    pub team_autostaff: Option<bool>,
    pub team_autostaff_loading: bool,
    pub team_autostaff_message: Option<crate::settings::SettingsNotice>,
    pub collaboration: crate::collaboration::Collaboration,
    pub remote: crate::settings::remote::Remote,
    pub workflows: crate::workflows::Workflows,
    pub workflow_navigation_focus: FocusHandle,
    pub failure_scroll: ScrollHandle,
    pub composer_scroll: ScrollHandle,
    pub memory: crate::memory::Memory,
    pub members: Vec<Member>,
    pub member_role: &'static str,
    pub member_backend: &'static str,
    pub member_profile: Option<crate::agent_profiles::ProfileRef>,
    pub member_panel: bool,
    pub member_scroll: ScrollHandle,
    pub member_focus_search: bool,
    pub member_form_open: bool,
    pub member_loading: bool,
    pub member_error: Option<String>,
    pub member_generation: u64,
    pub member_stopping: Option<String>,
    pub editing_member: Option<String>,
    pub member_details_open: HashMap<String, ScrollHandle>,
    pub deleting_member: Option<String>,
    pub runtime: Option<Arc<Runtime>>,
    pub backend_info: Option<crate::diagnostics::BackendInfo>,
    pub about_scroll: ScrollHandle,
    pub diagnostics_copied: bool,
    pub workspaces: Vec<Workspace>,
    pub(crate) scratch_projects: crate::scratch_project::ScratchProjects,
    pub workspace: Option<String>,
    pub workspace_removing: Option<String>,
    pub removed_sessions: HashSet<String>,
    pub projects_collapsed: bool,
    pub expanded_projects: HashSet<String>,
    pub project_sessions: HashMap<String, Vec<Session>>,
    pub sessions: Vec<Session>,
    pub session: Option<Session>,
    pub(crate) worktree_draft_creation: Option<crate::session_execution::WorktreeDraftCreation>,
    pub(crate) fresh_worktree_draft_retry: Option<crate::sessions::FreshWorktreeDraftRetry>,
    pub session_name_target: Option<String>,
    pub session_name: Entity<InputState>,
    pub session_name_loading: bool,
    pub session_pin_loading: HashSet<String>,
    pub pinned_sessions: crate::sessions::PinnedSessions,
    pub messages: Vec<Message>,
    pub(crate) message_indices: HashMap<String, usize>,
    pub expanded_tool_groups: HashSet<String>,
    pub tool_inputs: HashMap<String, Value>,
    pub pi_agent_dir: Option<std::path::PathBuf>,
    pub refreshing_models: bool,
    pub models: Vec<Model>,
    pub model: Option<ModelIdentity>,
    pub unavailable_model: Option<ModelIdentity>,
    pub(crate) resetting_draft_setting: Option<u64>,
    pub busy: HashSet<String>,
    pub(crate) sidebar_failed_turns: HashSet<String>,
    pub delivery_failure: Option<crate::failures::DeliveryFailure>,
    pub delivery_failure_expanded: bool,
    pub agent_diagnostic: Option<crate::failures::Diagnostic>,
    pub pending_reports: Vec<PendingReport>,
    pub pending_user_inputs: Vec<PendingUserInput>,
    pub(crate) steering_queued_inputs: HashSet<(u64, String, String)>,
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
    pub history_page: Option<crate::protocol::HistoryPageMeta>,
    pub readiness: Option<crate::protocol::ReadinessReport>,
    pub readiness_loading: bool,
    pub readiness_error: Option<String>,
    pub skills_rules: Option<crate::protocol::SkillsAndRulesReport>,
    pub local_usage_summary: Option<crate::protocol::LocalUsageSummary>,
    pub split_peek: Option<SplitPeekState>,
    pub mcp_probing: HashSet<String>,
    pub mcp_probe_results: HashMap<String, crate::protocol::McpProbeResult>,
    pub mcp_action_error: Option<String>,
    pub mcp_form: Option<ProjectMcpForm>,
    pub copied_key: Option<String>,
    pub copied_seq: u64,
    pub pending_events: Vec<PiEvent>,
    pub choosing_model: bool,
    pub usage: crate::usage::Usage,
    pub command_picker: crate::command_picker::CommandPicker,
    pub reasoning_picker: crate::reasoning_picker::ReasoningPicker,
    pub(crate) project_model_default: crate::model_picker::ProjectModelDefaultState,
    pub mode_picker: crate::mode_picker::ModePicker,
    pub closing: bool,
    pub restarting: bool,
    pub backend_initialized: bool,
    pub connection_generation: u64,
    pub inflight_inputs: HashMap<String, String>,
    pub status: Status,
    pub error: Option<ErrorNotice>,
    pub loading: bool,
    pub positions: HashMap<String, ConversationPosition>,
    pub(crate) reading_positions: crate::sessions::reading_position::ReadingPositions,
    pub conversation_list: crate::conversation_list::ConversationList,
    pub collaboration_scroll: ScrollHandle,
    pub welcome_scroll: ScrollHandle,
    pub navigation_scroll: ScrollHandle,
    pub focus_project_input: bool,
    pub program_scroll: ScrollHandle,
    pub appearance_scroll: ScrollHandle,
    pub general_scroll: ScrollHandle,
    pub diffs_scroll: ScrollHandle,
    pub archive_scroll: ScrollHandle,
    pub chat_settings_scroll: ScrollHandle,
    pub team_settings_scroll: ScrollHandle,
    pub settings_search_scroll: ScrollHandle,
    pub settings_search_selected: usize,
    pub settings_highlight_anchor: Option<(&'static str, u64)>,
    pub settings_highlight_generation: u64,
    pub showing_welcome_guide: bool,
    pub(crate) welcome_setup: crate::welcome::Setup,
    pub quit_confirm_open: bool,
    pub(crate) archive_settings: crate::settings::archive::ArchiveState,
    pub(crate) maintenance_state: crate::maintenance::MaintenanceState,
    pub(crate) desktop_updates: crate::settings::updates::State,
    pub data_dir: Option<std::path::PathBuf>,
    pub diffs_pr_base_input: Entity<InputState>,
    events: Task<()>,
    pub(crate) workspace_updates: Task<()>,
    pub(crate) new_session_creation: Option<u64>,
    pub(crate) scratch_session_retry: Option<(&'static str, String)>,
    _quit: Subscription,
    _composer_subscription: Subscription,
    _model_search_subscription: Subscription,
    _member_model_search_subscription: Subscription,
    _member_search_subscription: Subscription,
}
impl HiveApp {
    pub(crate) fn is_copied(&self, key: &str) -> bool {
        self.copied_key.as_deref() == Some(key)
    }

    pub(crate) fn mark_copied(&mut self, key: String, cx: &mut Context<Self>) {
        self.copied_seq = self.copied_seq.wrapping_add(1);
        let seq = self.copied_seq;
        self.copied_key = Some(key);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1800))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.copied_seq == seq {
                    this.copied_key = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::register_local_notifications(window, cx);
        Self::start_global_inbox_badge(cx);
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = weak.update(cx, |this, cx| this.request_exit(Some(window), cx));
            false
        });
        let weak = cx.weak_entity();
        let quit_handle = window.window_handle();
        gpui_kit::App::on_action(cx, move |_: &crate::Quit, cx| {
            let weak = weak.clone();
            cx.defer(move |cx| {
                if quit_handle
                    .update(cx, |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| this.request_exit(Some(window), cx));
                    })
                    .is_err()
                {
                    let _ = weak.update(cx, |this, cx| this.request_exit(None, cx));
                }
            });
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
        let weak = cx.weak_entity();
        let handle = window.window_handle();
        gpui_kit::App::on_action(cx, move |_: &crate::OpenCommandPalette, cx| {
            let weak = weak.clone();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.open_command_palette(window, cx));
                });
            });
        });
        Self::register_guarded_action::<crate::NewSessionShortcut>(
            cx,
            |this, window, cx| {
                if !window.context_stack().iter().any(|context| context.contains("HiveTerminal")) {
                    this.start_new_conversation(false, window, cx);
                }
            },
            window,
        );
        Self::register_guarded_action::<crate::keybindings::NewSessionInCurrentProject>(
            cx,
            |this, window, cx| {
                if !window.context_stack().iter().any(|context| context.contains("HiveTerminal")) {
                    this.start_new_conversation(true, window, cx);
                }
            },
            window,
        );
        Self::register_guarded_action::<crate::keybindings::FocusNextWorkspaceArea>(
            cx,
            |this, window, cx| this.focus_workspace_area(true, window, cx),
            window,
        );
        Self::register_guarded_action::<crate::keybindings::FocusPreviousWorkspaceArea>(
            cx,
            |this, window, cx| this.focus_workspace_area(false, window, cx),
            window,
        );
        Self::register_guarded_action::<crate::keybindings::FocusNextFromTerminal>(
            cx,
            |this, window, cx| this.focus_workspace_area(true, window, cx),
            window,
        );
        Self::register_guarded_action::<crate::keybindings::FocusPreviousFromTerminal>(
            cx,
            |this, window, cx| this.focus_workspace_area(false, window, cx),
            window,
        );
        Self::register_guarded_action::<crate::ToggleSidebar>(
            cx,
            |this, _window, cx| {
                this.sidebar_open = !this.sidebar_open;
                cx.notify();
            },
            window,
        );
        Self::register_guarded_action::<crate::ToggleWorkspaceTerminal>(
            cx,
            |this, window, cx| {
                if !this.connected || this.closing {
                    return;
                }
                if this.terminal.visible {
                    this.hide_terminal(cx);
                } else if this.workspace.is_some() {
                    this.show_terminal(window, cx);
                }
            },
            window,
        );
        Self::register_guarded_action::<crate::OpenGitWorkspaceShortcut>(
            cx,
            |this, window, cx| {
                if this.connected && !this.closing && this.workspace.is_some() {
                    this.open_git_workspace(window, cx);
                }
            },
            window,
        );
        Self::register_guarded_action::<crate::OpenProjectFilePicker>(
            cx,
            |this, window, cx| {
                if this.connected && !this.closing && this.can_reference_project_file() {
                    this.open_project_file_picker(window, cx);
                }
            },
            window,
        );
        Self::register_guarded_action::<crate::OpenModelPicker>(
            cx,
            |this, window, cx| {
                if !this.connected
                    || this.closing
                    || this.loading
                    || !this.viewing_conversation()
                    || this.session.is_none()
                    || this.conversation_is_read_only()
                {
                    return;
                }
                if this.unavailable_project_draft_model().is_none()
                    && this.connect_saved_session(crate::sessions::NativeSessionPanel::Models, cx) {
                    return;
                }
                this.choosing_model = !this.choosing_model;
                if this.choosing_model {
                    this.reset_model_picker_search(window, cx);
                    this.load_model_picker_contents(cx);
                }
                cx.notify();
            },
            window,
        );
        Self::register_guarded_action::<crate::ToggleSessionMode>(
            cx,
            |this, _window, cx| {
                if this.connected && !this.closing && this.viewing_conversation() {
                    this.cycle_session_mode(cx);
                }
            },
            window,
        );
        Self::register_guarded_action::<crate::SelectPreviousSession>(
            cx,
            |this, window, cx| this.step_session(-1, window, cx),
            window,
        );
        Self::register_guarded_action::<crate::SelectNextSession>(
            cx,
            |this, window, cx| this.step_session(1, window, cx),
            window,
        );
        let [pi_command, codex_command, claude_command] = std::array::from_fn(|_| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("留空时使用 PATH"));
            cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if this.executable_paths_changed(cx) {
                        this.settings_message = None;
                        this.program_save_message = None;
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
        let settings_search = cx.new(|cx| InputState::new(window, cx).placeholder(crate::language::tr("搜索设置")));
        cx.subscribe_in(&settings_search, window, |this, _, event: &InputEvent, window, cx| {
            match event {
                InputEvent::Change => {
                    this.settings_search_selected = 0;
                    this.settings_search_scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }
                InputEvent::PressEnter { secondary: false, shift: false } => this.open_first_settings_match(window, cx),
                _ => {}
            }
        }).detach();
        let model_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索模型、提供商…"));
        let model_search_subscription =
            cx.subscribe_in(&model_search, window, |this, _, event: &InputEvent, _, cx| {
                match event {
                    InputEvent::Change => this.model_picker_search_changed(cx),
                    InputEvent::Focus => cx.notify(),
                    InputEvent::Blur => {
                        this.reasoning_picker.model_search_navigation.cancel_enter();
                        cx.notify();
                    }
                    _ => {}
                }
            });
        let member_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索成员、职责、角色或 Agent…"));
        let member_search_subscription =
            cx.subscribe_in(&member_search, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.member_scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }
            });
        let member_model_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索提供商或模型"));
        let member_model_search_subscription = cx.subscribe_in(
            &member_model_search,
            window,
            |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.member_model_scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }
            },
        );
        let default_pr_base = crate::diffs_settings::current(cx).pr_default_base;
        let diffs_pr_base_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::language::tr("例如 main"))
                .default_value(default_pr_base)
        });
        cx.subscribe_in(
            &diffs_pr_base_input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter {
                    secondary: false,
                    shift: false,
                } => this.save_diffs_pr_base(window, cx),
                InputEvent::Blur => {
                    let current = crate::diffs_settings::current(cx).pr_default_base;
                    if this.diffs_pr_base_input.read(cx).value().trim() != current {
                        this.save_diffs_pr_base(window, cx);
                    }
                }
                _ => {}
            },
        )
        .detach();
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .submit_on_enter(true)
                .auto_grow(1, 7)
                .placeholder(crate::language::tr("输入任何问题，@ 引用文件，/ 命令"))
        });
        let composer_subscription = cx.subscribe_in(
            &input,
            window,
            move |this: &mut Self, input, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter {
                    secondary: false,
                    shift: false,
                } if this.composer_can_submit && this.composer_submit_on_enter =>
                {
                    if this.composer_key_is_focused(window, cx) {
                        this.submit_composer_key(crate::composer::SendIntent::Configured, window, cx)
                    }
                }
                InputEvent::Change => {
                    this.schedule_draft_save(cx);
                    let text = input.read(cx).value();
                    if !this.prompt_history.on_change(this.open_request, &text) {
                        this.composer_commands_changed(cx);
                    }
                    let enter_mode_changed = this.sync_composer_enter_mode(cx);
                    let has_text = !text.trim().is_empty();
                    if enter_mode_changed || this.composer_has_text != has_text {
                        cx.notify();
                    }
                }
                _ => {}
            },
        );
        let sidebar_owner = cx.weak_entity();
        let sidebar_search =
            cx.new(|cx| crate::navigation::SidebarSearch::new(sidebar_owner, window, cx));
        let scripts_owner = cx.weak_entity();
        let mut app = Self {
            conversation_sources: Vec::new(),
            navigation: NavigationSelection::default(),
            sidebar_open: window.viewport_size().width >= px(820.),
            sidebar: crate::navigation::SidebarPreferences::default(),
            sidebar_workspace_filter: None,
            sidebar_search,
            connected: false,
            interactions: crate::interactions::Interactions::default(),
            terminal: crate::terminal::WorkspaceTerminal::new(cx),
            history_navigation: crate::history_navigation::HistoryNavigation::default(),
            input,
            prompt_history: Default::default(),
            composer_can_submit: false,
            composer_submit_on_enter: true,
            composer_has_text: false,
            _composer_subscription: composer_subscription,
            draft_store: None,
            draft_copies: Default::default(),
            draft_save_task: None,
            attachments: Default::default(),
            session_archive_loading: HashSet::new(),
            worktree_release_loading: HashSet::new(),
            drafts: HashMap::new(),
            draft_owner: None,
            draft_target: None,
            input_placeholder: crate::language::tr("输入任何问题，@ 引用文件，/ 命令").into(),
            nav_back: Vec::new(),
            nav_forward: Vec::new(),
            nav_suppress_record: false,
            git_drawer_open: false,
            project_file_dock: None,
            project_file_dock_width: None,
            project_file_tree_expansions: cx.new(|_| Default::default()),
            git_drawer_width: 460.,
            git_drawer_drag: None,
            terminal_drawer_height: 240.,
            terminal_drawer_drag: None,
            workspace_focus: crate::workspace_focus::WorkspaceFocus::new(window, cx),
            git_workspace: None,
            interface_locale: None,
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
            member_model_scroll: ScrollHandle::new(),
            member_search,
            member_status_filter: "all",
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
            assistant_quote_navigation: None,
            settings_panel: false,
            preferences_focus: cx.focus_handle(),
            preferences_modal_open: false,
            preferences_search_open: false,
            preferences_return_focus: None,
            repair_accounts_focus: cx.focus_handle(),
            repair_models_focus: cx.focus_handle(),
            repair_program_focus: cx.focus_handle(),
            preferences_nav_scroll: ScrollHandle::new(),
            settings_section: SettingsSection::General,
            project_scripts: cx.new(|cx| crate::project_scripts::Page::new(scripts_owner, window, cx)),
            keybinding_settings: cx.new(|cx| crate::keybindings::Editor::new(window, cx)),
            settings_search,
            profiles: crate::agent_profiles::Profiles::new(window, cx),
            pi_accounts: crate::settings::pi_accounts::PiAccounts::new(window, cx),
            pi_models: crate::settings::pi_models::PiModels::new(window, cx),
            settings_loading: false,
            executable_picker_generation: 0,
            settings_message: None,
            program_save_message: None,
            executable_settings: None,
            program_checks: Default::default(),
            codex_account: Default::default(),
            codex_account_focus: cx.focus_handle(),
            program_setup_focus: None,
            local_alerts: Default::default(),
            notification_scroll: ScrollHandle::new(),
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
            remote: Default::default(),
            workflows: Default::default(),
            workflow_navigation_focus: cx.focus_handle(),
            failure_scroll: ScrollHandle::new(),
            composer_scroll: ScrollHandle::new(),
            memory: crate::memory::Memory::new(window, cx),
            members: vec![],
            member_role: "coder",
            member_backend: "pi",
            member_profile: None,
            member_panel: false,
            member_scroll: ScrollHandle::new(),
            member_focus_search: false,
            member_form_open: false,
            member_loading: false,
            member_error: None,
            member_generation: 0,
            member_stopping: None,
            editing_member: None,
            member_details_open: HashMap::new(),
            deleting_member: None,
            _model_search_subscription: model_search_subscription,
            _member_model_search_subscription: member_model_search_subscription,
            _member_search_subscription: member_search_subscription,
            runtime: None,
            backend_info: None,
            about_scroll: ScrollHandle::new(),
            diagnostics_copied: false,
            workspaces: vec![],
            scratch_projects: Default::default(),
            workspace: None,
            workspace_removing: None,
            removed_sessions: HashSet::new(),
            projects_collapsed: false,
            expanded_projects: HashSet::new(),
            project_sessions: HashMap::new(),
            sessions: vec![],
            session: None,
            worktree_draft_creation: None,
            fresh_worktree_draft_retry: None,
            session_name_target: None,
            session_name,
            session_name_loading: false,
            session_pin_loading: HashSet::new(),
            pinned_sessions: Default::default(),
            messages: vec![],
            message_indices: HashMap::new(),
            expanded_tool_groups: HashSet::new(),
            tool_inputs: HashMap::new(),
            pi_agent_dir: None,
            refreshing_models: false,
            models: vec![],
            model: None,
            unavailable_model: None,
            resetting_draft_setting: None,
            busy: HashSet::new(),
            sidebar_failed_turns: HashSet::new(),
            delivery_failure: None,
            delivery_failure_expanded: false,
            agent_diagnostic: None,
            pending_reports: Vec::new(),
            pending_user_inputs: Vec::new(),
            steering_queued_inputs: HashSet::new(),
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
            history_page: None,
            readiness: None,
            readiness_loading: false,
            readiness_error: None,
            skills_rules: None,
            local_usage_summary: None,
            split_peek: None,
            mcp_probing: HashSet::new(),
            mcp_probe_results: HashMap::new(),
            mcp_action_error: None,
            mcp_form: None,
            copied_key: None,
            copied_seq: 0,
            pending_events: vec![],
            choosing_model: false,
            usage: Default::default(),
            reasoning_picker: Default::default(),
            project_model_default: Default::default(),
            mode_picker: Default::default(),
            command_picker: Default::default(),
            closing: false,
            restarting: false,
            backend_initialized: false,
            connection_generation: 0,
            inflight_inputs: HashMap::new(),
            status: "正在启动 Hive 后台…".into(),
            error: None,
            loading: true,
            positions: HashMap::new(),
            reading_positions: Default::default(),
            conversation_list: crate::conversation_list::ConversationList::new(cx.focus_handle()),
            collaboration_scroll: ScrollHandle::new(),
            welcome_scroll: ScrollHandle::new(),
            navigation_scroll: ScrollHandle::new(),
            focus_project_input: false,
            program_scroll: ScrollHandle::new(),
            appearance_scroll: ScrollHandle::new(),
            general_scroll: ScrollHandle::new(),
            diffs_scroll: ScrollHandle::new(),
            archive_scroll: ScrollHandle::new(),
            chat_settings_scroll: ScrollHandle::new(),
            team_settings_scroll: ScrollHandle::new(),
            settings_search_scroll: ScrollHandle::new(),
            settings_search_selected: 0,
            settings_highlight_anchor: None,
            settings_highlight_generation: 0,
            showing_welcome_guide: false,
            welcome_setup: Default::default(),
            quit_confirm_open: false,
            archive_settings: crate::settings::archive::ArchiveState::new(window, cx),
            maintenance_state: crate::maintenance::MaintenanceState::default(),
            desktop_updates: crate::settings::updates::State::default(),
            data_dir: None,
            new_session_creation: None,
            scratch_session_retry: None,
            diffs_pr_base_input,
            events: Task::ready(()),
            workspace_updates: Task::ready(()),
            _quit: cx.on_app_quit(|this: &mut Self, cx| {
                this.flush_reading_positions();
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
        app.conversation_list
            .state
            .set_scroll_handler(cx.listener(|this, _, _, cx| {
                this.schedule_reading_position_save(cx);
                cx.notify();
            }));
        app.start_backend(window.window_handle(), cx);
        app.start_workspace_updates(cx);
        app
    }
    fn register_guarded_action<A: gpui_kit::Action>(
        cx: &mut Context<Self>,
        handler: fn(&mut Self, &mut Window, &mut Context<Self>),
        window: &Window,
    ) {
        let weak = cx.weak_entity();
        let handle = window.window_handle();
        gpui_kit::App::on_action(cx, move |_: &A, cx| {
            let weak = weak.clone();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    if window.has_active_dialog(cx) {
                        return;
                    }
                    let _ = weak.update(cx, |this, cx| {
                        if !this.settings_panel && !this.roles.visible {
                            handler(this, window, cx);
                        }
                    });
                });
            });
        });
    }
    pub fn has_busy_work(&self) -> bool {
        !self.busy.is_empty() || !self.inflight_inputs.is_empty()
    }

    pub fn request_exit(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let should_confirm = match crate::general::current(cx).quit_confirmation {
            crate::general::QuitConfirmation::Never => false,
            crate::general::QuitConfirmation::WhenBusy => self.has_busy_work(),
            crate::general::QuitConfirmation::Always => true,
        };
        if !should_confirm {
            self.exit(cx);
            return;
        }
        let Some(window) = window else {
            self.exit(cx);
            return;
        };
        if self.quit_confirm_open {
            return;
        }
        self.quit_confirm_open = true;
        window.activate_window();
        let weak_close = cx.weak_entity();
        let weak_ok = cx.weak_entity();
        let busy = self.has_busy_work();
        gpui_kit::component::WindowExt::open_alert_dialog(window, cx, move |dialog, _, _| {
            let weak_close = weak_close.clone();
            let weak_ok = weak_ok.clone();
            dialog
                .title(crate::language::tr("确认退出 Hive Desktop？"))
                .description(if busy {
                    crate::language::tr(
                        "当前仍有会话任务正在运行；退出将中断进行中的连接并关闭桌面端。",
                    )
                } else {
                    crate::language::tr("确定要关闭并退出 Hive Desktop 吗？")
                })
                .show_cancel(true)
                .ok_text(crate::language::tr("退出应用"))
                .cancel_text(crate::language::tr("取消"))
                .on_close(move |_, _, cx| {
                    let _ = weak_close.update(cx, |this, _| this.quit_confirm_open = false);
                })
                .on_ok(move |_, _, cx| {
                    let _ = weak_ok.update(cx, |this, cx| {
                        this.quit_confirm_open = false;
                        this.exit(cx);
                    });
                    true
                })
        });
        cx.notify();
    }

    pub fn exit(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if let Err(error) = self.flush_drafts(cx) {
            self.error = Some(crate::app::ErrorNotice::Detail("无法保存草稿，请复制内容或重试退出",error.to_string()));
            cx.notify();
            return;
        }
        self.flush_reading_positions();
        self.closing = true;
        self.status = "正在保存会话并退出…".into();
        if self.runtime.is_some() || !self.loading {
            self.shutdown_and_quit(cx);
        }
        cx.notify();
    }
    fn shutdown_and_quit(&mut self, cx: &mut Context<Self>) {
        let runtime = self.runtime.take();
        let installer_id = self
            .desktop_updates
            .installer
            .as_ref()
            .map(|installer| installer.id.clone());
        cx.spawn(async move |this, cx| {
            if let Some(runtime) = runtime {
                let grace = std::time::Duration::from_secs(3);
                let shutdown = match installer_id.as_deref() {
                    Some(id) => runtime.shutdown_for_install(grace, id).await,
                    None => runtime.shutdown_checked(grace).await.map(|_| Ok(())),
                };
                match shutdown {
                    Ok(Ok(())) => { let _ = this.update(cx, |this, cx| this.finish_exit(cx)); }
                    Ok(Err(error)) => {
                        let _ = this.update(cx, |this, cx| this.desktop_install_failed(error, cx));
                    }
                    Err(error) => {
                        let _ = this.update(cx, move |this, cx| {
                            // Keep the owner/identity so another explicit Exit can
                            // retry cleanup. Never create a replacement or resume work.
                            this.runtime = Some(runtime);
                            this.closing = false;
                            this.desktop_updates.installer.take();
                            this.status = "后台进程清理未确认，已保留窗口".into();
                            this.error = Some(ErrorNotice::Context("未能确认后台进程已退出",Box::new(error.into()),"请重试退出；当前不会启动新的后台。"));
                            cx.notify();
                        });
                    }
                }
            } else {
                let _ = this.update(cx, |this, cx| this.finish_exit(cx));
            }
        })
        .detach();
    }

    fn desktop_install_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.desktop_updates.installer.take();
        self.connected = false;
        self.closing = false;
        self.error = Some(ErrorNotice::Detail("无法启动桌面安装程序", error));
        cx.notify();
    }

    fn finish_exit(&mut self, cx: &mut Context<Self>) {
        if let Some(installer) = self.desktop_updates.installer.take() {
            if let Err(error) = installer.start() {
                self.desktop_install_failed(error.to_string(), cx);
                return;
            }
        }
        cx.quit();
    }
    pub fn request(
        &mut self,
        method: &'static str,
        params: Value,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Value, &mut Context<Self>) + 'static,
    ) {
        let workspace_write = method.starts_with("chat.")
            && !matches!(
                method,
                "chat.open"
                    | "chat.list"
                    | "chat.projectModelDefault"
                    | "chat.clearView"
                    | "chat.handoffTargetState"
                    | "chat.historyPage"
            )
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
                            if method == "chat.open" { this.status = "未能打开会话".into(); }
                            let error = error.into();
                            this.error = Some(if method == "chat.open" {
                                ErrorNotice::Conversation(Box::new(error))
                            } else {
                                error
                            });
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
