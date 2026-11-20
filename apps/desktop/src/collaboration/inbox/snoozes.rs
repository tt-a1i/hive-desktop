use super::ActionCenter;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy)]
pub(super) enum Delay {
    Minutes(u64),
    Indefinite,
}

impl Delay {
    pub(super) const OPTIONS: [(&str, Self); 4] = [
        ("15 分钟", Self::Minutes(15)),
        ("1 小时", Self::Minutes(60)),
        ("明天此时", Self::Minutes(24 * 60)),
        ("无限期", Self::Indefinite),
    ];

    pub(super) fn expires_at(self, now: u64) -> Option<u64> {
        match self {
            Self::Minutes(minutes) => Some(now.saturating_add(minutes * 60_000)),
            Self::Indefinite => None,
        }
    }
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reminder {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) expires_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) wake: Option<Wake>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Wake {
    Dispatch {
        id: String,
        sequence: Option<u64>,
        observed_at: u64,
    },
    Session {
        interaction_ids: BTreeSet<String>,
    },
}

impl Reminder {
    fn expired(&self, now: u64) -> bool {
        self.expires_at.is_some_and(|until| until <= now)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snoozes {
    workspace_id: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    keys: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    reminders: BTreeMap<String, Reminder>,
}

impl Snoozes {
    fn path(root: &Path, workspace: &str) -> io::Result<PathBuf> {
        let id = uuid::Uuid::try_parse(workspace)
            .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
        Ok(root
            .join("desktop-inbox-snoozes")
            .join(format!("{id}.json")))
    }

    fn read(path: &Path, workspace: &str) -> io::Result<Option<Self>> {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let saved: Self = serde_json::from_reader(file)?;
        if saved.workspace_id != workspace {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "收件箱返回了其他工作区的数据。",
            ));
        }
        Ok(Some(saved))
    }

    fn empty(workspace: &str) -> Self {
        Self {
            workspace_id: workspace.to_owned(),
            keys: BTreeSet::new(),
            reminders: BTreeMap::new(),
        }
    }

    pub(super) fn load(root: &Path, workspace: &str) -> io::Result<Self> {
        Ok(Self::read(&Self::path(root, workspace)?, workspace)?
            .unwrap_or_else(|| Self::empty(workspace)))
    }

    pub(super) fn active(&self, key: &str, now: u64) -> bool {
        self.keys.contains(key)
            || self
                .reminders
                .get(key)
                .is_some_and(|item| !item.expired(now))
    }

    pub(super) fn set(
        root: &Path,
        workspace: &str,
        key: &str,
        reminder: Option<Reminder>,
    ) -> io::Result<Self> {
        Self::update(root, workspace, |saved| {
            saved.keys.remove(key);
            if let Some(reminder) = reminder {
                saved.reminders.insert(key.to_owned(), reminder);
            } else {
                saved.reminders.remove(key);
            }
        })
    }

    pub(super) fn reconcile(
        self,
        root: &Path,
        now: u64,
        snapshot: Option<&ActionCenter>,
        interactions: &BTreeMap<String, BTreeSet<String>>,
    ) -> Result<Self, (Self, io::Error)> {
        let snapshot = snapshot.filter(|snapshot| snapshot.workspace_id == self.workspace_id);
        let needs_update = self.reminders.iter().any(|(key, item)| {
            Self::woke(key, item, now, snapshot, interactions)
                || item.wake.is_none()
                    && snapshot.is_some_and(|snapshot| snapshot.legacy_wake(key).is_some())
        }) || snapshot.is_some_and(|snapshot| {
            self.keys
                .iter()
                .any(|key| snapshot.legacy_wake(key).is_some())
        });
        if !needs_update {
            return Ok(self);
        }
        Self::update(root, &self.workspace_id, |latest| {
            latest
                .reminders
                .retain(|key, item| !Self::woke(key, item, now, snapshot, interactions));
            if let Some(snapshot) = snapshot {
                // Old files stored no baseline. Keep their indefinite state
                // and start watching for input after the first matching snapshot.
                latest.keys.retain(|key| {
                    if let Some(wake) = snapshot.legacy_wake(key) {
                        latest.reminders.entry(key.clone()).or_insert(Reminder {
                            expires_at: None,
                            wake: Some(wake),
                        });
                        false
                    } else {
                        true
                    }
                });
                for (key, item) in &mut latest.reminders {
                    if item.wake.is_none() {
                        item.wake = snapshot.legacy_wake(key);
                    }
                }
            }
        })
        .map_err(|error| (self, error))
    }

