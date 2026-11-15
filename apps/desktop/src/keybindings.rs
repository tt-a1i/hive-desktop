mod editor;
pub(crate) use editor::Editor;

use crate::language::tr;
use gpui_kit::{
    Action, App, Global, KeyBinding, Keystroke, Modifiers, OperatingSystem, Unbind,
    component::kbd::Kbd,
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(crate) const FILE: &str = "desktop-keybindings.json";

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
    Search,
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

const TARGETS: [Target; 12] = [
    Target::NewSession,
    Target::Search,
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
            Self::Search => 1,
            Self::Find => 2,
            Self::PreviousSession => 3,
            Self::NextSession => 4,
            Self::ToggleSidebar => 5,
            Self::ToggleTerminal => 6,
            Self::OpenGitWorkspace => 7,
            Self::SearchProjectFiles => 8,
            Self::ChooseModel => 9,
            Self::ToggleMode => 10,
            Self::Preferences => 11,
        }
    }

    pub(crate) fn group(self) -> TargetGroup {
        match self {
            Self::NewSession
            | Self::Search
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
            Self::Search => tr("搜索会话"),
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
            Self::NewSession => tr("在当前工作区以最近使用的 Agent 发起新对话"),
            Self::Search => tr("打开全局会话搜索窗口"),
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
            Self::Search => "shortcut-search",
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
            (Self::Search, true) => "cmd-k",
            (Self::Search, false) => "ctrl-k",
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
            Self::Search => Box::new(crate::OpenSessionSearch),
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

    fn key_binding(self, chord: &'static str) -> KeyBinding {
        match self {
            Self::NewSession => KeyBinding::new(chord, crate::NewSessionShortcut, None),
            Self::Search => KeyBinding::new(chord, crate::OpenSessionSearch, None),
            Self::Find => KeyBinding::new(chord, crate::OpenConversationFind, None),
            Self::PreviousSession => KeyBinding::new(chord, crate::SelectPreviousSession, None),
            Self::NextSession => KeyBinding::new(chord, crate::SelectNextSession, None),
            Self::ToggleSidebar => KeyBinding::new(chord, crate::ToggleSidebar, None),
            Self::ToggleTerminal => KeyBinding::new(chord, crate::ToggleWorkspaceTerminal, None),
            Self::OpenGitWorkspace => KeyBinding::new(chord, crate::OpenGitWorkspaceShortcut, None),
            Self::SearchProjectFiles => KeyBinding::new(chord, crate::OpenProjectFilePicker, None),
            Self::ChooseModel => KeyBinding::new(chord, crate::OpenModelPicker, None),
            Self::ToggleMode => KeyBinding::new(chord, crate::ToggleSessionMode, None),
            Self::Preferences => KeyBinding::new(chord, crate::OpenPreferences, None),
        }
    }

    #[cfg(test)]
    fn unbind_binding(self, chord: &'static str) -> KeyBinding {
        KeyBinding::new(chord, Unbind(self.action().name().into()), None)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Bindings {
    pub search: Option<String>,
    pub find: Option<String>,
    pub preferences: Option<String>,
    pub new_session: Option<String>,
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
    find: Option<String>,
    preferences: Option<String>,
    #[serde(default, deserialize_with = "some_present")]
    new_session: Option<Option<String>>,
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
    fn into_bindings(self, os: OperatingSystem) -> Bindings {
        let pick = |raw: Option<Option<String>>, target: Target| match raw {
            Some(explicit) => explicit,
            None => Some(target.default(os).to_owned()),
        };
        Bindings {
            search: self.search,
            find: self.find,
            preferences: self.preferences,
            new_session: pick(self.new_session, Target::NewSession),
            toggle_sidebar: pick(self.toggle_sidebar, Target::ToggleSidebar),
            toggle_terminal: pick(self.toggle_terminal, Target::ToggleTerminal),
            open_git_workspace: pick(self.open_git_workspace, Target::OpenGitWorkspace),
            search_project_files: pick(self.search_project_files, Target::SearchProjectFiles),
            choose_model: pick(self.choose_model, Target::ChooseModel),
            toggle_mode: pick(self.toggle_mode, Target::ToggleMode),
            previous_session: pick(self.previous_session, Target::PreviousSession),
            next_session: pick(self.next_session, Target::NextSession),
        }
    }
}

impl Bindings {
    pub(crate) fn defaults(os: OperatingSystem) -> Self {
        Self {
            search: Some(Target::Search.default(os).to_owned()),
            find: Some(Target::Find.default(os).to_owned()),
            preferences: Some(Target::Preferences.default(os).to_owned()),
            new_session: Some(Target::NewSession.default(os).to_owned()),
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
            Target::Search => &self.search,
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
            Target::Search => &mut self.search,
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
                    return Err("快捷键与另一个操作冲突，请选择不同的组合键。");
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

fn load(root: &Path, os: OperatingSystem) -> io::Result<Bindings> {
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
    stored.bindings.into_bindings(os).normalized(os).map_err(invalid)
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

pub(crate) fn startup_bindings(os: OperatingSystem) -> Vec<KeyBinding> {
    TARGETS
        .into_iter()
        .map(|target| target.key_binding(target.default(os)))
        .collect()
}

#[cfg(test)]
fn default_unbinds(bindings: &Bindings, os: OperatingSystem) -> Vec<KeyBinding> {
    TARGETS
        .into_iter()
        .filter(|target| bindings.get(*target).as_deref() != Some(target.default(os)))
        .map(|target| target.unbind_binding(target.default(os)))
        .collect()
}

#[cfg(test)]
pub(crate) fn keymap_delta_with<F>(prev: &Bindings, next: &Bindings, mut static_chord: F) -> Vec<KeyBinding>
where
    F: FnMut(&str) -> &'static str,
{
    let mut delta = Vec::new();
    for target in TARGETS {
        if prev.get(target) == next.get(target) {
            continue;
        }
        if let Some(old_chord) = prev.get(target) {
            delta.push(target.unbind_binding(static_chord(old_chord)));
        }
        if let Some(new_chord) = next.get(target) {
            delta.push(target.key_binding(static_chord(new_chord)));
        }
    }
    delta
}

fn protected_binding(binding: &KeyBinding) -> bool {
    let action = binding.action();
    if TARGETS
        .into_iter()
        .any(|target| target.action().name() == action.name())
    {
        return false;
    }
    if let Some(unbind) = action.as_any().downcast_ref::<Unbind>() {
        if TARGETS
            .into_iter()
            .any(|target| unbind.0.as_ref() == target.action().name())
        {
            return false;
        }
    }
    true
}

fn check_registered(bindings: &Bindings, cx: &App) -> Result<(), &'static str> {
    let os = cx.platform_info().os;
    for target in TARGETS {
        if let Some(chord) = bindings.get(target) {
            // These existing Hive global defaults already own their chords.
            if TARGETS
                .into_iter()
                .any(|existing| chord == existing.default(os))
            {
                continue;
            }
            let key = validate_chord(chord, os)?;
            if cx
                .all_bindings_for_input(&[key])
                .iter()
                .any(protected_binding)
            {
                return Err("此组合键已用于现有组件或文字编辑，请选择其他组合。");
            }
        }
    }
    Ok(())
}

fn live_delta(prev: &Bindings, next: &Bindings, cx: &App) -> Result<(Bindings, Vec<KeyBinding>), String> {
    let os = cx.platform_info().os;
    let normalized = next.normalized(os).map_err(str::to_owned)?;
    check_registered(&normalized, cx).map_err(str::to_owned)?;
    let mut delta = Vec::new();
    for target in TARGETS {
        if prev.get(target) == normalized.get(target) {
            continue;
        }
        if let Some(old_chord) = prev.get(target) {
            delta.push(
                KeyBinding::load(
                    old_chord,
                    Box::new(Unbind(target.action().name().into())),
                    None,
                    false,
                    None,
                    cx,
                )
                .map_err(|error| error.to_string())?,
            );
        }
        if let Some(new_chord) = normalized.get(target) {
            delta.push(
                KeyBinding::load(new_chord, target.action(), None, false, None, cx)
                    .map_err(|error| error.to_string())?,
            );
        }
    }
    Ok((normalized, delta))
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
    applied: bool,
    notice: Option<Notice>,
    preview: bool,
}

impl Global for Preferences {}

impl Preferences {
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
}

pub(crate) fn restore(root: &Path, cx: &mut App) {
    if cx.global::<Preferences>().root.is_some() {
        return;
    }
    // Register only this bounded delta; never clear component/input bindings.
    let active_before = cx.global::<Preferences>().active.clone();
    let loaded = load(root, cx.platform_info().os).and_then(|bindings| {
        live_delta(&active_before, &bindings, cx)
            .map(|(normalized, delta)| (normalized, delta))
            .map_err(io::Error::other)
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
            state.saved = bindings;
            state.draft = draft;
            cx.bind_keys(delta);
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
    let loaded = load(&root, cx.platform_info().os).and_then(|bindings| {
        live_delta(&active_before, &bindings, cx).map_err(io::Error::other)
    });
    match loaded {
        Ok((bindings, delta)) => {
            let state = cx.global_mut::<Preferences>();
            state.active = bindings.clone();
            state.saved = bindings.clone();
            state.draft = bindings;
            state.load_failed = false;
            state.notice = None;
            cx.bind_keys(delta);
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
    let (draft, active_before, load_failed, preview, has_root) = {
        let state = cx.global::<Preferences>();
        (
            state.draft.clone(),
            state.active.clone(),
            state.load_failed,
            state.preview,
            state.root.is_some(),
        )
    };
    if load_failed {
        return;
    }
    let result = live_delta(&active_before, &draft, cx);
    let os = cx.platform_info().os;
    match result {
        Ok((normalized, delta)) => {
            if preview {
                let state = cx.global_mut::<Preferences>();
                state.active = normalized.clone();
                state.saved = normalized.clone();
                state.draft = normalized;
                state.notice = Some(Notice::Saved);
                cx.bind_keys(delta);
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
                    cx.bind_keys(delta);
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
        "快捷键与另一个操作冲突，请选择不同的组合键。",
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
