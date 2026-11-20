use crate::{
    app::HiveApp,
    attachments::AttachmentRef,
    draft_stash::{MAX_STASH_ENTRIES, StashAction, StashedDraft},
};
use gpui_kit::Context;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::NamedTempFile;

mod discard;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Draft {
    key: String,
    input: String,
    failed: Vec<String>,
    #[serde(default)]
    attachments: Vec<AttachmentRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stash: Vec<StashedDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    task: Option<crate::collaboration::SavedTaskDraft>,
    #[serde(
        default,
        rename = "fork_imported",
        skip_serializing_if = "Option::is_none"
    )]
    imported_from: Option<String>,
}

impl Draft {
    fn empty(key: String) -> Self {
        Self {
            key,
            input: String::new(),
            failed: Vec::new(),
            attachments: Vec::new(),
            stash: Vec::new(),
            task: None,
            imported_from: None,
        }
    }

    fn filename(&self) -> io::Result<String> {
        let (scope, id) = self.key.split_once(':').unwrap_or_default();
        if !matches!(scope, "session" | "workspace" | "tasks")
            || (scope == "tasks") != self.task.is_some()
            || id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(io::Error::new(ErrorKind::InvalidData, "草稿标识无效"));
        }
        Ok(format!("{scope}-{id}.json"))
    }
}

pub(crate) struct DraftStore {
    dir: PathBuf,
    saved: HashMap<String, Draft>,
    discarded: HashMap<String, discard::PendingDiscard>,
}

