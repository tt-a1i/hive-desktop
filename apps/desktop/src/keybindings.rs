mod editor;
pub(crate) use editor::Editor;

use crate::language::tr;
use gpui_kit::{
    Action, App, Global, KeyBinding, KeyBindingContextPredicate, KeyContext, KeybindingKeystroke,
    Keymap, Keystroke, Modifiers, OperatingSystem, component::kbd::Kbd,
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

gpui_kit::actions!(
    hive_desktop,
    [
        NewSessionInCurrentProject, PromptStash, SendAndStartDraft, IncreaseReasoningEffort,
        DecreaseReasoningEffort, FocusNextWorkspaceArea, FocusPreviousWorkspaceArea,
        FocusNextFromTerminal, FocusPreviousFromTerminal
    ]
);

pub(crate) const FILE: &str = "desktop-keybindings.json";
const CHORD_CONFLICT: &str = "快捷键与另一个操作冲突，请选择不同的组合键。";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContextScope {
    Global,
    WorkspaceNonTerminal,
    Composer,
    #[cfg(test)]
    Terminal,
}

impl ContextScope {
    pub(crate) fn badge_key(self) -> &'static str {
        match self {
            Self::Global => "全局",
            Self::WorkspaceNonTerminal => "非终端",
            Self::Composer => "输入框",
            #[cfg(test)]
            Self::Terminal => "终端",
        }
    }

    pub(crate) fn detail_key(self) -> &'static str {
        match self {
            Self::Global => "全局可用（所有窗口与输入区域）",
            Self::WorkspaceNonTerminal => "仅非终端工作区生效（when: !HiveTerminal）",
            Self::Composer => "仅消息输入框内生效（when: HiveComposer）",
            #[cfg(test)]
            Self::Terminal => "仅内置终端内生效（when: HiveTerminal）",
        }
    }

    pub(crate) fn when_expr(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::WorkspaceNonTerminal => "!HiveTerminal",
            Self::Composer => "HiveComposer",
            #[cfg(test)]
            Self::Terminal => "HiveTerminal",
        }
    }

    pub(crate) fn overlaps(self, other: Self) -> bool {
        match (self, other) {
            (Self::Global, _) | (_, Self::Global) => true,
            #[cfg(test)]
            (Self::Terminal, Self::Terminal) => true,
            #[cfg(test)]
            (Self::Terminal, _) | (_, Self::Terminal) => false,
            _ => true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConflictKind {
    SameScope(Target),
    OverlappingScope(Target),
    ProtectedComponent(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TargetConflict {
    pub target: Target,
    pub kind: ConflictKind,
    pub chord: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TargetGroup {
    Navigation,
    Workspace,
    Conversation,
    App,
}

impl TargetGroup {
    pub(crate) const ALL: [Self; 4] = [
        Self::Navigation,
        Self::Workspace,
        Self::Conversation,
        Self::App,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Navigation => tr("导航与会话"),
            Self::Workspace => tr("工作区与面板"),
            Self::Conversation => tr("对话操作"),
            Self::App => tr("应用"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    NewSession,
    NewSessionCurrentProject,
    Search,
    CommandPalette,
    Find,
    PreviousSession,
    NextSession,
    FocusNextWorkspaceArea,
    FocusPreviousWorkspaceArea,
    ToggleSidebar,
    ToggleTerminal,
    OpenGitWorkspace,
    SearchProjectFiles,
    ChooseModel,
    IncreaseReasoningEffort,
    DecreaseReasoningEffort,
    ToggleMode,
    PromptStash,
    SendAndStartDraft,
    Preferences,
}

const TARGETS: [Target; 20] = [
    Target::NewSession,
    Target::NewSessionCurrentProject,
    Target::Search,
    Target::CommandPalette,
    Target::Find,
    Target::PreviousSession,
    Target::NextSession,
    Target::FocusNextWorkspaceArea,
    Target::FocusPreviousWorkspaceArea,
    Target::ToggleSidebar,
    Target::ToggleTerminal,
    Target::OpenGitWorkspace,
    Target::SearchProjectFiles,
    Target::ChooseModel,
    Target::IncreaseReasoningEffort,
    Target::DecreaseReasoningEffort,
    Target::ToggleMode,
    Target::Preferences,
    Target::PromptStash,
    Target::SendAndStartDraft,
];

impl Target {
    pub(crate) fn index(self) -> usize {
        match self {
            Self::NewSession => 0,
            Self::NewSessionCurrentProject => 13,
            Self::Search => 1,
            Self::CommandPalette => 2,
            Self::Find => 3,
            Self::PreviousSession => 4,
            Self::NextSession => 5,
            Self::FocusNextWorkspaceArea => 18,
            Self::FocusPreviousWorkspaceArea => 19,
            Self::ToggleSidebar => 6,
            Self::ToggleTerminal => 7,
            Self::OpenGitWorkspace => 8,
            Self::SearchProjectFiles => 9,
            Self::ChooseModel => 10,
            Self::IncreaseReasoningEffort => 16,
            Self::DecreaseReasoningEffort => 17,
            Self::ToggleMode => 11,
            Self::Preferences => 12,
            Self::PromptStash => 14,
            Self::SendAndStartDraft => 15,
        }
    }

    pub(crate) fn group(self) -> TargetGroup {
        match self {
            Self::NewSession
            | Self::NewSessionCurrentProject
            | Self::Search
            | Self::CommandPalette
            | Self::Find
            | Self::PreviousSession
            | Self::NextSession => TargetGroup::Navigation,
            Self::FocusNextWorkspaceArea
            | Self::FocusPreviousWorkspaceArea
            | Self::ToggleSidebar
            | Self::ToggleTerminal
            | Self::OpenGitWorkspace
            | Self::SearchProjectFiles => TargetGroup::Workspace,
            Self::ChooseModel
            | Self::IncreaseReasoningEffort
            | Self::DecreaseReasoningEffort
            | Self::ToggleMode
            | Self::PromptStash
            | Self::SendAndStartDraft => TargetGroup::Conversation,
            Self::Preferences => TargetGroup::App,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::NewSession => tr("新建会话"),
            Self::NewSessionCurrentProject => tr("在当前项目新建会话"),
            Self::Search => tr("搜索会话"),
            Self::CommandPalette => tr("快捷命令面板"),
            Self::Find => tr("查找会话内容"),
            Self::PreviousSession => tr("上一个会话"),
            Self::NextSession => tr("下一个会话"),
            Self::FocusNextWorkspaceArea => tr("聚焦下一个工作区区域"),
            Self::FocusPreviousWorkspaceArea => tr("聚焦上一个工作区区域"),
            Self::ToggleSidebar => tr("切换侧边栏"),
            Self::ToggleTerminal => tr("切换终端面板"),
            Self::OpenGitWorkspace => tr("打开 Git 工作区"),
            Self::SearchProjectFiles => tr("搜索项目文件"),
            Self::ChooseModel => tr("选择模型"),
            Self::IncreaseReasoningEffort => tr("提高推理强度"),
            Self::DecreaseReasoningEffort => tr("降低推理强度"),
            Self::ToggleMode => tr("切换会话模式"),
            Self::PromptStash => tr("草稿暂存"),
            Self::SendAndStartDraft => tr("发送并开始新草稿"),
            Self::Preferences => tr("打开设置"),
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::NewSession => tr("新建会话；有多个项目时先选择项目"),
            Self::NewSessionCurrentProject => tr("直接在当前项目中创建会话"),
            Self::Search => tr("打开全局会话搜索窗口"),
            Self::CommandPalette => tr("打开全局快捷命令面板，快速切换会话、项目、面板与设置"),
            Self::Find => tr("在当前对话消息中搜索内容"),
            Self::PreviousSession => tr("按侧边栏顺序切换到上一条会话"),
            Self::NextSession => tr("按侧边栏顺序切换到下一条会话"),
            Self::FocusNextWorkspaceArea => tr("将焦点移到下一个可见工作区区域"),
            Self::FocusPreviousWorkspaceArea => tr("将焦点移到上一个可见工作区区域"),
            Self::ToggleSidebar => tr("展开或收起左侧工作区与会话导航栏"),
            Self::ToggleTerminal => tr("打开或收起当前工作区的终端面板"),
            Self::OpenGitWorkspace => tr("查看当前工作区 Git 状态与 diff"),
            Self::SearchProjectFiles => tr("打开文件选择器并插入工作区文件引用"),
            Self::ChooseModel => tr("展开或关闭当前会话的模型选择器"),
            Self::IncreaseReasoningEffort => tr("切换到当前会话的下一档推理强度"),
            Self::DecreaseReasoningEffort => tr("切换到当前会话的上一档推理强度"),
            Self::ToggleMode => tr("在当前 Agent 支持的会话模式间切换"),
            Self::PromptStash => tr("暂存当前草稿，或从空输入框恢复"),
            Self::SendAndStartDraft => tr("从空闲会话发送消息，然后打开一个新草稿"),
            Self::Preferences => tr("打开应用设置面板"),
        }
    }

    pub(crate) fn anchor_id(self) -> &'static str {
        match self {
            Self::NewSession => "shortcut-new-session",
            Self::NewSessionCurrentProject => "shortcut-new-session-current-project",
            Self::Search => "shortcut-search",
            Self::CommandPalette => "shortcut-command-palette",
            Self::Find => "shortcut-find",
            Self::PreviousSession => "shortcut-previous-session",
            Self::NextSession => "shortcut-next-session",
            Self::FocusNextWorkspaceArea => "shortcut-focus-next-workspace-area",
            Self::FocusPreviousWorkspaceArea => "shortcut-focus-previous-workspace-area",
            Self::ToggleSidebar => "shortcut-toggle-sidebar",
            Self::ToggleTerminal => "shortcut-toggle-terminal",
            Self::OpenGitWorkspace => "shortcut-open-git",
            Self::SearchProjectFiles => "shortcut-project-files",
            Self::ChooseModel => "shortcut-choose-model",
            Self::IncreaseReasoningEffort => "shortcut-increase-reasoning-effort",
            Self::DecreaseReasoningEffort => "shortcut-decrease-reasoning-effort",
            Self::ToggleMode => "shortcut-toggle-mode",
            Self::PromptStash => "shortcut-prompt-stash",
            Self::SendAndStartDraft => "shortcut-send-and-start-draft",
            Self::Preferences => "shortcut-preferences",
        }
    }

    pub(crate) fn default(self, os: OperatingSystem) -> Option<&'static str> {
        let mac = os == OperatingSystem::Mac;
        Some(match (self, mac) {
            (Self::IncreaseReasoningEffort | Self::DecreaseReasoningEffort, _) => return None,
            (Self::NewSession, true) => "cmd-n",
            (Self::NewSession, false) => "ctrl-n",
            (Self::NewSessionCurrentProject, true) => "cmd-shift-n",
            (Self::NewSessionCurrentProject, false) => "ctrl-shift-n",
            (Self::Search, true) => "cmd-k",
            (Self::Search, false) => "ctrl-k",
            (Self::CommandPalette, true) => "cmd-shift-p",
            (Self::CommandPalette, false) => "ctrl-shift-p",
            (Self::Find, true) => "cmd-f",
            (Self::Find, false) => "ctrl-f",
            (Self::PreviousSession, true) => "cmd-;",
            (Self::PreviousSession, false) => "ctrl-;",
            (Self::NextSession, true) => "cmd-'",
            (Self::NextSession, false) => "ctrl-'",
            (Self::FocusNextWorkspaceArea, _) => "f6",
            (Self::FocusPreviousWorkspaceArea, _) => "shift-f6",
            (Self::ToggleSidebar, true) => "cmd-b",
            (Self::ToggleSidebar, false) => "ctrl-b",
            (Self::ToggleTerminal, true) => "cmd-`",
            (Self::ToggleTerminal, false) => "ctrl-`",
            (Self::OpenGitWorkspace, true) => "cmd-shift-g",
            (Self::OpenGitWorkspace, false) => "ctrl-shift-g",
            (Self::SearchProjectFiles, true) => "cmd-p",
            (Self::SearchProjectFiles, false) => "ctrl-p",
            (Self::ChooseModel, true) => "cmd-shift-m",
            (Self::ChooseModel, false) => "ctrl-shift-m",
            (Self::ToggleMode, true) => "cmd-/",
            (Self::ToggleMode, false) => "ctrl-/",
            (Self::PromptStash, true) => "cmd-s",
            (Self::PromptStash, false) => "ctrl-s",
            (Self::SendAndStartDraft, true) => "alt-cmd-enter",
            (Self::SendAndStartDraft, false) => "ctrl-alt-enter",
            (Self::Preferences, true) => "cmd-,",
            (Self::Preferences, false) => "ctrl-,",
        })
    }

    fn validate(self, chord: &str, os: OperatingSystem) -> Result<Keystroke, &'static str> {
        validate_chord_inner(chord, os, Some(self))
    }

    fn context(self) -> Option<std::rc::Rc<KeyBindingContextPredicate>> {
        let predicate = if self.composer_only() {
            "HiveComposer"
        } else if self.workspace_focus() {
            "!HiveTerminal"
        } else {
            return None;
        };
        Some(KeyBindingContextPredicate::parse(predicate).expect("valid shortcut context").into())
    }

    fn workspace_focus(self) -> bool {
        matches!(self, Self::FocusNextWorkspaceArea | Self::FocusPreviousWorkspaceArea)
    }

    fn composer_only(self) -> bool {
        matches!(
            self,
            Self::PromptStash | Self::SendAndStartDraft | Self::IncreaseReasoningEffort | Self::DecreaseReasoningEffort
        )
    }

    pub(crate) fn context_scope(self) -> ContextScope {
        if self.composer_only() {
            ContextScope::Composer
        } else if self.workspace_focus() {
            ContextScope::WorkspaceNonTerminal
        } else {
            ContextScope::Global
        }
    }

    fn action(self) -> Box<dyn Action> {
        match self {
            Self::NewSession => Box::new(crate::NewSessionShortcut),
            Self::NewSessionCurrentProject => Box::new(NewSessionInCurrentProject),
            Self::Search => Box::new(crate::OpenSessionSearch),
            Self::CommandPalette => Box::new(crate::OpenCommandPalette),
            Self::Find => Box::new(crate::OpenConversationFind),
            Self::PreviousSession => Box::new(crate::SelectPreviousSession),
            Self::NextSession => Box::new(crate::SelectNextSession),
            Self::FocusNextWorkspaceArea => Box::new(FocusNextWorkspaceArea),
            Self::FocusPreviousWorkspaceArea => Box::new(FocusPreviousWorkspaceArea),
            Self::ToggleSidebar => Box::new(crate::ToggleSidebar),
            Self::ToggleTerminal => Box::new(crate::ToggleWorkspaceTerminal),
            Self::OpenGitWorkspace => Box::new(crate::OpenGitWorkspaceShortcut),
            Self::SearchProjectFiles => Box::new(crate::OpenProjectFilePicker),
            Self::ChooseModel => Box::new(crate::OpenModelPicker),
            Self::IncreaseReasoningEffort => Box::new(IncreaseReasoningEffort),
            Self::DecreaseReasoningEffort => Box::new(DecreaseReasoningEffort),
            Self::ToggleMode => Box::new(crate::ToggleSessionMode),
            Self::PromptStash => Box::new(PromptStash),
            Self::SendAndStartDraft => Box::new(SendAndStartDraft),
            Self::Preferences => Box::new(crate::OpenPreferences),
        }
    }

}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Bindings {
    pub search: Option<String>,
    pub command_palette: Option<String>,
    pub find: Option<String>,
    pub preferences: Option<String>,
    pub new_session: Option<String>,
    pub new_session_current_project: Option<String>,
    pub focus_next_workspace_area: Option<String>,
    pub focus_previous_workspace_area: Option<String>,
    pub toggle_sidebar: Option<String>,
    pub toggle_terminal: Option<String>,
    pub open_git_workspace: Option<String>,
    pub search_project_files: Option<String>,
    pub choose_model: Option<String>,
    pub increase_reasoning_effort: Option<String>,
    pub decrease_reasoning_effort: Option<String>,
    pub toggle_mode: Option<String>,
    pub prompt_stash: Option<String>,
    pub send_and_start_draft: Option<String>,
    pub previous_session: Option<String>,
    pub next_session: Option<String>,
}

fn some_present<'de, D>(de: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(de).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredBindings {
    search: Option<String>,
    #[serde(default, deserialize_with = "some_present")]
    command_palette: Option<Option<String>>,
    find: Option<String>,
    preferences: Option<String>,
    #[serde(default, deserialize_with = "some_present")]
    new_session: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    new_session_current_project: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    focus_next_workspace_area: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    focus_previous_workspace_area: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    toggle_sidebar: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    toggle_terminal: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    open_git_workspace: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    search_project_files: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    choose_model: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    increase_reasoning_effort: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    decrease_reasoning_effort: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    toggle_mode: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    prompt_stash: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    send_and_start_draft: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    previous_session: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_present")]
    next_session: Option<Option<String>>,
}

impl StoredBindings {
    fn into_bindings(
        self,
        os: OperatingSystem,
        dispatch_key: impl Fn(Keystroke) -> Keystroke,
    ) -> Result<Bindings, &'static str> {
        let mut bindings = Bindings::defaults(os);
        let mut missing = Vec::new();
        for (target, value) in [
            (Target::Search, Some(self.search)),
            (Target::CommandPalette, self.command_palette),
            (Target::Find, Some(self.find)),
            (Target::Preferences, Some(self.preferences)),
            (Target::NewSession, self.new_session),
            (Target::NewSessionCurrentProject, self.new_session_current_project),
            (Target::FocusNextWorkspaceArea, self.focus_next_workspace_area),
            (Target::FocusPreviousWorkspaceArea, self.focus_previous_workspace_area),
            (Target::ToggleSidebar, self.toggle_sidebar),
            (Target::ToggleTerminal, self.toggle_terminal),
            (Target::OpenGitWorkspace, self.open_git_workspace),
            (Target::SearchProjectFiles, self.search_project_files),
            (Target::ChooseModel, self.choose_model),
            (Target::IncreaseReasoningEffort, self.increase_reasoning_effort),
            (Target::DecreaseReasoningEffort, self.decrease_reasoning_effort),
            (Target::ToggleMode, self.toggle_mode),
            (Target::PromptStash, self.prompt_stash),
            (Target::SendAndStartDraft, self.send_and_start_draft),
            (Target::PreviousSession, self.previous_session),
            (Target::NextSession, self.next_session),
        ] {
            if value.is_none() {
                missing.push(target);
            }
            bindings.set(target, value.flatten());
        }

        let mut bindings = bindings.normalized(os)?;
        let mut claimed = Vec::new();
        // Previously, custom bindings were appended after the original defaults.
        // GPUI gives the most recently registered binding priority.
        for use_default in [false, true] {
            for target in TARGETS.into_iter().rev() {
                if let Some(raw) = bindings.get(target)
                    && (Some(raw.as_str()) == target.default(os)) == use_default
                {
                    let chord = dispatch_key(target.validate(raw, os)?);
                    if claimed.contains(&chord) {
                        bindings.set(target, None);
                    } else {
                        claimed.push(chord);
                    }
                }
            }
        }
        // Newly introduced defaults yield to saved bindings on the same key.
        for target in missing {
            let Some(default) = target.default(os) else {
                continue;
            };
            let chord = target.validate(default, os)?;
            let effective = dispatch_key(chord.clone());
            if !claimed.contains(&effective) {
                claimed.push(effective);
                bindings.set(target, Some(chord.unparse(os)));
            }
        }
        Ok(bindings)
    }
}

impl Bindings {
    pub(crate) fn defaults(os: OperatingSystem) -> Self {
        Self {
            search: Target::Search.default(os).map(str::to_owned),
            command_palette: Target::CommandPalette.default(os).map(str::to_owned),
            find: Target::Find.default(os).map(str::to_owned),
            preferences: Target::Preferences.default(os).map(str::to_owned),
            new_session: Target::NewSession.default(os).map(str::to_owned),
            new_session_current_project: Target::NewSessionCurrentProject.default(os).map(str::to_owned),
            focus_next_workspace_area: Target::FocusNextWorkspaceArea.default(os).map(str::to_owned),
            focus_previous_workspace_area: Target::FocusPreviousWorkspaceArea.default(os).map(str::to_owned),
            toggle_sidebar: Target::ToggleSidebar.default(os).map(str::to_owned),
            toggle_terminal: Target::ToggleTerminal.default(os).map(str::to_owned),
            open_git_workspace: Target::OpenGitWorkspace.default(os).map(str::to_owned),
            search_project_files: Target::SearchProjectFiles.default(os).map(str::to_owned),
            choose_model: Target::ChooseModel.default(os).map(str::to_owned),
            increase_reasoning_effort: Target::IncreaseReasoningEffort.default(os).map(str::to_owned),
            decrease_reasoning_effort: Target::DecreaseReasoningEffort.default(os).map(str::to_owned),
            toggle_mode: Target::ToggleMode.default(os).map(str::to_owned),
            prompt_stash: Target::PromptStash.default(os).map(str::to_owned),
            send_and_start_draft: Target::SendAndStartDraft.default(os).map(str::to_owned),
            previous_session: Target::PreviousSession.default(os).map(str::to_owned),
            next_session: Target::NextSession.default(os).map(str::to_owned),
        }
    }

    pub(crate) fn get(&self, target: Target) -> &Option<String> {
        match target {
            Target::NewSession => &self.new_session,
            Target::NewSessionCurrentProject => &self.new_session_current_project,
            Target::Search => &self.search,
            Target::CommandPalette => &self.command_palette,
            Target::Find => &self.find,
            Target::PreviousSession => &self.previous_session,
            Target::NextSession => &self.next_session,
            Target::FocusNextWorkspaceArea => &self.focus_next_workspace_area,
            Target::FocusPreviousWorkspaceArea => &self.focus_previous_workspace_area,
            Target::ToggleSidebar => &self.toggle_sidebar,
            Target::ToggleTerminal => &self.toggle_terminal,
            Target::OpenGitWorkspace => &self.open_git_workspace,
            Target::SearchProjectFiles => &self.search_project_files,
            Target::ChooseModel => &self.choose_model,
            Target::IncreaseReasoningEffort => &self.increase_reasoning_effort,
            Target::DecreaseReasoningEffort => &self.decrease_reasoning_effort,
            Target::ToggleMode => &self.toggle_mode,
            Target::PromptStash => &self.prompt_stash,
            Target::SendAndStartDraft => &self.send_and_start_draft,
            Target::Preferences => &self.preferences,
        }
    }

    pub(crate) fn set(&mut self, target: Target, value: Option<String>) {
        let slot = match target {
            Target::NewSession => &mut self.new_session,
            Target::NewSessionCurrentProject => &mut self.new_session_current_project,
            Target::Search => &mut self.search,
            Target::CommandPalette => &mut self.command_palette,
            Target::Find => &mut self.find,
            Target::PreviousSession => &mut self.previous_session,
            Target::NextSession => &mut self.next_session,
            Target::FocusNextWorkspaceArea => &mut self.focus_next_workspace_area,
            Target::FocusPreviousWorkspaceArea => &mut self.focus_previous_workspace_area,
            Target::ToggleSidebar => &mut self.toggle_sidebar,
            Target::ToggleTerminal => &mut self.toggle_terminal,
            Target::OpenGitWorkspace => &mut self.open_git_workspace,
            Target::SearchProjectFiles => &mut self.search_project_files,
            Target::ChooseModel => &mut self.choose_model,
            Target::IncreaseReasoningEffort => &mut self.increase_reasoning_effort,
            Target::DecreaseReasoningEffort => &mut self.decrease_reasoning_effort,
            Target::ToggleMode => &mut self.toggle_mode,
            Target::PromptStash => &mut self.prompt_stash,
            Target::SendAndStartDraft => &mut self.send_and_start_draft,
            Target::Preferences => &mut self.preferences,
        };
        *slot = value;
    }

    pub(crate) fn normalized(&self, os: OperatingSystem) -> Result<Self, &'static str> {
        let mut next = self.clone();
        let mut seen: Vec<(Keystroke, ContextScope)> = Vec::with_capacity(TARGETS.len());
        for target in TARGETS {
            if let Some(raw) = self.get(target) {
                let key = target.validate(raw, os)?;
                let scope = target.context_scope();
                if seen
                    .iter()
                    .any(|(existing, existing_scope)| existing == &key && scope.overlaps(*existing_scope))
                {
                    return Err(CHORD_CONFLICT);
                }
                seen.push((key.clone(), scope));
                next.set(target, Some(key.unparse(os)));
            }
        }
        Ok(next)
    }

    fn preserve_pending(&self, saved: &Self, draft: &Self) -> Self {
        let mut next = self.clone();
        for target in TARGETS {
            if draft.get(target) != saved.get(target) {
                next.set(target, draft.get(target).clone());
            }
        }
        next
    }
}

#[cfg(test)]
fn validate_chord(value: &str, os: OperatingSystem) -> Result<Keystroke, &'static str> {
    validate_chord_inner(value, os, None)
}

