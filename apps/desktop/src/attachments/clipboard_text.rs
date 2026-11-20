use super::*;
use serde::Serialize;
use std::{fs, io, path::Path};
use tempfile::NamedTempFile;

pub(crate) const MIN_BYTES: usize = 32 * 1024;
pub(crate) const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
pub(super) struct ClipboardTextPaste {
    workspace: String,
    key: String,
    text: String,
    staged: Option<AttachmentRef>,
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

impl ClipboardTextPaste {
    fn file(&self, root: &Path) -> io::Result<PathBuf> {
        let (scope, id) = self.key.split_once(':').unwrap_or_default();
        if !safe_id(&self.workspace)
            || !safe_id(id)
            || !(scope == "session" || (scope == "workspace" && self.workspace == id))
            || !(MIN_BYTES..=MAX_BYTES).contains(&self.text.len())
            || self.staged.as_ref().is_some_and(|attachment| {
                attachment.workspace_id != self.workspace
                    || attachment.mime_type != "text/plain"
                    || attachment.size != self.text.len() as u64
            })
        {
            return Err(io::Error::other("Invalid clipboard text draft"));
        }
        Ok(root
            .join("desktop-clipboard-text")
            .join(format!("{scope}-{id}.json")))
    }

    fn save(&self, root: &Path) -> io::Result<()> {
        let path = self.file(root)?;
        let directory = path.parent().expect("clipboard text directory");
        fs::create_dir_all(directory)?;
        let mut file = NamedTempFile::new_in(directory)?;
        serde_json::to_writer(file.as_file_mut(), self)?;
        file.as_file().sync_all()?;
        if self.staged.is_some() {
            file.persist(&path)?;
        } else {
            file.persist_noclobber(&path)?;
        }
        #[cfg(unix)]
        {
            let sync = fs::File::open(directory)
                .and_then(|dir| dir.sync_all())
                .and_then(|_| fs::File::open(root)?.sync_all());
            if sync.is_err() && self.staged.is_none() {
                let _ = fs::remove_file(path);
            }
            sync?;
        }
        Ok(())
    }

    fn remove(&self, root: &Path) -> io::Result<()> {
        match fs::remove_file(self.file(root)?) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }

    fn load(root: &Path) -> io::Result<Vec<Self>> {
        let files = match fs::read_dir(root.join("desktop-clipboard-text")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            result => result?,
        };
        let mut pastes = Vec::new();
        for file in files {
            let file = file?;
            let path = file.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let metadata = file.metadata()?;
            if !file.file_type()?.is_file() || metadata.len() > (MAX_BYTES as u64 * 6 + 4096) {
                return Err(io::Error::other("Invalid clipboard text draft file"));
            }
            let paste: Self = serde_json::from_reader(fs::File::open(&path)?)?;
            if paste.file(root)? != path {
                return Err(io::Error::other("Clipboard text draft identity mismatch"));
            }
            pastes.push(paste);
        }
        Ok(pastes)
    }
}

#[cfg(test)]
mod tests {
    use super::{ClipboardTextPaste, MIN_BYTES};
    use crate::protocol::AttachmentRef;

    #[test]
    fn journal_reopens_exact_text_and_committed_attachment_under_original_owner() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let text = "\u{feff}界🐝\t\r\n\0".repeat(MIN_BYTES / 8);
        let mut paste = ClipboardTextPaste {
            workspace: "test-project".into(),
            key: "session:test-session".into(),
            text: text.clone(),
            staged: None,
        };
        paste.save(root).unwrap();
        assert!(
            paste.save(root).is_err(),
            "A second pending paste cannot overwrite the first"
        );
        let mut loaded = ClipboardTextPaste::load(root).unwrap();
        assert_eq!(loaded.len(), 1);
        let pending = loaded.pop().unwrap();
        assert_eq!(pending.text.as_bytes(), text.as_bytes());
        assert_eq!(pending.key, paste.key);
        assert!(pending.staged.is_none());

