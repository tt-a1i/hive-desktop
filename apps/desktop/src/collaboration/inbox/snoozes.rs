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
    Until(u64),
    Indefinite,
}

impl Delay {
    pub(super) fn reminder(self, now: u64, wake: &Option<Wake>) -> Result<Reminder, &'static str> {
        match self {
            Self::Until(until) if until <= now => return Err("所选暂缓时间已过，请重新选择。"),
            _ => {}
        }
        Ok(Reminder {
            expires_at: match self {
                Self::Until(until) => Some(until),
                Self::Indefinite => None,
            },
            wake: wake.clone(),
        })
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
    UnreadReply {
        revision: u64,
    },
}

pub(super) fn unread_session_key(session: &str) -> String {
    format!("unread-session:{session}")
}

impl Reminder {
    fn expired(&self, now: u64) -> bool {
        self.expires_at.is_some_and(|until| until <= now)
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snoozes {
    workspace_id: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    keys: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    reminders: BTreeMap<String, Reminder>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    released_unread: BTreeSet<String>,
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
            released_unread: BTreeSet::new(),
        }
    }

    pub(super) fn load(root: &Path, workspace: &str) -> io::Result<Self> {
        Ok(Self::read(&Self::path(root, workspace)?, workspace)?
            .unwrap_or_else(|| Self::empty(workspace)))
    }

    pub(super) fn next_expiry(&self) -> Option<u64> {
        self.reminders
            .values()
            .filter_map(|reminder| reminder.expires_at)
            .min()
    }

    pub(super) fn expired_reminders(&self, now: u64) -> impl Iterator<Item = (&str, &Reminder)> {
        self.reminders.iter().filter_map(move |(key, reminder)| {
            reminder.expired(now).then_some((key.as_str(), reminder))
        })
    }

    pub(super) fn expire(root: &Path, workspace: &str, now: u64) -> io::Result<Self> {
        Self::load(root, workspace)?
            .reconcile(root, now, None, &BTreeMap::new(), &BTreeMap::new())
            .map_err(|(_, error)| error)
    }

    pub(super) fn tracks_unread(&self, session: &str) -> bool {
        self.released_unread.contains(session)
            || self.reminders.contains_key(&unread_session_key(session))
    }

    pub(super) fn hides_unread(&self, session: &str, revision: u64, now: u64) -> bool {
        self.reminders.get(&unread_session_key(session)).is_some_and(|reminder| {
            !reminder.expired(now)
                && !matches!(reminder.wake, Some(Wake::UnreadReply {revision: saved}) if revision > saved)
        })
    }

    pub(super) fn clear_unread(root: &Path, workspace: &str, session: &str) -> io::Result<Self> {
        let saved = Self::load(root, workspace)?;
        if !saved.tracks_unread(session) {
            return Ok(saved);
        }
        Self::update(root, workspace, |saved| {
            saved.reminders.remove(&unread_session_key(session));
            saved.released_unread.remove(session);
        })
    }

    pub(super) fn active(&self, key: &str, now: u64) -> bool {
        self.keys.contains(key)
            || self
                .reminders
                .get(key)
                .is_some_and(|item| !item.expired(now))
    }

    pub(super) fn active_reminder(&self, key: &str, now: u64) -> Option<Reminder> {
        self.reminders
            .get(key)
            .filter(|reminder| !reminder.expired(now))
            .cloned()
            .or_else(|| self.keys.contains(key).then(Reminder::default))
    }

    pub(super) fn edit(
        root: &Path,
        workspace: &str,
        key: &str,
        previous: Option<&Reminder>,
        next: Option<Reminder>,
        now: u64,
    ) -> io::Result<(Self, bool)> {
        let mut applied = false;
        let saved = Self::update(root, workspace, |saved| {
            if saved.active_reminder(key, now).as_ref() == previous {
                if let Some(session) = key.strip_prefix("unread-session:") {
                    if next.is_none() {
                        saved.released_unread.insert(session.to_owned());
                    } else {
                        saved.released_unread.remove(session);
                    }
                }
                saved.assign(key, next);
                applied = true;
            }
        })?;
        Ok((saved, applied))
    }

