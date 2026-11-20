use crate::{
    app::{ErrorNotice, HiveApp},
    protocol::{AttachmentRef, ProfileRef, Session},
};
use gpui_kit::{Context, Window};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::NamedTempFile;
use uuid::{Uuid, Version};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DraftCopy {
    request_id: String,
    workspace_id: String,
    source_session_id: Option<String>,
    target_workspace_id: String,
    backend: String,
    profile: Option<ProfileRef>,
    text: String,
    attachments: Vec<AttachmentRef>,
}

#[derive(Deserialize)]
struct CopyResult {
    session: Session,
    attachments: Vec<AttachmentRef>,
}

#[derive(Default)]
pub(crate) struct DraftCopies {
    pending: BTreeMap<String, DraftCopy>,
    running: HashSet<String>,
    recovery_connection: Option<u64>,
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

impl DraftCopy {
    fn file(&self, root: &Path) -> io::Result<PathBuf> {
        let valid = Uuid::parse_str(&self.request_id).is_ok_and(|id| {
            id.get_version() == Some(Version::Random) && id.to_string() == self.request_id
        });
        if !valid || self.workspace_id == self.target_workspace_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid draft copy identity",
            ));
        }
        Ok(root
            .join("desktop-draft-copies")
            .join(format!("{}.json", self.request_id)))
    }

    fn save(&self, root: &Path) -> io::Result<()> {
        let path = self.file(root)?;
        let directory = path.parent().expect("draft copy journal directory");
        fs::create_dir_all(directory)?;
        let mut file = NamedTempFile::new_in(directory)?;
        serde_json::to_writer(file.as_file_mut(), self)?;
        file.as_file().sync_all()?;
        file.persist_noclobber(&path)?;
        sync_directory(directory)?;
        sync_directory(root)
    }

    fn remove(&self, root: &Path) -> io::Result<()> {
        match fs::remove_file(self.file(root)?) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }

    fn load(root: &Path) -> io::Result<Vec<Self>> {
        let files = match fs::read_dir(root.join("desktop-draft-copies")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            result => result?,
        };
        let mut copies = Vec::new();
        for file in files {
            let file = file?;
            let path = file.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            if !file.file_type()?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid draft copy journal",
                ));
            }
            let pending: Self = serde_json::from_reader(fs::File::open(&path)?)?;
            if pending.file(root)? != path {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Draft copy journal identity mismatch",
                ));
            }
            copies.push(pending);
        }
        Ok(copies)
    }

    fn params(&self) -> serde_json::Value {
        let mut params = json!({
            "requestId": self.request_id, "workspaceId": self.workspace_id,
            "targetWorkspaceId": self.target_workspace_id, "backend": self.backend,
            "attachmentIds": self.attachments.iter().map(|item| &item.id).collect::<Vec<_>>(),
        });
        if let Some(session) = &self.source_session_id {
            params["sourceSessionId"] = json!(session);
        }
        if let Some(profile) = &self.profile {
            params["profile"] = json!(profile);
        }
        params
    }

    fn accepts(&self, result: &CopyResult) -> bool {
        let session = &result.session;
        let mut ids: HashSet<&str> = self
            .attachments
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        session.id == self.request_id
            && session.workspace_id == self.target_workspace_id
            && session.backend.wire() == self.backend
            && session.agent_id.is_none()
            && session.origin_agent_id.is_none()
            && !session.archived
            && self
                .profile
                .as_ref()
                .is_none_or(|profile| session.profile.as_ref() == Some(profile))
            && result.attachments.len() == self.attachments.len()
            && result
                .attachments
                .iter()
                .zip(&self.attachments)
                .all(|(copy, source)| {
                    copy.workspace_id == self.target_workspace_id
                        && copy.project_file.is_none()
                        && copy.name == source.name
                        && copy.mime_type == source.mime_type
                        && copy.size == source.size
                        && !copy.id.is_empty()
                        && ids.insert(&copy.id)
                })
    }

    fn draft_key(&self) -> String {
        format!("session:{}", self.request_id)
    }
    fn import_marker(&self) -> String {
        format!("project-copy:{}", self.request_id)
    }
}