impl DraftStore {
    fn open(root: &Path) -> io::Result<Self> {
        let dir = root.join("desktop-drafts");
        fs::create_dir_all(&dir)?;
        let mut saved = HashMap::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.path().extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let draft: Draft = serde_json::from_reader(fs::File::open(entry.path())?)?;
            if entry.file_name().to_str() != Some(&draft.filename()?) {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "草稿标识与文件名不符",
                ));
            }
            saved.insert(draft.key.clone(), draft);
        }
        Ok(Self { dir, saved, discarded: HashMap::new() })
    }

    fn save(
        &mut self,
        inputs: &HashMap<String, String>,
        failed: &HashMap<String, Vec<String>>,
        images: &HashMap<String, Vec<crate::attachments::AttachmentRef>>,
        tasks: &HashMap<String, crate::collaboration::SavedTaskDraft>,
    ) -> io::Result<()> {
        let mut next = HashMap::new();
        // Each owner has one file. Autosave rebases the active composer
        // fields without losing durable stash entries or fork import markers.
        for (key, saved) in &self.saved {
            if saved.imported_from.is_some() || !saved.stash.is_empty() {
                let draft = next
                    .entry(key.clone())
                    .or_insert_with(|| Draft::empty(key.clone()));
                draft.imported_from.clone_from(&saved.imported_from);
                draft.stash.clone_from(&saved.stash);
            }
        }
        for (key, text) in inputs.iter().filter(|(_, text)| !text.is_empty()) {
            next.entry(key.clone())
                .or_insert_with(|| Draft::empty(key.clone()))
                .input
                .clone_from(text);
        }
        for (id, messages) in failed.iter().filter(|(_, messages)| !messages.is_empty()) {
            let key = format!("session:{id}");
            next.entry(key.clone())
                .or_insert_with(|| Draft::empty(key))
                .failed
                .clone_from(messages);
        }
        for (key, items) in images.iter().filter(|(_, items)| !items.is_empty()) {
            next.entry(key.clone())
                .or_insert_with(|| Draft::empty(key.clone()))
                .attachments
                .clone_from(items);
        }
        for (id, task) in tasks {
            let key = format!("tasks:{id}");
            next.entry(key.clone())
                .or_insert_with(|| Draft::empty(key))
                .task = Some(task.clone());
        }
        for (key, draft) in &next {
            if self.saved.get(key) == Some(draft) {
                continue;
            }
            let path = self.dir.join(draft.filename()?);
            let mut tmp = NamedTempFile::new_in(&self.dir)?;
            serde_json::to_writer(tmp.as_file_mut(), draft)?;
            tmp.persist(path)?;
            self.saved.insert(key.clone(), draft.clone());
        }
        let removed: Vec<_> = self
            .saved
            .keys()
            .filter(|key| !next.contains_key(*key))
            .cloned()
            .collect();
        for key in removed {
            match fs::remove_file(self.dir.join(self.saved[&key].filename()?)) {
                Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
                _ => {
                    self.saved.remove(&key);
                }
            }
        }
        Ok(())
    }
    pub(crate) fn stashed(&self, key: &str) -> &[StashedDraft] {
        self.saved
            .get(key)
            .map(|draft| draft.stash.as_slice())
            .unwrap_or_default()
    }

    pub(crate) fn change_stash(
        &mut self,
        key: &str,
        action: StashAction,
        mut text: String,
        mut attachments: Vec<AttachmentRef>,
        failed: Vec<String>,
    ) -> io::Result<(String, Vec<AttachmentRef>)> {
        if !key.starts_with("session:") && !key.starts_with("workspace:") {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "Expected a composer draft owner",
            ));
        }
        let mut draft = self
            .saved
            .get(key)
            .cloned()
            .unwrap_or_else(|| Draft::empty(key.to_owned()));
        let empty = text.is_empty() && attachments.is_empty();
        match action {
            StashAction::Save => {
                if empty || draft.stash.len() >= MAX_STASH_ENTRIES {
                    return Err(io::Error::new(
                        ErrorKind::InvalidInput,
                        "Composer stash is empty or full",
                    ));
                }
                draft.stash.insert(
                    0,
                    StashedDraft {
                        id: uuid::Uuid::new_v4().to_string(),
                        saved_at: chrono::Utc::now().timestamp_millis(),
                        text: std::mem::take(&mut text),
                        attachments: std::mem::take(&mut attachments),
                    },
                );
            }
            StashAction::Restore(id) | StashAction::Remove(id) => {
                if matches!(action, StashAction::Restore(_)) && !empty {
                    return Err(io::Error::new(
                        ErrorKind::InvalidInput,
                        "Composer has a newer draft",
                    ));
                }
                let id = id.to_string();
                let index = draft
                    .stash
                    .iter()
                    .position(|entry| entry.id == id)
                    .ok_or_else(|| {
                        io::Error::new(ErrorKind::NotFound, "Stashed draft no longer exists")
                    })?;
                let entry = draft.stash.remove(index);
                if matches!(action, StashAction::Restore(_)) {
                    text = entry.text;
                    attachments = entry.attachments;
                }
            }
        }
        draft.input.clone_from(&text);
        draft.attachments.clone_from(&attachments);
        draft.failed = failed;

        // Publication is the commit point. Update the active editor only once
        // this file contains both the new stash and the new active draft.
        self.publish_draft(draft)?;
        Ok((text, attachments))
    }

    pub(crate) fn remove_workspace_stashes(
        &mut self,
        belongs: impl Fn(&str) -> bool,
    ) -> io::Result<()> {
        let affected: Vec<_> = self
            .saved
            .iter()
            .filter(|(key, draft)| belongs(key) && !draft.stash.is_empty())
            .map(|(_, draft)| draft.clone())
            .collect();
        for mut draft in affected {
            draft.stash.clear();
            self.publish_draft(draft)?;
        }
        Ok(())
    }

    fn publish_draft(&mut self, draft: Draft) -> io::Result<()> {
        let mut tmp = NamedTempFile::new_in(&self.dir)?;
        serde_json::to_writer(tmp.as_file_mut(), &draft)?;
        tmp.as_file().sync_all()?;
        tmp.persist(self.dir.join(draft.filename()?))?;
        self.saved.insert(draft.key.clone(), draft);
        // The rename already committed. A directory sync error must not
        // cause the editor to keep the old value and save it over this one.
        #[cfg(unix)]
        if let Err(error) = fs::File::open(&self.dir).and_then(|file| file.sync_all()) {
            eprintln!("[hive-desktop] Could not sync composer draft directory: {error}");
        }
        Ok(())
    }

    pub(crate) fn fork_imported(&self, key: &str, operation: &str) -> bool {
        self.already_imported(key, operation)
    }
    pub(crate) fn already_imported(&self, key: &str, operation: &str) -> bool {
        self.saved
            .get(key)
            .is_some_and(|d| d.imported_from.as_deref() == Some(operation))
    }
    pub(crate) fn mark_existing_fork(&mut self, key: &str, operation: &str) -> io::Result<()> {
        let mut draft = self
            .saved
            .get(key)
            .cloned()
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "Draft missing"))?;
        draft.imported_from = Some(operation.to_owned());
        let mut tmp = NamedTempFile::new_in(&self.dir)?;
        serde_json::to_writer(tmp.as_file_mut(), &draft)?;
        tmp.as_file().sync_all()?;
        tmp.persist(self.dir.join(draft.filename()?))?;
        self.saved.insert(key.to_owned(), draft);
        Ok(())
    }
    pub(crate) fn import_fork(
        &mut self,
        key: &str,
        operation: &str,
        text: String,
        attachments: Vec<crate::attachments::AttachmentRef>,
    ) -> io::Result<()> {
        self.import_once(key, operation, text, attachments)
    }
    pub(crate) fn import_once(
        &mut self,
        key: &str,
        operation: &str,
        text: String,
        attachments: Vec<crate::attachments::AttachmentRef>,
    ) -> io::Result<()> {
        let prior = self.saved.get(key);
        if prior.is_some_and(|draft| {
            draft.stash.is_empty()
                || !draft.input.is_empty()
                || !draft.attachments.is_empty()
                || !draft.failed.is_empty()
                || draft.imported_from.is_some()
        }) {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                "Draft already exists",
            ));
        }
        let replacing_stash_file = prior.is_some();
        let draft = Draft {
            key: key.to_owned(),
            input: text,
            failed: vec![],
            attachments,
            stash: prior.map(|draft| draft.stash.clone()).unwrap_or_default(),
            task: None,
            imported_from: Some(operation.to_owned()),
        };
        let path = self.dir.join(draft.filename()?);
        let mut tmp = NamedTempFile::new_in(&self.dir)?;
        serde_json::to_writer(tmp.as_file_mut(), &draft)?;
        tmp.as_file().sync_all()?;
        if replacing_stash_file {
            tmp.persist(path)?;
        } else {
            tmp.persist_noclobber(path)?;
        }
        #[cfg(unix)]
        fs::File::open(&self.dir)?.sync_all()?;
        self.saved.insert(key.to_owned(), draft);
        Ok(())
    }
}