    fn woke(
        key: &str,
        item: &Reminder,
        now: u64,
        snapshot: Option<&ActionCenter>,
        interactions: &BTreeMap<String, BTreeSet<String>>,
    ) -> bool {
        item.expired(now)
            || match &item.wake {
                Some(Wake::Dispatch {
                    id,
                    sequence,
                    observed_at,
                }) => snapshot
                    .is_some_and(|snapshot| snapshot.has_new_input(id, *sequence, *observed_at)),
                Some(Wake::Session { interaction_ids }) => key
                    .strip_prefix("session:")
                    .and_then(|id| interactions.get(id))
                    .is_some_and(|current| !current.is_subset(interaction_ids)),
                None => false,
            }
    }

    fn update(root: &Path, workspace: &str, change: impl FnOnce(&mut Self)) -> io::Result<Self> {
        let path = Self::path(root, workspace)?;
        let dir = path.parent().expect("snooze path has a directory");
        fs::create_dir_all(dir)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.try_lock().map_err(io::Error::from)?;
        let existing = Self::read(&path, workspace)?;
        let create = existing.is_none();
        let mut saved = existing.unwrap_or_else(|| Self::empty(workspace));
        let before_keys = saved.keys.clone();
        let before_reminders = saved.reminders.clone();
        change(&mut saved);
        if before_keys != saved.keys || before_reminders != saved.reminders {
            // Upgrade old indefinite reminders on the first write.
            for key in std::mem::take(&mut saved.keys) {
                saved.reminders.entry(key).or_default();
            }
            let mut temp = tempfile::NamedTempFile::new_in(dir)?;
            serde_json::to_writer(temp.as_file_mut(), &saved)?;
            temp.as_file().sync_all()?;
            if create {
                temp.persist_noclobber(path)?;
            } else {
                temp.persist(path)?;
            }
        }
        Ok(saved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const WORKSPACE: &str = "a3b2cb99-3959-4adc-a770-b70f9d2a4cb9";
    const OTHER_WORKSPACE: &str = "c7761168-b30c-4020-80bb-23c1c357e111";
    const DISPATCH: &str = "eecc5d9c-4874-4286-8f73-ea6143246780";

    fn message(
        workspace: &str,
        dispatch: &str,
        id: &str,
        sequence: u64,
        at: u64,
    ) -> serde_json::Value {
        json!({
            "id": id, "workspace_id": workspace, "dispatch_id": dispatch, "sequence": sequence,
            "created_at": at, "from_agent_id": "worker", "text": "fixture", "delivery_state": "recorded",
        })
    }

    fn snapshot(messages: &[serde_json::Value]) -> ActionCenter {
        serde_json::from_value(json!({
            "workspace_id": WORKSPACE, "generated_at": 2000, "attention": [],
            "recent_dispatch_messages": messages,
        }))
        .unwrap()
    }

    #[test]
    fn legacy_reminder_survives_migration_and_wakes_only_for_the_same_dispatch() {
        let root = tempfile::tempdir().unwrap();
        let path = Snoozes::path(root.path(), WORKSPACE).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            json!({"workspace_id":WORKSPACE,"keys":["message:old"]}).to_string(),
        )
        .unwrap();
        let old = message(WORKSPACE, DISPATCH, "old", 1, 100);
        let unrelated = message(WORKSPACE, "other-dispatch", "next", 3, 200);
        let already_visible = message(WORKSPACE, DISPATCH, "existing", 2, 250);
        let fresh = message(WORKSPACE, DISPATCH, "fresh", 3, 300);
        let key = "message:old";
        let interactions = BTreeMap::new();

        assert!(
            Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .active(key, 400)
        );
        assert!(
            !Snoozes::load(root.path(), OTHER_WORKSPACE)
                .unwrap()
                .active(key, 400)
        );
        Snoozes::set(
            root.path(),
            WORKSPACE,
            "report:unrelated",
            Some(Reminder::default()),
        )
        .unwrap();
        let state = Snoozes::load(root.path(), WORKSPACE).unwrap().reconcile(
            root.path(),
            400,
            Some(&snapshot(&[old.clone(), unrelated, already_visible])),
            &interactions,
        );
        assert!(
            state
                .unwrap_or_else(|(_, error)| panic!("{error}"))
                .active(key, 400)
        );
        let state = Snoozes::load(root.path(), WORKSPACE).unwrap().reconcile(
            root.path(),
            400,
            Some(&snapshot(&[old, fresh])),
            &interactions,
        );
        assert!(
            !state
                .unwrap_or_else(|(_, error)| panic!("{error}"))
                .active(key, 400)
        );
        assert!(
            !Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .active(key, 400)
        );
    }

    #[test]
    fn timed_reminders_reopen_until_the_exact_deadline_and_expiry_is_durable() {
        let root = tempfile::tempdir().unwrap();
        let key = "message:old";
        let until = Delay::Minutes(15).expires_at(500).unwrap();
        Snoozes::set(
            root.path(),
            WORKSPACE,
            key,
            Some(Reminder {
                expires_at: Some(until),
                wake: None,
            }),
        )
        .unwrap();
        let saved = Snoozes::load(root.path(), WORKSPACE).unwrap();
        assert!(saved.active(key, until - 1));
        assert!(!saved.active(key, until));
        saved
            .reconcile(root.path(), until, None, &BTreeMap::new())
            .unwrap_or_else(|(_, error)| panic!("{error}"));
        let reopened = Snoozes::load(root.path(), WORKSPACE).unwrap();
        assert!(!reopened.active(key, until - 1));
        Snoozes::set(root.path(), WORKSPACE, key, Some(Reminder::default())).unwrap();
        assert!(
            Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .active(key, u64::MAX)
        );
        Snoozes::set(root.path(), WORKSPACE, key, None).unwrap();
        assert!(
            !Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .active(key, until)
        );
    }

    #[test]
    fn same_message_delivery_change_and_old_recent_rows_do_not_wake_new_reminders() {
        let root = tempfile::tempdir().unwrap();
        let key = "message:old";
        for sequence in [Some(7), None] {
            let saved = Snoozes::set(
                root.path(),
                WORKSPACE,
                key,
                Some(Reminder {
                    expires_at: None,
                    wake: Some(Wake::Dispatch {
                        id: DISPATCH.to_owned(),
                        sequence,
                        observed_at: 200,
                    }),
                }),
            )
            .unwrap();
            let same = snapshot(&[message(WORKSPACE, DISPATCH, "old", 7, 100)]);
            assert!(
                saved
                    .reconcile(root.path(), 500, Some(&same), &BTreeMap::new())
                    .unwrap_or_else(|(_, error)| panic!("{error}"))
                    .active(key, 500)
            );
            let newer = snapshot(&[message(WORKSPACE, DISPATCH, "fresh", 8, 300)]);
            assert!(
                !Snoozes::load(root.path(), WORKSPACE)
                    .unwrap()
                    .reconcile(root.path(), 500, Some(&newer), &BTreeMap::new())
                    .unwrap_or_else(|(_, error)| panic!("{error}"))
                    .active(key, 500)
            );
        }
    }

    #[test]
    fn an_exact_pending_input_for_the_same_session_wakes_its_reminder() {
        let root = tempfile::tempdir().unwrap();
        let key = "session:first";
        let original = BTreeSet::from(["original".to_owned()]);
        let saved = Snoozes::set(
            root.path(),
            WORKSPACE,
            key,
            Some(Reminder {
                expires_at: None,
                wake: Some(Wake::Session {
                    interaction_ids: original.clone(),
                }),
            }),
        )
        .unwrap();
        let mut interactions = BTreeMap::from([
            ("first".to_owned(), original),
            ("second".to_owned(), BTreeSet::from(["new".to_owned()])),
        ]);
        assert!(
            saved
                .reconcile(root.path(), 50, None, &interactions)
                .unwrap_or_else(|(_, error)| panic!("{error}"))
                .active(key, 50)
        );
        interactions
            .get_mut("first")
            .unwrap()
            .insert("fresh".to_owned());
        assert!(
            !Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .reconcile(root.path(), 50, None, &interactions)
                .unwrap_or_else(|(_, error)| panic!("{error}"))
                .active(key, 50)
        );
    }

    #[test]
    fn corrupt_or_locked_files_leave_committed_and_other_workspace_data_intact() {
        let root = tempfile::tempdir().unwrap();
        let path = Snoozes::path(root.path(), WORKSPACE).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let invalid = b"{invalid json";
        fs::write(&path, invalid).unwrap();
        assert!(Snoozes::load(root.path(), WORKSPACE).is_err());
        assert!(
            Snoozes::set(
                root.path(),
                WORKSPACE,
                "message:one",
                Some(Reminder::default())
            )
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), invalid);

        let other = Snoozes::path(root.path(), OTHER_WORKSPACE).unwrap();
        Snoozes::set(
            root.path(),
            OTHER_WORKSPACE,
            "message:one",
            Some(Reminder::default()),
        )
        .unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(other.with_extension("lock"))
            .unwrap();
        lock.try_lock().unwrap();
        assert!(
            Snoozes::set(
                root.path(),
                OTHER_WORKSPACE,
                "report:two",
                Some(Reminder::default())
            )
            .is_err()
        );
        assert!(
            Snoozes::load(root.path(), OTHER_WORKSPACE)
                .unwrap()
                .active("message:one", 1)
        );
        drop(lock);
        Snoozes::set(
            root.path(),
            OTHER_WORKSPACE,
            "report:two",
            Some(Reminder::default()),
        )
        .unwrap();
        let saved = Snoozes::load(root.path(), OTHER_WORKSPACE).unwrap();
        assert!(saved.active("report:two", 1) && saved.active("message:one", 1));
        assert_eq!(fs::read(&path).unwrap(), invalid);
    }
}