fn validate_chord_inner(
    value: &str,
    os: OperatingSystem,
    target: Option<Target>,
) -> Result<Keystroke, &'static str> {
    if value.is_empty()
        || value.len() > 80
        || value.split_whitespace().count() != 1
        || value.contains("->")
    {
        return Err("请录制一个带修饰键的组合键，不支持连续按键。");
    }
    let key = Keystroke::parse_with_os(value, os).map_err(|_| "快捷键格式无效，请重新录制。")?;
    if target == Some(Target::SendAndStartDraft)
        && Some(key.unparse(os).as_str()) == Target::SendAndStartDraft.default(os)
    {
        return Ok(key);
    }
    let m = key.modifiers;
    if target.is_some_and(Target::workspace_focus)
        && key.key == "f6"
        && (m == Modifiers::none() || m == Modifiers::shift())
    {
        return Ok(key);
    }
    if (os == OperatingSystem::Mac && (!m.platform || m.control))
        || (os != OperatingSystem::Mac && (!m.control || m.platform || m.alt))
        || m.function
    {
        return Err("macOS 请使用 Command 组合键；Windows 请使用 Ctrl 组合，不使用 Win 或 AltGr。");
    }
    let useful = key.key.len() == 1 && key.key.as_bytes()[0].is_ascii_graphic();
    let screenshot =
        os == OperatingSystem::Mac && m.shift && matches!(key.key.as_str(), "3" | "4" | "5" | "6");
    let dock = os == OperatingSystem::Mac && m.alt && key.key == "d";
    let screenshot_symbol =
        os == OperatingSystem::Mac && matches!(key.key.as_str(), "#" | "$" | "%" | "^");
    if !useful
        || screenshot
        || screenshot_symbol
        || dock
        || (!m.shift && matches!(key.key.as_str(), "m" | "o"))
        || (!m.shift && key.key == "s"
            && !(target == Some(Target::PromptStash) && Some(key.unparse(os).as_str()) == Target::PromptStash.default(os)))
        || matches!(
            key.key.as_str(),
            "a" | "c" | "i" | "u" | "v" | "x" | "z" | "y" | "q" | "w" | "h"
        )
    {
        return Err("此组合键保留给系统、文字编辑或导航，请选择其他组合。");
    }
    Ok(key)
}

