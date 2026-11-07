use crate::app::HiveApp;
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Draft {
    key: String,
    input: String,
    failed: Vec<String>,
    #[serde(default)]
    attachments: Vec<crate::attachments::AttachmentRef>,
}

impl Draft {
    fn filename(&self) -> io::Result<String> {
        let (scope, id) = self.key.split_once(':').unwrap_or_default();
        if !matches!(scope, "session" | "workspace")
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
        Ok(Self { dir, saved })
    }

    fn save(
        &mut self,
        inputs: &HashMap<String, String>,
        failed: &HashMap<String, Vec<String>>,
        images: &HashMap<String, Vec<crate::attachments::AttachmentRef>>,
    ) -> io::Result<()> {
        let mut next = HashMap::new();
        for (key, text) in inputs.iter().filter(|(_, text)| !text.is_empty()) {
            next.insert(
                key.clone(),
                Draft {
                    key: key.clone(),
                    input: text.clone(),
                    failed: Vec::new(),
                    attachments: Vec::new(),
                },
            );
        }
        for (id, messages) in failed.iter().filter(|(_, messages)| !messages.is_empty()) {
            let key = format!("session:{id}");
            next.entry(key.clone())
                .or_insert_with(|| Draft {
                    key,
                    input: String::new(),
                    failed: Vec::new(),
                    attachments: Vec::new(),
                })
                .failed
                .clone_from(messages);
        }
        for (key, items) in images.iter().filter(|(_, items)| !items.is_empty()) {
            next.entry(key.clone())
                .or_insert_with(|| Draft {
                    key: key.clone(),
                    input: String::new(),
                    failed: Vec::new(),
                    attachments: Vec::new(),
                })
                .attachments
                .clone_from(items);
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
}

impl HiveApp {
    pub(crate) fn restore_drafts(&mut self, root: &Path) -> io::Result<()> {
        let store = DraftStore::open(root)?;
        for draft in store.saved.values() {
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
        self.capture_editor(cx);
        if let Some(store) = &mut self.draft_store {
            let mut inputs = self.drafts.clone();
            for (id, text) in &self.inflight_inputs {
                let key = format!("session:{id}");
                if self
                    .attachments
                    .drafts
                    .get(&key)
                    .is_some_and(|items| !items.is_empty())
                {
                    inputs.entry(key).or_insert_with(|| text.clone());
                }
            }
            store.save(&inputs, &self.failed_drafts, &self.attachments.drafts)?;
        }
        Ok(())
    }

    pub(crate) fn schedule_draft_save(&mut self, cx: &mut Context<Self>) {
        if self.draft_store.is_none() {
            return;
        }
        self.draft_save_task = Some(cx.spawn(async move |this, cx| {
            smol::Timer::after(Duration::from_millis(450)).await;
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = this.flush_drafts(cx) {
                    this.error = Some(format!("无法保存本地草稿：{error}"));
                    cx.notify();
                }
            });
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_only_draft_preserves_upload_identity_and_legacy_records() {
        let root = tempfile::tempdir().unwrap();
        let key = "session:images".to_owned();
        let refs = vec![crate::attachments::AttachmentRef {
            id: "image-id".into(),
            workspace_id: "workspace".into(),
            name: "截图.png".into(),
            mime_type: "image/png".into(),
            size: 128,
        }];
        let images = HashMap::from([(key.clone(), refs.clone())]);
        let mut store = DraftStore::open(root.path()).unwrap();
        store
            .save(&HashMap::new(), &HashMap::new(), &images)
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
            .save(&HashMap::new(), &HashMap::new(), &HashMap::new())
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
        first.save(&inputs, &failed, &HashMap::new()).unwrap();
        other
            .save(
                &HashMap::from([("workspace:beta".into(), "下一步".into())]),
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();
        let store = DraftStore::open(root.path()).unwrap();
        assert_eq!(store.saved["session:alpha"].input, "请检查\n第二行");
        assert_eq!(store.saved["session:alpha"].failed, ["第一次"]);
        first
            .save(&HashMap::new(), &HashMap::new(), &HashMap::new())
            .unwrap();
        let store = DraftStore::open(root.path()).unwrap();
        assert_eq!(store.saved.len(), 1);
        assert_eq!(store.saved["workspace:beta"].input, "下一步");
    }
}
