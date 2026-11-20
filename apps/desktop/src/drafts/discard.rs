use super::{Draft, DraftStore};
use crate::{
    app::{ErrorNotice, HiveApp},
    attachments::AttachmentRef,
    language::tr,
    protocol::Session,
};
use gpui_kit::{
    App, Context, EntityInputHandler, Window,
    component::{
        Sizable as _, WindowExt as _,
        button::Button,
        notification::{Notification, NotificationDelivery},
    },
};
use std::{
    io,
    time::{Duration, Instant},
};
use uuid::Uuid;

const UNDO_TIME: Duration = Duration::from_secs(5);
struct DraftDiscardToast;

#[derive(Clone, Default, PartialEq, Eq)]
struct ComposerDraft {
    text: String,
    attachments: Vec<AttachmentRef>,
}

impl ComposerDraft {
    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.attachments.is_empty()
    }
}

pub(super) struct PendingDiscard {
    id: Uuid,
    deadline: Instant,
    draft: ComposerDraft,
}

impl DraftStore {
    fn replace_composer_draft(
        &mut self,
        key: &str,
        draft: &ComposerDraft,
        failed: Vec<String>,
    ) -> io::Result<()> {
        let mut record = self
            .saved
            .get(key)
            .cloned()
            .unwrap_or_else(|| Draft::empty(key.to_owned()));
        record.input.clone_from(&draft.text);
        record.attachments.clone_from(&draft.attachments);
        record.failed = failed;
        self.publish_draft(record)
    }

    fn discard_composer_draft(
        &mut self,
        key: &str,
        draft: ComposerDraft,
        failed: Vec<String>,
    ) -> io::Result<Uuid> {
        let id = Uuid::new_v4();
        self.replace_composer_draft(key, &ComposerDraft::default(), failed)?;
        self.discarded.insert(
            key.to_owned(),
            PendingDiscard {
                id,
                deadline: Instant::now() + UNDO_TIME,
                draft,
            },
        );
        Ok(id)
    }

    fn undo_composer_discard(
        &mut self,
        key: &str,
        id: Uuid,
        failed: Vec<String>,
    ) -> io::Result<Option<ComposerDraft>> {
        let Some(discard) = self
            .discarded
            .get(key)
            .filter(|discard| discard.id == id && Instant::now() < discard.deadline)
        else {
            return Ok(None);
        };
        if self
            .saved
            .get(key)
            .is_some_and(|record| !record.input.is_empty() || !record.attachments.is_empty())
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Draft changed after discard",
            ));
        }
        let draft = discard.draft.clone();
        self.replace_composer_draft(key, &draft, failed)?;
        self.discarded.remove(key);
        Ok(Some(draft))
    }

    fn expire_composer_discard(&mut self, key: &str, id: Uuid) {
        if self
            .discarded
            .get(key)
            .is_some_and(|discard| discard.id == id)
        {
            self.discarded.remove(key);
        }
    }
}