        let attachment = AttachmentRef {
            id: "file-id".into(),
            workspace_id: paste.workspace.clone(),
            name: "pasted-text.txt".into(),
            mime_type: "text/plain".into(),
            size: text.len() as u64,
            project_file: None,
        };
        paste.staged = Some(AttachmentRef {
            workspace_id: "other-project".into(),
            ..attachment.clone()
        });
        assert!(paste.save(root).is_err());
        paste.staged = Some(attachment);
        paste.save(root).unwrap();
        let staged = ClipboardTextPaste::load(root).unwrap().pop().unwrap();
        assert_eq!(staged.staged.as_ref().unwrap().id, "file-id");
        assert_eq!(staged.text.as_bytes(), text.as_bytes());
        staged.remove(root).unwrap();
        assert!(ClipboardTextPaste::load(root).unwrap().is_empty());
    }
}

impl HiveApp {
    pub(crate) fn restore_clipboard_text_pastes(&mut self, root: &Path) -> io::Result<()> {
        for paste in ClipboardTextPaste::load(root)? {
            if paste
                .staged
                .as_ref()
                .is_some_and(|attachment| self.attachments.contains(Some(&paste.key), attachment))
            {
                // The draft was committed before a process exit interrupted journal cleanup.
                let _ = paste.remove(root);
            } else {
                self.attachments
                    .clipboard_text
                    .insert(paste.key.clone(), paste);
            }
        }
        Ok(())
    }

    pub(super) fn pending_clipboard_text(&self) -> Option<&ClipboardTextPaste> {
        self.draft_target
            .as_ref()
            .and_then(|key| self.attachments.clipboard_text.get(key))
            .filter(|paste| self.workspace.as_deref() == Some(&paste.workspace))
    }

    fn clipboard_attachment_target(&self) -> Option<(&str, &str)> {
        let workspace = self.workspace.as_deref()?;
        let key = self.draft_target.as_deref()?;
        let (scope, owner) = key.split_once(':')?;
        let exact_owner = match self.session.as_ref() {
            Some(session) => {
                scope == "session" && owner == session.id && workspace == session.workspace_id
            }
            None => scope == "workspace" && owner == workspace,
        };
        (exact_owner
            && self.draft_target == self.draft_owner
            && self.connected
            && !self.closing
            && !self.loading
            && self.viewing_conversation()
            && !self.conversation_is_read_only()
            && self.workspace_writes_allowed()
            && self.runtime.is_some()
            && self.draft_store.is_some()
            && !self.attachments.loading.contains(key)
            && self
                .attachments
                .drafts
                .get(key)
                .is_none_or(|items| items.len() < 10)
            && self
                .session
                .as_ref()
                .is_none_or(|session| !self.inflight_inputs.contains_key(&session.id)))
        .then_some((workspace, key))
    }

    pub(crate) fn stage_clipboard_text(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        let (Some(root), Some((workspace, key))) =
            (self.data_dir.as_ref(), self.clipboard_attachment_target())
        else {
            return false;
        };
        if self.attachments.clipboard_text.contains_key(key) {
            return false;
        }
        let paste = ClipboardTextPaste {
            workspace: workspace.to_owned(),
            key: key.to_owned(),
            text,
            staged: None,
        };
        if paste.save(root).is_err() {
            // The native paste can still put the exact text into the composer.
            return false;
        }
        self.attachments
            .clipboard_text
            .insert(paste.key.clone(), paste);
        self.retry_clipboard_text(cx);
        true
    }