    #[cfg(test)]
    pub(super) fn set(
        root: &Path,
        workspace: &str,
        key: &str,
        reminder: Option<Reminder>,
    ) -> io::Result<Self> {
        Self::update(root, workspace, |saved| saved.assign(key, reminder))
    }

    fn assign(&mut self, key: &str, reminder: Option<Reminder>) {
        self.keys.remove(key);
        if let Some(reminder) = reminder {
            self.reminders.insert(key.to_owned(), reminder);
        } else {
            self.reminders.remove(key);
        }
    }

    pub(super) fn watched_dispatches(
        &self,
        now: u64,
        snapshot: Option<&ActionCenter>,
    ) -> BTreeMap<String, bool> {
        let snapshot = snapshot.filter(|snapshot| snapshot.workspace_id == self.workspace_id);
        let wakes = self
            .reminders
            .iter()
            .filter(|(_, reminder)| !reminder.expired(now))
            .filter_map(|(key, reminder)| {
                reminder
                    .wake
                    .clone()
                    .or_else(|| snapshot.and_then(|snapshot| snapshot.legacy_wake(key)))
            })
            .chain(
                self.keys
                    .iter()
                    .filter_map(|key| snapshot.and_then(|snapshot| snapshot.legacy_wake(key))),
            );
        let mut dispatches = BTreeMap::new();
        for wake in wakes {
            if let Wake::Dispatch { id, sequence, .. } = wake
                && uuid::Uuid::try_parse(&id).is_ok()
            {
                *dispatches.entry(id).or_default() |= sequence.is_none();
            }
        }
        dispatches
    }

    pub(super) fn reconcile_dispatch_inputs(
        self,
        root: &Path,
        inputs: &BTreeMap<String, super::DispatchInput>,
    ) -> Result<Self, (Self, io::Error)> {
        let woke = |item: &Reminder| match &item.wake {
            Some(Wake::Dispatch {
                id,
                sequence,
                observed_at,
            }) => inputs
                .get(id)
                .is_some_and(|input| input.is_newer(*sequence, *observed_at)),
            _ => false,
        };
        if !self.reminders.values().any(woke) {
            return Ok(self);
        }
        Self::update(root, &self.workspace_id, |latest| {
            latest.reminders.retain(|_, item| !woke(item));
        })
        .map_err(|error| (self, error))
    }