impl HiveApp {
    pub(crate) fn restore_drafts(&mut self, root: &Path) -> io::Result<()> {
        self.pi_models.set_draft_root(root);
        let store = DraftStore::open(root)?;
        for draft in store.saved.values() {
            if let Some(task) = &draft.task {
                self.collaboration.recovered_task_drafts.insert(
                    draft
                        .key
                        .strip_prefix("tasks:")
                        .expect("validated task scope")
                        .to_owned(),
                    task.clone(),
                );
                continue;
            }
            if !draft.attachments.is_empty() {
                self.attachments
                    .drafts
                    .insert(draft.key.clone(), draft.attachments.clone());
            }
            if !draft.input.is_empty() {
                self.drafts.insert(draft.key.clone(), draft.input.clone());
            }
            if let Some(id) = draft.key.strip_prefix("session:")
                && !draft.failed.is_empty()
            {
                self.failed_drafts
                    .insert(id.to_owned(), draft.failed.clone());
            }
        }
        self.draft_store = Some(store);
        self.restore_clipboard_text_pastes(root)?;
        Ok(())
    }

    pub(crate) fn capture_editor(&mut self, cx: &Context<Self>) {
        if self.draft_owner != self.draft_target {
            return;
        }
        if let Some(key) = &self.draft_owner {
            let value = self.input.read(cx).value();
            if value.is_empty() {
                self.drafts.remove(key);
            } else {
                self.drafts.insert(key.clone(), value.to_string());
            }
        }
    }

    pub(crate) fn flush_drafts(&mut self, cx: &Context<Self>) -> io::Result<()> {
        self.pi_models.flush_draft(cx)?;
        self.capture_editor(cx);
        let tasks = self.task_draft_snapshots(cx);
        if !tasks.is_empty() && self.draft_store.is_none() {
            return Err(io::Error::other(
                "本地草稿存储尚未可用，请复制任务草稿或丢弃改动后退出",
            ));
        }
        if let Some(store) = &mut self.draft_store {
            let mut inputs = self.drafts.clone();
            for (id, text) in &self.inflight_inputs {
                let key = format!("session:{id}");
                inputs.entry(key).or_insert_with(|| text.clone());
            }
            store.save(
                &inputs,
                &self.failed_drafts,
                &self.attachments.drafts,
                &tasks,
            )?;
        }
        Ok(())
    }

