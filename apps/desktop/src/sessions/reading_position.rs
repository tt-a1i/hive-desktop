use crate::{
    app::HiveApp,
    conversation_list::{ReadingAnchor, RowKey},
};
use gpui_kit::{Context, Task, px};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path, time::Duration};

const FILE: &str = "desktop-reading-positions.json";
const SAVED_POSITIONS: usize = 64;
const MAX_FILE_BYTES: u64 = 512 * 1024;

#[derive(Deserialize, PartialEq, Serialize)]
struct StoredPosition {
    workspace: String,
    session: String,
    row: RowKey,
    offset: f32,
}

impl StoredPosition {
    fn valid(&self) -> bool {
        let id = match &self.row {
            RowKey::Message(id) | RowKey::Group(id) => id,
            RowKey::Tail => return false,
        };
        self.offset.is_finite()
            && self.offset >= 0.
            && [&self.workspace, &self.session, id]
                .into_iter()
                .all(|id| !id.is_empty() && id.len() <= 1024 && !id.chars().any(char::is_control))
    }
}

#[derive(Default)]
pub(crate) struct ReadingPositions {
    entries: Vec<StoredPosition>,
    dirty: bool,
    save_task: Option<Task<()>>,
}

impl ReadingPositions {
    pub(crate) fn restore(&mut self, root: &Path) {
        let load = || -> io::Result<Vec<StoredPosition>> {
            let file = fs::File::open(root.join(FILE))?;
            if file.metadata()?.len() > MAX_FILE_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Reading positions exceed the file limit",
                ));
            }
            Ok(serde_json::from_reader(file)?)
        };
        match load() {
            Ok(mut entries) => {
                entries.retain(StoredPosition::valid);
                entries.drain(..entries.len().saturating_sub(SAVED_POSITIONS));
                self.entries = entries;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => eprintln!("Could not restore conversation reading positions: {error}"),
        }
    }

    pub(crate) fn anchor(&self, workspace: &str, session: &str) -> Option<ReadingAnchor> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.workspace == workspace && entry.session == session)
            .map(|entry| ReadingAnchor {
                key: entry.row.clone(),
                offset: px(entry.offset),
            })
    }

    fn remember(&mut self, workspace: &str, session: &str, anchor: Option<ReadingAnchor>) {
        let entry = anchor
            .map(|anchor| StoredPosition {
                workspace: workspace.to_owned(),
                session: session.to_owned(),
                row: anchor.key,
                offset: anchor.offset.into(),
            })
            .filter(StoredPosition::valid);
        if entry.is_some() && self.entries.last() == entry.as_ref() {
            return;
        }
        let before = self.entries.len();
        self.entries
            .retain(|entry| entry.workspace != workspace || entry.session != session);
        self.dirty |= before != self.entries.len();
        if let Some(entry) = entry {
            self.entries.push(entry);
            self.entries
                .drain(..self.entries.len().saturating_sub(SAVED_POSITIONS));
            self.dirty = true;
        }
    }

    pub(crate) fn remove_workspace(&mut self, workspace: &str) {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.workspace != workspace);
        self.dirty |= self.entries.len() != before;
    }

    fn save(&mut self, root: &Path) -> Result<(), String> {
        if self.dirty {
            let data = serde_json::to_vec(&self.entries).map_err(|error| error.to_string())?;
            crate::file_save::save_file(&root.join(FILE), &data)?;
            self.dirty = false;
        }
        Ok(())
    }
}

impl HiveApp {
    pub(crate) fn schedule_reading_position_save(&mut self, cx: &mut Context<Self>) {
        if !self.conversation_ready || self.loading || self.closing || self.data_dir.is_none() {
            return;
        }
        // GPUI holds the list's state during user scroll callbacks. Read the
        // final offset after scrolling settles, outside that borrow.
        self.reading_positions.save_task = Some(cx.spawn(async move |this, cx| {
            smol::Timer::after(Duration::from_millis(450)).await;
            let _ = this.update(cx, |this, _| this.flush_reading_positions());
        }));
    }

    pub(crate) fn flush_reading_positions(&mut self) {
        self.reading_positions.save_task.take();
        if self.conversation_ready
            && !self.loading
            && !self.messages.is_empty()
            && let Some(session) = self.session.as_ref()
            && self.workspace.as_ref() == Some(&session.workspace_id)
        {
            let list = &self.conversation_list;
            let anchor = list.pending_anchor.clone().or_else(|| {
                (!list.state.is_following_tail())
                    .then(|| list.anchor())
                    .flatten()
            });
            self.reading_positions
                .remember(&session.workspace_id, &session.id, anchor);
        }
        self.save_reading_positions();
    }

    pub(crate) fn save_reading_positions(&mut self) {
        if let Some(root) = self.data_dir.as_deref()
            && let Err(error) = self.reading_positions.save(root)
        {
            eprintln!("Could not save conversation reading positions: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_the_anchor_and_clears_only_the_conversation_that_returned_to_tail() {
        let root = tempfile::tempdir().unwrap();
        let mut store = ReadingPositions::default();
        store.remember(
            "work",
            "chat",
            Some(ReadingAnchor {
                key: RowKey::Group("previous-tool".into()),
                offset: px(37.5),
            }),
        );
        store.remember(
            "another",
            "other-chat",
            Some(ReadingAnchor {
                key: RowKey::Message("previous-message".into()),
                offset: px(15.),
            }),
        );
        store.save(root.path()).unwrap();
        store = ReadingPositions::default();
        store.restore(root.path());
        assert!(store.anchor("another", "chat").is_none());
        let location = store.anchor("work", "chat").unwrap();
        assert_eq!(location.key, RowKey::Group("previous-tool".into()));
        assert_eq!(location.offset, px(37.5));
        store.remember("work", "chat", None);
        store.save(root.path()).unwrap();
        store = ReadingPositions::default();
        store.restore(root.path());
        assert!(store.anchor("work", "chat").is_none());
        assert!(store.anchor("another", "other-chat").is_some());
        store.remove_workspace("another");
        assert!(store.anchor("another", "other-chat").is_none());
    }

    #[test]
    fn retains_recent_anchors_and_ignores_non_message_positions() {
        let root = tempfile::tempdir().unwrap();
        let mut store = ReadingPositions::default();
        store.remember(
            "work",
            "tail",
            Some(ReadingAnchor {
                key: RowKey::Tail,
                offset: px(0.),
            }),
        );
        assert!(store.anchor("work", "tail").is_none());
        for index in 0..=SAVED_POSITIONS {
            store.remember(
                "work",
                &format!("chat-{index}"),
                Some(ReadingAnchor {
                    key: RowKey::Message("previous-message".into()),
                    offset: px(2.),
                }),
            );
        }
        store.save(root.path()).unwrap();
        store = ReadingPositions::default();
        store.restore(root.path());
        assert_eq!(store.entries.len(), SAVED_POSITIONS);
        assert!(store.anchor("work", "chat-0").is_none());
        assert!(
            store
                .anchor("work", &format!("chat-{SAVED_POSITIONS}"))
                .is_some()
        );
    }
}