// Some layouts normalize Shift+punctuation into a different key and clear
// Shift. Releasing the primary modifier is a layout-independent completion
// boundary if the corresponding KeyUp used a different spelling.
fn primary_released(candidate: &Keystroke, modifiers: Modifiers) -> bool {
    (candidate.modifiers.platform && !modifiers.platform)
        || (candidate.modifiers.control && !modifiers.control)
}

#[derive(Serialize)]
struct StoredSave<'a> {
    version: u32,
    bindings: &'a Bindings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRead {
    #[allow(dead_code)]
    version: u32,
    bindings: StoredBindings,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn load(
    root: &Path,
    os: OperatingSystem,
    dispatch_key: impl Fn(Keystroke) -> Keystroke,
) -> io::Result<Bindings> {
    let file = match fs::File::open(root.join(FILE)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Bindings::defaults(os)),
        Err(error) => return Err(error),
    };
    let value: serde_json::Value = serde_json::from_reader(file)?;
    if value["version"] != 1 {
        return Err(invalid("快捷键文件版本不受支持，原文件未修改。"));
    }
    if !value["bindings"].as_object().is_some_and(|object| {
        ["search", "find", "preferences"]
            .iter()
            .all(|key| object.contains_key(*key))
    }) {
        return Err(invalid("快捷键文件缺少操作字段，原文件未修改。"));
    }
    let stored: StoredRead = serde_json::from_value(value)?;
    stored.bindings.into_bindings(os, dispatch_key).map_err(invalid)
}

