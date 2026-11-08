use gpui_kit::component::diff::DiffFile;
use serde_json::Value;
use std::time::Duration;

pub(crate) struct Change {
    pub original_path: Option<String>,
    pub modified_path: Option<String>,
    pub source: Source,
}
pub(crate) enum Source {
    Patch(String),
    Pair { old: String, new: String },
}
#[derive(Debug)]
pub(crate) enum ParseFailure { NoFiles, Invalid(String) }
pub(crate) struct Document {
    pub files: Vec<DiffFile>,
    pub raw: String,
    pub error: Option<ParseFailure>,
}

pub(crate) fn available(name: &str, args: &Value, patch: Option<&str>) -> bool {
    if patch.is_some_and(|p| !p.is_empty()) {
        return true;
    }
    if name == "fileChange" {
        return args["changes"].as_array().is_some_and(|items| {
            items.iter().any(|c| {
                c["diff"].is_string()
                    && c["path"].is_string()
                    && matches!(
                        c["kind"]["type"].as_str(),
                        Some("add" | "delete" | "update")
                    )
            })
        });
    }
    if !matches!(name.to_ascii_lowercase().as_str(), "edit" | "multiedit") {
        return false;
    }
    let pair = |e: &Value| {
        (e["oldText"].is_string() && e["newText"].is_string())
            || (e["old_string"].is_string() && e["new_string"].is_string())
    };
    args["edits"]
        .as_array()
        .map(|edits| edits.iter().any(pair))
        .unwrap_or_else(|| pair(args))
}

pub(crate) fn changes(name: &str, args: &Value, patch: Option<&str>, english_labels: bool) -> Vec<Change> {
    let path = args["file_path"]
        .as_str()
        .or_else(|| args["path"].as_str())
        .unwrap_or(if english_labels { "Change excerpt" } else { "改动片段" });
    if let Some(patch) = patch.filter(|v| !v.is_empty()) {
        return vec![Change {
            original_path: Some(path.into()),
            modified_path: Some(path.into()),
            source: Source::Patch(patch.into()),
        }];
    }
    if name == "fileChange" {
        return args["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| {
                let text = c["diff"].as_str()?;
                let original = c["path"].as_str()?;
                let path = c["kind"]["move_path"].as_str().unwrap_or(original);
                let source = match c["kind"]["type"].as_str()? {
                    "add" => Source::Pair {
                        old: String::new(),
                        new: text.into(),
                    },
                    "delete" => Source::Pair {
                        old: text.into(),
                        new: String::new(),
                    },
                    "update" => Source::Patch(text.into()),
                    _ => return None,
                };
                Some(Change {
                    original_path: (c["kind"]["type"] != "add").then(|| original.into()),
                    modified_path: (c["kind"]["type"] != "delete").then(|| path.into()),
                    source,
                })
            })
            .collect();
    }
    if !matches!(name.to_ascii_lowercase().as_str(), "edit" | "multiedit") {
        return vec![];
    }
    let edits = args["edits"]
        .as_array()
        .map(|v| v.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| vec![args]);
    let multiple = edits.len() > 1;
    edits
        .into_iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let old = e["oldText"].as_str().or_else(|| e["old_string"].as_str())?;
            let new = e["newText"].as_str().or_else(|| e["new_string"].as_str())?;
            let path = if multiple {
                if english_labels {format!("{path} (excerpt {})",i+1)} else {format!("{path}（片段 {}）",i+1)}
            } else {
                path.into()
            };
            Some(Change {
                original_path: Some(path.clone()),
                modified_path: Some(path),
                source: Source::Pair {
                    old: old.into(),
                    new: new.into(),
                },
            })
        })
        .collect()
}

