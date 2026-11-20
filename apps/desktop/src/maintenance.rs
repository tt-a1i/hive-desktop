use std::{
    fs, io,
    path::{Path, PathBuf},
};

const DISPOSABLE_DIRS: &[&str] = &[
    "logs",
    "log",
    "tmp",
    "temp",
    "cache",
    "caches",
    "crash-reports",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaintenanceReport {
    pub disposable_files: usize,
    pub disposable_bytes: u64,
    pub paths: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanOutcome {
    pub removed_files: usize,
    pub freed_bytes: u64,
    pub failed_files: usize,
}

#[derive(Default)]
pub struct MaintenanceState {
    pub scanning: bool,
    pub cleaning: bool,
    pub report: Option<MaintenanceReport>,
    pub notice: Option<(bool, String)>,
}

fn is_protected_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".sqlite")
        || lower.ends_with(".sqlite3")
        || lower.ends_with(".db")
        || lower.ends_with(".db-wal")
        || lower.ends_with(".db-shm")
        || lower.ends_with(".sqlite-wal")
        || lower.ends_with(".sqlite-shm")
        || lower.ends_with("-wal")
        || lower.ends_with("-shm")
        || lower.ends_with("-journal")
        || lower.ends_with(".key")
        || lower.ends_with(".pem")
        || lower.starts_with("desktop-")
        || lower.starts_with("chat-index")
        || matches!(
            lower.as_str(),
            "sidebar.json"
                | "language.json"
                | "composer.json"
                | "keybindings.json"
                | "chats"
                | "profiles"
                | "accounts"
                | "drafts"
                | "checkpoints"
                | "worktrees"
                | "queues"
                | "deliveries"
        )
}

fn is_disposable_top_level_file(name: &str) -> bool {
    if is_protected_name(name) {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".log")
        || lower.ends_with(".tmp")
        || lower.contains(".log.")
}

fn collect_dir_files(dir: &Path, report: &mut MaintenanceReport) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if is_protected_name(&name_str) {
            continue;
        }
        if meta.file_type().is_dir() {
            collect_dir_files(&path, report)?;
        } else if meta.file_type().is_file() {
            report.disposable_files += 1;
            report.disposable_bytes = report.disposable_bytes.saturating_add(meta.len());
            report.paths.push(path);
        }
    }
    Ok(())
}

pub fn scan_disposable(root: &Path) -> io::Result<MaintenanceReport> {
    let mut report = MaintenanceReport::default();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(report),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let lower = name_str.to_ascii_lowercase();
        if meta.file_type().is_dir() && DISPOSABLE_DIRS.contains(&lower.as_str()) {
            collect_dir_files(&path, &mut report)?;
        } else if meta.file_type().is_file() && is_disposable_top_level_file(&name_str) {
            report.disposable_files += 1;
            report.disposable_bytes = report.disposable_bytes.saturating_add(meta.len());
            report.paths.push(path);
        }
    }
    Ok(report)
}

pub fn clean_disposable(root: &Path) -> io::Result<CleanOutcome> {
    let report = scan_disposable(root)?;
    let mut outcome = CleanOutcome::default();
    for path in report.paths {
        let size = fs::symlink_metadata(&path)
            .ok()
            .filter(|meta| meta.file_type().is_file())
            .map(|meta| meta.len())
            .unwrap_or(0);
        match fs::remove_file(&path) {
            Ok(()) => {
                outcome.removed_files += 1;
                outcome.freed_bytes = outcome.freed_bytes.saturating_add(size);
            }
            Err(_) => outcome.failed_files += 1,
        }
    }
    Ok(outcome)
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_and_cleans_only_disposable_files_while_protecting_db_and_configs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("logs/nested")).unwrap();
        fs::create_dir_all(root.join("tmp")).unwrap();
        fs::create_dir_all(root.join("cache")).unwrap();
        fs::create_dir_all(root.join("cache/claude-history-pages/workspace")).unwrap();
        fs::create_dir_all(root.join("chats/deliveries")).unwrap();
        fs::create_dir_all(root.join("drafts")).unwrap();

        fs::write(root.join("logs/app.log"), b"12345").unwrap();
        fs::write(root.join("logs/nested/old.log"), b"67890").unwrap();
        fs::write(root.join("tmp/scratch.txt"), b"abc").unwrap();
        fs::write(root.join("cache/http.bin"), b"abcd").unwrap();
        fs::write(
            root.join("cache/claude-history-pages/workspace/saved.index"),
            b"derived",
        )
        .unwrap();
        fs::write(root.join("runtime.log"), b"xy").unwrap();
        fs::write(root.join("runtime.tmp"), b"z").unwrap();

        // Protected files
        fs::write(root.join("hive.sqlite"), b"db").unwrap();
        fs::write(root.join("hive.sqlite-wal"), b"wal").unwrap();
        fs::write(root.join("logs/sneaky.sqlite"), b"keep").unwrap();
        fs::write(root.join("chats/chat-index.json"), b"{}").unwrap();
        fs::write(root.join("chats/deliveries/q.json"), b"[]").unwrap();
        fs::write(root.join("drafts/d1.json"), b"{}").unwrap();
        fs::write(root.join("desktop-appearance.json"), b"{}").unwrap();
        fs::write(root.join("keybindings.json"), b"{}").unwrap();

        let scan = scan_disposable(root).unwrap();
        assert_eq!(scan.disposable_files, 7);
        assert_eq!(scan.disposable_bytes, 5 + 5 + 3 + 4 + 7 + 2 + 1);

        let outcome = clean_disposable(root).unwrap();
        assert_eq!(outcome.removed_files, 7);
        assert_eq!(outcome.freed_bytes, 27);
        assert_eq!(outcome.failed_files, 0);

        assert!(!root.join("logs/app.log").exists());
        assert!(!root.join("runtime.log").exists());
        assert!(
            !root
                .join("cache/claude-history-pages/workspace/saved.index")
                .exists()
        );
        assert!(root.join("hive.sqlite").exists());
        assert!(root.join("hive.sqlite-wal").exists());
        assert!(root.join("logs/sneaky.sqlite").exists());
        assert!(root.join("chats/chat-index.json").exists());
        assert!(root.join("chats/deliveries/q.json").exists());
        assert!(root.join("drafts/d1.json").exists());
        assert!(root.join("desktop-appearance.json").exists());
        assert!(root.join("keybindings.json").exists());
    }
}