fn save(root: &Path, bindings: &Bindings, os: OperatingSystem) -> io::Result<()> {
    let bindings = bindings.normalized(os).map_err(invalid)?;
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(
        file.as_file_mut(),
        &StoredSave {
            version: 1,
            bindings: &bindings,
        },
    )?;
    file.as_file().sync_all()?;
    file.persist(root.join(FILE))?;
    Ok(())
}

pub(crate) fn startup_bindings(cx: &App) -> Vec<KeyBinding> {
    let os = cx.platform_info().os;
    let mut bindings: Vec<_> = TARGETS
        .into_iter()
        .filter_map(|target| {
            let chord = target.default(os)?;
            Some(
                KeyBinding::load(chord, target.action(), target.context(), false, None, cx)
                    .expect("valid default Hive shortcut"),
            )
        })
        .collect();
    let (next, previous) = if os == OperatingSystem::Mac {
        ("cmd-f6", "cmd-shift-f6")
    } else {
        ("ctrl-f6", "ctrl-shift-f6")
    };
    let context = KeyBindingContextPredicate::parse("HiveTerminal").expect("valid terminal context");
    for (chord, action) in [
        (next, Box::new(FocusNextFromTerminal) as Box<dyn Action>),
        (previous, Box::new(FocusPreviousFromTerminal)),
    ] {
        bindings.push(
            KeyBinding::load(chord, action, Some(context.clone().into()), false, None, cx)
                .expect("valid terminal focus shortcut"),
        );
    }
    bindings
}