impl HiveApp {
    fn copyable_draft_key(&self, cx: &Context<Self>) -> Option<&String> {
        let workspace = self.workspace.as_ref()?;
        let key = self.draft_target.as_ref()?;
        if !self.connected
            || self.closing
            || self.loading
            || !self.workspace_writes_allowed()
            || !self.viewing_conversation()
            || self.draft_store.is_none()
            || self.draft_owner.as_ref() != Some(key)
            || self.attachments.loading.contains(key)
            || !self.draft_copies.running.is_empty()
        {
            return None;
        }
        if let Some(session) = &self.session {
            if key != &format!("session:{}", session.id)
                || !self.conversation_ready
                || !self.messages.is_empty()
                || session.archived
                || session.agent_id.is_some()
                || session.origin_agent_id.is_some()
                || session.identity_state.as_deref() != Some("new")
                || self.inflight_inputs.contains_key(&session.id)
                || self.failed_drafts.contains_key(&session.id)
                || self.session_archive_blocked(&session.id)
            {
                return None;
            }
        } else if key != &format!("workspace:{workspace}") {
            return None;
        }
        (!self.input.read(cx).value().trim().is_empty()
            || !self.attachments.refs(Some(key)).is_empty())
        .then_some(key)
    }

    pub(crate) fn can_copy_draft(&self, cx: &Context<Self>) -> bool {
        self.copyable_draft_key(cx).is_some()
            && self.workspaces.iter().any(|workspace| {
                self.workspace.as_ref() != Some(&workspace.id)
                    && self.workspace_removing.as_ref() != Some(&workspace.id)
            })
    }

