use crate::language::tr;
use gpui_kit::{
    App, BorrowAppContext, Global,
    component::diff::{DiffInlineUnit, DiffMode},
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const FILE: &str = "desktop-diffs.json";
pub const MAX_BASE_LEN: usize = 120;
pub const INLINE_DIFF_MAX_LINE_BYTES: usize = 1_000;
pub const DEFAULT_FILE_TREE_SPLIT_BPS: u16 = 3500;
pub const MIN_FILE_TREE_SPLIT_BPS: u16 = 2000;
pub const MAX_FILE_TREE_SPLIT_BPS: u16 = 5500;

fn default_file_tree_split_bps() -> u16 {
    DEFAULT_FILE_TREE_SPLIT_BPS
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffColorScheme {
    #[default]
    RedGreen,
    BlueOrange,
}

impl DiffColorScheme {
    pub fn label(self) -> &'static str {
        match self {
            Self::RedGreen => tr("默认（红绿）"),
            Self::BlueOrange => tr("蓝橙（色盲友好）"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DiffPalette {
    pub added: gpui_kit::Rgba,
    pub removed: gpui_kit::Rgba,
}

pub(crate) fn blue_orange_palette(dark: bool) -> DiffPalette {
    if dark {
        DiffPalette {
            added: gpui_kit::rgb(0x38bdf8),
            removed: gpui_kit::rgb(0xfb923c),
        }
    } else {
        DiffPalette {
            added: gpui_kit::rgb(0x0369a1),
            removed: gpui_kit::rgb(0x9a3412),
        }
    }
}

pub(crate) fn palette(scheme: DiffColorScheme, cx: &App) -> DiffPalette {
    match scheme {
        DiffColorScheme::RedGreen => DiffPalette {
            added: crate::palette::Color::Success.get(cx),
            removed: crate::palette::Color::Error.get(cx),
        },
        DiffColorScheme::BlueOrange => {
            blue_orange_palette(gpui_kit::component::Theme::global(cx).is_dark())
        }
    }
}

pub(crate) fn current_palette(cx: &App) -> DiffPalette {
    let scheme = cx
        .try_global::<DiffPreferences>()
        .map(|prefs| prefs.settings.color_scheme)
        .unwrap_or_default();
    palette(scheme, cx)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InlineDiffMode {
    #[default]
    Word,
    Character,
    Off,
}

impl InlineDiffMode {
    pub fn into_inline_unit(self) -> Option<DiffInlineUnit> {
        match self {
            Self::Word => Some(DiffInlineUnit::Word),
            Self::Character => Some(DiffInlineUnit::Character),
            Self::Off => None,
        }
    }

    pub fn from_inline_unit(unit: Option<DiffInlineUnit>) -> Self {
        match unit {
            Some(DiffInlineUnit::Character) => Self::Character,
            Some(_) => Self::Word,
            None => Self::Off,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Word => Self::Character,
            Self::Character => Self::Off,
            Self::Off => Self::Word,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Word => tr("词级高亮"),
            Self::Character => tr("字符高亮"),
            Self::Off => tr("关闭行内高亮"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferredDiffMode {
    #[default]
    Unified,
    Split,
}

impl PreferredDiffMode {
    pub fn into_diff_mode(self) -> DiffMode {
        match self {
            Self::Unified => DiffMode::Unified,
            Self::Split => DiffMode::Split,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiffSettings {
    pub default_diff_mode: PreferredDiffMode,
    pub inline_diff: InlineDiffMode,
    pub collapse_multi_file_diffs: bool,
    pub group_changes_by_folder: bool,
    pub ignore_whitespace: bool,
    pub pr_default_draft: bool,
    pub pr_default_base: String,
    pub color_scheme: DiffColorScheme,
    #[serde(default = "default_file_tree_split_bps")]
    pub file_tree_split_bps: u16,
}

impl Default for DiffSettings {
    fn default() -> Self {
        Self {
            default_diff_mode: PreferredDiffMode::Unified,
            inline_diff: InlineDiffMode::Word,
            collapse_multi_file_diffs: false,
            group_changes_by_folder: false,
            ignore_whitespace: false,
            pr_default_draft: true,
            pr_default_base: "main".to_owned(),
            color_scheme: DiffColorScheme::RedGreen,
            file_tree_split_bps: DEFAULT_FILE_TREE_SPLIT_BPS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Notice {
    Saved,
    Validation(String),
    ReadError(String),
    WriteError(String),
}

pub struct DiffPreferences {
    root: Option<PathBuf>,
    pub settings: DiffSettings,
    loading: bool,
    notice: Option<Notice>,
    preview: bool,
}

impl Global for DiffPreferences {}

impl DiffPreferences {
    fn new() -> Self {
        let preview = cfg!(feature = "ui-preview")
            && std::env::var("HIVE_DESKTOP_PREVIEW_SCREEN")
                .is_ok_and(|value| !value.trim().is_empty());
        Self {
            root: None,
            settings: DiffSettings::default(),
            loading: !preview,
            notice: None,
            preview,
        }
    }
}

pub fn validate_base_branch(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(tr("默认目标分支不能为空。").to_owned());
    }
    if trimmed.len() > MAX_BASE_LEN {
        return Err(tr("默认目标分支名称过长。").to_owned());
    }
    if trimmed.starts_with('-')
        || trimmed.starts_with('/')
        || trimmed.ends_with('/')
        || trimmed.contains("..")
        || trimmed.chars().any(|ch| ch.is_whitespace() || ch.is_control())
        || trimmed.contains(['~', '^', ':', '?', '*', '[', '\\'])
    {
        return Err(tr("默认目标分支名称包含无效字符。").to_owned());
    }
    Ok(trimmed.to_owned())
}

fn validate(mut settings: DiffSettings) -> io::Result<DiffSettings> {
    settings.pr_default_base = validate_base_branch(&settings.pr_default_base)
        .map_err(|msg| io::Error::new(io::ErrorKind::InvalidData, msg))?;
    if !(MIN_FILE_TREE_SPLIT_BPS..=MAX_FILE_TREE_SPLIT_BPS).contains(&settings.file_tree_split_bps) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            tr("文件树分栏比例超出允许范围。"),
        ));
    }
    Ok(settings)
}

fn load(root: &Path) -> io::Result<DiffSettings> {
    match fs::read_to_string(root.join(FILE)) {
        Ok(content) => {
            let parsed = serde_json::from_str::<DiffSettings>(&content)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            validate(parsed)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(DiffSettings::default()),
        Err(error) => Err(error),
    }
}

fn save(root: &Path, settings: &DiffSettings) -> io::Result<()> {
    let validated = validate(settings.clone())?;
    fs::create_dir_all(root)?;
    let mut bytes = serde_json::to_vec_pretty(&validated)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    bytes.push(b'\n');

    use io::Write;
    let mut temp = tempfile::NamedTempFile::new_in(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o600))?;
    }
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(root.join(FILE)).map_err(|error| error.error)?;
    Ok(())
}

pub fn init(cx: &mut App) {
    cx.set_global(DiffPreferences::new());
}

pub fn restore(root: &Path, cx: &mut App) {
    let preview = cx.global::<DiffPreferences>().preview;
    let root_buf = root.to_path_buf();
    let result = if preview {
        Ok(DiffSettings::default())
    } else {
        load(root)
    };
    cx.update_global::<DiffPreferences, _>(|state, _| {
        state.root = Some(root_buf);
        state.loading = false;
        match result {
            Ok(settings) => {
                state.settings = settings;
                state.notice = None;
            }
            Err(error) => {
                state.settings = DiffSettings::default();
                state.notice = Some(Notice::ReadError(error.to_string()));
            }
        }
    });
    cx.refresh_windows();
}

#[allow(dead_code)]
pub fn reload(cx: &mut App) {
    let (root, preview) = {
        let state = cx.global::<DiffPreferences>();
        (state.root.clone(), state.preview)
    };
    let Some(root) = root else {
        return;
    };
    let result = if preview {
        Ok(DiffSettings::default())
    } else {
        load(&root)
    };
    cx.update_global::<DiffPreferences, _>(|state, _| {
        state.loading = false;
        match result {
            Ok(settings) => {
                state.settings = settings;
                state.notice = None;
            }
            Err(error) => {
                state.notice = Some(Notice::ReadError(error.to_string()));
            }
        }
    });
    cx.refresh_windows();
}

pub fn set(settings: DiffSettings, cx: &mut App) -> bool {
    let validated = match validate(settings) {
        Ok(value) => value,
        Err(error) => {
            cx.update_global::<DiffPreferences, _>(|state, _| {
                state.notice = Some(Notice::Validation(error.to_string()));
            });
            cx.refresh_windows();
            return false;
        }
    };
    let (root, loading, preview) = {
        let state = cx.global::<DiffPreferences>();
        (state.root.clone(), state.loading, state.preview)
    };
    if preview {
        cx.update_global::<DiffPreferences, _>(|state, _| {
            state.settings = validated;
            state.notice = Some(Notice::Saved);
        });
        cx.refresh_windows();
        return true;
    }
    if loading {
        return false;
    }
    let Some(root) = root else {
        return false;
    };
    match save(&root, &validated) {
        Ok(()) => {
            cx.update_global::<DiffPreferences, _>(|state, _| {
                state.settings = validated;
                state.notice = Some(Notice::Saved);
            });
            cx.refresh_windows();
            true
        }
        Err(error) => {
            cx.update_global::<DiffPreferences, _>(|state, _| {
                state.notice = Some(Notice::WriteError(error.to_string()));
            });
            cx.refresh_windows();
            false
        }
    }
}

pub fn reset(cx: &mut App) -> bool {
    set(DiffSettings::default(), cx)
}

pub(crate) fn set_file_tree_split_bps(bps: u16, cx: &mut App) -> bool {
    let bps = bps.clamp(MIN_FILE_TREE_SPLIT_BPS, MAX_FILE_TREE_SPLIT_BPS);
    let (unchanged, root, loading, preview, mut next) = {
        let state = cx.global::<DiffPreferences>();
        (
            state.settings.file_tree_split_bps == bps,
            state.root.clone(),
            state.loading,
            state.preview,
            state.settings.clone(),
        )
    };
    if unchanged {
        return true;
    }
    if preview {
        cx.update_global::<DiffPreferences, _>(|state, _| {
            state.settings.file_tree_split_bps = bps;
        });
        cx.refresh_windows();
        return true;
    }
    if loading {
        return false;
    }
    let Some(root) = root else {
        return false;
    };
    next.file_tree_split_bps = bps;
    match save(&root, &next) {
        Ok(()) => {
            cx.update_global::<DiffPreferences, _>(|state, _| {
                state.settings = next;
            });
            cx.refresh_windows();
            true
        }
        Err(error) => {
            cx.update_global::<DiffPreferences, _>(|state, _| {
                state.notice = Some(Notice::WriteError(error.to_string()));
            });
            cx.refresh_windows();
            false
        }
    }
}

pub fn current(cx: &App) -> DiffSettings {
    cx.global::<DiffPreferences>().settings.clone()
}

pub(crate) fn group_changes_by_folder(cx: &App) -> bool {
    cx.global::<DiffPreferences>().settings.group_changes_by_folder
}

pub fn notice(cx: &App) -> Option<(bool, String)> {
    match cx.global::<DiffPreferences>().notice.as_ref()? {
        Notice::Saved => Some((false, tr("Diff 与 PR 设置已保存。").to_owned())),
        Notice::Validation(error) => Some((true, error.clone())),
        Notice::ReadError(error) => Some((true, format!("{}: {error}", tr("读取 Diff 与 PR 设置失败")))),
        Notice::WriteError(error) => Some((true, format!("{}: {error}", tr("保存 Diff 与 PR 设置失败")))),
    }
}

fn normalized_code(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

pub(crate) fn parse_hunk_coords(header: &str) -> Option<(usize, usize, usize, usize, &str)> {
    let rest = header.strip_prefix("@@ -")?;
    let (coords, tail) = match rest.find(" @@") {
        Some(pos) => (&rest[..pos], &rest[pos + 3..]),
        None => return None,
    };
    let (old_part, new_part) = coords.split_once(" +")?;
    let (old_start, old_count) = match old_part.split_once(',') {
        Some((s, c)) => (s.parse().ok()?, c.parse().ok()?),
        None => (old_part.parse().ok()?, 1),
    };
    let (new_start, new_count) = match new_part.split_once(',') {
        Some((s, c)) => (s.parse().ok()?, c.parse().ok()?),
        None => (new_part.parse().ok()?, 1),
    };
    Some((old_start, old_count, new_start, new_count, tail))
}

fn filter_hunk(lines: &[&str]) -> Option<Vec<String>> {
    let header = *lines.first()?;
    let (old_start, mut old_rem, new_start, mut new_rem, tail) = parse_hunk_coords(header)?;

    let mut body: Vec<(char, &str)> = Vec::new();
    let mut ix = 1;
    while ix < lines.len() && (old_rem > 0 || new_rem > 0) {
        let line = lines[ix];
        if let Some(rest) = line.strip_prefix(' ') {
            old_rem = old_rem.saturating_sub(1);
            new_rem = new_rem.saturating_sub(1);
            body.push((' ', rest));
        } else if let Some(rest) = line.strip_prefix('-') {
            old_rem = old_rem.saturating_sub(1);
            body.push(('-', rest));
        } else if let Some(rest) = line.strip_prefix('+') {
            new_rem = new_rem.saturating_sub(1);
            body.push(('+', rest));
        } else if line.starts_with('\\') {
            body.push(('\\', line));
        } else {
            break;
        }
        ix += 1;
    }
    if ix < lines.len() && lines[ix].starts_with('\\') {
        body.push(('\\', lines[ix]));
    }

    let mut transformed: Vec<(char, String)> = Vec::with_capacity(body.len());
    let mut pos = 0;
    while pos < body.len() {
        let (kind, text) = body[pos];
        if kind == ' ' || kind == '\\' {
            transformed.push((kind, text.to_owned()));
            pos += 1;
            continue;
        }
        let mut removed: Vec<(&str, bool)> = Vec::new();
        let mut added: Vec<(&str, bool)> = Vec::new();
        while pos < body.len() && matches!(body[pos].0, '-' | '+' | '\\') {
            match body[pos].0 {
                '-' => removed.push((body[pos].1, false)),
                '+' => added.push((body[pos].1, false)),
                '\\' => {
                    if let Some(last) = added.last_mut() {
                        last.1 = true;
                    } else if let Some(last) = removed.last_mut() {
                        last.1 = true;
                    }
                }
                _ => break,
            }
            pos += 1;
        }

        let mut prefix = 0;
        while prefix < removed.len()
            && prefix < added.len()
            && normalized_code(removed[prefix].0) == normalized_code(added[prefix].0)
            && removed[prefix].1 == added[prefix].1
        {
            prefix += 1;
        }
        let mut suffix = 0;
        while suffix < (removed.len() - prefix) && suffix < (added.len() - prefix) {
            let ri = removed.len() - 1 - suffix;
            let ai = added.len() - 1 - suffix;
            if normalized_code(removed[ri].0) == normalized_code(added[ai].0)
                && removed[ri].1 == added[ai].1
            {
                suffix += 1;
            } else {
                break;
            }
        }

        for i in 0..prefix {
            transformed.push((' ', added[i].0.to_owned()));
            if added[i].1 {
                transformed.push(('\\', "\\ No newline at end of file".to_owned()));
            }
        }
        for i in prefix..(removed.len() - suffix) {
            transformed.push(('-', removed[i].0.to_owned()));
            if removed[i].1 {
                transformed.push(('\\', "\\ No newline at end of file".to_owned()));
            }
        }
        for i in prefix..(added.len() - suffix) {
            transformed.push(('+', added[i].0.to_owned()));
            if added[i].1 {
                transformed.push(('\\', "\\ No newline at end of file".to_owned()));
            }
        }
        for i in 0..suffix {
            let ai = added.len() - suffix + i;
            transformed.push((' ', added[ai].0.to_owned()));
            if added[ai].1 {
                transformed.push(('\\', "\\ No newline at end of file".to_owned()));
            }
        }
    }

    let has_changes = transformed.iter().any(|(kind, _)| *kind == '-' || *kind == '+');
    if !has_changes {
        return None;
    }

    let old_count = transformed
        .iter()
        .filter(|(kind, _)| *kind == ' ' || *kind == '-')
        .count();
    let new_count = transformed
        .iter()
        .filter(|(kind, _)| *kind == ' ' || *kind == '+')
        .count();

    let mut out = Vec::with_capacity(transformed.len() + 1);
    out.push(format!("@@ -{old_start},{old_count} +{new_start},{new_count} @@{tail}"));
    for (kind, text) in transformed {
        if kind == '\\' {
            out.push(text);
        } else {
            out.push(format!("{kind}{text}"));
        }
    }
    Some(out)
}

/// Splits a multi-file unified diff patch into per-file patch sections.
pub(crate) fn split_patch_by_file(patch: &str) -> Vec<String> {
    if patch.trim().is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = patch.lines().collect();
    let mut sections: Vec<String> = Vec::new();
    let mut ix = 0;

    while ix < lines.len() {
        let start = ix;
        let mut saw_header = false;
        let mut saw_plus_header = false;
        while ix < lines.len() && !lines[ix].starts_with("@@ ") {
            if saw_header
                && (lines[ix].starts_with("diff --git ")
                    || (saw_plus_header && lines[ix].starts_with("--- ")))
            {
                break;
            }
            if lines[ix].starts_with("+++ ") {
                saw_plus_header = true;
            }
            saw_header = true;
            ix += 1;
        }
        while ix < lines.len() && lines[ix].starts_with("@@ ") {
            let Some((_, mut old_rem, _, mut new_rem, _)) = parse_hunk_coords(lines[ix]) else {
                return vec![patch.to_owned()];
            };
            ix += 1;
            while ix < lines.len() && (old_rem > 0 || new_rem > 0) {
                let line = lines[ix];
                if line.starts_with(' ') {
                    old_rem = old_rem.saturating_sub(1);
                    new_rem = new_rem.saturating_sub(1);
                    ix += 1;
                } else if line.starts_with('-') {
                    old_rem = old_rem.saturating_sub(1);
                    ix += 1;
                } else if line.starts_with('+') {
                    new_rem = new_rem.saturating_sub(1);
                    ix += 1;
                } else if line.starts_with('\\') {
                    ix += 1;
                } else {
                    return vec![patch.to_owned()];
                }
            }
            if ix < lines.len() && lines[ix].starts_with('\\') {
                ix += 1;
            }
        }
        if ix > start {
            let mut section = lines[start..ix].join("\n");
            section.push('\n');
            sections.push(section);
        } else {
            ix += 1;
        }
    }

    sections
}

/// Filters lines whose only difference within a hunk block is whitespace, recalculating hunk headers.
pub(crate) fn filter_patch_whitespace(patch: &str) -> String {
    if patch.trim().is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = patch.lines().collect();
    if !lines.iter().any(|line| line.starts_with("@@ ")) {
        return patch.to_owned();
    }

    let mut out_files: Vec<String> = Vec::new();
    let mut ix = 0;

    while ix < lines.len() {
        let mut header_lines: Vec<&str> = Vec::new();
        while ix < lines.len() && !lines[ix].starts_with("@@ ") {
            if !header_lines.is_empty() && lines[ix].starts_with("diff --git ") {
                break;
            }
            header_lines.push(lines[ix]);
            ix += 1;
        }

        let mut hunk_lines: Vec<String> = Vec::new();
        let mut saw_hunk = false;
        while ix < lines.len() && lines[ix].starts_with("@@ ") {
            saw_hunk = true;
            let hunk_start = ix;
            let Some((_, mut old_rem, _, mut new_rem, _)) = parse_hunk_coords(lines[ix]) else {
                // Fallback: return unmodified patch rather than corrupting unknown hunk format.
                return patch.to_owned();
            };
            ix += 1;
            while ix < lines.len() && (old_rem > 0 || new_rem > 0) {
                let line = lines[ix];
                if line.starts_with(' ') {
                    old_rem = old_rem.saturating_sub(1);
                    new_rem = new_rem.saturating_sub(1);
                    ix += 1;
                } else if line.starts_with('-') {
                    old_rem = old_rem.saturating_sub(1);
                    ix += 1;
                } else if line.starts_with('+') {
                    new_rem = new_rem.saturating_sub(1);
                    ix += 1;
                } else if line.starts_with('\\') {
                    ix += 1;
                } else {
                    return patch.to_owned();
                }
            }
            if ix < lines.len() && lines[ix].starts_with('\\') {
                ix += 1;
            }
            if let Some(filtered) = filter_hunk(&lines[hunk_start..ix]) {
                hunk_lines.extend(filtered);
            }
        }

        let keep_file = if saw_hunk {
            !hunk_lines.is_empty()
        } else {
            !header_lines.is_empty()
        };
        if keep_file {
            let mut section = String::new();
            for line in header_lines {
                section.push_str(line);
                section.push('\n');
            }
            for line in hunk_lines {
                section.push_str(&line);
                section.push('\n');
            }
            out_files.push(section);
        }
    }

    out_files.concat()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::diff::DiffFile;

    #[test]
    fn git_changes_grouping_defaults_to_flat_and_roundtrips() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join(FILE), r#"{"ignore_whitespace": true}"#).unwrap();
        let mut settings = load(root.path()).unwrap();
        assert!(!settings.group_changes_by_folder);
        assert!(settings.ignore_whitespace);

        settings.group_changes_by_folder = true;
        save(root.path(), &settings).unwrap();
        assert_eq!(load(root.path()).unwrap(), settings);

        settings.group_changes_by_folder = false;
        save(root.path(), &settings).unwrap();
        assert!(!load(root.path()).unwrap().group_changes_by_folder);
    }

    #[test]
    fn roundtrip_and_base_branch_validation() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(load(root.path()).unwrap(), DiffSettings::default());

        let custom = DiffSettings {
            default_diff_mode: PreferredDiffMode::Split,
            inline_diff: InlineDiffMode::Character,
            collapse_multi_file_diffs: true,
            group_changes_by_folder: true,
            ignore_whitespace: true,
            pr_default_draft: false,
            pr_default_base: "develop".to_owned(),
            color_scheme: DiffColorScheme::BlueOrange,
            file_tree_split_bps: 4200,
        };
        assert_eq!(InlineDiffMode::Word.next(), InlineDiffMode::Character);
        assert_eq!(InlineDiffMode::Character.next(), InlineDiffMode::Off);
        assert_eq!(InlineDiffMode::Off.next(), InlineDiffMode::Word);
        save(root.path(), &custom).unwrap();
        assert_eq!(load(root.path()).unwrap(), custom);

        assert!(validate_base_branch("").is_err());
        assert!(validate_base_branch("main branch").is_err());
        assert!(validate_base_branch("main..dev").is_err());
        assert!(validate_base_branch("-flag").is_err());
        assert_eq!(validate_base_branch(" release/v2 ").unwrap(), "release/v2");

        fs::write(
            root.path().join(FILE),
            r#"{"pr_default_base": ""}"#,
        )
        .unwrap();
        assert_eq!(
            load(root.path()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        // Legacy file defaults to RedGreen + 3500 bps.
        fs::write(root.path().join(FILE), r#"{"pr_default_base": "main"}"#).unwrap();
        let legacy = load(root.path()).unwrap();
        assert_eq!(legacy.color_scheme, DiffColorScheme::RedGreen);
        assert_eq!(legacy.file_tree_split_bps, DEFAULT_FILE_TREE_SPLIT_BPS);

        // Out-of-range split ratio is rejected.
        fs::write(root.path().join(FILE), r#"{"file_tree_split_bps": 1000}"#).unwrap();
        assert_eq!(
            load(root.path()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::write(root.path().join(FILE), r#"{"file_tree_split_bps": 9000}"#).unwrap();
        assert_eq!(
            load(root.path()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[gpui_kit::test]
    fn preview_mode_updates_memory_without_writing_disk_even_before_restore(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        let _locale = crate::language::test_locale_guard();
        let root = tempfile::tempdir().unwrap();
        let sentinel = root.path().join(FILE);
        fs::write(&sentinel, "{\"color_scheme\":\"red_green\"}\n").unwrap();
        let before = fs::read(&sentinel).unwrap();

        cx.update(|cx| {
            gpui_kit::init(cx);
            init(cx);
            cx.update_global::<DiffPreferences, _>(|state, _| {
                state.root = Some(root.path().to_path_buf());
                state.loading = true;
                state.preview = true;
            });
            let mut next = current(cx);
            next.color_scheme = DiffColorScheme::BlueOrange;
            assert!(set(next, cx));
            assert_eq!(current(cx).color_scheme, DiffColorScheme::BlueOrange);
            assert!(set_file_tree_split_bps(4400, cx));
            assert_eq!(current(cx).file_tree_split_bps, 4400);
        });

        assert_eq!(fs::read(&sentinel).unwrap(), before);
    }

    #[test]
    fn ignore_whitespace_drops_whitespace_only_hunks_and_keeps_valid_diff() {
        let patch = "\
diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1,3 +1,3 @@
 fn a() {
-  let x = 1;
+    let   x = 1;
 }
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -1,4 +1,4 @@
 fn b() {
-  let y = 1;
+    let y = 1;
-  let z = 2;
+  let z = 3;
 }
";
        let filtered = filter_patch_whitespace(patch);
        let files = DiffFile::parse(&filtered).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path().as_ref(), "b.rs");
        assert_eq!((files[0].additions(), files[0].deletions()), (1, 1));
    }

    #[test]
    fn blue_orange_palette_meets_wcag_aa_on_all_standard_and_enhanced_surfaces() {
        use crate::appearance::Contrast;
        use crate::palette::Color;

        let linearize = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let rel_luma = |c: gpui_kit::Rgba| {
            0.2126 * linearize(c.r) + 0.7152 * linearize(c.g) + 0.0722 * linearize(c.b)
        };
        let contrast_ratio = |a: gpui_kit::Rgba, b: gpui_kit::Rgba| {
            let (la, lb) = (rel_luma(a), rel_luma(b));
            (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
        };

        for dark in [false, true] {
            let diff = blue_orange_palette(dark);
            for contrast in [Contrast::Standard, Contrast::Enhanced] {
                for surface in [
                    Color::Sidebar,
                    Color::Surface,
                    Color::Card,
                    Color::Overlay,
                    Color::Inset,
                    Color::Bubble,
                ] {
                    let bg = surface.for_mode_with_contrast(dark, contrast);
                    let add_ratio = contrast_ratio(diff.added, bg);
                    let rem_ratio = contrast_ratio(diff.removed, bg);
                    assert!(
                        add_ratio >= 4.5,
                        "BlueOrange added contrast {add_ratio:.2} < 4.5 on {surface:?} (dark={dark}, contrast={contrast:?})"
                    );
                    assert!(
                        rem_ratio >= 4.5,
                        "BlueOrange removed contrast {rem_ratio:.2} < 4.5 on {surface:?} (dark={dark}, contrast={contrast:?})"
                    );
                }
            }
        }
    }
}