    pub(super) fn reconcile(
        self,
        root: &Path,
        now: u64,
        snapshot: Option<&ActionCenter>,
        interactions: &BTreeMap<String, BTreeSet<String>>,
        replies: &BTreeMap<String, u64>,
    ) -> Result<Self, (Self, io::Error)> {
        let snapshot = snapshot.filter(|snapshot| snapshot.workspace_id == self.workspace_id);
        let needs_update = self.reminders.iter().any(|(key, item)| {
            Self::woke(key, item, now, snapshot, interactions, replies)
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
            latest.reminders.retain(|key, item| {
                let woke = Self::woke(key, item, now, snapshot, interactions, replies);
                if woke && let Some(session) = key.strip_prefix("unread-session:") {
                    latest.released_unread.insert(session.to_owned());
                }
                !woke
            });
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
        replies: &BTreeMap<String, u64>,
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
                Some(Wake::UnreadReply { revision }) => key
                    .strip_prefix("unread-session:")
                    .and_then(|id| replies.get(id))
                    .is_some_and(|current| current > revision),
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
        let before_released = saved.released_unread.clone();
        change(&mut saved);
        if before_keys != saved.keys
            || before_reminders != saved.reminders
            || before_released != saved.released_unread
        {
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

    #[test]
    fn local_deadline_reads_rescheduled_file_and_releases_only_due_unread() {
        let root = tempfile::tempdir().unwrap();
        let unread = unread_session_key("first");
        let reminder = |expires_at| Reminder {
            expires_at,
            wake: Some(Wake::UnreadReply { revision: 1 }),
        };
        Snoozes::set(root.path(), WORKSPACE, &unread, Some(reminder(Some(20)))).unwrap();
        Snoozes::set(
            root.path(),
            WORKSPACE,
            "message:indefinite",
            Some(Reminder::default()),
        )
        .unwrap();
        let armed = Snoozes::load(root.path(), WORKSPACE).unwrap();
        assert_eq!(armed.next_expiry(), Some(20));
        Snoozes::set(root.path(), WORKSPACE, &unread, Some(reminder(Some(40)))).unwrap();
        let reloaded = Snoozes::expire(root.path(), WORKSPACE, 20).unwrap();
        assert!(reloaded.active(&unread, 20));
        assert_eq!(reloaded.next_expiry(), Some(40));
        assert!(
            Snoozes::expire(root.path(), WORKSPACE, 40)
                .unwrap()
                .next_expiry()
                .is_none()
        );
        let reopened = Snoozes::load(root.path(), WORKSPACE).unwrap();
        assert!(reopened.tracks_unread("first"));
        assert!(!reopened.active(&unread, 40));
        assert!(reopened.active("message:indefinite", 40));
    }

    #[test]
    fn unread_reply_wakes_exact_snooze_across_reopen_without_losing_the_item() {
        let root = tempfile::tempdir().unwrap();
        let key = unread_session_key("first");
        let other = unread_session_key("second");
        let reminder = Reminder {
            expires_at: None,
            wake: Some(Wake::UnreadReply { revision: 7 }),
        };
        for workspace in [WORKSPACE, OTHER_WORKSPACE] {
            Snoozes::set(root.path(), workspace, &key, Some(reminder.clone())).unwrap();
        }
        Snoozes::set(root.path(), WORKSPACE, &other, Some(reminder.clone())).unwrap();
        let reconciling = |replies: BTreeMap<String, u64>| {
            Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .reconcile(root.path(), 30, None, &BTreeMap::new(), &replies)
                .unwrap_or_else(|(_, error)| panic!("{error}"))
        };
        let unchanged = reconciling(BTreeMap::from([
            ("first".into(), 7),
            ("unrelated".into(), 99),
        ]));
        assert!(unchanged.active(&key, 30));
        assert!(unchanged.active(&other, 30));
        let woke = reconciling(BTreeMap::from([("first".into(), 8)]));
        assert!(!woke.active(&key, 30));
        assert!(woke.active(&other, 30));
        assert!(
            Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .tracks_unread("first")
        );
        assert!(
            Snoozes::load(root.path(), OTHER_WORKSPACE)
                .unwrap()
                .active(&key, 30)
        );
        Snoozes::clear_unread(root.path(), WORKSPACE, "first").unwrap();
        assert!(
            !Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .tracks_unread("first")
        );
    }

    #[test]
    fn expired_passive_unread_reminder_survives_restart_and_manual_undo() {
        let root = tempfile::tempdir().unwrap();
        let key = unread_session_key("first");
        let reminder = Reminder {
            expires_at: Some(40),
            wake: Some(Wake::UnreadReply { revision: 2 }),
        };
        Snoozes::set(root.path(), WORKSPACE, &key, Some(reminder.clone())).unwrap();
        let (_, changed) =
            Snoozes::edit(root.path(), WORKSPACE, &key, Some(&reminder), None, 20).unwrap();
        assert!(changed);
        assert!(
            Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .tracks_unread("first")
        );
        let (_, restored) =
            Snoozes::edit(root.path(), WORKSPACE, &key, None, Some(reminder), 22).unwrap();
        assert!(restored);
        let reopened = Snoozes::load(root.path(), WORKSPACE).unwrap();
        assert!(reopened.active(&key, 39));
        assert!(reopened.tracks_unread("first"));
        reopened
            .reconcile(root.path(), 40, None, &BTreeMap::new(), &BTreeMap::new())
            .unwrap_or_else(|(_, error)| panic!("{error}"));
        let released = Snoozes::load(root.path(), WORKSPACE).unwrap();
        assert!(!released.active(&key, 40));
        assert!(released.tracks_unread("first"));
    }

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
            &BTreeMap::new(),
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
            &BTreeMap::new(),
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
        let until = 15 * 60_000 + 500;
        Snoozes::set(
            root.path(),
            WORKSPACE,
            key,
            Some(Delay::Until(until).reminder(500, &None).unwrap()),
        )
        .unwrap();
        let saved = Snoozes::load(root.path(), WORKSPACE).unwrap();
        assert_eq!(saved.reminders[key].expires_at, Some(until));
        assert!(Delay::Until(until).reminder(until, &None).is_err());
        assert!(saved.active(key, until - 1));
        assert!(!saved.active(key, until));
        saved
            .reconcile(root.path(), until, None, &BTreeMap::new(), &BTreeMap::new())
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
    fn rescheduling_checks_the_old_reminder_and_cannot_recreate_it_after_wake() {
        let root = tempfile::tempdir().unwrap();
        let key = "message:old";
        let wake = Some(Wake::Dispatch {
            id: DISPATCH.into(),
            sequence: Some(7),
            observed_at: 0,
        });
        let original = Delay::Until(5_000).reminder(100, &wake).unwrap();
        Snoozes::set(root.path(), WORKSPACE, key, Some(original.clone())).unwrap();
        let revision = Delay::Until(10_000).reminder(200, &wake).unwrap();
        let (_, applied) = Snoozes::edit(
            root.path(),
            WORKSPACE,
            key,
            Some(&original),
            Some(revision.clone()),
            200,
        )
        .unwrap();
        assert!(applied);
        assert_eq!(
            Snoozes::load(root.path(), WORKSPACE)
                .unwrap()
                .active_reminder(key, 200),
            Some(revision.clone())
        );
        let path = Snoozes::path(root.path(), WORKSPACE).unwrap();
        let saved_bytes = fs::read(&path).unwrap();
        for (expected, now) in [(&original, 200), (&revision, 10_000)] {
            let next = Delay::Until(30_000).reminder(now, &wake).unwrap();
            let (_, applied) =
                Snoozes::edit(root.path(), WORKSPACE, key, Some(expected), Some(next), now)
                    .unwrap();
            assert!(!applied);
            assert_eq!(fs::read(&path).unwrap(), saved_bytes);
        }
        let fresh = snapshot(&[message(WORKSPACE, DISPATCH, "fresh", 8, 300)]);
        Snoozes::load(root.path(), WORKSPACE)
            .unwrap()
            .reconcile(
                root.path(),
                300,
                Some(&fresh),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap_or_else(|(_, error)| panic!("{error}"));
        let next = Delay::Until(30_000).reminder(300, &wake).unwrap();
        let (saved, applied) = Snoozes::edit(
            root.path(),
            WORKSPACE,
            key,
            Some(&revision),
            Some(next),
            300,
        )
        .unwrap();
        assert!(!applied);
        assert!(!saved.active(key, 300));
    }

    #[test]
    fn legacy_indefinite_reminders_can_be_rescheduled() {
        let root = tempfile::tempdir().unwrap();
        let key = "message:old";
        let path = Snoozes::path(root.path(), WORKSPACE).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            json!({"workspace_id": WORKSPACE, "keys": [key]}).to_string(),
        )
        .unwrap();
        let previous = Snoozes::load(root.path(), WORKSPACE)
            .unwrap()
            .active_reminder(key, 100)
            .unwrap();
        assert_eq!(previous.expires_at, None);
        let next = Delay::Until(50_000).reminder(100, &None).unwrap();
        let (saved, applied) = Snoozes::edit(
            root.path(),
            WORKSPACE,
            key,
            Some(&previous),
            Some(next),
            100,
        )
        .unwrap();
        assert!(applied);
        assert_eq!(
            saved.active_reminder(key, 100).unwrap().expires_at,
            Some(50_000)
        );
        assert!(saved.keys.is_empty());
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
                Some(
                    Delay::Until(20_000)
                        .reminder(
                            400,
                            &Some(Wake::Dispatch {
                                id: DISPATCH.to_owned(),
                                sequence,
                                observed_at: 200,
                            }),
                        )
                        .unwrap(),
                ),
            )
            .unwrap();
            let same = snapshot(&[message(WORKSPACE, DISPATCH, "old", 7, 100)]);
            assert!(
                saved
                    .reconcile(
                        root.path(),
                        500,
                        Some(&same),
                        &BTreeMap::new(),
                        &BTreeMap::new()
                    )
                    .unwrap_or_else(|(_, error)| panic!("{error}"))
                    .active(key, 500)
            );
            let newer = snapshot(&[message(WORKSPACE, DISPATCH, "fresh", 8, 300)]);
            assert!(
                !Snoozes::load(root.path(), WORKSPACE)
                    .unwrap()
                    .reconcile(
                        root.path(),
                        500,
                        Some(&newer),
                        &BTreeMap::new(),
                        &BTreeMap::new()
                    )
                    .unwrap_or_else(|(_, error)| panic!("{error}"))
                    .active(key, 500)
            );
        }
    }

    #[test]
    fn persisted_dispatch_watch_wakes_behind_the_preview_for_both_anchor_types() {
        use super::super::DispatchInput;

        for sequence in [Some(7), None] {
            let root = tempfile::tempdir().unwrap();
            let key = format!("report:{DISPATCH}");
            Snoozes::set(
                root.path(),
                WORKSPACE,
                &key,
                Some(Reminder {
                    expires_at: None,
                    wake: Some(Wake::Dispatch {
                        id: DISPATCH.into(),
                        sequence,
                        observed_at: 200,
                    }),
                }),
            )
            .unwrap();
            let reopen = || Snoozes::load(root.path(), WORKSPACE).unwrap();
            let mut center = snapshot(&[message(WORKSPACE, "other-dispatch", "newer", 99, 400)]);
            assert_eq!(
                reopen().watched_dispatches(500, None),
                BTreeMap::from([(DISPATCH.into(), sequence.is_none())])
            );

            center.dispatch_inputs.insert(
                DISPATCH.into(),
                DispatchInput {
                    dispatch_id: DISPATCH.into(),
                    sequence: Some(7),
                    created_at: Some(200),
                },
            );
            assert!(
                reopen()
                    .reconcile_dispatch_inputs(root.path(), &center.dispatch_inputs)
                    .unwrap_or_else(|(_, error)| panic!("{error}"))
                    .active(&key, 500)
            );
            center.dispatch_inputs.insert(
                DISPATCH.into(),
                DispatchInput {
                    dispatch_id: DISPATCH.into(),
                    sequence: Some(8),
                    created_at: Some(201),
                },
            );
            if sequence.is_some() {
                reopen()
                    .reconcile_dispatch_inputs(root.path(), &center.dispatch_inputs)
                    .unwrap_or_else(|(_, error)| panic!("{error}"));
            } else {
                reopen()
                    .reconcile(
                        root.path(),
                        500,
                        Some(&center),
                        &BTreeMap::new(),
                        &BTreeMap::new(),
                    )
                    .unwrap_or_else(|(_, error)| panic!("{error}"));
            }
            assert!(!reopen().active(&key, 500));
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
                .reconcile(root.path(), 50, None, &interactions, &BTreeMap::new())
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
                .reconcile(root.path(), 50, None, &interactions, &BTreeMap::new())
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