    pub(crate) fn restore_pr_review_draft(
        &mut self,
        session_id: &str,
        prompt: String,
        cx: &mut Context<Self>,
    ) {
        let key = format!("session:{session_id}");
        // Use the same durable one-time import marker as an edited history fork.
        let source = "github-pr-review";
        if self
            .draft_store
            .as_ref()
            .is_some_and(|store| store.fork_imported(&key, source))
        {
            return;
        }
        self.capture_editor(cx);
        let existing =
            self.drafts.contains_key(&key) || !self.attachments.refs(Some(&key)).is_empty();
        let result = (|| -> io::Result<()> {
            if existing {
                self.flush_drafts(cx)?;
            }
            let store = self
                .draft_store
                .as_mut()
                .ok_or_else(|| io::Error::other("Draft store unavailable"))?;
            if existing {
                store.mark_existing_fork(&key, source)
            } else {
                store.import_fork(&key, source, prompt.clone(), vec![])
            }
        })();
        match result {
            Ok(()) if !existing => {
                self.drafts.insert(key, prompt);
                self.draft_owner = None;
            }
            Ok(()) => {}
            Err(error) => {
                self.error = Some(crate::app::ErrorNotice::Detail(
                    "无法恢复 PR 评审草稿",
                    error.to_string(),
                ));
            }
        }
    }

