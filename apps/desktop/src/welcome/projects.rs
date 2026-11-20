use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_RESULTS: usize = 20;
const MAX_STATE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DIR_ENTRIES: usize = 512;
const MAX_SESSION_FILES: usize = 256;
const MAX_HEADER_BYTES: u64 = 16 * 1024;

/// Read local project-path metadata only. Call on a background thread after the
/// user requests suggestions; this never imports projects or changes settings.
pub(super) fn discover_recent_projects() -> Vec<PathBuf> {
    let Some(home) =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
    else {
        return Vec::new();
    };
    let codex = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    let pi = std::env::var_os("PI_CODING_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".pi/agent"));
    let claude = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".claude"));
    discover(&codex, &pi, &claude)
}

fn discover(codex: &Path, pi: &Path, claude: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    read_codex_projects(&codex.join(".codex-global-state.json"), &mut candidates);
    read_sessions(&pi.join("sessions"), false, &mut candidates);
    read_sessions(&claude.join("projects"), true, &mut candidates);
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter_map(|(_, path)| project_root(&path))
        .filter(|path| seen.insert(path.clone()))
        .take(MAX_RESULTS)
        .collect()
}

fn read_codex_projects(path: &Path, candidates: &mut Vec<(u64, PathBuf)>) {
    let Some(file) = open_regular(path) else {
        return;
    };
    // Refuse oversized state rather than parsing a truncated JSON document.
    if file.metadata().map_or(true, |m| m.len() > MAX_STATE_BYTES) {
        return;
    }
    let Ok(state) = serde_json::from_reader::<_, serde_json::Value>(file.take(MAX_STATE_BYTES + 1))
    else {
        return;
    };
    let Some(projects) = state.get("local-projects").and_then(|v| v.as_object()) else {
        return;
    };
    for project in projects.values().take(MAX_DIR_ENTRIES) {
        let updated = project
            .get("updatedAt")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if let Some(paths) = project.get("rootPaths").and_then(|v| v.as_array()) {
            candidates.extend(
                paths
                    .iter()
                    .take(16)
                    .filter_map(|v| v.as_str())
                    .map(|p| (updated, PathBuf::from(p))),
            );
        }
    }
}

// Reject symlinks and non-regular files before opening. On Unix, also prevent
// symlink replacement and blocking on a FIFO between the check and open.
fn open_regular(path: &Path) -> Option<File> {
    if !fs::symlink_metadata(path).ok()?.file_type().is_file() {
        return None;
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).ok()?;
    file.metadata().ok()?.is_file().then_some(file)
}

#[derive(serde::Deserialize)]
struct SessionMetadata {
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

fn read_sessions(dir: &Path, claude: bool, candidates: &mut Vec<(u64, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files = Vec::new();
    let mut remaining_entries = 4096;
    for entry in entries.take(MAX_DIR_ENTRIES).flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_file() && !claude {
            collect_session(entry, &mut files);
        } else if kind.is_dir() {
            let Ok(children) = fs::read_dir(entry.path()) else {
                continue;
            };
            for child in children.take(remaining_entries).flatten() {
                remaining_entries -= 1;
                collect_session(child, &mut files);
            }
            if remaining_entries == 0 {
                break;
            }
        }
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (modified, path) in files.into_iter().take(MAX_SESSION_FILES) {
        let Some(file) = open_regular(&path) else {
            continue;
        };
        let mut reader = BufReader::new(file.take(MAX_HEADER_BYTES * if claude { 8 } else { 1 }));
        for _ in 0..if claude { 8 } else { 1 } {
            let mut line = Vec::new();
            // Bound each record independently, including malformed or long lines.
            let Ok(n) = reader
                .by_ref()
                .take(MAX_HEADER_BYTES)
                .read_until(b'\n', &mut line)
            else {
                break;
            };
            if n == 0 || (n as u64 == MAX_HEADER_BYTES && !line.ends_with(b"\n")) {
                break;
            }
            let Ok(record) = serde_json::from_slice::<SessionMetadata>(&line) else {
                continue;
            };
            if !claude && record.kind.as_deref() != Some("session") {
                break;
            }
            if let Some(cwd) = record.cwd {
                candidates.push((modified, PathBuf::from(cwd)));
                break;
            }
        }
    }
}

fn collect_session(entry: fs::DirEntry, files: &mut Vec<(u64, PathBuf)>) {
    if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
        return;
    }
    let path = entry.path();
    if !path.extension().is_some_and(|ext| ext == "jsonl") {
        return;
    }
    let modified = entry
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0);
    files.push((modified, path));
}