fn managed_scoped_binding(binding: &KeyBinding) -> bool {
    TARGETS.into_iter().any(|target| {
        (target.composer_only() || target.workspace_focus())
            && target.action().name() == binding.action().name()
    })
}

fn protected_binding(binding: &KeyBinding) -> bool {
    if managed_scoped_binding(binding) {
        return false;
    }
    binding.predicate().is_some() || !TARGETS
        .into_iter()
        .any(|target| target.action().name() == binding.action().name())
}

fn dispatch_key(key: Keystroke, cx: &App) -> Keystroke {
    KeybindingKeystroke::new_with_mapper(key, false, cx.keyboard_mapper().as_ref())
        .inner()
        .clone()
}

fn canonical_stacks_for_scope(scope: ContextScope) -> Vec<Vec<KeyContext>> {
    let parse = |names: &[&str]| -> Vec<KeyContext> {
        names
            .iter()
            .filter_map(|name| KeyContext::parse(name).ok())
            .collect()
    };
    let ws = parse(&["HiveWorkspace"]);
    let ws_conv = parse(&["HiveWorkspace", "HiveConversation"]);
    let composer = parse(&["HiveWorkspace", "HiveConversation", "HiveComposer", "Input", "Textarea"]);
    let queued_inputs = parse(&["HiveWorkspace", "HiveConversation", "HiveQueuedInputs"]);
    let diff_find = parse(&["HiveWorkspace", "HiveConversation", "HiveGitDiff", "HiveGitDiffFind", "Input"]);
    let terminal = parse(&["HiveWorkspace", "HiveConversation", "HiveTerminal"]);
    match scope {
        ContextScope::Global => vec![ws, ws_conv, composer, queued_inputs, diff_find, terminal],
        ContextScope::WorkspaceNonTerminal => {
            vec![ws, ws_conv, composer, queued_inputs, diff_find]
        }
        ContextScope::Composer => vec![composer],
        #[cfg(test)]
        ContextScope::Terminal => vec![terminal],
    }
}