pub(crate) fn build(changes: Vec<Change>) -> Document {
    let mut document = Document {
        files: vec![],
        raw: String::new(),
        error: None,
    };
    for change in changes {
        let metadata_only = change.original_path.is_none() || change.modified_path.is_none();
        // Quote paths rather than allowing tabs/newlines in tool input to become patch headers.
        let old_path = change
            .original_path
            .map(|p| serde_json::to_string(&format!("a/{p}")).unwrap())
            .unwrap_or_else(|| "/dev/null".into());
        let new_path = change
            .modified_path
            .map(|p| serde_json::to_string(&format!("b/{p}")).unwrap())
            .unwrap_or_else(|| "/dev/null".into());
        let mut patch = match change.source {
            Source::Pair { old, new } => similar::TextDiff::configure()
                .timeout(Duration::from_millis(500))
                .diff_lines(&old, &new)
                .unified_diff()
                .context_radius(3)
                .header(&old_path, &new_path)
                .to_string(),
            Source::Patch(patch) if patch.starts_with("@@ ") => {
                format!("--- {old_path}\n+++ {new_path}\n{patch}")
            }
            Source::Patch(patch) => patch,
        };
        if patch.is_empty() && metadata_only {
            patch = format!("--- {old_path}\n+++ {new_path}\n");
        }
        if patch.is_empty() {
            continue;
        }
        match DiffFile::parse(&patch) {
            Ok(files) if !files.is_empty() => document.files.extend(files),
            Ok(_) => document.error = Some(ParseFailure::NoFiles),
            Err(e) => document.error = Some(ParseFailure::Invalid(e.to_string())),
        }
        if !document.raw.is_empty() {
            document.raw.push('\n');
        }
        document.raw.push_str(&patch);
    }
    document
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn preserves_native_patches_and_builds_requested_snippets_without_disk() {
        let patch = "--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let input = json!({"path":"a.txt","oldText":"wrong","newText":"wrong"});
        let d = build(changes("edit", &input, Some(patch), false));
        assert_eq!(d.raw, patch);
        let english_patch = build(changes("edit",&input,Some(patch),true));
        assert_eq!(english_patch.raw,patch);
        let english_excerpt = changes("edit",&json!({"oldText":"原文","newText":"新文"}),None,true);
        assert_eq!(english_excerpt[0].original_path.as_deref(),Some("Change excerpt"));
        let edits=json!({"path":"原路径.txt","edits":[{"oldText":"a","newText":"b"},{"oldText":"c","newText":"d"}]});
        let english_edits=changes("edit",&edits,None,true);
        assert_eq!(english_edits[0].original_path.as_deref(),Some("原路径.txt (excerpt 1)"));
        assert_eq!(english_edits[1].modified_path.as_deref(),Some("原路径.txt (excerpt 2)"));
        let original_edits=changes("edit",&edits,None,false);
        assert_eq!(original_edits[0].original_path.as_deref(),Some("原路径.txt（片段 1）"));
        assert!(d.error.is_none());
        assert_eq!((d.files[0].additions(), d.files[0].deletions()), (1, 1));
        let d = build(changes(
            "MultiEdit",
            &json!({"file_path":"目录/空 格\t\".rs","edits":[{"old_string":"a\n","new_string":"b\n"},{"old_string":"","new_string":"尾巴 🐝"}]}),
            None, false));
        assert!(d.error.is_none(), "{:?}", d.error);
        assert_eq!(d.files.len(), 2);
        assert_eq!(d.files[0].path().as_ref(), "目录/空 格\t\".rs（片段 1）");
        assert_eq!((d.files[1].additions(), d.files[1].deletions()), (1, 0));
        assert!(d.raw.contains("\\ No newline at end of file"));
        let d = build(changes(
            "edit",
            &json!({"path":"a","edits":[{"oldText":"unchanged\n","newText":"unchanged\n"}]}),
            None, false));
        assert!(d.files.is_empty());
        assert!(d.error.is_none());
        assert!(changes("bash", &json!({"oldText":"a","newText":"b"}), None, false).is_empty());
    }
    #[test]
    fn codex_add_delete_update_and_malformed_patch_remain_distinct() {
        let d = build(changes(
            "fileChange",
            &json!({"changes":[
                {"path":"new.txt","kind":{"type":"add"},"diff":"new\n"},
                {"path":"gone.txt","kind":{"type":"delete"},"diff":"gone\n"},
                {"path":"old.txt","kind":{"type":"update","move_path":"moved.txt"},"diff":"@@ -1 +1 @@\n-before\n+after\n"}
            ]}),
            None, false));
        assert!(d.error.is_none(), "{:?}", d.error);
        assert_eq!(d.files.len(), 3);
        assert_eq!((d.files[0].additions(), d.files[0].deletions()), (1, 0));
        assert_eq!((d.files[1].additions(), d.files[1].deletions()), (0, 1));
        assert_eq!(d.files[2].path().as_ref(), "moved.txt");
        assert_eq!(d.files[2].original_path().unwrap().as_ref(), "old.txt");
        assert_eq!(
            d.files[0].status(),
            gpui_kit::component::diff::DiffFileStatus::Added
        );
        assert_eq!(
            d.files[1].status(),
            gpui_kit::component::diff::DiffFileStatus::Deleted
        );
        assert_eq!((d.files[2].additions(), d.files[2].deletions()), (1, 1));
        let empty = build(changes(
            "fileChange",
            &json!({"changes":[{"path":"empty.txt","kind":{"type":"add"},"diff":""}]}),
            None, false));
        assert!(empty.error.is_none());
        assert_eq!(empty.files.len(), 1);
        assert_eq!(
            empty.files[0].status(),
            gpui_kit::component::diff::DiffFileStatus::Added
        );
        let d = build(vec![Change {
            original_path: Some("bad".into()),
            modified_path: Some("bad".into()),
            source: Source::Patch("not a patch".into()),
        }]);
        assert!(matches!(d.error,Some(ParseFailure::Invalid(_))));
        assert_eq!(d.raw, "not a patch");
    }
}
