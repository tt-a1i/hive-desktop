mod editor;
use crate::language::tr;
pub(crate) use editor::Editor;
use gpui_kit::{component::kbd::Kbd, *};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const FILE: &str = "desktop-keybindings.json";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Search,
    Find,
    Preferences,
}
const TARGETS: [Target; 3] = [Target::Search, Target::Find, Target::Preferences];
impl Target {
    fn index(self) -> usize {
        match self {
            Self::Search => 0,
            Self::Find => 1,
            Self::Preferences => 2,
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Search => tr("搜索会话"),
            Self::Find => tr("查找当前会话"),
            Self::Preferences => tr("打开设置"),
        }
    }
    fn default(self, os: OperatingSystem) -> &'static str {
        match (self, os == OperatingSystem::Mac) {
            (Self::Search, true) => "cmd-k",
            (Self::Search, false) => "ctrl-k",
            (Self::Find, true) => "cmd-f",
            (Self::Find, false) => "ctrl-f",
            (Self::Preferences, true) => "cmd-,",
            (Self::Preferences, false) => "ctrl-,",
        }
    }
    fn action(self) -> Box<dyn Action> {
        match self {
            Self::Search => Box::new(crate::OpenSessionSearch),
            Self::Find => Box::new(crate::OpenConversationFind),
            Self::Preferences => Box::new(crate::OpenPreferences),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bindings {
    search: Option<String>,
    find: Option<String>,
    preferences: Option<String>,
}
impl Bindings {
    fn defaults(os: OperatingSystem) -> Self {
        Self {
            search: Some(Target::Search.default(os).to_owned()),
            find: Some(Target::Find.default(os).to_owned()),
            preferences: Some(Target::Preferences.default(os).to_owned()),
        }
    }
    fn get(&self, target: Target) -> &Option<String> {
        match target {
            Target::Search => &self.search,
            Target::Find => &self.find,
            Target::Preferences => &self.preferences,
        }
    }
    fn set(&mut self, target: Target, value: Option<String>) {
        *match target {
            Target::Search => &mut self.search,
            Target::Find => &mut self.find,
            Target::Preferences => &mut self.preferences,
        } = value;
    }
    fn preserve_pending(&self, previous_saved: &Self, previous_draft: &Self) -> Self {
        let mut draft = self.clone();
        for target in TARGETS {
            if previous_saved.get(target) != previous_draft.get(target) {
                draft.set(target, previous_draft.get(target).clone());
            }
        }
        draft
    }
    fn normalized(&self, os: OperatingSystem) -> Result<Self, &'static str> {
        let mut normalized = self.clone();
        let mut used = std::collections::HashSet::new();
        for target in TARGETS {
            if let Some(value) = self.get(target) {
                let key = validate_chord(value, os)?;
                let canonical = key.unparse(os);
                if !used.insert(canonical.clone()) {
                    return Err("快捷键与另一个操作冲突，请选择不同的组合键。");
                }
                normalized.set(target, Some(canonical));
            }
        }
        Ok(normalized)
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
        || matches!(
            key.key.as_str(),
            "a" | "b"
                | "c"
                | "i"
                | "u"
                | "v"
                | "x"
                | "z"
                | "y"
                | "q"
                | "w"
                | "h"
                | "m"
                | "n"
                | "o"
                | "s"
                | "p"
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
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    bindings: Bindings,
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
    let stored: Stored = serde_json::from_value(value)?;
    stored.bindings.normalized(os).map_err(invalid)
}
fn save(root: &Path, bindings: &Bindings, os: OperatingSystem) -> io::Result<()> {
    let bindings = bindings.normalized(os).map_err(invalid)?;
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(
        file.as_file_mut(),
        &Stored {
            version: 1,
            bindings,
        },
    )?;
    file.as_file().sync_all()?;
    file.persist(root.join(FILE))?;
    Ok(())
}
fn default_unbinds(bindings: &Bindings, os: OperatingSystem) -> Vec<KeyBinding> {
    TARGETS
        .into_iter()
        .filter(|target| bindings.get(*target).as_deref() != Some(target.default(os)))
        .map(|target| {
            KeyBinding::new(
                target.default(os),
                Unbind(target.action().name().into()),
                None,
            )
        })
        .collect()
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
                .any(|target| chord == target.default(os))
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
fn dynamic_bindings(bindings: &Bindings, cx: &App) -> Result<Vec<KeyBinding>, String> {
    let os = cx.platform_info().os;
    let normalized = bindings.normalized(os).map_err(str::to_owned)?;
    check_registered(&normalized, cx).map_err(str::to_owned)?;
    let mut delta = default_unbinds(&normalized, os);
    for target in TARGETS {
        if let Some(chord) = normalized
            .get(target)
            .as_deref()
            .filter(|chord| *chord != target.default(os))
        {
            delta.push(
                KeyBinding::load(chord, target.action(), None, false, None, cx)
                    .map_err(|error| error.to_string())?,
            );
        }
    }
    Ok(delta)
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
    // Register only this bounded delta, once; never clear component/input bindings.
    let loaded = load(root, cx.platform_info().os).and_then(|bindings| {
        dynamic_bindings(&bindings, cx)
            .map(|delta| (bindings, delta))
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
    let loaded = load(&root, cx.platform_info().os).and_then(|bindings| {
        dynamic_bindings(&bindings, cx)
            .map(|_| bindings)
            .map_err(io::Error::other)
    });
    let state = cx.global_mut::<Preferences>();
    match loaded {
        Ok(bindings) => {
            state.saved = bindings.clone();
            state.draft = bindings;
            state.load_failed = false;
            state.notice = None;
        }
        Err(error) => {
            state.load_failed = true;
            state.notice = Some(Notice::Read(error.to_string()));
        }
    }
    // Reload is a disk review, not a hot keymap replacement.
    cx.refresh_windows();
}
fn save_pending(cx: &mut App) {
    let state = cx.global::<Preferences>();
    if state.load_failed || state.preview {
        return;
    }
    if state.root.is_none() {
        return;
    }
    let draft = state.draft.clone();
    let result = dynamic_bindings(&draft, cx);
    let os = cx.platform_info().os;
    let state = cx.global_mut::<Preferences>();
    match result {
        Ok(_) => state.persist_pending(os),
        Err(error) => state.notice = Some(Notice::Save(error)),
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
    format!(
        "{} ({})",
        tr(label),
        display(
            cx.global::<Preferences>().active.get(target),
            cx.platform_info().os
        )
    )
}
#[cfg(test)]
mod tests;