pub(crate) fn predicate_overlaps_scope(
    pred: Option<&KeyBindingContextPredicate>,
    scope: ContextScope,
) -> bool {
    let Some(pred) = pred else {
        return true;
    };
    canonical_stacks_for_scope(scope)
        .iter()
        .any(|stack| pred.depth_of(stack).is_some())
}

fn protected_reason_in_scope(
    key: &Keystroke,
    scope: ContextScope,
    cx: &App,
) -> Option<&'static str> {
    cx.all_bindings_for_input(std::slice::from_ref(key))
        .iter()
        .find(|binding| {
            protected_binding(binding) && predicate_overlaps_scope(binding.predicate().as_deref(), scope)
        })
        .map(|binding| {
            let name = binding.action().name();
            if name.contains("Quit") {
                "退出应用"
            } else if name.contains("input") || name.contains("textarea") {
                "文本编辑组件"
            } else {
                "现有组件或终端"
            }
        })
}

pub(crate) fn analyze_conflicts(
    bindings: &Bindings,
    os: OperatingSystem,
    dispatch_key: impl Fn(Keystroke) -> Keystroke,
    protected_for: impl Fn(&Keystroke, ContextScope) -> Option<&'static str>,
) -> Vec<TargetConflict> {
    let mut resolved: Vec<(Target, Keystroke, ContextScope, String)> = Vec::new();
    for target in TARGETS {
        let Some(chord) = bindings.get(target) else {
            continue;
        };
        let Ok(validated) = target.validate(chord, os) else {
            continue;
        };
        resolved.push((
            target,
            dispatch_key(validated),
            target.context_scope(),
            chord.clone(),
        ));
    }

    let mut conflicts = Vec::new();
    for (i, (target_a, key_a, scope_a, chord_a)) in resolved.iter().enumerate() {
        for (j, (target_b, key_b, scope_b, _)) in resolved.iter().enumerate() {
            if i == j || key_a != key_b || !scope_a.overlaps(*scope_b) {
                continue;
            }
            let kind = if scope_a == scope_b {
                ConflictKind::SameScope(*target_b)
            } else {
                ConflictKind::OverlappingScope(*target_b)
            };
            conflicts.push(TargetConflict {
                target: *target_a,
                kind,
                chord: chord_a.clone(),
            });
        }
        let existing_default = TARGETS
            .into_iter()
            .any(|existing| Some(chord_a.as_str()) == existing.default(os));
        if !existing_default && let Some(component_label) = protected_for(key_a, *scope_a) {
            conflicts.push(TargetConflict {
                target: *target_a,
                kind: ConflictKind::ProtectedComponent(component_label),
                chord: chord_a.clone(),
            });
        }
    }
    conflicts
}