fn project_root(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() || !path.is_dir() {
        return None;
    }
    let path = path.canonicalize().ok()?;
    // A Git worktree also has a .git file, not necessarily a directory.
    Some(
        path.ancestors()
            .take(32)
            .find(|dir| dir.join(".git").exists())
            .unwrap_or(&path)
            .to_path_buf(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovers_valid_deduplicated_roots_from_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let codex = tmp.path().join("codex");
        let pi = tmp.path().join("pi");
        let root = tmp.path().join("repo");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir_all(&codex).unwrap();
        fs::create_dir_all(pi.join("sessions/project")).unwrap();
        fs::write(codex.join(".codex-global-state.json"), serde_json::json!({"local-projects": {"a": {"rootPaths": [root, "relative", "/nonexistent-hive-project"], "updatedAt": 1}}}).to_string()).unwrap();
        fs::write(
            pi.join("sessions/project/session.jsonl"),
            format!(
                "{}\n{{\"type\":\"message\"}}\n",
                serde_json::json!({"type": "session", "cwd": nested})
            ),
        )
        .unwrap();
        assert_eq!(
            discover(&codex, &pi, tmp.path()),
            vec![root.canonicalize().unwrap()]
        );
    }

    #[test]
    fn ignores_bad_or_oversized_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let pi = tmp.path().join("pi");
        fs::create_dir_all(pi.join("sessions/project")).unwrap();
        fs::write(pi.join("sessions/project/bad.jsonl"), "{bad}\n").unwrap();
        fs::write(
            pi.join("sessions/project/long.jsonl"),
            vec![b'x'; MAX_HEADER_BYTES as usize + 1],
        )
        .unwrap();
        assert!(discover(tmp.path(), &pi, tmp.path()).is_empty());
    }
    #[test]
    fn reads_claude_early_cwd_and_flat_pi_session() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = tmp.path().join("claude");
        let pi = tmp.path().join("pi");
        let first = tmp.path().join("first");
        let second = tmp.path().join("second");
        fs::create_dir_all(claude.join("projects/encoded-project")).unwrap();
        fs::create_dir_all(pi.join("sessions")).unwrap();
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        fs::write(
            claude.join("projects/encoded-project/a.jsonl"),
            format!(
                "{{\"type\":\"queue-operation\"}}\n{}\n",
                serde_json::json!({"cwd": first, "message": "private"})
            ),
        )
        .unwrap();
        fs::write(
            pi.join("sessions/flat.jsonl"),
            format!(
                "{}\n",
                serde_json::json!({"type": "session", "cwd": second})
            ),
        )
        .unwrap();
        let results = discover(tmp.path(), &pi, &claude);
        assert_eq!(results.len(), 2);
        assert!(results.contains(&first.canonicalize().unwrap()));
        assert!(results.contains(&second.canonicalize().unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn skips_session_symlinks_and_special_files() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("sessions");
        fs::create_dir_all(&dir).unwrap();
        let target = tmp.path().join("target.jsonl");
        fs::write(
            &target,
            format!(
                "{}\n",
                serde_json::json!({"type":"session", "cwd":tmp.path()})
            ),
        )
        .unwrap();
        symlink(&target, dir.join("link.jsonl")).unwrap();
        let fifo = dir.join("fifo.jsonl");
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let mut results = Vec::new();
        read_sessions(&dir, false, &mut results);
        assert!(results.is_empty());
        assert!(open_regular(&fifo).is_none());
    }
}
