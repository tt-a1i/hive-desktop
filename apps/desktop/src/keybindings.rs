mod editor;
pub(crate) use editor::Editor;

use crate::language::tr;
use gpui_kit::{
    Action, App, Global, KeyBinding, KeybindingKeystroke, Keymap, Keystroke, Modifiers,
    OperatingSystem, component::kbd::Kbd,
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

gpui_kit::actions!(hive_desktop, [NewSessionInCurrentProject]);

pub(crate) const FILE: &str = "desktop-keybindings.json";
const CHORD_CONFLICT: &str = "快捷键与另一个操作冲突，请选择不同的组合键。";

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
    ToggleSidebar,
    ToggleTerminal,
    OpenGitWorkspace,
    SearchProjectFiles,
    ChooseModel,
    ToggleMode,
    Preferences,
}

const TARGETS: [Target; 14] = [
    Target::NewSession,
    Target::NewSessionCurrentProject,
    Target::Search,
    Target::CommandPalette,
    Target::Find,
    Target::PreviousSession,
    Target::NextSession,
    Target::ToggleSidebar,
    Target::ToggleTerminal,
    Target::OpenGitWorkspace,
    Target::SearchProjectFiles,
    Target::ChooseModel,
    Target::ToggleMode,
    Target::Preferences,
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
            Self::ToggleSidebar => 6,
            Self::ToggleTerminal => 7,
            Self::OpenGitWorkspace => 8,
            Self::SearchProjectFiles => 9,
            Self::ChooseModel => 10,
            Self::ToggleMode => 11,
            Self::Preferences => 12,
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
            Self::ToggleSidebar
            | Self::ToggleTerminal
            | Self::OpenGitWorkspace
            | Self::SearchProjectFiles => TargetGroup::Workspace,
            Self::ChooseModel | Self::ToggleMode => TargetGroup::Conversation,
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
            Self::ToggleSidebar => tr("切换侧边栏"),
            Self::ToggleTerminal => tr("切换终端面板"),
            Self::OpenGitWorkspace => tr("打开 Git 工作区"),
            Self::SearchProjectFiles => tr("搜索项目文件"),
            Self::ChooseModel => tr("选择模型"),
            Self::ToggleMode => tr("切换会话模式"),
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
            Self::ToggleSidebar => tr("展开或收起左侧工作区与会话导航栏"),
            Self::ToggleTerminal => tr("打开或收起当前工作区的终端面板"),
            Self::OpenGitWorkspace => tr("查看当前工作区 Git 状态与 diff"),
            Self::SearchProjectFiles => tr("打开文件选择器并插入工作区文件引用"),
            Self::ChooseModel => tr("展开或关闭当前会话的模型选择器"),
            Self::ToggleMode => tr("在当前 Agent 支持的会话模式间切换"),
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
            Self::ToggleSidebar => "shortcut-toggle-sidebar",
            Self::ToggleTerminal => "shortcut-toggle-terminal",
            Self::OpenGitWorkspace => "shortcut-open-git",
            Self::SearchProjectFiles => "shortcut-project-files",
            Self::ChooseModel => "shortcut-choose-model",
            Self::ToggleMode => "shortcut-toggle-mode",
            Self::Preferences => "shortcut-preferences",
        }
    }

    pub(crate) fn default(self, os: OperatingSystem) -> &'static str {
        let mac = os == OperatingSystem::Mac;
        match (self, mac) {
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
            (Self::Preferences, true) => "cmd-,",
            (Self::Preferences, false) => "ctrl-,",
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
            Self::ToggleSidebar => Box::new(crate::ToggleSidebar),
            Self::ToggleTerminal => Box::new(crate::ToggleWorkspaceTerminal),
            Self::OpenGitWorkspace => Box::new(crate::OpenGitWorkspaceShortcut),
            Self::SearchProjectFiles => Box::new(crate::OpenProjectFilePicker),
            Self::ChooseModel => Box::new(crate::OpenModelPicker),
            Self::ToggleMode => Box::new(crate::ToggleSessionMode),
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
    pub toggle_sidebar: Option<String>,
    pub toggle_terminal: Option<String>,
    pub open_git_workspace: Option<String>,
    pub search_project_files: Option<String>,
    pub choose_model: Option<String>,
    pub toggle_mode: Option<String>,
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
    toggle_mode: Option<Option<String>>,
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
            (Target::ToggleSidebar, self.toggle_sidebar),
            (Target::ToggleTerminal, self.toggle_terminal),
            (Target::OpenGitWorkspace, self.open_git_workspace),
            (Target::SearchProjectFiles, self.search_project_files),
            (Target::ChooseModel, self.choose_model),
            (Target::ToggleMode, self.toggle_mode),
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
                    && (raw == target.default(os)) == use_default
                {
                    let chord = dispatch_key(validate_chord(raw, os)?);
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
            let chord = validate_chord(target.default(os), os)?;
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
            search: Some(Target::Search.default(os).to_owned()),
            command_palette: Some(Target::CommandPalette.default(os).to_owned()),
            find: Some(Target::Find.default(os).to_owned()),
            preferences: Some(Target::Preferences.default(os).to_owned()),
            new_session: Some(Target::NewSession.default(os).to_owned()),
            new_session_current_project: Some(Target::NewSessionCurrentProject.default(os).to_owned()),
            toggle_sidebar: Some(Target::ToggleSidebar.default(os).to_owned()),
            toggle_terminal: Some(Target::ToggleTerminal.default(os).to_owned()),
            open_git_workspace: Some(Target::OpenGitWorkspace.default(os).to_owned()),
            search_project_files: Some(Target::SearchProjectFiles.default(os).to_owned()),
            choose_model: Some(Target::ChooseModel.default(os).to_owned()),
            toggle_mode: Some(Target::ToggleMode.default(os).to_owned()),
            previous_session: Some(Target::PreviousSession.default(os).to_owned()),
            next_session: Some(Target::NextSession.default(os).to_owned()),
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
            Target::ToggleSidebar => &self.toggle_sidebar,
            Target::ToggleTerminal => &self.toggle_terminal,
            Target::OpenGitWorkspace => &self.open_git_workspace,
            Target::SearchProjectFiles => &self.search_project_files,
            Target::ChooseModel => &self.choose_model,
            Target::ToggleMode => &self.toggle_mode,
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
            Target::ToggleSidebar => &mut self.toggle_sidebar,
            Target::ToggleTerminal => &mut self.toggle_terminal,
            Target::OpenGitWorkspace => &mut self.open_git_workspace,
            Target::SearchProjectFiles => &mut self.search_project_files,
            Target::ChooseModel => &mut self.choose_model,
            Target::ToggleMode => &mut self.toggle_mode,
            Target::Preferences => &mut self.preferences,
        };
        *slot = value;
    }

    pub(crate) fn normalized(&self, os: OperatingSystem) -> Result<Self, &'static str> {
        let mut next = self.clone();
        let mut seen: Vec<Keystroke> = Vec::with_capacity(TARGETS.len());
        for target in TARGETS {
            if let Some(raw) = self.get(target) {
                let key = validate_chord(raw, os)?;
                if seen.contains(&key) {
                    return Err(CHORD_CONFLICT);
                }
                seen.push(key.clone());
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

fn validate_chord(value: &str, os: OperatingSystem) -> Result<Keystroke, &'static str> {
    if value.is_empty()
        || value.len() > 80
        || value.split_whitespace().count() != 1
        || value.contains("->")
    {
        return Err("请录制一个带修饰键的组合键，不支持连续按键。");
    }
    let key = Keystroke::parse_with_os(value, os).map_err(|_| "快捷键格式无效，请重新录制。")?;
    let m = key.modifiers;
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
        || (!m.shift && matches!(key.key.as_str(), "m" | "o" | "s"))
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
    TARGETS
        .into_iter()
        .map(|target| {
            KeyBinding::load(target.default(os), target.action(), None, false, None, cx)
                .expect("valid default Hive shortcut")
        })
        .collect()
}

fn protected_binding(binding: &KeyBinding) -> bool {
    binding.predicate().is_some() || !TARGETS
        .into_iter()
        .any(|target| target.action().name() == binding.action().name())
}

fn dispatch_key(key: Keystroke, cx: &App) -> Keystroke {
    KeybindingKeystroke::new_with_mapper(key, false, cx.keyboard_mapper().as_ref())
        .inner()
        .clone()
}

fn check_conflicts(
    bindings: &Bindings,
    os: OperatingSystem,
    dispatch_key: impl Fn(Keystroke) -> Keystroke,
    is_protected: impl Fn(&Keystroke) -> bool,
) -> Result<(), &'static str> {
    let mut claimed = Vec::with_capacity(TARGETS.len());
    for target in TARGETS {
        if let Some(chord) = bindings.get(target) {
            let key = dispatch_key(validate_chord(chord, os)?);
            if claimed.contains(&key) {
                return Err(CHORD_CONFLICT);
            }
            // These existing Hive global defaults already own their chords.
            let existing_default = TARGETS
                .into_iter()
                .any(|existing| chord == existing.default(os));
            if !existing_default && is_protected(&key) {
                return Err("此组合键已用于现有组件或文字编辑，请选择其他组合。");
            }
            claimed.push(key);
        }
    }
    Ok(())
}

fn check_registered(bindings: &Bindings, cx: &App) -> Result<(), &'static str> {
    check_conflicts(
        bindings,
        cx.platform_info().os,
        |key| dispatch_key(key, cx),
        |key| {
            cx.all_bindings_for_input(std::slice::from_ref(key))
                .iter()
                .any(protected_binding)
        },
    )
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
            .map(|chord| KeyBinding::load(chord, target.action(), None, false, None, cx))
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
        binding.predicate().is_some()
            || !changes.iter().any(|(action, _)| *action == binding.action().name())
    });
    keymap.add_bindings(changes.into_iter().filter_map(|(_, binding)| binding));
}

fn full_delta(bindings: &Bindings, cx: &App) -> Result<BindingChanges, String> {
    TARGETS.into_iter().map(|target| {
        let binding = bindings.get(target).as_ref()
            .map(|chord| KeyBinding::load(chord, target.action(), None, false, None, cx))
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

#[cfg(test)]
mod tests;