#[cfg(test)]
fn check_conflicts(
    bindings: &Bindings,
    os: OperatingSystem,
    dispatch_key: impl Fn(Keystroke) -> Keystroke,
    is_protected: impl Fn(&Keystroke) -> bool,
) -> Result<(), &'static str> {
    let mut claimed: Vec<(Keystroke, ContextScope)> = Vec::with_capacity(TARGETS.len());
    for target in TARGETS {
        if let Some(chord) = bindings.get(target) {
            let key = dispatch_key(target.validate(chord, os)?);
            let scope = target.context_scope();
            if claimed
                .iter()
                .any(|(existing, existing_scope)| existing == &key && scope.overlaps(*existing_scope))
            {
                return Err(CHORD_CONFLICT);
            }
            // These existing Hive global defaults already own their chords.
            let existing_default = TARGETS
                .into_iter()
                .any(|existing| Some(chord.as_str()) == existing.default(os));
            if !existing_default && is_protected(&key) {
                return Err("此组合键已用于现有组件或文字编辑，请选择其他组合。");
            }
            claimed.push((key, scope));
        }
    }
    Ok(())
}

fn check_registered(bindings: &Bindings, cx: &App) -> Result<(), &'static str> {
    for target in TARGETS {
        if let Some(chord) = bindings.get(target) {
            target.validate(chord, cx.platform_info().os)?;
        }
    }
    let conflicts = analyze_conflicts(
        bindings,
        cx.platform_info().os,
        |key| dispatch_key(key, cx),
        |key, scope| protected_reason_in_scope(key, scope, cx),
    );
    if let Some(first) = conflicts.first() {
        return Err(match first.kind {
            ConflictKind::ProtectedComponent(_) => {
                "此组合键已用于现有组件或文字编辑，请选择其他组合。"
            }
            ConflictKind::SameScope(_) | ConflictKind::OverlappingScope(_) => CHORD_CONFLICT,
        });
    }
    Ok(())
}

