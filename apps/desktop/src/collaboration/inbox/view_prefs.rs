use super::ActivityRange;
use crate::protocol::{Backend, Member, Workspace};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

const FILE: &str = "desktop-inbox-view.json";
const READ_FAILED: &str = "无法读取收件箱视图偏好";
const SAVE_FAILED: &str = "无法保存收件箱视图偏好";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MemberFilter {
    pub(super) workspace_id: String,
    pub(super) member_id: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct InboxViewPreferences {
    pub(super) project_filter: Option<String>,
    pub(super) member_filter: Option<MemberFilter>,
    pub(super) backend_filter: Option<Backend>,
    pub(super) activity_filter: Option<ActivityRange>,
    pub(super) show_waiting: bool,
    pub(super) show_snoozed: bool,
}

#[derive(Clone, Default)]
pub(super) struct ViewPreferences {
    pub(super) values: InboxViewPreferences,
    pub(super) error: Option<&'static str>,
    root: Option<PathBuf>,
    dirty: bool,
}

impl ViewPreferences {
    pub(super) fn restore_root(&mut self, root: Option<&Path>) -> bool {
        let Some(root) = root.filter(|root| Some(*root) != self.root.as_deref()) else {
            return false;
        };
        self.root = Some(root.to_owned());
        self.dirty = false;
        match load(root) {
            Ok(values) => {
                self.values = values;
                self.error = None;
            }
            Err(_) => {
                self.values = InboxViewPreferences::default();
                self.error = Some(READ_FAILED);
            }
        }
        true
    }

    pub(super) fn forget_unavailable(&mut self, workspaces: Option<&[Workspace]>) -> bool {
        let Some(workspaces) = workspaces else {
            return false;
        };
        let project_removed = self
            .values
            .project_filter
            .as_ref()
            .is_some_and(|id| !workspaces.iter().any(|project| &project.id == id));
        let member_removed = self.values.member_filter.as_ref().is_some_and(|member| {
            !workspaces
                .iter()
                .any(|project| project.id == member.workspace_id)
        });
        if project_removed {
            self.values.project_filter = None;
        }
        if member_removed {
            self.values.member_filter = None;
        }
        if project_removed || member_removed {
            self.save_changed();
        }
        project_removed || member_removed
    }

    pub(super) fn select_project(&mut self, project: Option<String>) {
        if let Some(project) = &project
            && self
                .values
                .member_filter
                .as_ref()
                .is_some_and(|member| &member.workspace_id != project)
        {
            self.values.member_filter = None;
        }
        self.values.project_filter = project;
        self.save_changed();
    }

    pub(super) fn has_filters(&self) -> bool {
        self.filter_count() > 0
    }

    pub(super) fn filter_count(&self) -> usize {
        [
            self.values.project_filter.is_some(),
            self.values.member_filter.is_some(),
            self.values.backend_filter.is_some(),
            self.values.activity_filter.is_some(),
        ]
        .into_iter()
        .filter(|selected| *selected)
        .count()
    }

    pub(super) fn clear_filters(&mut self) -> bool {
        if !self.has_filters() {
            return false;
        }
        self.values.project_filter = None;
        self.values.member_filter = None;
        self.values.backend_filter = None;
        self.values.activity_filter = None;
        self.save_changed();
        true
    }

    pub(super) fn forget_missing_member(&mut self, workspace: &str, members: &[Member]) -> bool {
        if self.values.member_filter.as_ref().is_some_and(|member| {
            member.workspace_id == workspace
                && !members.iter().any(|item| item.id == member.member_id)
        }) {
            self.values.member_filter = None;
            self.save_changed();
            return true;
        }
        false
    }

    pub(super) fn save_changed(&mut self) {
        self.dirty = true;
        self.retry_save();
    }

    pub(super) fn retry_save(&mut self) {
        if let Some(root) = self.root.as_deref().filter(|_| self.dirty) {
            self.dirty = save(root, &self.values).is_err();
            self.error = self.dirty.then_some(SAVE_FAILED);
        }
    }
}

fn load(root: &Path) -> io::Result<InboxViewPreferences> {
    match fs::read(root.join(FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok(InboxViewPreferences::default())
        }
        Err(error) => Err(error),
    }
}

fn save(root: &Path, values: &InboxViewPreferences) -> io::Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(&mut file, values).map_err(io::Error::other)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(root.join(FILE)).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ActivityRange, Backend, FILE, MemberFilter, SAVE_FAILED, ViewPreferences, load};
    use crate::protocol::Workspace;

    fn workspace(id: &str) -> Workspace {
        Workspace {
            id: id.into(),
            name: "Same name".into(),
            path: format!("/projects/{id}"),
        }
    }

    #[test]
    fn inbox_preferences_restore_exact_project_and_sections_only_in_that_directory() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let mut view = ViewPreferences::default();
        view.restore_root(Some(first.path()));
        view.values.project_filter = Some("selected".into());
        view.values.backend_filter = Some(Backend::PiRpc);
        view.values.activity_filter = Some(ActivityRange::ThreeDays);
        view.values.show_waiting = true;
        view.values.show_snoozed = true;
        view.save_changed();

        let mut reopened = ViewPreferences::default();
        reopened.restore_root(Some(first.path()));
        assert!(!reopened.forget_unavailable(Some(&[workspace("other"), workspace("selected")])));
        assert_eq!(reopened.values.project_filter.as_deref(), Some("selected"));
        assert_eq!(reopened.values.backend_filter, Some(Backend::PiRpc));
        assert_eq!(
            reopened.values.activity_filter,
            Some(ActivityRange::ThreeDays)
        );
        assert!(reopened.values.show_waiting);
        assert!(reopened.values.show_snoozed);
        assert!(reopened.restore_root(Some(second.path())));
        assert!(reopened.values.project_filter.is_none());
        assert!(reopened.values.backend_filter.is_none());
        assert!(reopened.values.activity_filter.is_none());
        assert!(!reopened.values.show_waiting);
        assert!(!reopened.values.show_snoozed);
        assert!(!second.path().join(FILE).exists());
    }

    #[test]
    fn clearing_all_inbox_filters_saves_only_filter_defaults() {
        let root = tempfile::tempdir().unwrap();
        let mut view = ViewPreferences::default();
        view.restore_root(Some(root.path()));
        assert!(!view.clear_filters());
        assert!(!root.path().join(FILE).exists());

        view.values.project_filter = Some("beacon".into());
        view.values.member_filter = Some(MemberFilter {
            workspace_id: "beacon".into(),
            member_id: "reviewer".into(),
        });
        view.values.backend_filter = Some(Backend::Codex);
        view.values.activity_filter = Some(ActivityRange::OneDay);
        view.values.show_waiting = true;
        view.values.show_snoozed = true;
        view.save_changed();

        assert_eq!(view.filter_count(), 4);
        assert!(view.clear_filters());
        assert_eq!(view.filter_count(), 0);
        let saved = load(root.path()).unwrap();
        assert!(saved.project_filter.is_none());
        assert!(saved.member_filter.is_none());
        assert!(saved.backend_filter.is_none());
        assert!(saved.activity_filter.is_none());
        assert!(saved.show_waiting);
        assert!(saved.show_snoozed);
    }

    #[test]
    fn inbox_preferences_wait_for_an_authoritative_list_before_saving_all_projects() {
        let root = tempfile::tempdir().unwrap();
        let mut view = ViewPreferences::default();
        view.restore_root(Some(root.path()));
        view.values.project_filter = Some("missing".into());
        view.values.member_filter = Some(MemberFilter {
            workspace_id: "missing".into(),
            member_id: "worker".into(),
        });
        view.values.show_waiting = true;
        view.values.show_snoozed = true;
        view.save_changed();

        assert!(!view.forget_unavailable(None));
        assert!(load(root.path()).unwrap().member_filter.is_some());
        assert_eq!(
            load(root.path()).unwrap().project_filter.as_deref(),
            Some("missing")
        );
        assert!(view.forget_unavailable(Some(&[])));
        let saved = load(root.path()).unwrap();
        assert!(saved.project_filter.is_none());
        assert!(saved.member_filter.is_none());
        assert!(saved.show_waiting);
        assert!(saved.show_snoozed);
    }

    #[test]
    fn inbox_preferences_keep_an_exact_member_until_their_project_or_roster_excludes_it() {
        let root = tempfile::tempdir().unwrap();
        let mut view = ViewPreferences::default();
        view.restore_root(Some(root.path()));
        let member = MemberFilter {
            workspace_id: "left".into(),
            member_id: "worker".into(),
        };
        view.values.member_filter = Some(member.clone());
        view.select_project(Some("left".into()));
        let mut reopened = ViewPreferences::default();
        reopened.restore_root(Some(root.path()));
        assert_eq!(reopened.values.member_filter.as_ref(), Some(&member));
        assert!(!reopened.forget_missing_member("right", &[]));
        assert_eq!(reopened.values.member_filter.as_ref(), Some(&member));
        assert!(reopened.forget_missing_member("left", &[]));
        assert!(load(root.path()).unwrap().member_filter.is_none());
        assert_eq!(
            load(root.path()).unwrap().project_filter.as_deref(),
            Some("left")
        );

        view.values.member_filter = Some(member.clone());
        view.select_project(None);
        assert_eq!(view.values.member_filter.as_ref(), Some(&member));
        view.select_project(Some("right".into()));
        let saved = load(root.path()).unwrap();
        assert!(saved.member_filter.is_none());
        assert_eq!(saved.project_filter.as_deref(), Some("right"));
    }

    #[test]
    fn inbox_preferences_retry_a_failed_save_without_losing_the_selected_view() {
        let root = tempfile::tempdir().unwrap();
        let mut view = ViewPreferences::default();
        view.restore_root(Some(root.path()));
        let dest = root.path().join(FILE);
        std::fs::create_dir(&dest).unwrap();
        view.values.show_waiting = true;
        view.save_changed();
        assert!(view.values.show_waiting);
        assert_eq!(view.error, Some(SAVE_FAILED));
        std::fs::remove_dir(&dest).unwrap();

        view.retry_save();
        assert!(view.error.is_none());
        assert!(load(root.path()).unwrap().show_waiting);
    }
}