    pub(crate) fn schedule_draft_save(&mut self, cx: &mut Context<Self>) {
        if self.draft_store.is_none() && !self.pi_models.has_draft_root() {
            return;
        }
        self.draft_save_task = Some(cx.spawn(async move |this, cx| {
            smol::Timer::after(Duration::from_millis(450)).await;
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = this.flush_drafts(cx) {
                    this.error = Some(crate::app::ErrorNotice::Detail(
                        "无法保存本地草稿",
                        error.to_string(),
                    ));
                    cx.notify();
                }
            });
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stash_file() -> AttachmentRef {
        AttachmentRef {
            id: "immutable-file-id".into(),
            workspace_id: "workspace".into(),
            name: "draft.txt".into(),
            mime_type: "text/plain".into(),
            size: 9,
            project_file: None,
        }
    }

    #[test]
    fn stash_preserves_attachments_and_other_draft_state_on_reopen() {
        let root = tempfile::tempdir().unwrap();
        let key = "session:conversation";
        let failed = vec!["older failed delivery".into()];
        let attachment = stash_file();
        let mut store = DraftStore::open(root.path()).unwrap();
        store
            .import_fork(key, "fork", "original".into(), vec![])
            .unwrap();
        let saved = store
            .change_stash(
                key,
                StashAction::Save,
                "multiline\n草稿".into(),
                vec![attachment.clone()],
                failed.clone(),
            )
            .unwrap();
        assert_eq!(saved, (String::new(), vec![]));
        store
            .change_stash(
                key,
                StashAction::Save,
                "".into(),
                vec![attachment.clone()],
                failed.clone(),
            )
            .unwrap();
        store
            .save(
                &HashMap::new(),
                &HashMap::from([("conversation".into(), failed.clone())]),
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();

        let mut store = DraftStore::open(root.path()).unwrap();
        assert_eq!(store.stashed(key).len(), 2);
        assert!(store.stashed("session:other").is_empty());
        assert!(store.saved[key].input.is_empty());
        assert!(store.saved[key].attachments.is_empty());
        assert_eq!(store.saved[key].failed, failed);
        assert!(store.fork_imported(key, "fork"));
        let old = uuid::Uuid::parse_str(&store.stashed(key)[1].id).unwrap();
        assert!(
            store
                .change_stash(
                    key,
                    StashAction::Restore(old),
                    "new typing".into(),
                    vec![],
                    failed.clone()
                )
                .is_err()
        );
        assert!(
            store
                .change_stash(
                    "session:other",
                    StashAction::Restore(old),
                    String::new(),
                    vec![],
                    vec![]
                )
                .is_err()
        );
        assert_eq!(store.stashed(key).len(), 2);
        let restored = store
            .change_stash(
                key,
                StashAction::Restore(old),
                String::new(),
                vec![],
                failed.clone(),
            )
            .unwrap();
        assert_eq!(
            restored,
            ("multiline\n草稿".into(), vec![attachment.clone()])
        );
        let mut store = DraftStore::open(root.path()).unwrap();
        assert_eq!(
            (
                store.saved[key].input.as_str(),
                &store.saved[key].attachments
            ),
            ("multiline\n草稿", &vec![attachment])
        );
        let leftover = uuid::Uuid::parse_str(&store.stashed(key)[0].id).unwrap();
        store
            .change_stash(
                key,
                StashAction::Remove(leftover),
                "later text".into(),
                vec![],
                failed.clone(),
            )
            .unwrap();
        let store = DraftStore::open(root.path()).unwrap();
        assert_eq!(store.saved[key].input, "later text");
        assert!(store.stashed(key).is_empty());
        assert!(store.fork_imported(key, "fork"));
        assert_eq!(store.saved[key].failed, failed);
    }

    #[test]
    fn full_stash_and_failed_write_leave_saved_draft_unchanged() {
        let root = tempfile::tempdir().unwrap();
        let key = "workspace:project";
        let mut store = DraftStore::open(root.path()).unwrap();
        for i in 0..MAX_STASH_ENTRIES {
            store
                .change_stash(key, StashAction::Save, format!("draft {i}"), vec![], vec![])
                .unwrap();
        }
        let file = root.path().join("desktop-drafts/workspace-project.json");
        let before = fs::read(&file).unwrap();
        assert!(
            store
                .change_stash(key, StashAction::Save, "new".into(), vec![], vec![])
                .is_err()
        );
        assert_eq!(fs::read(&file).unwrap(), before);
        let id = uuid::Uuid::parse_str(&store.stashed(key)[0].id).unwrap();
        let dir = root.path().join("desktop-drafts");
        fs::rename(&dir, root.path().join("moved")).unwrap();
        assert!(
            store
                .change_stash(key, StashAction::Restore(id), "".into(), vec![], vec![])
                .is_err()
        );
        assert_eq!(store.stashed(key).len(), MAX_STASH_ENTRIES);
        fs::rename(root.path().join("moved"), &dir).unwrap();
        assert_eq!(fs::read(&file).unwrap(), before);
    }

    #[test]
    fn imported_fork_marker_survives_clearing_and_reopen() {
        let root = tempfile::tempdir().unwrap();
        let mut store = DraftStore::open(root.path()).unwrap();
        store
            .import_fork("session:child", "operation-1", "draft".into(), vec![])
            .unwrap();
        store
            .save(
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();
        let mut reopened = DraftStore::open(root.path()).unwrap();
        assert!(reopened.fork_imported("session:child", "operation-1"));
        assert!(!reopened.fork_imported("session:child", "operation-2"));
        assert!(
            reopened
                .import_fork("session:child", "operation-1", "again".into(), vec![])
                .is_err()
        );
    }
    #[test]
    fn existing_draft_is_preserved_when_fork_is_marked() {
        let root = tempfile::tempdir().unwrap();
        let mut store = DraftStore::open(root.path()).unwrap();
        let inputs = HashMap::from([("session:child".to_owned(), "existing".to_owned())]);
        store
            .save(&inputs, &HashMap::new(), &HashMap::new(), &HashMap::new())
            .unwrap();
        store
            .mark_existing_fork("session:child", "operation-1")
            .unwrap();
        let reopened = DraftStore::open(root.path()).unwrap();
        assert_eq!(reopened.saved["session:child"].input, "existing");
        assert!(reopened.fork_imported("session:child", "operation-1"));
    }
    #[test]
    fn task_drafts_preserve_empty_edits_and_conflict_base_and_remove_only_after_disk_success() {
        use crate::collaboration::SavedTaskDraft;
        let root = tempfile::tempdir().unwrap();
        let mut store = DraftStore::open(root.path()).unwrap();
        let task = SavedTaskDraft {
            input: String::new(),
            base: "original task document".into(),
            conflicting_content: Some("someone else's update".into()),
        };
        let tasks = HashMap::from([("alpha".into(), task.clone())]);
        store
            .save(&HashMap::new(), &HashMap::new(), &HashMap::new(), &tasks)
            .unwrap();
        let reopened = DraftStore::open(root.path()).unwrap();
        let restored = reopened.saved["tasks:alpha"].task.as_ref().unwrap();
        assert_eq!(restored.input, "");
        assert_eq!(restored.base, task.base);
        assert_eq!(restored.conflicting_content, task.conflicting_content);
        let file = root.path().join("desktop-drafts/tasks-alpha.json");
        let held = root.path().join("held.json");
        fs::rename(&file, &held).unwrap();
        fs::create_dir(&file).unwrap();
        assert!(
            store
                .save(
                    &HashMap::new(),
                    &HashMap::new(),
                    &HashMap::new(),
                    &HashMap::new()
                )
                .is_err()
        );
        assert!(store.saved.contains_key("tasks:alpha"));
        fs::remove_dir(&file).unwrap();
        fs::rename(&held, &file).unwrap();
        store
            .save(
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();
        assert!(!file.exists());
        assert!(DraftStore::open(root.path()).unwrap().saved.is_empty());
    }

    #[test]
    fn project_reference_draft_cold_reopen_preserves_snapshot_metadata_and_text() {
        let root = tempfile::tempdir().unwrap();
        let key = "session:project-reference".to_owned();
        let input = "保留普通草稿\n第二行".to_owned();
        let attachment = crate::attachments::AttachmentRef {
            id: "snapshot-id".into(),
            workspace_id: "project".into(),
            name: "需求.md".into(),
            mime_type: "text/markdown".into(),
            size: 24,
            project_file: Some(crate::protocol::ProjectFileReference {
                relative_path: "docs/需求.md".into(),
                sha256: "ab".repeat(32),
                source_session_id: Some("worktree-source".into()),
            }),
        };
        DraftStore::open(root.path())
            .unwrap()
            .save(
                &HashMap::from([(key.clone(), input.clone())]),
                &HashMap::new(),
                &HashMap::from([(key.clone(), vec![attachment.clone()])]),
                &HashMap::new(),
            )
            .unwrap();
        let reopened = DraftStore::open(root.path()).unwrap();
        assert_eq!(reopened.saved[&key].input, input);
        assert_eq!(reopened.saved[&key].attachments, vec![attachment]);
    }

    #[test]
    fn image_only_draft_preserves_upload_identity_and_legacy_records() {
        let root = tempfile::tempdir().unwrap();
        let key = "session:images".to_owned();
        let refs = vec![crate::attachments::AttachmentRef {
            id: "image-id".into(),
            workspace_id: "workspace".into(),
            project_file: None,
            name: "截图.png".into(),
            mime_type: "image/png".into(),
            size: 128,
        }];
        let images = HashMap::from([(key.clone(), refs.clone())]);
        let mut store = DraftStore::open(root.path()).unwrap();
        store
            .save(&HashMap::new(), &HashMap::new(), &images, &HashMap::new())
            .unwrap();
        assert!(
            root.path()
                .join("desktop-drafts/session-images.json")
                .exists()
        );
        let reopened = DraftStore::open(root.path()).unwrap();
        assert_eq!(reopened.saved[&key].attachments, refs);
        assert!(reopened.saved[&key].input.is_empty());
        std::fs::write(
            root.path().join("desktop-drafts/session-legacy.json"),
            r#"{"key":"session:legacy","input":"原有草稿","failed":[]}"#,
        )
        .unwrap();
        let legacy = DraftStore::open(root.path()).unwrap();
        assert_eq!(legacy.saved["session:legacy"].input, "原有草稿");
        assert!(legacy.saved["session:legacy"].attachments.is_empty());
        store
            .save(
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();
        assert!(
            !root
                .path()
                .join("desktop-drafts/session-images.json")
                .exists()
        );
    }

    #[test]
    fn reopen_preserves_multiline_and_failures_without_overwriting_other_windows() {
        let root = tempfile::tempdir().unwrap();
        let mut first = DraftStore::open(root.path()).unwrap();
        let mut other = DraftStore::open(root.path()).unwrap();
        let inputs = HashMap::from([("session:alpha".into(), "请检查\n第二行".into())]);
        let failed = HashMap::from([("alpha".into(), vec!["第一次".into()])]);
        first
            .save(&inputs, &failed, &HashMap::new(), &HashMap::new())
            .unwrap();
        other
            .save(
                &HashMap::from([("workspace:beta".into(), "下一步".into())]),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();
        let store = DraftStore::open(root.path()).unwrap();
        assert_eq!(store.saved["session:alpha"].input, "请检查\n第二行");
        assert_eq!(store.saved["session:alpha"].failed, ["第一次"]);
        first
            .save(
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();
        let store = DraftStore::open(root.path()).unwrap();
        assert_eq!(store.saved.len(), 1);
        assert_eq!(store.saved["workspace:beta"].input, "下一步");
    }
}