impl HiveApp {
    fn writable_session_draft(&self, session: &Session, key: &str) -> bool {
        self.connected
            && !self.closing
            && !session.worktree_released
            && !self.worktree_release_loading.contains(&session.id)
            && self.draft_store.is_some()
            && !self.removed_sessions.contains(&session.id)
            && !self.inflight_inputs.contains_key(&session.id)
            && !self.attachments.loading.contains(key)
            && self.workspace_removing.as_ref() != Some(&session.workspace_id)
            && self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == session.workspace_id)
            && self
                .session
                .iter()
                .chain(&self.sessions)
                .chain(
                    self.project_sessions
                        .get(&session.workspace_id)
                        .into_iter()
                        .flatten(),
                )
                .chain(
                    self.pinned_sessions
                        .records
                        .iter()
                        .map(|entry| &entry.session),
                )
                .any(|record| {
                    record.id == session.id && record.workspace_id == session.workspace_id
                })
    }

    fn editor_owns_session_draft(&self, key: &str) -> bool {
        self.draft_owner.as_deref() == Some(key) && self.draft_target == self.draft_owner
    }

    fn composer_draft(&self, key: &str, cx: &App) -> ComposerDraft {
        let text = if self.editor_owns_session_draft(key) {
            self.input.read(cx).value().to_string()
        } else {
            self.drafts.get(key).cloned().unwrap_or_default()
        };
        ComposerDraft {
            text,
            attachments: self
                .attachments
                .drafts
                .get(key)
                .cloned()
                .unwrap_or_default(),
        }
    }

    pub(crate) fn can_discard_session_draft(&self, session: &Session, cx: &App) -> bool {
        let key = format!("session:{}", session.id);
        self.writable_session_draft(session, &key) && !self.composer_draft(&key, cx).is_empty()
    }

    fn apply_composer_draft(
        &mut self,
        key: &str,
        draft: ComposerDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if draft.text.is_empty() {
            self.drafts.remove(key);
        } else {
            self.drafts.insert(key.to_owned(), draft.text.clone());
        }
        if draft.attachments.is_empty() {
            self.attachments.drafts.remove(key);
        } else {
            self.attachments
                .drafts
                .insert(key.to_owned(), draft.attachments);
        }
        if self.editor_owns_session_draft(key) {
            self.input.update(cx, |input, cx| {
                let end = draft.text.len();
                input.set_value(draft.text, window, cx);
                input.set_selected_range(end..end, cx);
            });
        }
        self.prune_draft_attachments(cx);
        cx.notify();
    }

    fn session_draft_composing(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.editor_owns_session_draft(key)
            && self.input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            })
    }

    pub(crate) fn discard_session_draft(
        &mut self,
        session: Session,
        connection: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = format!("session:{}", session.id);
        if connection != self.connection_generation
            || !self.writable_session_draft(&session, &key)
            || self.session_draft_composing(&key, window, cx)
        {
            return;
        }
        let draft = self.composer_draft(&key, cx);
        if draft.is_empty()
            || draft
                .attachments
                .iter()
                .any(|file| file.workspace_id != session.workspace_id)
        {
            return;
        }
        let failed = self
            .failed_drafts
            .get(&session.id)
            .cloned()
            .unwrap_or_default();
        let store = self.draft_store.as_mut().unwrap();
        let previous = store.discarded.get(&key).map(|discard| discard.id);
        let id = match store.discard_composer_draft(&key, draft, failed) {
            Ok(id) => id,
            Err(error) => {
                self.error = Some(ErrorNotice::Detail("无法丢弃草稿", error.to_string()));
                cx.notify();
                return;
            }
        };
        if let Some(previous) = previous {
            window
                .remove_notification1::<DraftDiscardToast>(format!("{connection}:{previous}"), cx);
        }
        self.apply_composer_draft(&key, ComposerDraft::default(), window, cx);
        self.offer_draft_discard_undo(session, key, id, window, cx);
    }

    fn undo_draft_discard(
        &mut self,
        session: &Session,
        key: &str,
        id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.writable_session_draft(session, key) {
            return;
        }
        if !self
            .draft_store
            .as_ref()
            .unwrap()
            .discarded
            .get(key)
            .is_some_and(|discard| discard.id == id && Instant::now() < discard.deadline)
        {
            return;
        }
        if self.session_draft_composing(key, window, cx) || !self.composer_draft(key, cx).is_empty()
        {
            self.error = Some("这个会话已有新草稿，无法撤销丢弃。".into());
            cx.notify();
            return;
        }
        let failed = self
            .failed_drafts
            .get(&session.id)
            .cloned()
            .unwrap_or_default();
        match self
            .draft_store
            .as_mut()
            .unwrap()
            .undo_composer_discard(key, id, failed)
        {
            Ok(Some(draft)) => self.apply_composer_draft(key, draft, window, cx),
            Ok(None) => (),
            Err(error) => {
                self.error = Some(ErrorNotice::Detail("无法撤销丢弃草稿", error.to_string()));
                cx.notify();
            }
        }
    }

    fn offer_draft_discard_undo(
        &self,
        session: Session,
        key: String,
        id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let connection = self.connection_generation;
        let window_handle = window.window_handle();
        let app = cx.weak_entity();
        let action_key = key.clone();
        let toast_key = format!("{connection}:{id}");
        let action_toast = toast_key.clone();
        window.push_notification(
            Notification::success(tr("草稿已丢弃"))
                .id1::<DraftDiscardToast>(toast_key.clone())
                .title(session.title.clone())
                .delivery(NotificationDelivery::InApp)
                .action(move |_, _, _| {
                    let app = app.clone();
                    let key = action_key.clone();
                    let session = session.clone();
                    let toast = action_toast.clone();
                    Button::new("undo-draft-discard")
                        .compact()
                        .small()
                        .label(tr("撤销丢弃草稿"))
                        .on_click(move |_, window, cx| {
                            window.remove_notification1::<DraftDiscardToast>(toast.clone(), cx);
                            let _ = app.update(cx, |this, cx| {
                                if this.connection_generation == connection {
                                    this.undo_draft_discard(&session, &key, id, window, cx);
                                }
                            });
                        })
                }),
            cx,
        );
        let timer = cx.background_executor().timer(UNDO_TIME);
        cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, _| {
                if this.connection_generation == connection
                    && let Some(store) = this.draft_store.as_mut()
                {
                    store.expire_composer_discard(&key, id);
                }
            });
            let _ = window_handle.update(cx, |_, window, cx| {
                window.remove_notification1::<DraftDiscardToast>(toast_key, cx)
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft_stash::StashAction;

    #[test]
    fn discarded_draft_restores_only_its_own_snapshot_and_refuses_newer_content() {
        let root = tempfile::tempdir().unwrap();
        let mut store = DraftStore::open(root.path()).unwrap();
        let abandoned = ComposerDraft {
            text: "Original text".into(),
            attachments: vec![AttachmentRef {
                id: "saved-upload".into(),
                workspace_id: "beacon".into(),
                name: "fixture.txt".into(),
                mime_type: "text/plain".into(),
                size: 16,
                project_file: None,
            }],
        };
        let other = ComposerDraft {
            text: "Independent draft".into(),
            attachments: vec![],
        };
        store
            .change_stash(
                "session:beacon",
                StashAction::Save,
                "Stashed".into(),
                vec![],
                vec![],
            )
            .unwrap();
        store
            .replace_composer_draft("session:atlas", &other, vec![])
            .unwrap();
        let id = store
            .discard_composer_draft(
                "session:beacon",
                abandoned.clone(),
                vec!["Failed to send".into()],
            )
            .unwrap();
        let reopened = DraftStore::open(root.path()).unwrap();
        assert_eq!(reopened.saved["session:beacon"].input, "");
        assert!(reopened.saved["session:beacon"].attachments.is_empty());
        assert_eq!(reopened.saved["session:beacon"].failed, ["Failed to send"]);
        assert_eq!(reopened.saved["session:beacon"].stash[0].text, "Stashed");
        assert_eq!(reopened.saved["session:atlas"].input, other.text);
        assert!(
            store
                .undo_composer_discard("session:atlas", id, vec![])
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .undo_composer_discard("session:beacon", id, vec!["Failed to send".into()])
                .unwrap()
                == Some(abandoned.clone())
        );
        let id = store
            .discard_composer_draft("session:beacon", abandoned, vec![])
            .unwrap();
        store
            .replace_composer_draft("session:beacon", &other, vec![])
            .unwrap();
        assert_eq!(
            store
                .undo_composer_discard("session:beacon", id, vec![])
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(store.saved["session:beacon"].input, other.text);
        store.expire_composer_discard("session:beacon", id);
        assert!(
            store
                .undo_composer_discard("session:beacon", id, vec![])
                .unwrap()
                .is_none()
        );
    }
}
