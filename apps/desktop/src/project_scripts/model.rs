use gpui_kit::SharedString;
use serde::Deserialize;
use std::cell::OnceCell;

#[derive(Debug, PartialEq, Eq)]
pub struct Navigation<'a> {
    pub workspace: &'a str,
    pub session: Option<&'a str>,
    pub connection: u64,
    pub open: u64,
}

pub const UI_LOG_LIMIT: usize = 1024 * 1024;
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: u32,
    pub revision: String,
    pub scripts: Vec<Script>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub id: String,
    pub version: String,
    pub name: String,
    pub command: String,
    #[serde(default, rename = "runOnWorktreeCreate")]
    pub run_on_worktree_create: bool,
}
impl Snapshot {
    pub fn saved_selection(
        &self,
        previous_ids: &[String],
        key: &str,
        name: &str,
        command: &str,
        run_on_worktree_create: bool,
        remove: bool,
    ) -> Result<String, String> {
        if remove {
            return if self.scripts.iter().any(|script| script.id == key) {
                Err("Script removal was not acknowledged".into())
            } else {
                Ok(String::new())
            };
        }
        let candidates = self
            .scripts
            .iter()
            .filter(|script| {
                (if key.is_empty() {
                    !previous_ids.contains(&script.id)
                } else {
                    script.id == key
                }) && script.name == name
                    && script.command == command
                    && script.run_on_worktree_create == run_on_worktree_create
            })
            .collect::<Vec<_>>();
        if candidates.len() == 1 {
            Ok(candidates[0].id.clone())
        } else {
            Err("Saved script contents were not acknowledged".into())
        }
    }
    pub fn validate(self) -> Result<Self, String> {
        if self.version != 2
            || !(uuid4(&self.revision) || self.revision == "0" && self.scripts.is_empty())
            || self.scripts.len() > 100
        {
            return Err("Invalid script settings response".into());
        }
        if self
            .scripts
            .iter()
            .filter(|s| s.run_on_worktree_create)
            .count()
            > 1
        {
            return Err("Multiple worktree setup scripts".into());
        }
        let mut ids = std::collections::HashSet::new();
        for script in &self.scripts {
            if !uuid4(&script.id)
                || !uuid4(&script.version)
                || !ids.insert(&script.id)
                || script.name.trim().is_empty()
                || script.command.trim().is_empty()
                || script.name.encode_utf16().count() > 120
                || script.command.encode_utf16().count() > 16384
                || script.name.bytes().any(|byte| byte < 32 || byte == 127)
                || script
                    .command
                    .bytes()
                    .any(|byte| matches!(byte,0..=8|11..=12|14..=31|127))
            {
                return Err("Invalid script settings response".into());
            }
        }
        Ok(self)
    }
}
fn uuid4(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.get_version_num() == 4)
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub version: u32,
    pub run_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub session_id: Option<String>,
    pub cwd: String,
    pub script: Script,
    pub status: Status,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub native_run_id: Option<String>,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub cancel_requested: bool,
    pub log_bytes: usize,
    pub log_truncated: bool,
    pub error_code: Option<String>,
}
impl Run {
    pub fn validate(&self, workspace: &str) -> Result<(), String> {
        if self.version != 1
            || self.workspace_id != workspace
            || !uuid4(&self.run_id)
            || !uuid4(&self.request_id)
            || !uuid4(&self.script.id)
            || !uuid4(&self.script.version)
            || self.cwd.is_empty()
            || self.log_bytes > 16 * 1024 * 1024
        {
            Err("Invalid script run response".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Starting,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
    Unknown,
}
impl Status {
    pub fn active(&self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }
    pub fn label(&self) -> &'static str {
        use crate::language::tr;
        match self {
            Self::Starting => tr("正在启动"),
            Self::Running => tr("正在运行"),
            Self::Succeeded => tr("已成功"),
            Self::Failed => tr("已失败"),
            Self::Cancelled => tr("已取消"),
            Self::Interrupted => tr("已中断"),
            Self::Unknown => tr("结果未知"),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Runs {
    pub runs: Vec<Run>,
    #[serde(default)]
    pub unreadable_run_count: usize,
}
#[derive(Deserialize)]
pub struct RunPage {
    pub run: Run,
    pub output: OutputPage,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputPage {
    pub offset: usize,
    pub next_offset: usize,
    pub bytes: Vec<u8>,
    pub truncated: bool,
}
#[derive(Default)]
pub struct Output {
    pub start: usize,
    pub next: usize,
    pub bytes: Vec<u8>,
    pub truncated: bool,
    display: OnceCell<SharedString>,
}
impl Output {
    pub fn append(&mut self, page: OutputPage, requested: usize) -> Result<(), String> {
        if page.offset != self.next
            || page.bytes.len() > requested
            || page.next_offset
                != page
                    .offset
                    .checked_add(page.bytes.len())
                    .ok_or("Invalid output offset")?
            || self.bytes.len() + page.bytes.len() > UI_LOG_LIMIT
        {
            return Err("Invalid script output page".into());
        }
        self.next = page.next_offset;
        self.truncated = page.truncated;
        if !page.bytes.is_empty() {
            self.display.take();
        }
        self.bytes.extend(page.bytes);
        Ok(())
    }
    pub fn next_segment(&self) -> Self {
        // Carry a split final UTF-8 scalar into the next bounded display segment.
        let tail = (1..=3.min(self.bytes.len()))
            .find(|length| {
                std::str::from_utf8(&self.bytes[self.bytes.len() - length..])
                    .err()
                    .is_some_and(|error| error.error_len().is_none())
            })
            .unwrap_or(0);
        let carry = self.bytes[self.bytes.len() - tail..].to_vec();
        Self {
            start: self.next - carry.len(),
            next: self.next,
            bytes: carry,
            truncated: self.truncated,
            display: OnceCell::new(),
        }
    }
    pub fn text(&self) -> SharedString {
        self.display
            .get_or_init(|| SharedString::new(String::from_utf8_lossy(&self.bytes)))
            .clone()
    }
}
pub fn decode<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| format!("Invalid script response: {error}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_only_rebuilds_its_display_when_bytes_change() {
        let mut output = Output::default();
        let first = vec![b'a'; 4096];
        output.append(OutputPage {
            offset: 0, next_offset: first.len(), bytes: first, truncated: false,
        }, 65536).unwrap();
        let display = output.text();
        assert_eq!(display.as_str().as_ptr(), output.text().as_str().as_ptr());
        output.append(OutputPage {
            offset: 4096, next_offset: 4097, bytes: vec![b'!'], truncated: false,
        }, 65536).unwrap();
        assert_eq!(display.len(), 4096);
        assert!(output.text().ends_with('!'));
    }

    #[test]
    fn byte_pages_reassemble_split_utf8_and_reject_skipped_or_oversized_pages() {
        let mut output = Output::default();
        output
            .append(
                OutputPage {
                    offset: 0,
                    next_offset: 2,
                    bytes: vec![0xe4, 0xb8],
                    truncated: false,
                },
                65536,
            )
            .unwrap();
        output
            .append(
                OutputPage {
                    offset: 2,
                    next_offset: 3,
                    bytes: vec![0xad],
                    truncated: false,
                },
                65536,
            )
            .unwrap();
        assert_eq!(output.text().as_str(), "中");
        assert!(
            output
                .append(
                    OutputPage {
                        offset: 4,
                        next_offset: 5,
                        bytes: vec![1],
                        truncated: false
                    },
                    65536
                )
                .is_err()
        );
        assert!(
            output
                .append(
                    OutputPage {
                        offset: 3,
                        next_offset: 5,
                        bytes: vec![1, 2],
                        truncated: false
                    },
                    1
                )
                .is_err()
        );
        assert_eq!(output.next, 3);
        output.bytes.extend([0xf0, 0x9f]);
        output.next += 2;
        output = output.next_segment();
        output
            .append(
                OutputPage {
                    offset: 5,
                    next_offset: 7,
                    bytes: vec![0x98, 0x80],
                    truncated: true,
                },
                65536,
            )
            .unwrap();
        assert_eq!(output.text().as_str(), "😀");
        assert_eq!(output.start, 3);
        assert!(output.truncated);
    }
    #[test]
    fn navigation_and_connection_changes_reject_same_id_reopen_and_roundtrips() {
        let captured = Navigation {
            workspace: "A",
            session: Some("same"),
            connection: 2,
            open: 10,
        };
        assert_eq!(
            captured,
            Navigation {
                workspace: "A",
                session: Some("same"),
                connection: 2,
                open: 10
            }
        );
        assert_ne!(
            captured,
            Navigation {
                workspace: "A",
                session: Some("same"),
                connection: 2,
                open: 11
            }
        );
        assert_ne!(
            captured,
            Navigation {
                workspace: "A",
                session: Some("same"),
                connection: 2,
                open: 12
            }
        );
        assert_ne!(
            captured,
            Navigation {
                workspace: "A",
                session: Some("same"),
                connection: 3,
                open: 10
            }
        );
        assert_ne!(
            captured,
            Navigation {
                workspace: "A",
                session: Some("different"),
                connection: 2,
                open: 10
            }
        );
        assert_ne!(
            captured,
            Navigation {
                workspace: "A",
                session: None,
                connection: 2,
                open: 10
            }
        );
        assert_ne!(
            captured,
            Navigation {
                workspace: "B",
                session: Some("same"),
                connection: 2,
                open: 10
            }
        );
    }
    #[test]
    fn save_acknowledgement_identifies_new_entry_among_duplicate_names_and_preserves_failed_edits()
    {
        let old = "00000000-0000-4000-8000-000000000001";
        let new = "00000000-0000-4000-8000-000000000002";
        let snapshot = Snapshot {
            version: 2,
            revision: new.into(),
            scripts: vec![
                Script {
                    id: old.into(),
                    version: old.into(),
                    name: "build".into(),
                    command: "make".into(),
                    run_on_worktree_create: false,
                },
                Script {
                    id: new.into(),
                    version: new.into(),
                    name: "build".into(),
                    command: "make".into(),
                    run_on_worktree_create: false,
                },
            ],
        };
        assert_eq!(
            snapshot
                .saved_selection(&[old.into()], "", "build", "make", false, false)
                .unwrap(),
            new
        );
        assert!(
            snapshot
                .saved_selection(&[old.into()], old, "build", "changed draft", false, false)
                .is_err()
        );
        assert!(
            snapshot
                .saved_selection(&[old.into()], old, "", "", false, true)
                .is_err()
        );
    }
    #[test]
    fn run_wire_retains_signal_and_server_status_even_with_exit_zero() {
        let mut value = serde_json::json!({"version":1,"runId":"00000000-0000-4000-8000-000000000001","requestId":"00000000-0000-4000-8000-000000000002","workspaceId":"workspace","cwd":"/actual/directory","script":{"id":"00000000-0000-4000-8000-000000000003","version":"00000000-0000-4000-8000-000000000004","name":"signal","command":"kill -TERM $$"},"status":"failed","startedAt":100,"finishedAt":101,"exitCode":0,"signal":15,"cancelRequested":false,"logBytes":0,"logTruncated":false,"errorCode":"SCRIPT_SIGNALLED"});
        let run: Run = decode(value.clone()).unwrap();
        run.validate("workspace").unwrap();
        assert!(run.status == Status::Failed);
        assert_eq!(run.exit_code, Some(0));
        assert_eq!(run.signal, Some(15));
        assert!(!run.status.active());
        value.as_object_mut().unwrap().remove("signal");
        assert_eq!(decode::<Run>(value.clone()).unwrap().signal, None);
        assert!(
            decode::<Run>(value)
                .unwrap()
                .validate("other-workspace")
                .is_err()
        );
    }
    #[test]
    fn abandoning_requires_exact_preflight_error_then_explicit_absent_refresh() {
        let mut recovery = Recovery::default();
        for error in [
            "CONFLICT",
            "transport: [SCRIPT_RUN_INPUT_INVALID] failure",
            "[SCRIPT_RUN_WRITE_FAILED] failed",
            "[SCRIPT_STOP_UNCONFIRMED] failed",
            "[SCRIPT_NEW_UNKNOWN] failed",
        ] {
            recovery.rejected(error);
            recovery.refreshed(true, false);
            assert!(!recovery.releasable());
        }
        for code in [
            "SCRIPT_RUN_INPUT_INVALID",
            "SCRIPT_SETTINGS_INVALID",
            "SCRIPT_SETTINGS_CONFLICT",
            "SCRIPT_SETTINGS_NOT_FOUND",
        ] {
            recovery.rejected(&format!("[{code}] Rejected before dispatch"));
            recovery.refreshed(false, false);
            assert!(!recovery.releasable());
            recovery.refreshed(true, false);
            assert!(recovery.releasable());
        }
        for code in [
            "SCRIPT_INPUT_INVALID",
            "SCRIPT_SETTINGS_CORRUPT",
            "SCRIPT_SOURCE_UNAVAILABLE",
            "SCRIPT_REQUEST_CONFLICT",
            "SCRIPT_RUN_WRITE_FAILED",
            "SQLITE_BUSY",
            "DESKTOP_ERROR",
        ] {
            recovery.rejected(&format!("[{code}] failure"));
            recovery.refreshed(true, false);
            assert!(!recovery.releasable());
        }
        recovery.rejected("[SCRIPT_RUN_INPUT_INVALID] Rejected before dispatch");
        assert!(!recovery.releasable());
        recovery.refreshed(false, false);
        assert!(!recovery.releasable());
        recovery.refreshed(true, true);
        assert!(!recovery.releasable());
        recovery.refreshed(true, false);
        assert!(recovery.releasable());
        recovery.refreshed(false, true);
        assert!(!recovery.releasable());
        recovery.refreshed(true, false);
        assert!(recovery.releasable());
        recovery.invalidate_refresh();
        assert!(!recovery.releasable());
        recovery.rejected("Backend connection closed");
        assert!(!recovery.releasable());
    }
    #[test]
    fn version_two_wire_accepts_migrated_scripts_and_an_explicit_setup_role() {
        // The backend upgrades stored v1 rows to this v2 response without
        // changing their script identity or adding an enabled setup role.
        let wire = serde_json::json!({
            "version": 2,
            "revision": "00000000-0000-4000-8000-000000000001",
            "scripts": [
                {"id":"00000000-0000-4000-8000-000000000002","version":"00000000-0000-4000-8000-000000000003","name":"Existing command","command":"printf existing"},
                {"id":"00000000-0000-4000-8000-000000000004","version":"00000000-0000-4000-8000-000000000005","name":"Setup","command":"printf setup","runOnWorktreeCreate":true}
            ]
        });
        let snapshot = decode::<Snapshot>(wire.clone())
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(snapshot.scripts.len(), 2);
        assert!(!snapshot.scripts[0].run_on_worktree_create);
        assert!(snapshot.scripts[1].run_on_worktree_create);
        for version in [0, 1, 3] {
            let mut incompatible = wire.clone();
            incompatible["version"] = version.into();
            assert!(
                decode::<Snapshot>(incompatible)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
    }
    #[test]
    fn malformed_or_unknown_settings_are_not_an_empty_editable_table() {
        assert!(
            decode::<Snapshot>(serde_json::json!({"version":1,"revision":"invalid","scripts":[]}))
                .unwrap()
                .validate()
                .is_err()
        );
        assert!(
            decode::<Snapshot>(serde_json::json!({"version":3,"revision":"x","scripts":[]}))
                .unwrap()
                .validate()
                .is_err()
        );
        assert!(
            decode::<Snapshot>(
                serde_json::json!({"version":2,"revision":"0","scripts":[],"unexpected":true})
            )
            .is_err()
        );
        assert!(
            decode::<Snapshot>(serde_json::json!({"version":2,"revision":"0","scripts":[]}))
                .unwrap()
                .validate()
                .is_ok()
        );
    }
}

#[derive(Default, Clone)]
pub struct Recovery {
    preflight_rejected: bool,
    absent_after_refresh: bool,
}
impl Recovery {
    pub fn rejected(&mut self, error: &str) {
        self.preflight_rejected = error
            .strip_prefix('[')
            .and_then(|message| message.split_once("] "))
            .is_some_and(|(code, _)| {
                matches!(
                    code,
                    "SCRIPT_RUN_INPUT_INVALID"
                        | "SCRIPT_SETTINGS_INVALID"
                        | "SCRIPT_SETTINGS_CONFLICT"
                        | "SCRIPT_SETTINGS_NOT_FOUND"
                )
            });
        self.absent_after_refresh = false;
    }
    pub fn refreshed(&mut self, manual: bool, request_exists: bool) {
        if request_exists {
            self.absent_after_refresh = false;
        } else if manual {
            self.absent_after_refresh = self.preflight_rejected;
        }
    }
    pub fn invalidate_refresh(&mut self) {
        self.absent_after_refresh = false;
    }
    pub fn releasable(&self) -> bool {
        self.preflight_rejected && self.absent_after_refresh
    }
}

#[derive(Clone, Copy)]
pub struct AttemptStamp {
    pub attempt: u64,
    pub selection: u64,
}
impl AttemptStamp {
    pub fn current(self, attempt: u64) -> bool {
        self.attempt == attempt
    }
    pub fn may_select(self, selection: u64) -> bool {
        self.selection == selection
    }
}