    pub(crate) fn pending_draft_copy_targets(&self) -> Vec<(String, String)> {
        self.draft_copies
            .pending
            .values()
            .filter_map(|pending| {
                if self.draft_copies.running.contains(&pending.request_id) {
                    return None;
                }
                let name = self
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.id == pending.target_workspace_id)?
                    .name
                    .clone();
                Some((pending.request_id.clone(), name))
            })
            .collect()
    }

    pub(crate) fn recover_draft_copies(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.data_dir.clone() else {
            return;
        };
        let connection = self.connection_generation;
        if self.draft_store.is_none() || self.draft_copies.recovery_connection == Some(connection) {
            return;
        }
        self.draft_copies.recovery_connection = Some(connection);
        self.draft_copies.running.clear();
        match DraftCopy::load(&root) {
            Ok(records) => {
                self.draft_copies.pending = records
                    .into_iter()
                    .map(|record| (record.request_id.clone(), record))
                    .collect();
                let ids: Vec<_> = self.draft_copies.pending.keys().cloned().collect();
                for id in ids {
                    self.run_draft_copy(id, None, cx);
                }
            }
            Err(error) => {
                self.error = Some(ErrorNotice::Detail(
                    "无法读取待完成的草稿复制",
                    error.to_string(),
                ))
            }
        }
    }

    pub(crate) fn begin_draft_copy(
        &mut self,
        project: String,
        origin: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self
            .copyable_draft_key(cx)
            .filter(|_| self.open_request == origin)
            .cloned()
        else {
            return;
        };
        let Some(workspace) = self
            .workspace
            .clone()
            .filter(|workspace| *workspace != project)
        else {
            return;
        };
        let Some(root) = self.data_dir.clone() else {
            return;
        };
        if !self.workspaces.iter().any(|item| item.id == project)
            || self.workspace_removing.as_ref() == Some(&project)
        {
            return;
        }
        let text = self.input.read(cx).value().to_string();
        let attachments = self.attachments.refs(Some(&key));
        let source_session = self.session.as_ref();
        let profile = source_session.and_then(|session| session.profile.clone());
        let backend = source_session
            .map_or("pi", |session| session.backend.wire())
            .to_owned();
        // Selecting the same destination again can retry an unconfirmed snapshot.
        let existing = self
            .draft_copies
            .pending
            .values()
            .find(|pending| {
                pending.workspace_id == workspace
                    && pending.target_workspace_id == project
                    && pending.source_session_id.as_ref() == source_session.map(|s| &s.id)
                    && pending.text == text
                    && pending.attachments == attachments
                    && pending.backend == backend
                    && pending.profile == profile
            })
            .map(|pending| pending.request_id.clone());
        let id = if let Some(id) = existing {
            id
        } else {
            let copy = DraftCopy {
                request_id: Uuid::new_v4().to_string(),
                workspace_id: workspace,
                source_session_id: source_session.map(|session| session.id.clone()),
                target_workspace_id: project,
                backend,
                profile,
                text,
                attachments,
            };
            if let Err(error) = self.flush_drafts(cx).and_then(|()| copy.save(&root)) {
                self.error = Some(ErrorNotice::Detail(
                    "无法记录待复制的草稿",
                    error.to_string(),
                ));
                cx.notify();
                return;
            }
            let id = copy.request_id.clone();
            self.draft_copies.pending.insert(id.clone(), copy);
            id
        };
        self.run_draft_copy(id, Some(window), cx);
    }

    pub(crate) fn run_draft_copy(
        &mut self,
        id: String,
        window: Option<&Window>,
        cx: &mut Context<Self>,
    ) {
        let (Some(runtime), Some(root), Some(copy)) = (
            self.runtime.clone(),
            self.data_dir.clone(),
            self.draft_copies.pending.get(&id).cloned(),
        ) else {
            return;
        };
        if !self.connected
            || self.closing
            || self.draft_store.is_none()
            || self.workspace_removing.as_ref() == Some(&copy.target_workspace_id)
            || !self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == copy.target_workspace_id)
        {
            return;
        }
        if self
            .draft_store
            .as_ref()
            .is_some_and(|store| store.already_imported(&copy.draft_key(), &copy.import_marker()))
        {
            self.clear_draft_copy(&copy, &root);
            cx.notify();
            return;
        }
        if !self.draft_copies.running.insert(id.clone()) {
            return;
        }
        let connection = self.connection_generation;
        let origin = self.open_request;
        let window = window.map(Window::window_handle);
        let result = runtime.begin_request("chat.copyDraftRecord", copy.params());
        self.status = "正在复制草稿…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = smol::future::race(result, async {
                smol::Timer::after(Duration::from_secs(25)).await;
                Err(crate::language::tr("尚未确认复制结果，可从项目菜单重试。").to_owned())
            })
            .await
            .and_then(|body| {
                serde_json::from_value::<CopyResult>(body).map_err(|error| error.to_string())
            });
            let opened = this
                .update(cx, |this, cx| {
                    if connection != this.connection_generation {
                        return None;
                    }
                    this.draft_copies.running.remove(&id);
                    let result = result.and_then(|result| {
                        if !copy.accepts(&result) {
                            return Err(crate::language::tr("复制结果与原草稿不匹配").to_owned());
                        }
                        this.publish_copied_draft(&copy, &result, cx)
                            .map_err(|error| error.to_string())?;
                        Ok(result.session)
                    });
                    match result {
                        Ok(session) => {
                            this.status = "草稿已复制".into();
                            this.clear_draft_copy(&copy, &root);
                            this.refresh_project_sessions(session.workspace_id.clone(), cx);
                            cx.notify();
                            (this.open_request == origin && this.viewing_conversation())
                                .then_some(session)
                        }
                        Err(error) => {
                            this.status = "草稿复制待重试".into();
                            this.error = Some(ErrorNotice::Detail("无法完成草稿复制", error));
                            cx.notify();
                            None
                        }
                    }
                })
                .ok()
                .flatten();
            if let (Some(window), Some(session)) = (window, opened) {
                let _ = window.update(cx, |_, window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if this.open_request == origin
                            && this.connection_generation == connection
                            && this.viewing_conversation()
                            && this.connected
                            && !this.closing
                            && this.workspace_removing.as_ref() != Some(&session.workspace_id)
                            && !this.removed_sessions.contains(&session.id)
                            && this
                                .workspaces
                                .iter()
                                .any(|item| item.id == session.workspace_id)
                        {
                            this.navigation
                                .sessions
                                .insert(session.workspace_id.clone(), session.id.clone());
                            if this.navigate_to_session(session, window, cx) {
                                this.focus_project_input = true;
                            }
                        }
                    });
                });
            }
        })
        .detach();
    }

    fn publish_copied_draft(
        &mut self,
        copy: &DraftCopy,
        result: &CopyResult,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        if self.workspace_removing.as_ref() == Some(&copy.target_workspace_id)
            || !self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == copy.target_workspace_id)
        {
            return Err(io::Error::other(crate::language::tr("目标项目已不可用")));
        }
        self.capture_editor(cx);
        let key = copy.draft_key();
        let marker = copy.import_marker();
        let store = self
            .draft_store
            .as_mut()
            .ok_or_else(|| io::Error::other("Draft store unavailable"))?;
        if store.already_imported(&key, &marker) {
            return Ok(());
        }
        if self.drafts.contains_key(&key) || self.attachments.drafts.contains_key(&key) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                crate::language::tr("目标会话已有草稿"),
            ));
        }
        store.import_once(&key, &marker, copy.text.clone(), result.attachments.clone())?;
        if !copy.text.is_empty() {
            self.drafts.insert(key.clone(), copy.text.clone());
        }
        if !result.attachments.is_empty() {
            self.attachments
                .drafts
                .insert(key.clone(), result.attachments.clone());
        }
        if self.draft_owner.as_ref() == Some(&key) {
            self.draft_owner = None;
        }
        Ok(())
    }

    fn clear_draft_copy(&mut self, copy: &DraftCopy, root: &Path) {
        match copy.remove(root) {
            Ok(()) => {
                self.draft_copies.pending.remove(&copy.request_id);
            }
            Err(error) => {
                self.error = Some(ErrorNotice::Detail(
                    "草稿已复制，无法清理复制记录",
                    error.to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> DraftCopy {
        DraftCopy {
            request_id: Uuid::new_v4().to_string(),
            workspace_id: "source".into(),
            source_session_id: Some("original".into()),
            target_workspace_id: "target".into(),
            backend: "pi".into(),
            profile: None,
            text: "The next message".into(),
            attachments: vec![AttachmentRef {
                id: "from-source".into(),
                workspace_id: "source".into(),
                name: "notes.txt".into(),
                mime_type: "text/plain".into(),
                size: 24,
                project_file: None,
            }],
        }
    }

    #[test]
    fn journal_reopens_exact_snapshot_without_sending_draft_text_to_runtime() {
        let root = tempfile::tempdir().unwrap();
        let pending = snapshot();
        pending.save(root.path()).unwrap();
        assert_eq!(
            pending.save(root.path()).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        let recovered = DraftCopy::load(root.path()).unwrap().pop().unwrap();
        assert_eq!(recovered.text, pending.text);
        assert_eq!(recovered.attachments, pending.attachments);
        assert_eq!(recovered.params(), pending.params());
        assert!(recovered.params().get("text").is_none());
        assert_eq!(recovered.params()["attachmentIds"], json!(["from-source"]));
        recovered.remove(root.path()).unwrap();
        assert!(DraftCopy::load(root.path()).unwrap().is_empty());
    }

    #[test]
    fn validates_returned_destination_session_and_fresh_attachment_identity() {
        let pending = snapshot();
        let session: Session = serde_json::from_value(json!({
            "id": pending.request_id, "title": "Draft", "workspaceId": "target", "identityState": "new",
        })).unwrap();
        let mut attachment = pending.attachments[0].clone();
        attachment.id = "target-attachment".into();
        attachment.workspace_id = "target".into();
        let mut result = CopyResult {
            session,
            attachments: vec![attachment],
        };
        assert!(pending.accepts(&result));
        result.attachments[0].id = "from-source".into();
        assert!(!pending.accepts(&result));
        result.attachments[0].id = "target-attachment".into();
        result.session.workspace_id = "source".into();
        assert!(!pending.accepts(&result));
    }
}