type BindingChanges = Vec<(&'static str, Option<KeyBinding>)>;

fn live_delta(prev: &Bindings, next: &Bindings, cx: &App) -> Result<(Bindings, BindingChanges), String> {
    let os = cx.platform_info().os;
    let normalized = next.normalized(os).map_err(str::to_owned)?;
    check_registered(&normalized, cx).map_err(str::to_owned)?;
    let mut delta = Vec::new();
    for target in TARGETS {
        if prev.get(target) == normalized.get(target) {
            continue;
        }
        let binding = normalized.get(target).as_ref()
            .map(|chord| KeyBinding::load(chord, target.action(), target.context(), false, None, cx))
            .transpose().map_err(|error| error.to_string())?;
        delta.push((target.action().name(), binding));
    }
    Ok((normalized, delta))
}

fn replace_bindings(keymap: &mut Keymap, changes: BindingChanges) {
    if changes.is_empty() {
        return;
    }
    keymap.retain_bindings(|binding| {
        (binding.predicate().is_some() && !managed_scoped_binding(binding))
            || !changes.iter().any(|(action, _)| *action == binding.action().name())
    });
    keymap.add_bindings(changes.into_iter().filter_map(|(_, binding)| binding));
}

fn full_delta(bindings: &Bindings, cx: &App) -> Result<BindingChanges, String> {
    TARGETS.into_iter().map(|target| {
        let binding = bindings.get(target).as_ref()
            .map(|chord| KeyBinding::load(chord, target.action(), target.context(), false, None, cx))
            .transpose().map_err(|error| error.to_string())?;
        Ok((target.action().name(), binding))
    }).collect()
}

fn remap_layout(cx: &mut App) {
    let active = cx.global::<Preferences>().active.clone();
    let result = check_registered(&active, cx).map_err(str::to_owned)
        .and_then(|()| full_delta(&active, cx));
    match result {
        Ok(changes) => {
            replace_bindings(&mut cx.key_bindings().borrow_mut(), changes);
            cx.global_mut::<Preferences>().record_layout_result(Ok(()));
        }
        Err(error) => cx.global_mut::<Preferences>().record_layout_result(Err(error)),
    }
    cx.refresh_windows();
}

enum Notice {
    Read(String),
    Save(String),
    Saved,
}

pub(crate) struct Preferences {
    active: Bindings,
    saved: Bindings,
    draft: Bindings,
    root: Option<PathBuf>,
    load_failed: bool,
    layout_blocked: bool,
    layout_error: Option<String>,
    applied: bool,
    notice: Option<Notice>,
    preview: bool,
}

impl Global for Preferences {}

impl Preferences {
    fn record_layout_result(&mut self, result: Result<(), String>) {
        self.layout_blocked = result.is_err();
        self.layout_error = result.err();
    }

    fn persist_pending(&mut self, os: OperatingSystem) {
        if self.load_failed || self.preview {
            return;
        }
        let Some(root) = &self.root else {
            return;
        };
        match save(root, &self.draft, os) {
            Ok(()) => {
                self.saved = self.draft.clone();
                self.notice = Some(Notice::Saved);
            }
            Err(error) => self.notice = Some(Notice::Save(error.to_string())),
        }
    }

    fn new(os: OperatingSystem) -> Self {
        let defaults = Bindings::defaults(os);
        Self {
            active: defaults.clone(),
            saved: defaults.clone(),
            draft: defaults,
            root: None,
            load_failed: false,
            layout_blocked: false,
            layout_error: None,
            applied: false,
            notice: None,
            preview: cfg!(feature = "ui-preview")
                && std::env::var("HIVE_DESKTOP_PREVIEW_SCREEN")
                    .is_ok_and(|value| !value.trim().is_empty()),
        }
    }
}

pub(crate) fn init(cx: &mut App) {
    cx.set_global(Preferences::new(cx.platform_info().os));
    cx.on_keyboard_layout_change(remap_layout).detach();
}

pub(crate) fn restore(root: &Path, cx: &mut App) {
    if cx.global::<Preferences>().root.is_some() {
        return;
    }
    // Replace only changed Hive bindings. Component and input bindings stay registered.
    let active_before = cx.global::<Preferences>().active.clone();
    let layout_blocked = cx.global::<Preferences>().layout_blocked;
    let loaded = load(root, cx.platform_info().os, |key| dispatch_key(key, cx))
        .and_then(|bindings| live_delta(&active_before, &bindings, cx).map_err(io::Error::other))
        .and_then(|(bindings, delta)| {
            if layout_blocked { Ok((bindings.clone(), full_delta(&bindings, cx).map_err(io::Error::other)?)) }
            else { Ok((bindings, delta)) }
        });
    let state = cx.global_mut::<Preferences>();
    state.root = Some(root.to_owned());
    if state.applied {
        return;
    }
    state.applied = true;
    match loaded {
        Ok((bindings, delta)) => {
            let draft = bindings.preserve_pending(&state.saved, &state.draft);
            state.active = bindings.clone();
            state.record_layout_result(Ok(()));
            state.saved = bindings;
            state.draft = draft;
            replace_bindings(&mut cx.key_bindings().borrow_mut(), delta);
        }
        Err(error) => {
            state.load_failed = true;
            state.notice = Some(Notice::Read(error.to_string()));
        }
    }
    cx.refresh_windows();
}

fn reload(cx: &mut App) {
    let Some(root) = cx.global::<Preferences>().root.clone() else {
        return;
    };
    let active_before = cx.global::<Preferences>().active.clone();
    let layout_blocked = cx.global::<Preferences>().layout_blocked;
    let loaded = load(&root, cx.platform_info().os, |key| dispatch_key(key, cx))
        .and_then(|bindings| live_delta(&active_before, &bindings, cx).map_err(io::Error::other))
        .and_then(|(bindings, delta)| {
            if layout_blocked { Ok((bindings.clone(), full_delta(&bindings, cx).map_err(io::Error::other)?)) }
            else { Ok((bindings, delta)) }
        });
    match loaded {
        Ok((bindings, delta)) => {
            let state = cx.global_mut::<Preferences>();
            state.active = bindings.clone();
            state.saved = bindings.clone();
            state.draft = bindings;
            state.load_failed = false;
            state.record_layout_result(Ok(()));
            state.notice = None;
            replace_bindings(&mut cx.key_bindings().borrow_mut(), delta);
        }
        Err(error) => {
            let state = cx.global_mut::<Preferences>();
            state.load_failed = true;
            state.notice = Some(Notice::Read(error.to_string()));
        }
    }
    cx.refresh_windows();
}

fn save_pending(cx: &mut App) {
    let (draft, active_before, load_failed, preview, has_root, layout_blocked) = {
        let state = cx.global::<Preferences>();
        (
            state.draft.clone(),
            state.active.clone(),
            state.load_failed,
            state.preview,
            state.root.is_some(),
            state.layout_blocked,
        )
    };
    if load_failed {
        return;
    }
    let result = live_delta(&active_before, &draft, cx).and_then(|(normalized, delta)| {
        if layout_blocked { Ok((normalized.clone(), full_delta(&normalized, cx)?)) }
        else { Ok((normalized, delta)) }
    });
    let os = cx.platform_info().os;
    match result {
        Ok((normalized, delta)) => {
            if preview {
                let state = cx.global_mut::<Preferences>();
                state.active = normalized.clone();
                state.saved = normalized.clone();
                state.draft = normalized;
                state.notice = Some(Notice::Saved);
                state.record_layout_result(Ok(()));
                replace_bindings(&mut cx.key_bindings().borrow_mut(), delta);
            } else if has_root {
                let saved_ok = {
                    let state = cx.global_mut::<Preferences>();
                    state.draft = normalized;
                    state.persist_pending(os);
                    matches!(state.notice, Some(Notice::Saved))
                };
                if saved_ok {
                    let state = cx.global_mut::<Preferences>();
                    state.active = state.saved.clone();
                    state.record_layout_result(Ok(()));
                    replace_bindings(&mut cx.key_bindings().borrow_mut(), delta);
                }
            }
        }
        Err(error) => {
            cx.global_mut::<Preferences>().notice = Some(Notice::Save(error));
        }
    }
    cx.refresh_windows();
}

fn explain_error(error: &str) -> String {
    let mut text = crate::language::localize_owned_error(error);
    for message in [
        "快捷键文件版本不受支持，原文件未修改。",
        "快捷键文件缺少操作字段，原文件未修改。",
        CHORD_CONFLICT,
        "请录制一个带修饰键的组合键，不支持连续按键。",
        "快捷键格式无效，请重新录制。",
        "macOS 请使用 Command 组合键；Windows 请使用 Ctrl 组合，不使用 Win 或 AltGr。",
        "此组合键保留给系统、文字编辑或导航，请选择其他组合。",
        "此组合键已用于现有组件或文字编辑，请选择其他组合。",
    ] {
        text = text.replace(message, tr(message));
    }
    text
}

fn display(value: &Option<String>, os: OperatingSystem) -> String {
    value
        .as_ref()
        .and_then(|raw| Keystroke::parse_with_os(raw, os).ok())
        .map(|key| Kbd::format(&key))
        .unwrap_or_else(|| tr("快捷键已停用").to_owned())
}

pub(crate) fn hint(target: Target, label: &'static str, cx: &App) -> String {
    match cx.global::<Preferences>().active.get(target) {
        Some(raw) => {
            let formatted = display(&Some(raw.clone()), cx.platform_info().os);
            format!("{} ({formatted})", tr(label))
        }
        None => tr(label).to_owned(),
    }
}

pub(crate) fn chord_label(target: Target, cx: &App) -> Option<String> {
    let raw = cx.global::<Preferences>().active.get(target).as_deref()?;
    let key = Keystroke::parse_with_os(raw, cx.platform_info().os).ok()?;
    Some(Kbd::format(&key))
}

#[cfg(test)]
mod tests;
