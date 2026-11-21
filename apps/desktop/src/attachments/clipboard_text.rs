use super::*;
use serde::Serialize;
use std::{fs, io, path::Path};
use tempfile::NamedTempFile;

use std::collections::HashSet;

pub(crate) const MIN_BYTES: usize = 32 * 1024;
pub(crate) const MAX_BYTES: usize = 8 * 1024 * 1024;

fn default_pasted_text_name() -> String {
    "pasted-text.txt".to_owned()
}

pub(crate) fn is_valid_pasted_text_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".txt") else {
        return false;
    };
    if stem == "pasted-text" {
        return true;
    }
    let Some(num) = stem.strip_prefix("pasted-text-") else {
        return false;
    };
    if !num.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    match num.as_bytes() {
        [b'2'..=b'9'] => true,
        [b'1'..=b'9', _, ..] => true,
        _ => false,
    }
}

pub(crate) fn next_pasted_text_name(existing: &[AttachmentRef]) -> String {
    let used: HashSet<String> = existing
        .iter()
        .map(|item| item.name.to_ascii_lowercase())
        .collect();
    if !used.contains("pasted-text.txt") {
        return "pasted-text.txt".to_owned();
    }
    for n in 2usize.. {
        let candidate = format!("pasted-text-{n}.txt");
        if !used.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded integer sequence")
}

#[derive(Deserialize, Serialize)]
pub(super) struct ClipboardTextPaste {
    workspace: String,
    key: String,
    text: String,
    #[serde(default = "default_pasted_text_name")]
    name: String,
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
            || !is_valid_pasted_text_name(&self.name)
            || self.staged.as_ref().is_some_and(|attachment| {
                attachment.workspace_id != self.workspace
                    || attachment.mime_type != "text/plain"
                    || attachment.name != self.name
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
    use super::{ClipboardTextPaste, MIN_BYTES, is_valid_pasted_text_name, next_pasted_text_name};
    use crate::protocol::AttachmentRef;

    fn attachment_with_name(name: &str) -> AttachmentRef {
        AttachmentRef {
            id: format!("id-{name}"),
            workspace_id: "ws".into(),
            name: name.into(),
            mime_type: "text/plain".into(),
            size: 1024,
            project_file: None,
        }
    }

    #[test]
    fn journal_reopens_exact_text_and_committed_attachment_under_original_owner() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let text = "\u{feff}界🐝\t\r\n\0".repeat(MIN_BYTES / 8);
        let mut paste = ClipboardTextPaste {
            workspace: "test-project".into(),
            key: "session:test-session".into(),
            text: text.clone(),
            name: "pasted-text.txt".into(),
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

    #[test]
    fn pasted_text_name_increments_case_insensitively() {
        assert_eq!(next_pasted_text_name(&[]), "pasted-text.txt");
        assert_eq!(
            next_pasted_text_name(&[attachment_with_name("notes.md")]),
            "pasted-text.txt"
        );
        assert_eq!(
            next_pasted_text_name(&[attachment_with_name("Pasted-Text.TXT")]),
            "pasted-text-2.txt"
        );
        assert_eq!(
            next_pasted_text_name(&[
                attachment_with_name("pasted-text.txt"),
                attachment_with_name("PASTED-TEXT-2.txt"),
            ]),
            "pasted-text-3.txt"
        );
        assert_eq!(
            next_pasted_text_name(&[
                attachment_with_name("pasted-text.txt"),
                attachment_with_name("pasted-text-3.txt"),
            ]),
            "pasted-text-2.txt"
        );
        assert!(is_valid_pasted_text_name("pasted-text.txt"));
        assert!(is_valid_pasted_text_name("pasted-text-2.txt"));
        assert!(is_valid_pasted_text_name("pasted-text-10.txt"));
        assert!(is_valid_pasted_text_name("pasted-text-19.txt"));
        assert!(is_valid_pasted_text_name("pasted-text-20.txt"));
        assert!(is_valid_pasted_text_name("pasted-text-100.txt"));
        assert!(!is_valid_pasted_text_name("pasted-text-0.txt"));
        assert!(!is_valid_pasted_text_name("pasted-text-1.txt"));
        assert!(!is_valid_pasted_text_name("pasted-text-02.txt"));
        assert!(!is_valid_pasted_text_name("pasted-text-010.txt"));
        assert!(!is_valid_pasted_text_name("pasted-text-.txt"));
        assert!(!is_valid_pasted_text_name("../pasted-text.txt"));

        // 10-attachment boundary: after 9 pasted text files, the 10th gets `pasted-text-10.txt`
        // and must pass `is_valid_pasted_text_name`.
        let mut nine_pasted = vec![attachment_with_name("pasted-text.txt")];
        for n in 2..=9 {
            nine_pasted.push(attachment_with_name(&format!("pasted-text-{n}.txt")));
        }
        let tenth = next_pasted_text_name(&nine_pasted);
        assert_eq!(tenth, "pasted-text-10.txt");
        assert!(is_valid_pasted_text_name(&tenth));

        // Backward compatibility: legacy journal JSON without `name` defaults to `pasted-text.txt`.
        let legacy_json = serde_json::json!({
            "workspace": "ws",
            "key": "session:s1",
            "text": "x".repeat(MIN_BYTES),
            "staged": null
        });
        let legacy: ClipboardTextPaste = serde_json::from_value(legacy_json).unwrap();
        assert_eq!(legacy.name, "pasted-text.txt");
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

    pub(crate) fn has_pending_clipboard_text(&self) -> bool {
        self.draft_target.as_ref().is_some_and(|key| {
            self.attachments.clipboard_text.contains_key(key)
                || self.attachments.loading.contains(key)
        })
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
        let existing = self
            .attachments
            .drafts
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let name = next_pasted_text_name(existing);
        let paste = ClipboardTextPaste {
            workspace: workspace.to_owned(),
            key: key.to_owned(),
            text,
            name,
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
        let expected_name = paste.name.clone();
        let params = json!({"workspaceId": workspace, "text": paste.text, "name": paste.name});
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
                            && item.attachment.name == expected_name
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
        let size = format!("{:.1} KB", paste.text.len() as f64 / 1024.);
        Some(
            h_flex()
                .h(px(30.))
                .max_w_full()
                .min_w_0()
                .items_center()
                .gap_1()
                .pl_2()
                .pr_1()
                .rounded_lg()
                .bg(Color::Inset.get(cx))
                .border_1()
                .border_color(Color::Error.get(cx).opacity(0.35))
                .child(
                    Icon::new(Glyph::FileText)
                        .size(px(13.))
                        .text_color(Color::Error.get(cx))
                        .flex_shrink_0(),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Color::Secondary.get(cx))
                        .child(paste.name.clone()),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(11.))
                        .text_color(Color::Muted.get(cx))
                        .child(size),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(11.))
                        .text_color(Color::Error.get(cx))
                        .child(tr("待保存为附件")),
                )
                .child(
                    Button::new("retry-clipboard-text")
                        .custom(crate::palette::quiet_icon_variant(cx))
                        .compact()
                        .xsmall()
                        .h(px(22.))
                        .px_1p5()
                        .rounded_md()
                        .icon(Glyph::RefreshCw)
                        .label(tr("重试"))
                        .tooltip(tr("重试保存粘贴文本"))
                        .accessibility_label(tr("重试保存粘贴文本"))
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
                        .custom(crate::palette::subtle_icon_variant(cx))
                        .compact()
                        .xsmall()
                        .h(px(22.))
                        .w(px(22.))
                        .rounded_md()
                        .icon(Glyph::Copy)
                        .tooltip(tr("复制粘贴文本"))
                        .accessibility_label(tr("复制粘贴文本"))
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
                        .custom(crate::palette::subtle_icon_variant(cx))
                        .compact()
                        .xsmall()
                        .h(px(22.))
                        .w(px(22.))
                        .rounded_md()
                        .icon(Glyph::X)
                        .tooltip(tr("移除待保存的粘贴文本"))
                        .accessibility_label(tr("移除待保存的粘贴文本"))
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
