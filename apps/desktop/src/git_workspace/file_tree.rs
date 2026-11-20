use super::{Status, decode};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChangeMark {
    Added,
    Deleted,
    Renamed,
    Conflicted,
    Modified,
    Descendant,
}

pub(crate) struct FileTreeChanges {
    files: HashMap<String, ChangeMark>,
    directories: HashSet<String>,
    pub(crate) partial: bool,
}

impl FileTreeChanges {
    pub(crate) fn from_wire(value: Value) -> Result<Option<Self>, String> {
        let status = match decode::<Status>(value, "status") {
            Ok(status) if status.scope == "project" && status.offset == 0 => status,
            Ok(_) => return Err(crate::language::tr("读取 Git 信息失败").into()),
            Err(error) if error.code == "NOT_REPOSITORY" => return Ok(None),
            Err(error) => return Err(error.text()),
        };
        let mut changes = Self {
            files: HashMap::new(),
            directories: HashSet::new(),
            partial: !status.complete || status.has_more || status.entries.len() < status.total,
        };
        for change in status.entries {
            let copied = change
                .rename_score
                .as_deref()
                .is_some_and(|score| score.starts_with('C'));
            if copied && change.path_role.as_deref() == Some("rename-source") {
                continue;
            }
            changes.record_parents(&change.path);
            if change.path_role.as_deref() == Some("rename-source") {
                continue;
            }
            let mark = match change.badge().0 {
                "A" => ChangeMark::Added,
                "D" => ChangeMark::Deleted,
                "R" if copied => ChangeMark::Added,
                "R" => ChangeMark::Renamed,
                "U" => ChangeMark::Conflicted,
                _ => ChangeMark::Modified,
            };
            if !copied && let Some(original) = &change.original_path {
                changes.record_parents(original);
            }
            // A staged deletion can be followed by an untracked file at the same path.
            changes
                .files
                .entry(change.path)
                .and_modify(|previous| {
                    if *previous == ChangeMark::Deleted {
                        *previous = mark;
                    }
                })
                .or_insert(mark);
        }
        Ok(Some(changes))
    }

    fn record_parents(&mut self, mut path: &str) {
        while let Some((parent, _)) = path.rsplit_once('/') {
            self.directories.insert(parent.to_owned());
            path = parent;
        }
    }

    pub(crate) fn mark(&self, path: &str, directory: bool) -> Option<ChangeMark> {
        self.files.get(path).copied().or_else(|| {
            (directory && self.directories.contains(path)).then_some(ChangeMark::Descendant)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ChangeMark, FileTreeChanges};
    use serde_json::{Value, json};

    fn status(entries: Vec<Value>, total: usize, complete: bool, has_more: bool) -> Value {
        json!({"ok": true, "status": {
            "projectRoot": "/worktree/subproject", "repositoryRoot": "/worktree",
            "projectPrefix": "subproject/", "linkedWorktree": true,
            "detached": false, "unborn": false,
            "entries": entries, "total": total, "offset": 0, "limit": 500,
            "hasMore": has_more, "complete": complete, "warnings": [],
            "scope": "project", "submoduleDetails": "not-scanned", "globalConfiguration": "read-only",
        }})
    }

    #[test]
    fn marks_rename_targets_and_both_ancestor_chains_without_marking_old_files() {
        let changes = FileTreeChanges::from_wire(status(vec![
            json!({"path":"to/nested/目标.txt", "repositoryPath":"subproject/to/nested/目标.txt",
                "originalPath":"from/original.txt", "originalRepositoryPath":"subproject/from/original.txt",
                "pathRole":"target", "kind":"rename", "renameScore":"R100",
                "indexStatus":"R", "worktreeStatus":"."}),
            json!({"path":"moved-out/old.txt", "repositoryPath":"outside.txt",
                "originalPath":"moved-out/old.txt", "pathRole":"rename-source", "kind":"rename",
                "indexStatus":"R", "worktreeStatus":"."}),
            json!({"path":"other/copy.txt", "repositoryPath":"subproject/other/copy.txt",
                "originalPath":"copy-source/unchanged.txt", "pathRole":"target", "kind":"rename",
                "renameScore":"C100", "indexStatus":"C", "worktreeStatus":"."}),
        ], 3, true, false)).unwrap().unwrap();
        assert!(!changes.partial);
        assert_eq!(
            changes.mark("to/nested/目标.txt", false),
            Some(ChangeMark::Renamed)
        );
        for path in ["to", "to/nested", "from", "moved-out", "other"] {
            assert_eq!(changes.mark(path, true), Some(ChangeMark::Descendant));
        }
        for path in [
            "subproject/to/nested/目标.txt",
            "from/original.txt",
            "moved-out/old.txt",
            "outside.txt",
        ] {
            assert_eq!(changes.mark(path, false), None);
        }
        assert_eq!(changes.mark("copy-source", true), None);
        assert_eq!(
            changes.mark("other/copy.txt", false),
            Some(ChangeMark::Added)
        );
    }

    #[test]
    fn partial_status_keeps_positive_marks_and_accepts_a_recreated_file() {
        let changes = FileTreeChanges::from_wire(status(vec![
            json!({"path":"removed/gone.txt", "repositoryPath":"subproject/removed/gone.txt",
                "kind":"ordinary", "indexStatus":"D", "worktreeStatus":"."}),
            json!({"path":"removed/recreated.txt", "repositoryPath":"subproject/removed/recreated.txt",
                "kind":"ordinary", "indexStatus":"D", "worktreeStatus":"."}),
            json!({"path":"removed/recreated.txt", "repositoryPath":"subproject/removed/recreated.txt",
                "kind":"untracked", "indexStatus":"?", "worktreeStatus":"?"}),
        ], 501, true, true)).unwrap().unwrap();
        assert!(changes.partial);
        assert_eq!(changes.mark("removed", true), Some(ChangeMark::Descendant));
        assert_eq!(
            changes.mark("removed/gone.txt", false),
            Some(ChangeMark::Deleted)
        );
        assert_eq!(
            changes.mark("removed/recreated.txt", false),
            Some(ChangeMark::Added)
        );
        assert_eq!(changes.mark("unread", true), None);
        assert!(
            FileTreeChanges::from_wire(status(vec![], 0, false, false))
                .unwrap()
                .unwrap()
                .partial
        );
    }

    #[test]
    fn a_busy_read_or_wrong_page_does_not_report_clean_status() {
        let error = |code| json!({"ok": false, "error": {"code": code, "message": "Fixture"}});
        assert!(
            FileTreeChanges::from_wire(error("NOT_REPOSITORY"))
                .unwrap()
                .is_none()
        );
        assert!(FileTreeChanges::from_wire(error("CONCURRENT_CHANGE")).is_err());
        let mut off_page = status(vec![], 0, true, false);
        off_page["status"]["offset"] = json!(500);
        assert!(FileTreeChanges::from_wire(off_page).is_err());
    }
}