    fn retry_clipboard_text(&mut self, cx: &mut Context<Self>) {
        let Some((workspace, key)) = self.clipboard_attachment_target() else {
            return;
        };
        let Some(paste) = self.attachments.clipboard_text.get(key) else {
            return;
        };
        if paste.workspace != workspace {
            return;
        }
        let (key, workspace) = (key.to_owned(), workspace.to_owned());
        if paste.staged.is_some() {
            self.finish_clipboard_text(&key, cx);
            return;
        }
        let expected_size = paste.text.len() as u64;
        let params = json!({"workspaceId": workspace, "text": paste.text});
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.attachments.loading.insert(key.clone());
        let connection = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.attachments.clipboardText", params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<StagedImage>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.attachments.loading.remove(&key);
                if this.workspace_removing.as_ref() == Some(&workspace)
                    || !this
                        .workspaces
                        .iter()
                        .any(|project| project.id == workspace)
                    || key
                        .strip_prefix("session:")
                        .is_some_and(|id| this.removed_sessions.contains(id))
                {
                    cx.notify();
                    return;
                }
                if let Some(paste) = this.attachments.clipboard_text.get_mut(&key) {
                    if let Ok(item) = result {
                        if item.image.is_none()
                            && item.attachment.workspace_id == workspace
                            && item.attachment.mime_type == "text/plain"
                            && item.attachment.size == expected_size
                        {
                            paste.staged = Some(item.attachment);
                            this.finish_clipboard_text(&key, cx);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn finish_clipboard_text(&mut self, key: &str, cx: &mut Context<Self>) {
        let (Some(root), Some(paste)) = (
            self.data_dir.clone(),
            self.attachments.clipboard_text.get(key),
        ) else {
            return;
        };
        let Some(attachment) = paste.staged.clone() else {
            return;
        };
        if paste.save(&root).is_err() {
            return;
        }
        let items = self.attachments.drafts.entry(key.to_owned()).or_default();
        let existing = items.iter().any(|item| item.id == attachment.id);
        if !existing {
            if items.len() >= 10 {
                return;
            }
            items.push(attachment.clone());
        }
        if self.flush_drafts(cx).is_err() {
            if !existing {
                self.attachments
                    .drafts
                    .get_mut(key)
                    .unwrap()
                    .retain(|item| item.id != attachment.id);
            }
            return;
        }
        if let Some(paste) = self.attachments.clipboard_text.remove(key) {
            let _ = paste.remove(&root);
        }
        self.attachments.verified.insert(attachment.id);
        cx.notify();
    }

    pub(super) fn render_pending_clipboard_text(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let paste = self.pending_clipboard_text()?;
        if self.attachments.loading.contains(&paste.key) {
            return None;
        }
        let retry_key = paste.key.clone();
        let copy_key = paste.key.clone();
        let dismiss_key = paste.key.clone();
        Some(
            h_flex()
                .gap_2()
                .flex_wrap()
                .items_center()
                .child(div().text_xs().child(tr("粘贴文本尚未存为附件。")))
                .child(
                    Button::new("retry-clipboard-text")
                        .small()
                        .outline()
                        .label(tr("重试保存粘贴文本"))
                        .disabled(self.clipboard_attachment_target().is_none())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this
                                .pending_clipboard_text()
                                .is_some_and(|paste| paste.key == retry_key)
                            {
                                this.retry_clipboard_text(cx);
                            }
                        })),
                )
                .child(
                    Button::new("copy-clipboard-text")
                        .small()
                        .outline()
                        .label(tr("复制粘贴文本"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(paste) = this
                                .pending_clipboard_text()
                                .filter(|paste| paste.key == copy_key)
                            {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    paste.text.clone(),
                                ));
                            }
                        })),
                )
                .child(
                    Button::new("dismiss-clipboard-text")
                        .small()
                        .outline()
                        .label(tr("移除待保存的粘贴文本"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let (Some(root), Some(paste)) = (
                                this.data_dir.as_ref(),
                                this.pending_clipboard_text()
                                    .filter(|paste| paste.key == dismiss_key),
                            ) else {
                                return;
                            };
                            if this.attachments.loading.contains(&paste.key) {
                                return;
                            }
                            if paste.remove(root).is_ok() {
                                let key = paste.key.clone();
                                this.attachments.clipboard_text.remove(&key);
                                cx.notify();
                            }
                        })),
                )
                .into_any_element(),
        )
    }
}
