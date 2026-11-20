pub(crate) mod patch_reader;
mod remotes;
mod review;
use patch_reader::PatchReader;
use crate::{
    app::HiveApp,
    focus_scroll::{Reveal, Viewport},
    language::tr,
    palette::Color,
    transport::Runtime,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonCustomVariant, ButtonVariants},
        checkbox::Checkbox,
        diff::{
            Diff, DiffChangeIndicator, DiffFile, DiffHoverHighlight, DiffHunkSeparator,
            DiffLineRange, DiffMode, DiffSide, DiffState,
        },
        input::{Textarea, TextareaState},
        scroll::ScrollableElement,
        tooltip::Tooltip,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{borrow::Cow, sync::Arc};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    project_root: String,
    repository_root: String,
    project_prefix: String,
    linked_worktree: bool,
    branch: Option<String>,
    head_oid: Option<String>,
    detached: bool,
    unborn: bool,
}
impl Identity {
    fn branch_label(&self) -> String {
        if self.unborn {
            format!(
                "{} · {}",
                self.branch.as_deref().unwrap_or("HEAD"),
                tr("尚无提交")
            )
        } else if self.detached {
            tr("分离 HEAD").to_owned()
        } else {
            self.branch.clone().unwrap_or_else(|| "HEAD".to_owned())
        }
    }
    fn description(&self) -> String {
        let branch = self.branch_label();
        format!(
            "{}: {}\n{}: {}\n{}: {}\n{}: {}\nHEAD: {}{}",
            tr("项目路径"),
            self.project_root,
            tr("仓库路径"),
            self.repository_root,
            tr("仓库内项目范围"),
            if self.project_prefix.is_empty() {
                "."
            } else {
                &self.project_prefix
            },
            tr("Git 分支"),
            branch,
            self.head_oid.as_deref().unwrap_or("—"),
            if self.linked_worktree {
                format!("\n{}", tr("链接工作树"))
            } else {
                String::new()
            }
        )
    }
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stage {
    stage: u32,
    mode: String,
    oid: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Change {
    path: String,
    repository_path: String,
    original_path: Option<String>,
    original_repository_path: Option<String>,
    path_role: Option<String>,
    kind: String,
    index_status: String,
    worktree_status: String,
    submodule: Option<String>,
    head_mode: Option<String>,
    index_mode: Option<String>,
    worktree_mode: Option<String>,
    head_oid: Option<String>,
    index_oid: Option<String>,
    rename_score: Option<String>,
    conflict_stages: Option<Vec<Stage>>,
}
impl Change {
    fn reveal_path(&self) -> Option<&str> {
        (self.path_role.as_deref() != Some("rename-source")
            && self.worktree_status != "D"
            && !matches!(self.worktree_mode.as_deref(), Some("000000" | "120000" | "160000")))
            .then_some(self.path.as_str())
    }

    fn badge(&self) -> (&'static str, &'static str) {
        if self.kind == "unmerged"
            || self.index_status == "U"
            || self.worktree_status == "U"
        {
            return ("U", "conflict");
        }
        if self.index_status == "?" || self.worktree_status == "?" {
            return ("A", "added");
        }
        if self.index_status == "D" || self.worktree_status == "D" {
            return ("D", "deleted");
        }
        if self.index_status == "R"
            || self.worktree_status == "R"
            || self.original_path.is_some()
        {
            return ("R", "renamed");
        }
        if self.index_status == "A" || self.worktree_status == "A" {
            return ("A", "added");
        }
        ("M", "modified")
    }
    fn file_name(&self) -> &str {
        self.path
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.path)
    }
    fn dir_name(&self) -> Option<&str> {
        let (dir, _) = self.path.rsplit_once('/')?;
        (!dir.is_empty()).then_some(dir)
    }
    fn has_staged(&self) -> bool {
        !matches!(self.index_status.as_str(), "" | " " | "." | "?")
    }
    fn has_unstaged(&self) -> bool {
        !matches!(self.worktree_status.as_str(), "" | " " | ".")
    }
    fn title(&self) -> String {
        let path = self.path.escape_debug().to_string();
        let rename = self
            .original_path
            .as_ref()
            .map(|source| format!("{} → ", source.escape_debug()))
            .unwrap_or_default();
        format!(
            "{}{} · {}{} · {}",
            rename, path, self.index_status, self.worktree_status, self.kind
        )
    }
    fn metadata(&self) -> String {
        let mut lines = vec![
            format!("{}: {}", tr("项目内路径"), self.path),
            format!("{}: {}", tr("仓库内路径"), self.repository_path),
            format!(
                "{}: {} · index={} · worktree={}",
                tr("Git 状态"),
                self.kind,
                self.index_status,
                self.worktree_status
            ),
        ];
        for (key, value) in [
            ("originalPath", &self.original_path),
            ("originalRepositoryPath", &self.original_repository_path),
            ("pathRole", &self.path_role),
            ("renameScore", &self.rename_score),
            ("submodule", &self.submodule),
            ("headMode", &self.head_mode),
            ("indexMode", &self.index_mode),
            ("worktreeMode", &self.worktree_mode),
            ("headOid", &self.head_oid),
            ("indexOid", &self.index_oid),
        ] {
            if let Some(value) = value {
                lines.push(format!("{key}: {value}"));
            }
        }
        if let Some(stages) = &self.conflict_stages {
            for stage in stages {
                lines.push(format!(
                    "stage {}: mode={} oid={}",
                    stage.stage, stage.mode, stage.oid
                ));
            }
        }
        lines.join("\n")
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    #[serde(flatten)]
    identity: Identity,
    entries: Vec<Change>,
    total: usize,
    offset: usize,
    limit: usize,
    has_more: bool,
    #[serde(default)]
    repository_staged_total: Option<usize>,
    complete: bool,
    warnings: Vec<String>,
    scope: String,
    submodule_details: String,
    global_configuration: String,
}
impl Status {
    fn can_preview_commit(&self) -> bool {
        // Legacy status snapshots have no repository-wide count. Let the
        // read-only backend preflight decide instead of guessing from one page.
        !self.complete || self.repository_staged_total != Some(0)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    kind: String,
    bytes: u64,
    mode: Option<String>,
    oid: Option<String>,
    sha256: Option<String>,
    text: Option<String>,
}
impl Snapshot {
    fn description(&self, label: &str) -> String {
        let mut value = format!("{label}: {} · {} bytes", self.kind, self.bytes);
        if let Some(mode) = &self.mode {
            value.push_str(&format!("\nmode: {mode}"));
        }
        if let Some(oid) = &self.oid {
            value.push_str(&format!("\noid: {oid}"));
        }
        if let Some(hash) = &self.sha256 {
            value.push_str(&format!("\nSHA-256: {hash}"));
        }
        if self.kind == "symlink" {
            if let Some(target) = &self.text {
                value.push_str(&format!("\n{}: {target}", tr("符号链接目标")));
            }
        }
        value
    }
}
#[derive(Deserialize)]
struct DiffStage {
    stage: u32,
    snapshot: Snapshot,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileDiff {
    #[serde(flatten)]
    identity: Identity,
    path: String,
    repository_path: String,
    comparison: String,
    #[serde(default)]
    before_path: Option<String>,
    #[serde(default)]
    after_path: Option<String>,
    before: Snapshot,
    after: Snapshot,
    patch: Option<SharedString>,
    binary: bool,
    complete: bool,
    limitations: Vec<String>,
    conflict_stages: Option<Vec<DiffStage>>,
    content: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum IndexOperation { Stage, Unstage }
impl IndexOperation {
    fn wire(self) -> &'static str { match self { Self::Stage => "stage", Self::Unstage => "unstage" } }
    fn confirm_label(self) -> &'static str { match self { Self::Stage => tr("确认暂存"), Self::Unstage => tr("确认撤销暂存") } }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexPreview {
    operation: IndexOperation,
    version: String,
    workspace: Identity,
    paths: Vec<String>,
    content_filters: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StagedChange {
    repository_path: String,
    original_repository_path: Option<String>,
    project_path: Option<String>,
    status: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitPreview {
    version: String,
    workspace: Identity,
    changes: Vec<StagedChange>,
    outside_project_paths: Vec<String>,
    signing_required: bool,
    hooks: Vec<String>,
    hooks_may_modify_index: bool,
}
#[derive(Deserialize)]
struct CommitPatchResponse {
    version: String,
    patch: SharedString,
}
enum CommitPatch {
    Loading { _task: Task<()> },
    Ready(Result<PatchReader, Fault>),
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitResult {
    ok: bool,
    committed: Value,
    applied: Value,
    commit_oid: Option<String>,
    observed_head_oid: Option<String>,
    index_updated: bool,
    workspace: Option<Identity>,
    #[serde(default)]
    warnings: Vec<String>,
    error: Option<Fault>,
}
fn commit_result_label(state: &str, extra_lines: &[String]) -> String {
    std::iter::once(state).chain(extra_lines.iter().map(String::as_str)).collect::<Vec<_>>().join("\n")
}
fn valid_write_state(value: &Value) -> bool {
    value == &json!(true) || value == &json!(false) || value == &json!("unknown")
}
fn git_params(mut params: Value, session: &Option<String>) -> Value {
    if let Some(session) = session { params["sessionId"] = json!(session); }
    params
}
struct PendingIndexPreview { preview: IndexPreview, selected_path: String }
#[derive(Deserialize)]
struct IndexApplyResponse {
    ok: bool,
    operation: Option<IndexOperation>,
    paths: Option<Vec<String>>,
    workspace: Option<Identity>,
    error: Option<Fault>,
    #[serde(default)]
    applied: bool,
}

#[derive(Debug, Deserialize)]
struct Fault {
    code: String,
    message: String,
}
impl Fault {
    fn transport(message: String) -> Self {
        Self {
            code: "RPC_ERROR".to_owned(),
            message,
        }
    }
    fn text(&self) -> String {
        let label = match self.code.as_str() {
            "GIT_NOT_FOUND" => tr("未找到 Git，请安装后刷新"),
            "NOT_REPOSITORY" => tr("此项目不在 Git 仓库中"),
            "UNSUPPORTED_FILTER" => tr("仓库包含不支持的 Git 过滤配置，未读取转换后的内容"),
            "FILE_LIMIT" => tr("文件超过读取大小限制"),
            "DIFF_LIMIT" | "OUTPUT_LIMIT" => tr("Git 输出超过大小限制"),
            "CONCURRENT_CHANGE" => tr("读取期间仓库发生变化，请刷新后重试"),
            "STALE_PREVIEW" => tr("暂存内容已变化，请重新预览提交。"),
            "GIT_TIMEOUT" => tr("Git 读取超时，请刷新后重试"),
            "OUTSIDE_PROJECT_RENAME" => tr("重命名源文件位于项目范围外，请从仓库根项目查看"),
            _ => tr("读取 Git 信息失败"),
        };
        format!(
            "{label}\n[{}] {}",
            self.code,
            crate::language::localize_owned_error(&self.message)
        )
    }
}
fn decode<T: serde::de::DeserializeOwned>(mut value: Value, field: &str) -> Result<T, Fault> {
    if value["ok"] == true {
        serde_json::from_value(value.get_mut(field).map(Value::take).unwrap_or_default()).map_err(|error| Fault {
            code: "INVALID_RESPONSE".to_owned(),
            message: error.to_string(),
        })
    } else {
        Err(
            serde_json::from_value(value["error"].clone()).unwrap_or_else(|_| Fault {
                code: "INVALID_RESPONSE".to_owned(),
                message: value.to_string(),
            }),
        )
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Comparison {
    Staged,
    Unstaged,
    HeadWorktree,
}
impl Comparison {
    fn wire(self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::Unstaged => "unstaged",
            Self::HeadWorktree => "head-worktree",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Staged => tr("已暂存：HEAD → 索引"),
            Self::Unstaged => tr("未暂存：索引 → 工作区"),
            Self::HeadWorktree => tr("全部：HEAD → 工作区"),
        }
    }
}

struct DiffReview {
    comment: Entity<gpui_kit::component::input::InputState>,
    selection: Option<Result<ReviewedLines, review::ReviewError>>,
}

struct ReviewedLines {
    location: String,
    patch: String,
}

pub(crate) struct GitWorkspace {
    app: WeakEntity<HiveApp>,
    runtime: Option<Arc<Runtime>>,
    workspace: String,
    session: Option<String>,
    project_name: String,
    connection: u64,
    open_request: u64,
    in_drawer: bool,
    preview_mock: bool,
    status: Option<Status>,
    status_error: Option<Fault>,
    status_loading: bool,
    status_generation: u64,
    offset: usize,
    selected: usize,
    comparison: Comparison,
    diff: Option<FileDiff>,
    diff_error: Option<Fault>,
    diff_loading: bool,
    diff_generation: u64,
    reload_diff_on_open: bool,
    body_scroll: ScrollHandle,
    files_scroll: ScrollHandle,
    file_patch_reader: Option<PatchReader>,
    diff_state: Entity<DiffState>,
    diff_raw: bool,
    ignore_whitespace: bool,
    diff_has_structured: bool,
    index_generation: u64,
    index_busy: bool,
    index_preview: Option<PendingIndexPreview>,
    index_error: Option<Fault>,
    index_notice: Option<&'static str>,
    index_refresh_required: bool,
    commit_preview: Option<CommitPreview>,
    commit_message: Entity<TextareaState>,
    commit_trigger_focus: FocusHandle,
    commit_include_outside: bool,
    commit_result: Option<CommitResult>,
    commit_scroll: ScrollHandle,
    outside_scroll: ScrollHandle,
    commit_patch: Option<CommitPatch>,
    reload_commit_patch_on_open: bool,
    remotes: Option<Entity<remotes::RemotePanel>>,
    show_remotes: bool,
    show_history: bool,
    branches: Option<crate::protocol::GitBranchesSnapshot>,
    stashes: Vec<crate::protocol::GitStashEntry>,
    commits: Vec<crate::protocol::GitCommitSummary>,
    commits_loading: bool,
    selected_commit_oid: Option<String>,
    selected_commit_diff: Option<crate::protocol::GitCommitDiffResult>,
    selected_commit_loading: bool,
    discard_confirm_path: Option<String>,
    review_open: bool,
    diff_review: Option<DiffReview>,
    branch_panel_open: bool,
    stash_panel_open: bool,
    new_branch_input: Entity<gpui_kit::component::input::InputState>,
    ops_busy: bool,
    ops_notice: Option<(bool, String)>,
}
impl HiveApp {
    fn build_git_workspace(
        &self,
        in_drawer: bool,
        preview_mock: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<GitWorkspace>> {
        let workspace = self.workspace.clone()?;
        let runtime = if preview_mock {
            self.runtime.clone()
        } else {
            if !self.connected || self.closing {
                return None;
            }
            Some(self.runtime.clone()?)
        };
        let project_name = self
            .workspaces
            .iter()
            .find(|project| project.id == workspace)
            .map(|project| project.name.clone())
            .unwrap_or_else(|| workspace.clone());
        let session = self
            .session
            .as_ref()
            .filter(|session| session.workspace_id == workspace)
            .map(|session| session.id.clone());
        let app = cx.weak_entity();
        let connection = self.connection_generation;
        let open_request = self.open_request;
        let state = cx.new(|cx| {
            if let Some(parent) = app.upgrade() {
                cx.observe(&parent, |this: &mut GitWorkspace, _, cx| {
                    if !this.current(cx) {
                        this.index_generation += 1;
                        this.index_preview = None;
                        this.clear_commit_preview();
                        if this.index_busy {
                            this.index_refresh_required = true;
                        }
                    }
                    cx.notify();
                })
                .detach();
            }
            let remotes = runtime.as_ref().map(|rt| {
                let entity = cx.new(|cx| {
                    remotes::RemotePanel::new(
                        app.clone(),
                        rt.clone(),
                        workspace.clone(),
                        session.clone(),
                        connection,
                        open_request,
                        in_drawer,
                        window,
                        cx,
                    )
                });
                cx.observe(&entity, |_, _, cx| cx.notify()).detach();
                entity
            });
            let state = GitWorkspace {
                app,
                runtime,
                workspace,
                session,
                project_name,
                connection,
                open_request,
                in_drawer,
                preview_mock,
                status: None,
                status_error: None,
                status_loading: false,
                status_generation: 0,
                offset: 0,
                selected: 0,
                comparison: Comparison::Unstaged,
                diff: None,
                diff_error: None,
                diff_loading: false,
                diff_generation: 0,
                reload_diff_on_open: false,
                body_scroll: ScrollHandle::new(),
                files_scroll: ScrollHandle::new(),
                file_patch_reader: None,
                diff_state: cx.new(|cx| {
                    let prefs = crate::diffs_settings::current(cx);
                    DiffState::new([], cx)
                        .with_mode(prefs.default_diff_mode.into_diff_mode())
                        .with_inline_unit(prefs.inline_diff.into_inline_unit())
                        .with_inline_max_line_length(
                            crate::diffs_settings::INLINE_DIFF_MAX_LINE_BYTES,
                        )
                }),
                diff_raw: false,
                ignore_whitespace: crate::diffs_settings::current(cx).ignore_whitespace,
                diff_has_structured: false,
                index_generation: 0,
                index_busy: false,
                index_preview: None,
                index_error: None,
                index_notice: None,
                index_refresh_required: false,
                commit_preview: None,
                commit_message: cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .auto_grow(2, 5)
                        .placeholder(tr("提交说明"))
                }),
                commit_trigger_focus: cx.focus_handle(),
                commit_include_outside: false,
                commit_result: None,
                commit_scroll: ScrollHandle::new(),
                outside_scroll: ScrollHandle::new(),
                commit_patch: None,
                reload_commit_patch_on_open: false,
                remotes,
                show_remotes: false,
                show_history: false,
                branches: None,
                stashes: vec![],
                commits: vec![],
                commits_loading: false,
                selected_commit_oid: None,
                selected_commit_diff: None,
                selected_commit_loading: false,
                discard_confirm_path: None,
                review_open: false,
                diff_review: None,
                branch_panel_open: false,
                stash_panel_open: false,
                new_branch_input: cx.new(|cx| {
                    gpui_kit::component::input::InputState::new(window, cx)
                        .placeholder(tr("输入新分支名，如 feat/login"))
                }),
                ops_busy: false,
                ops_notice: None,
            };
            if !preview_mock {
                let pending = cx.weak_entity();
                cx.defer(move |cx| {
                    let _ = pending.update(cx, |state, cx| state.refresh(cx));
                });
            }
            state
        });
        Some(state)
    }

    pub(crate) fn sync_git_workspace(&mut self, cx: &mut Context<Self>) {
        let matches = self.git_workspace.as_ref().is_some_and(|entity| {
            let ws = entity.read(cx);
            ws.preview_mock || ws.matches_app(self)
        });
        if !matches {
            self.git_workspace = None;
        }
    }

    pub(crate) fn ensure_git_workspace(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<GitWorkspace>> {
        self.sync_git_workspace(cx);
        if self.git_workspace.is_none() {
            self.git_workspace = self.build_git_workspace(true, false, window, cx);
        }
        self.git_workspace.clone()
    }

    pub(crate) fn open_git_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected || self.closing || self.workspace.is_none() {
            return;
        }
        self.git_drawer_open = true;
        if window.viewport_size().width < px(820.) {
            self.sidebar_open = false;
        }
        if let Some(state) = self.ensure_git_workspace(window, cx) {
            // The drawer validates its selection against HiveApp. Wait until
            // the parent is readable again before calling into the drawer.
            let drawer = state.downgrade();
            cx.defer(move |cx| {
                let _ = drawer.update(cx, |ws, cx| {
                    if ws.status.is_none() && !ws.status_loading {
                        ws.refresh(cx);
                    } else if ws.reload_diff_on_open {
                        ws.load_diff(cx);
                    }
                    if std::mem::take(&mut ws.reload_commit_patch_on_open) {
                        ws.load_commit_patch(cx);
                    }
                });
            });
        }
        cx.notify();
    }

    pub(crate) fn toggle_git_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.git_drawer_open {
            self.close_git_workspace(cx);
        } else {
            self.open_git_workspace(window, cx);
        }
    }

    fn close_git_workspace(&mut self, cx: &mut Context<Self>) {
        self.git_drawer_open = false;
        if let Some(state) = &self.git_workspace {
            state.update(cx, |ws, cx| {
                if !ws.preview_mock && (ws.diff.is_some() || ws.diff_loading || ws.diff_error.is_some()) {
                    ws.reload_diff_on_open = true;
                    ws.clear_file_diff(cx);
                }
                if ws.commit_patch.is_some() {
                    ws.hide_commit_patch();
                    ws.reload_commit_patch_on_open = true;
                }
            });
        }
        cx.notify();
    }

    pub(crate) fn open_git_workspace_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected || self.closing || window.has_active_dialog(cx) {
            return;
        }
        let Some(state) = self.build_git_workspace(false, false, window, cx) else {
            return;
        };
        let width = (window.viewport_size().width - px(40.)).min(px(1000.));
        window.open_dialog(cx, move |dialog, _, cx| {
            let busy = state.read(cx).busy(cx);
            let cancel_guard = state.clone();
            dialog
                .title(tr("Git 状态与差异"))
                .w(width)
                .on_ok(|_, _, _| false)
                .keyboard(!busy)
                .close_button(!busy)
                .overlay_closable(!busy)
                .on_cancel(move |_, _, cx| !cancel_guard.read(cx).busy(cx))
                .child(state.clone())
        });
    }

    #[cfg(feature = "ui-preview")]
    pub(crate) fn preview_seed_git_workspace(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.build_git_workspace(true, true, window, cx) else {
            return;
        };
        let patch = concat!(
            "--- a/apps/desktop/src/view.rs\n",
            "+++ b/apps/desktop/src/view.rs\n",
            "@@ -1242,7 +1242,9 @@\n",
            "                     .when(!narrow || !self.sidebar_open, |view| {\n",
            "-                        view.child(if self.terminal.visible {\n",
            "+                        // Render chat alongside bottom terminal and right Git drawer\n",
            "+                        view.child(if false {\n",
            "                             self.render_terminal(cx).into_any_element()\n",
            "                         } else {\n"
        );
        state.update(cx, |ws, cx| {
            ws.status = Some(Status {
                identity: Identity {
                    project_root: "/Users/demo/hive-desktop".into(),
                    repository_root: "/Users/demo/hive-desktop".into(),
                    project_prefix: String::new(),
                    linked_worktree: false,
                    branch: Some("feat/t3-ui-parity".into()),
                    head_oid: Some("9f4a21c8e".into()),
                    detached: false,
                    unborn: false,
                },
                entries: vec![
                    Change {
                        path: "apps/desktop/src/view.rs".into(),
                        repository_path: "apps/desktop/src/view.rs".into(),
                        original_path: None,
                        original_repository_path: None,
                        path_role: None,
                        kind: "modified".into(),
                        index_status: "M".into(),
                        worktree_status: "M".into(),
                        submodule: None,
                        head_mode: Some("100644".into()),
                        index_mode: Some("100644".into()),
                        worktree_mode: Some("100644".into()),
                        head_oid: None,
                        index_oid: None,
                        rename_score: None,
                        conflict_stages: None,
                    },
                    Change {
                        path: "apps/desktop/src/git_workspace.rs".into(),
                        repository_path: "apps/desktop/src/git_workspace.rs".into(),
                        original_path: None,
                        original_repository_path: None,
                        path_role: None,
                        kind: "modified".into(),
                        index_status: " ".into(),
                        worktree_status: "M".into(),
                        submodule: None,
                        head_mode: None,
                        index_mode: None,
                        worktree_mode: None,
                        head_oid: None,
                        index_oid: None,
                        rename_score: None,
                        conflict_stages: None,
                    },
                    Change {
                        path: "apps/desktop/src/settings/general.rs".into(),
                        repository_path: "apps/desktop/src/settings/general.rs".into(),
                        original_path: None,
                        original_repository_path: None,
                        path_role: None,
                        kind: "untracked".into(),
                        index_status: "?".into(),
                        worktree_status: "?".into(),
                        submodule: None,
                        head_mode: None,
                        index_mode: None,
                        worktree_mode: None,
                        head_oid: None,
                        index_oid: None,
                        rename_score: None,
                        conflict_stages: None,
                    },
                ],
                total: 3,
                repository_staged_total: Some(1),
                offset: 0,
                limit: 100,
                has_more: false,
                complete: true,
                warnings: Vec::new(),
                scope: "project".into(),
                submodule_details: "none".into(),
                global_configuration: "standard".into(),
            });
            ws.diff = Some(FileDiff {
                identity: ws.status.as_ref().unwrap().identity.clone(),
                path: "apps/desktop/src/view.rs".into(),
                repository_path: "apps/desktop/src/view.rs".into(),
                comparison: "unstaged".into(),
                before_path: Some("apps/desktop/src/view.rs".into()),
                after_path: Some("apps/desktop/src/view.rs".into()),
                before: Snapshot {
                    kind: "text".into(),
                    bytes: 4096,
                    mode: Some("100644".into()),
                    oid: None,
                    sha256: None,
                    text: None,
                },
                after: Snapshot {
                    kind: "text".into(),
                    bytes: 4160,
                    mode: Some("100644".into()),
                    oid: None,
                    sha256: None,
                    text: None,
                },
                patch: Some(patch.into()),
                binary: false,
                complete: true,
                limitations: Vec::new(),
                conflict_stages: None,
                content: "raw".into(),
            });
            ws.sync_diff_state(false, cx);
        });
        self.git_workspace = Some(state);
        self.git_drawer_open = true;
    }
}
impl GitWorkspace {
    fn busy(&self, cx: &App) -> bool {
        self.index_busy || self.remotes.as_ref().is_some_and(|r| r.read(cx).busy)
    }
    fn matches_app(&self, app: &HiveApp) -> bool {
        let session = app
            .session
            .as_ref()
            .filter(|session| session.workspace_id == self.workspace)
            .map(|session| &session.id);
        app.workspace.as_ref() == Some(&self.workspace)
            && app.connection_generation == self.connection
            && app.open_request == self.open_request
            && session == self.session.as_ref()
            && match (&app.runtime, &self.runtime) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => self.preview_mock,
                _ => false,
            }
    }
    fn current(&self, cx: &App) -> bool {
        if self.preview_mock {
            return true;
        }
        self.app.upgrade().is_some_and(|app| {
            let app = app.read(cx);
            app.connected && !app.closing && self.matches_app(app)
        })
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.preview_mock || self.busy(cx) || !self.current(cx) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        if let Some(remotes) = &self.remotes {
            remotes.update(cx, |remote, cx| remote.invalidate(cx));
        }
        self.index_generation += 1;
        self.index_preview = None;
        self.clear_commit_preview();
        self.status_generation += 1;
        self.reload_diff_on_open = false;
        let generation = self.status_generation;
        let offset = self.offset;
        let selected_path = self.selected_change().map(|change| change.path.clone());
        self.status = None;
        self.status_error = None;
        self.status_loading = true;
        self.clear_file_diff(cx);
        self.files_scroll = ScrollHandle::new();
        self.load_branches_and_stashes(cx);
        cx.notify();
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| {
                    if this.status_generation != generation {
                        return false;
                    }
                    if !this.current(cx) {
                        this.status_loading = false;
                        cx.notify();
                        return false;
                    }
                    true
                })
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request(
                    "git.status",
                    git_params(json!({"workspaceId":workspace,"offset":offset,"limit":100}), &session),
                )
                .await
                .map_err(Fault::transport)
                .and_then(|value| decode::<Status>(value, "status"));
            let _ = this.update(cx, |this, cx| {
                if this.status_generation != generation {
                    return;
                }
                this.status_loading = false;
                if !this.current(cx) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(status) => {
                        this.offset = status.offset;
                        this.selected = selected_path
                            .as_ref()
                            .and_then(|path| {
                                status
                                    .entries
                                    .iter()
                                    .position(|change| &change.path == path)
                            })
                            .unwrap_or(0);
                        this.status = Some(status);
                        this.index_refresh_required = false;
                        this.index_error = None;
                        if this.index_notice == Some("Git 索引已更改，请刷新状态后再操作。") {
                            this.index_notice = None;
                        }
                    }
                    Err(error) => this.status_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn selected_change(&self) -> Option<&Change> {
        self.status.as_ref()?.entries.get(self.selected)
    }

    fn reveal_selected_file(&self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let source = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            let Some(source) = source.upgrade() else { return; };
            let source = source.read(cx);
            if source.preview_mock
                || !source.current(cx)
                || source.busy(cx)
                || source.selected_change().and_then(Change::reveal_path) != Some(path.as_str())
            {
                return;
            }
            let app = source.app.clone();
            if source.in_drawer {
                if window.has_active_dialog(cx) {
                    return;
                }
            } else {
                window.close_dialog(cx);
            }
            let _ = app.update(cx, |app, cx| {
                if app.git_drawer_open {
                    app.close_git_workspace(cx);
                }
                app.reveal_project_file(path, window, cx);
            });
        });
    }
    fn select(&mut self, index: usize, load: bool, cx: &mut Context<Self>) {
        if self.status_loading
            || self.index_busy
            || !self.current(cx)
            || self
                .status
                .as_ref()
                .is_none_or(|status| index >= status.entries.len())
        {
            return;
        }
        self.selected = index;
        self.index_generation += 1;
        self.index_preview = None;
        self.clear_commit_preview();
        self.index_error = None;
        self.index_notice = None;
        self.reload_diff_on_open = false;
        self.clear_file_diff(cx);
        self.files_scroll.scroll_to_item(index);
        cx.notify();
        if load {
            self.load_diff(cx);
        }
    }
    fn clear_file_diff(&mut self, cx: &mut Context<Self>) {
        self.diff_generation += 1;
        self.diff = None;
        self.review_open = false;
        self.diff_review = None;
        if std::mem::take(&mut self.diff_has_structured) {
            self.diff_state.update(cx, |state, cx| state.set_files([], cx));
        }
        self.diff_error = None;
        self.diff_loading = false;
        self.file_patch_reader = None;
    }

    fn load_diff(&mut self, cx: &mut Context<Self>) {
        if self.preview_mock || !self.current(cx) || self.status_loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let Some(path) = self.selected_change().map(|change| change.path.clone()) else {
            return;
        };
        self.reload_diff_on_open = false;
        self.clear_file_diff(cx);
        let generation = self.diff_generation;
        self.diff_loading = true;
        cx.notify();
        let workspace = self.workspace.clone();
        let comparison = self.comparison.wire();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "git.diff",
                    git_params(json!({"workspaceId":workspace,"path":path,"comparison":comparison}), &session),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.diff_generation != generation {
                    return;
                }
                this.diff_loading = false;
                if !this.current(cx) {
                    cx.notify();
                    return;
                }
                match result.map_err(Fault::transport).and_then(|value| decode::<FileDiff>(value, "diff")) {
                    Ok(diff) => {
                        this.diff = Some(diff);
                        let collapse =
                            crate::diffs_settings::current(cx).collapse_multi_file_diffs;
                        this.sync_diff_state(collapse, cx);
                    }
                    Err(error) => this.diff_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn sync_diff_state(&mut self, collapse_multi: bool, cx: &mut App) {
        self.file_patch_reader = None;
        let Some(patch) = self.diff.as_ref().and_then(|diff| diff.patch.as_deref()) else {
            self.diff_has_structured = false;
            self.diff_state.update(cx, |s, cx| s.set_files([], cx));
            return;
        };
        if patch.trim().is_empty() {
            self.diff_has_structured = false;
            self.diff_state.update(cx, |s, cx| s.set_files([], cx));
            return;
        }
        let effective = if self.ignore_whitespace {
            Cow::Owned(crate::diffs_settings::filter_patch_whitespace(patch))
        } else {
            Cow::Borrowed(patch)
        };
        if effective.trim().is_empty() {
            self.diff_has_structured = false;
            self.diff_state.update(cx, |s, cx| s.set_files([], cx));
            return;
        }
        match DiffFile::parse(&effective) {
            Ok(files) if !files.is_empty() => {
                let paths: Vec<String> = files.iter().map(|f| f.path().to_string()).collect();
                self.diff_state.update(cx, |s, cx| {
                    s.set_files(files, cx);
                    if collapse_multi && paths.len() > 1 {
                        for path in paths {
                            s.set_file_collapsed(&path, true, cx);
                        }
                    }
                });
                self.diff_has_structured = true;
            }
            _ => {
                self.diff_has_structured = false;
                self.diff_state.update(cx, |s, cx| s.set_files([], cx));
            }
        }
    }
    fn preview_index(&mut self, operation: IndexOperation, cx: &mut Context<Self>) {
        if !self.current(cx) || self.status_loading || self.busy(cx) || self.index_refresh_required { return; }
        let Some(runtime) = self.runtime.clone() else { return; };
        let Some(path) = self.selected_change().map(|change| change.path.clone()) else { return; };
        self.index_generation += 1;
        let generation = self.index_generation;
        self.index_busy = true;
        self.index_preview = None;
        self.clear_commit_preview();
        self.index_error = None;
        self.index_notice = None;
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this.update(cx, |this, cx| {
                let current = this.index_generation == generation && this.current(cx);
                if !current { this.index_busy = false; cx.notify(); }
                current
            }).ok() != Some(true) { return; }
            let result = runtime.request("git.index.preview", git_params(json!({"workspaceId":workspace,"operation":operation.wire(),"paths":[path]}), &session)).await
                .map_err(Fault::transport).and_then(|value| decode::<IndexPreview>(value, "preview"));
            let _ = this.update(cx, |this, cx| {
                this.index_busy = false;
                if this.index_generation != generation || !this.current(cx) { cx.notify(); return; }
                match result {
                    Ok(preview) if preview.operation == operation && !preview.version.is_empty() && preview.paths.contains(&path)
                        && preview.workspace.project_root == this.status.as_ref().map(|s| s.identity.project_root.as_str()).unwrap_or("") => {
                        this.index_preview = Some(PendingIndexPreview { preview, selected_path: path });
                    }
                    Ok(_) => this.index_error = Some(Fault { code: "INVALID_RESPONSE".into(), message: "Preview does not match the selected project and file".into() }),
                    Err(error) => this.index_error = Some(error),
                }
                cx.notify();
            });
        }).detach();
    }
    fn apply_index(&mut self, cx: &mut Context<Self>) {
        if !self.current(cx) || self.busy(cx) || self.index_refresh_required { return; }
        let Some(runtime) = self.runtime.clone() else { return; };
        let Some(pending) = self.index_preview.take() else { return; };
        if self.selected_change().map(|change| &change.path) != Some(&pending.selected_path) { return; }
        self.index_generation += 1;
        let generation = self.index_generation;
        self.index_busy = true;
        self.index_error = None;
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        let operation = pending.preview.operation;
        let path = pending.selected_path;
        let version = pending.preview.version;
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this.update(cx, |this, cx| {
                let current = this.index_generation == generation && this.current(cx);
                if !current { this.index_busy = false; cx.notify(); }
                current
            }).ok() != Some(true) { return; }
            let result = runtime.request("git.index.apply", git_params(json!({"workspaceId":workspace,"operation":operation.wire(),"paths":[path],"expectedVersion":version}), &session)).await
                .map_err(Fault::transport).and_then(|value| serde_json::from_value::<IndexApplyResponse>(value).map_err(|error| Fault {code:"INVALID_RESPONSE".into(),message:error.to_string()}));
            let _ = this.update(cx, |this, cx| {
                this.index_busy = false;
                if this.index_generation != generation || !this.current(cx) { cx.notify(); return; }
                match result {
                    Ok(response) if response.ok && response.operation == Some(operation)
                        && response.paths.as_ref().is_some_and(|paths| paths.contains(&path))
                        && response.workspace.as_ref().is_some_and(|workspace| workspace.project_root == this.status.as_ref().map(|s| s.identity.project_root.as_str()).unwrap_or("")) => {
                        this.index_notice = Some("Git 索引已更新。");
                        this.index_refresh_required = true;
                        this.refresh(cx);
                    }
                    Ok(response) if !response.ok => {
                        this.index_refresh_required = response.applied || response.error.is_none();
                        if response.applied { this.index_notice = Some("Git 索引已更改，请刷新状态后再操作。"); }
                        this.index_error = Some(response.error.unwrap_or(Fault {code:"INVALID_RESPONSE".into(), message:"Missing error details".into()}));
                    }
                    Ok(_) => {
                        this.index_refresh_required = true;
                        this.index_error = Some(Fault { code: "INVALID_RESPONSE".into(), message: "Could not confirm the index operation response".into() });
                    }
                    Err(error) => {
                        this.index_refresh_required = true;
                        this.index_error = Some(error);
                    }
                }
                cx.notify();
            });
        }).detach();
    }
    fn preview_commit(&mut self, cx: &mut Context<Self>) {
        if !self.current(cx) || self.status_loading || self.busy(cx) || self.index_refresh_required || self.status.is_none() { return; }
        let Some(runtime) = self.runtime.clone() else { return; };
        self.index_generation += 1;
        let generation = self.index_generation;
        self.index_busy = true;
        self.index_preview = None;
        self.clear_commit_preview();
        self.commit_result = None;
        self.index_error = None;
        self.index_notice = None;
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this.update(cx, |this, cx| {
                let valid = this.index_generation == generation && this.current(cx);
                if !valid { this.index_busy = false; cx.notify(); }
                valid
            }).ok() != Some(true) { return; }
            let result = runtime.request("git.commit.preview", git_params(json!({"workspaceId":workspace,"includePatch":false}), &session)).await
                .map_err(Fault::transport).and_then(|value| decode::<CommitPreview>(value, "preview"));
            let _ = this.update(cx, |this, cx| {
                this.index_busy = false;
                if this.index_generation != generation || !this.current(cx) { cx.notify(); return; }
                match result {
                    Ok(preview) if !preview.version.is_empty() && !preview.changes.is_empty()
                        && preview.workspace.project_root == this.status.as_ref().map(|s| s.identity.project_root.as_str()).unwrap_or("") => this.commit_preview = Some(preview),
                    Ok(_) => this.index_error = Some(Fault { code: "INVALID_RESPONSE".into(), message: "Commit preview does not match the selected project".into() }),
                    Err(error) => this.index_error = Some(error),
                }
                cx.notify();
            });
        }).detach();
    }
    fn hide_commit_patch(&mut self) {
        self.commit_patch = None;
        self.reload_commit_patch_on_open = false;
    }
    fn clear_commit_preview(&mut self) {
        self.commit_preview = None;
        self.commit_include_outside = false;
        self.hide_commit_patch();
    }
    fn load_commit_patch(&mut self, cx: &mut Context<Self>) {
        if self.commit_patch.is_some() || !self.current(cx) {
            return;
        }
        let (Some(preview), Some(runtime)) = (&self.commit_preview, self.runtime.clone()) else {
            return;
        };
        let version = preview.version.clone();
        let params = git_params(
            json!({"workspaceId": self.workspace, "includePatch":true, "expectedVersion":version}),
            &self.session,
        );
        let task = cx.spawn(async move |this, cx| {
            let result = runtime
                .request("git.commit.preview", params)
                .await
                .map_err(Fault::transport)
                .and_then(|value| decode::<CommitPatchResponse>(value, "preview"))
                .and_then(|response| {
                    if response.version != version {
                        Err(Fault {
                            code: "STALE_PREVIEW".into(),
                            message: "Patch response did not match the reviewed commit version".into(),
                        })
                    } else {
                        Ok(PatchReader::new(response.patch))
                    }
                });
            let _ = this.update(cx, |this, cx| {
                if this.current(cx) {
                    this.commit_patch = Some(CommitPatch::Ready(result));
                } else {
                    this.hide_commit_patch();
                }
                cx.notify();
            });
        });
        self.commit_patch = Some(CommitPatch::Loading { _task: task });
        cx.notify();
    }
    fn staged_patch_reader(&mut self) -> Option<&mut PatchReader> {
        match self.commit_patch.as_mut() {
            Some(CommitPatch::Ready(Ok(reader))) => Some(reader),
            _ => None,
        }
    }
    fn apply_commit(&mut self, cx: &mut Context<Self>) {
        if !self.current(cx) || self.busy(cx) || self.status_loading || self.index_refresh_required
            || matches!(self.commit_patch, Some(CommitPatch::Loading { .. } | CommitPatch::Ready(Err(_)))) { return; }
        let Some(runtime) = self.runtime.clone() else { return; };
        let message = self.commit_message.read(cx).value().to_string();
        if message.trim().is_empty() {
            self.index_error = Some(Fault { code: "INVALID_MESSAGE".into(), message: tr("请输入提交说明。").into() });
            cx.notify();
            return;
        }
        let Some(preview) = self.commit_preview.as_ref() else { return; };
        if !preview.outside_project_paths.is_empty() && !self.commit_include_outside { return; }
        let version = preview.version.clone();
        let include_outside = self.commit_include_outside;
        self.clear_commit_preview();
        self.index_generation += 1;
        let generation = self.index_generation;
        self.index_busy = true;
        self.index_error = None;
        self.commit_result = None;
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this.update(cx, |this, cx| {
                let valid = this.index_generation == generation && this.current(cx);
                if !valid { this.index_busy = false; cx.notify(); }
                valid
            }).ok() != Some(true) { return; }
            let result = runtime.request("git.commit.apply", git_params(json!({"workspaceId":workspace,"expectedVersion":version,"message":message,"includeOutsideProject":include_outside}), &session)).await
                .map_err(Fault::transport).and_then(|value| serde_json::from_value::<CommitResult>(value).map_err(|error| Fault {code:"INVALID_RESPONSE".into(),message:error.to_string()}));
            let _ = this.update(cx, |this, cx| {
                this.index_busy = false;
                let current = this.index_generation == generation && this.current(cx);
                match result {
                    Ok(result) if valid_write_state(&result.committed) && valid_write_state(&result.applied) => {
                        let complete = result.ok && result.committed == json!(true) && result.applied == json!(true) && result.index_updated;
                        this.index_refresh_required = true;
                        this.commit_result = Some(result);
                        if complete && current { this.refresh(cx); }
                    }
                    result => {
                        let fault = match result { Err(fault) => fault, _ => Fault {code:"INVALID_RESPONSE".into(), message:"Invalid commit outcome".into()} };
                        this.index_refresh_required = true;
                        this.commit_result = Some(CommitResult { ok:false, committed:json!("unknown"), applied:json!("unknown"), commit_oid:None, observed_head_oid:None, index_updated:false, workspace:None, warnings:vec![], error:Some(fault) });
                    }
                }
                cx.notify();
            });
        }).detach();
    }
    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let m = event.keystroke.modifiers;
        if m.control || m.platform || m.alt || m.shift {
            return;
        }
        let len = self
            .status
            .as_ref()
            .map(|status| status.entries.len())
            .unwrap_or(0);
        if len == 0 {
            return;
        }
        match event.keystroke.key.as_str() {
            "down" => self.select((self.selected + 1) % len, false, cx),
            "up" => self.select((self.selected + len - 1) % len, false, cx),
            "enter" => self.load_diff(cx),
            _ => return,
        }
        cx.stop_propagation();
    }
    fn load_branches_and_stashes(&mut self, cx: &mut Context<Self>) {
        if self.preview_mock || !self.current(cx) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let branches_res = runtime
                .request(
                    "git.branches",
                    git_params(json!({ "workspaceId": workspace }), &session),
                )
                .await
                .ok()
                .and_then(|v| decode::<crate::protocol::GitBranchesSnapshot>(v, "branches").ok());
            let stashes_res = runtime
                .request(
                    "git.stashList",
                    git_params(json!({ "workspaceId": workspace }), &session),
                )
                .await
                .ok()
                .and_then(|v| decode::<Vec<crate::protocol::GitStashEntry>>(v, "stashes").ok());
            let _ = this.update(cx, |this, cx| {
                if let Some(branches) = branches_res {
                    this.branches = Some(branches);
                }
                if let Some(stashes) = stashes_res {
                    this.stashes = stashes;
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_git_branch_op(
        &mut self,
        method: &'static str,
        payload: serde_json::Value,
        success_fallback: &'static str,
        close_panels: bool,
        cx: &mut Context<Self>,
    ) {
        if self.preview_mock || self.ops_busy || !self.current(cx) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.ops_busy = true;
        self.ops_notice = None;
        cx.notify();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let res = runtime.request(method, git_params(payload, &session)).await;
            let _ = this.update(cx, |this, cx| {
                this.ops_busy = false;
                match res {
                    Ok(value) => {
                        if value.get("ok").and_then(|v| v.as_bool()) == Some(false) {
                            let msg = value
                                .get("error")
                                .and_then(|e| e.get("message"))
                                .and_then(|m| m.as_str())
                                .unwrap_or("Git 操作失败")
                                .to_owned();
                            this.ops_notice = Some((true, msg));
                        } else {
                            let summary = value
                                .get("summary")
                                .and_then(|s| s.as_str())
                                .filter(|s| !s.trim().is_empty())
                                .map(str::to_owned)
                                .unwrap_or_else(|| tr(success_fallback).to_owned());
                            this.ops_notice = Some((false, summary));
                            if close_panels {
                                this.branch_panel_open = false;
                                this.stash_panel_open = false;
                            }
                            this.refresh(cx);
                        }
                    }
                    Err(err) => {
                        this.ops_notice = Some((true, err.to_string()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_branch_stash_toolbar(&self, current: bool, cx: &Context<Self>) -> AnyElement {
        if self.preview_mock || !current {
            return div().into_any_element();
        }
        let current_branch = self
            .branches
            .as_ref()
            .and_then(|b| b.current.clone())
            .or_else(|| self.status.as_ref().map(|s| s.identity.branch_label()))
            .unwrap_or_else(|| tr("分支").to_owned());
        let branch_button_label: String = if current_branch.chars().count() > 20 {
            format!("{}…", current_branch.chars().take(19).collect::<String>())
        } else {
            current_branch.clone()
        };
        let stash_count = self.stashes.len();
        let ws_id = self.workspace.clone();
        let ws_fetch = ws_id.clone();
        let ws_pull = ws_id.clone();
        let ws_stash_push = ws_id.clone();

        v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_1p5()
                    .px_2p5()
                    .py_1p5()
                    .rounded_lg()
                    .bg(Color::Inset.get(cx))
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("git-branch-switcher-toggle")
                                    .outline()
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .icon(Glyph::GitBranch)
                                    .label(branch_button_label)
                                    .tooltip(current_branch.clone())
                                    .accessibility_label(current_branch)
                                    .disabled(self.ops_busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.branch_panel_open = !this.branch_panel_open;
                                        if this.branch_panel_open {
                                            this.stash_panel_open = false;
                                            this.load_branches_and_stashes(cx);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("git-fetch-btn")
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .label("Fetch")
                                    .disabled(self.ops_busy)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.run_git_branch_op(
                                            "git.fetch",
                                            json!({ "workspaceId": ws_fetch, "prune": true }),
                                            "已完成 Fetch",
                                            false,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("git-pull-btn")
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .label("Pull")
                                    .disabled(self.ops_busy)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.run_git_branch_op(
                                            "git.pull",
                                            json!({ "workspaceId": ws_pull, "rebase": false }),
                                            "已完成 Pull",
                                            false,
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("git-stash-push-btn")
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .label(tr("挂起改动"))
                                    .disabled(self.ops_busy)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.run_git_branch_op(
                                            "git.stashPush",
                                            json!({ "workspaceId": ws_stash_push, "includeUntracked": true }),
                                            "已挂起当前改动到 Stash",
                                            false,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("git-stash-list-toggle")
                                    .outline()
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .px_2()
                                    .rounded_md()
                                    .label(if stash_count > 0 {
                                        format!("Stash ({stash_count})")
                                    } else {
                                        "Stash".to_owned()
                                    })
                                    .disabled(self.ops_busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.stash_panel_open = !this.stash_panel_open;
                                        if this.stash_panel_open {
                                            this.branch_panel_open = false;
                                            this.load_branches_and_stashes(cx);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .when_some(self.ops_notice.as_ref(), |view, (is_err, msg)| {
                view.child(
                    div()
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .bg(if *is_err {
                            Color::ErrorSurface.get(cx)
                        } else {
                            Color::Inset.get(cx)
                        })
                        .text_xs()
                        .text_color(if *is_err {
                            Color::Error.get(cx)
                        } else {
                            Color::Secondary.get(cx)
                        })
                        .child(msg.clone()),
                )
            })
            .when(self.branch_panel_open, |view| {
                let ws_create = ws_id.clone();
                let new_branch_input = self.new_branch_input.clone();
                let local_branches = self
                    .branches
                    .as_ref()
                    .map(|b| b.local.clone())
                    .unwrap_or_default();
                view.child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .p_2p5()
                        .rounded_lg()
                        .bg(Color::Card.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .shadow(crate::palette::card_shadow(cx))
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(gpui_kit::component::input::Input::new(&new_branch_input).small()),
                                )
                                .child(
                                    Button::new("git-create-branch-submit")
                                        .primary()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .px_2p5()
                                        .rounded_md()
                                        .label(tr("新建并切换"))
                                        .disabled(self.ops_busy)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            let name = this.new_branch_input.read(cx).value().trim().to_owned();
                                            if name.is_empty() {
                                                return;
                                            }
                                            this.new_branch_input.update(cx, |input, cx| {
                                                input.set_value("", window, cx);
                                            });
                                            this.run_git_branch_op(
                                                "git.checkoutBranch",
                                                json!({
                                                    "workspaceId": ws_create,
                                                    "branch": name,
                                                    "create": true,
                                                }),
                                                "已创建并切换到新分支",
                                                true,
                                                cx,
                                            );
                                        })),
                                ),
                        )
                        .child(
                            v_flex()
                                .w_full()
                                .gap_1()
                                .children(local_branches.into_iter().take(16).enumerate().map(|(i, b)| {
                                    let ws_sw = ws_id.clone();
                                    let b_name = b.name.clone();
                                    let is_cur = b.is_current;
                                    h_flex()
                                        .id(("git-branch-item", i))
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(if is_cur {
                                            Color::AccentSoft.get(cx)
                                        } else {
                                            Color::Inset.get(cx)
                                        })
                                        .child(
                                            h_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .items_center()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(if is_cur {
                                                            FontWeight::SEMIBOLD
                                                        } else {
                                                            FontWeight::NORMAL
                                                        })
                                                        .text_color(Color::Text.get(cx))
                                                        .truncate()
                                                        .child(b.name.clone()),
                                                )
                                                .when(b.ahead > 0 || b.behind > 0, |row| {
                                                    row.child(
                                                        div()
                                                            .text_size(px(10.5))
                                                            .text_color(Color::Muted.get(cx))
                                                            .child(format!("↑{} ↓{}", b.ahead, b.behind)),
                                                    )
                                                }),
                                        )
                                        .child(if is_cur {
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(Color::Accent.get(cx))
                                                .child(tr("当前"))
                                                .into_any_element()
                                        } else {
                                            Button::new(("git-branch-switch", i))
                                                .custom(crate::palette::subtle_icon_variant(cx))
                                                .compact()
                                                .xsmall()
                                                .h(px(22.))
                                                .px_2()
                                                .rounded_sm()
                                                .label(tr("切换"))
                                                .disabled(self.ops_busy)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.run_git_branch_op(
                                                        "git.checkoutBranch",
                                                        json!({
                                                            "workspaceId": ws_sw,
                                                            "branch": b_name,
                                                            "create": false,
                                                        }),
                                                        "已切换分支",
                                                        true,
                                                        cx,
                                                    );
                                                }))
                                                .into_any_element()
                                        })
                                })),
                        ),
                )
            })
            .when(self.stash_panel_open, |view| {
                view.child(
                    v_flex()
                        .w_full()
                        .gap_1p5()
                        .p_2p5()
                        .rounded_lg()
                        .bg(Color::Card.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .shadow(crate::palette::card_shadow(cx))
                        .when(self.stashes.is_empty(), |col| {
                            col.child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("当前没有已挂起的 Stash 记录。")),
                            )
                        })
                        .children(self.stashes.iter().take(12).enumerate().map(|(i, s)| {
                            let ws_pop = ws_id.clone();
                            let ws_drop = ws_id.clone();
                            let ref_pop = s.ref_name.clone();
                            let ref_drop = s.ref_name.clone();
                            h_flex()
                                .id(("git-stash-item", i))
                                .w_full()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(Color::Inset.get(cx))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Text.get(cx))
                                                .truncate()
                                                .child(s.message.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(Color::Muted.get(cx))
                                                .child(format!("{} · {}", s.ref_name, s.relative_time)),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            Button::new(("git-stash-pop", i))
                                                .custom(crate::palette::subtle_icon_variant(cx))
                                                .compact()
                                                .xsmall()
                                                .h(px(22.))
                                                .px_2()
                                                .rounded_sm()
                                                .label(tr("弹出"))
                                                .disabled(self.ops_busy)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.run_git_branch_op(
                                                        "git.stashPop",
                                                        json!({ "workspaceId": ws_pop, "ref": ref_pop }),
                                                        "已弹出 Stash",
                                                        true,
                                                        cx,
                                                    );
                                                })),
                                        )
                                        .child(
                                            Button::new(("git-stash-drop", i))
                                                .custom(crate::palette::subtle_icon_variant(cx))
                                                .compact()
                                                .xsmall()
                                                .h(px(22.))
                                                .px_2()
                                                .rounded_sm()
                                                .label(tr("删除"))
                                                .disabled(self.ops_busy)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.run_git_branch_op(
                                                        "git.stashDrop",
                                                        json!({ "workspaceId": ws_drop, "ref": ref_drop }),
                                                        "已删除 Stash",
                                                        false,
                                                        cx,
                                                    );
                                                })),
                                        ),
                                )
                        })),
                )
            })
            .into_any_element()
    }
    fn load_commit_log(&mut self, cx: &mut Context<Self>) {
        if self.preview_mock || !self.current(cx) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.commits_loading = true;
        cx.notify();
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let res = runtime
                .request(
                    "git.log",
                    git_params(json!({ "workspaceId": workspace, "limit": 30 }), &session),
                )
                .await
                .ok()
                .and_then(|v| decode::<Vec<crate::protocol::GitCommitSummary>>(v, "commits").ok());
            let _ = this.update(cx, |this, cx| {
                this.commits_loading = false;
                if let Some(commits) = res {
                    this.commits = commits;
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_commit_diff(&mut self, oid: String, cx: &mut Context<Self>) {
        if self.preview_mock || !self.current(cx) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        if self.selected_commit_oid.as_deref() == Some(oid.as_str()) && self.selected_commit_diff.is_some() {
            self.selected_commit_oid = None;
            self.selected_commit_diff = None;
            cx.notify();
            return;
        }
        self.selected_commit_oid = Some(oid.clone());
        self.selected_commit_diff = None;
        self.selected_commit_loading = true;
        cx.notify();
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let res = runtime
                .request(
                    "git.commitDiff",
                    git_params(json!({ "workspaceId": workspace, "oid": oid }), &session),
                )
                .await
                .ok()
                .and_then(|v| decode::<crate::protocol::GitCommitDiffResult>(v, "diff").ok());
            let _ = this.update(cx, |this, cx| {
                if this.selected_commit_oid.as_deref() != Some(oid.as_str()) {
                    return;
                }
                this.selected_commit_loading = false;
                this.selected_commit_diff = res;
                cx.notify();
            });
        })
        .detach();
    }

    fn open_diff_review(
        &mut self,
        range: Option<DiffLineRange>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preview_mock || !self.current(cx) || self.diff_loading {
            return;
        }
        let selection = range.as_ref().map(|range| self.reviewed_lines(range, cx));
        let draft = self.diff_review.get_or_insert_with(|| DiffReview {
            comment: cx.new(|cx| {
                gpui_kit::component::input::InputState::new(window, cx)
                    .placeholder(tr("输入评审意见，一键发给当前 Agent 修改…"))
            }),
            selection: None,
        });
        draft.selection = selection;
        self.review_open = true;
        self.ops_notice = None;
        let focus = draft.comment.read(cx).focus_handle(cx);
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
        cx.notify();
    }

    fn reviewed_lines(
        &self,
        range: &DiffLineRange,
        cx: &App,
    ) -> Result<ReviewedLines, review::ReviewError> {
        let [file] = self.diff_state.read(cx).files() else {
            return Err(review::ReviewError::Unavailable);
        };
        if file.path() != range.path() {
            return Err(review::ReviewError::Unavailable);
        }
        let diff = self.diff.as_ref().ok_or(review::ReviewError::Unavailable)?;
        let patch = diff.patch.as_deref().ok_or(review::ReviewError::Unavailable)?;
        let path_on = |side| match side {
            DiffSide::Original => diff.before_path.as_deref().unwrap_or(&diff.path),
            DiffSide::Modified => diff.after_path.as_deref().unwrap_or(&diff.path),
        };
        let side_label = |side| match side {
            DiffSide::Original => tr("原始侧"),
            DiffSide::Modified => tr("修改侧"),
        };
        let location = if range.side() == range.end_side() {
            let start = range.start().min(range.end());
            let end = range.start().max(range.end());
            let lines = if start == end {
                format!("L{start}")
            } else {
                format!("L{start}–L{end}")
            };
            format!("{} {:?}:{lines}", side_label(range.side()), path_on(range.side()))
        } else {
            format!(
                "{} {:?}:L{} ↔ {} {:?}:L{}",
                side_label(range.side()), path_on(range.side()), range.start(),
                side_label(range.end_side()), path_on(range.end_side()), range.end(),
            )
        };
        Ok(ReviewedLines { location, patch: review::focused_patch(patch, range)? })
    }

    fn send_diff_review_to_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.current(cx) || !self.review_open || self.diff_loading {
            return;
        }
        let (Some(change), Some(draft)) = (self.selected_change(), &self.diff_review) else {
            return;
        };
        let comment = draft.comment.read(cx).value();
        let comment = comment.trim();
        if comment.is_empty() {
            self.ops_notice = Some((true, tr("请先输入评审意见").to_owned()));
            cx.notify();
            return;
        }
        let comparison = self.comparison.label();
        let file = format!("{:?}", change.path);
        let mut prompt = format!("{} {file} · {comparison}", tr("[Diff 评审]"));
        let (patch, abbreviated) = match &draft.selection {
            Some(Ok(lines)) => {
                prompt.push_str(&format!("\n{}", lines.location));
                (lines.patch.as_str(), false)
            }
            Some(Err(_)) => return,
            None => review::file_excerpt(
                self.diff.as_ref().and_then(|diff| diff.patch.as_deref()).unwrap_or_default(),
            ),
        };
        prompt.push_str(&format!("\n{comment}"));
        if !patch.is_empty() {
            prompt.push_str(&format!("\n\n{}", review::fenced_patch(patch)));
        }
        if abbreviated {
            prompt.push_str(&format!("\n{}", tr("文件补丁预览已截断。")));
        }
        let Ok(focus) = self.app.update(cx, |this, cx| {
            this.input.update(cx, |state, cx| {
                let end = state.value().len();
                if end != 0 {
                    prompt.insert_str(0, "\n\n");
                }
                state.set_selected_range(end..end, cx);
                state.insert(prompt, window, cx);
            });
            cx.notify();
            this.input.read(cx).focus_handle(cx)
        }) else {
            return;
        };
        self.diff_review = None;
        self.review_open = false;
        self.ops_notice = Some((false, tr("已将 Diff 评审意见插入对话输入框").to_owned()));
        if !self.in_drawer {
            window.close_dialog(cx);
        }
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
        cx.notify();
    }

    fn render_diff_review_and_discard_bar(&self, current: bool, cx: &Context<Self>) -> AnyElement {
        if self.preview_mock || !current {
            return div().into_any_element();
        }
        let Some(change) = self.selected_change().cloned() else {
            return div().into_any_element();
        };
        let path_for_confirm = change.path.clone();
        let path_for_discard = change.path.clone();
        let path_for_reveal = change.reveal_path().map(str::to_owned);
        let ws_discard = self.workspace.clone();
        let can_discard = change.kind != "untracked"
            && !change.worktree_status.is_empty()
            && change.worktree_status != "."
            && change.worktree_status != " "
            && !self.ops_busy;
        let confirming = self.discard_confirm_path.as_deref() == Some(change.path.as_str());

        v_flex()
            .w_full()
            .gap_1p5()
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_1p5()
                    .px_2p5()
                    .py_1()
                    .rounded_lg()
                    .bg(Color::Inset.get(cx))
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .truncate()
                            .child(change.path.clone()),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .max_w_full()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("git-diff-reveal-file")
                                    .outline()
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_sm()
                                    .label(tr("在文件面板中定位"))
                                    .tooltip(tr(if path_for_reveal.is_some() {
                                        "在文件面板中定位"
                                    } else {
                                        "此改动没有可定位的当前文件。"
                                    }))
                                    .disabled(path_for_reveal.is_none() || self.busy(cx))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Some(path) = path_for_reveal.clone() {
                                            this.reveal_selected_file(path, window, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("git-diff-review-toggle")
                                    .outline()
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_sm()
                                    .label(tr("评审批注 → Agent"))
                                    .accessibility_label(tr("评审批注 → Agent"))
                                    .disabled(self.diff_loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.review_open {
                                            this.review_open = false;
                                            cx.notify();
                                        } else {
                                            let range = if this.diff_raw {
                                                None
                                            } else {
                                                this.diff_state.read(cx).selected_lines()
                                            };
                                            this.open_diff_review(range, window, cx);
                                        }
                                    })),
                            )
                            .when(can_discard && !confirming, |row| {
                                row.child(
                                    Button::new("git-discard-file-trigger")
                                        .custom(crate::palette::subtle_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(22.))
                                        .px_2()
                                        .rounded_sm()
                                        .label(tr("放弃改动"))
                                        .accessibility_label(tr("放弃改动"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.discard_confirm_path = Some(path_for_confirm.clone());
                                            cx.notify();
                                        })),
                                )
                            })
                            .when(confirming, |row| {
                                row.child(
                                    Button::new("git-discard-file-confirm")
                                        .danger()
                                        .compact()
                                        .xsmall()
                                        .h(px(22.))
                                        .px_2()
                                        .rounded_sm()
                                        .label(tr("确认放弃"))
                                        .accessibility_label(tr("确认放弃"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.discard_confirm_path = None;
                                            this.run_git_branch_op(
                                                "git.discardFile",
                                                json!({ "workspaceId": ws_discard, "path": path_for_discard }),
                                                "已放弃该文件未暂存修改",
                                                false,
                                                cx,
                                            );
                                        })),
                                )
                                .child(
                                    Button::new("git-discard-file-cancel")
                                        .custom(crate::palette::subtle_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(22.))
                                        .px_1p5()
                                        .rounded_sm()
                                        .label(tr("取消"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.discard_confirm_path = None;
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            )
            .when_some(self.diff_review.as_ref().filter(|_| self.review_open), |col, draft| {
                let (location, invalid) = match &draft.selection {
                    Some(Ok(selection)) => (selection.location.as_str(), false),
                    Some(Err(review::ReviewError::Unavailable)) => (
                        tr("所选行无法在当前补丁中定位，请重新选择。"), true,
                    ),
                    Some(Err(review::ReviewError::TooLarge)) => (
                        tr("所选行超出评审上下文上限，请缩小范围。"), true,
                    ),
                    None => (tr("文件级评审"), false),
                };
                col.child(
                    Reveal::new("git-diff-review-form", &self.body_scroll, v_flex()
                        .w_full()
                        .gap_1p5()
                        .p_2p5()
                        .rounded_lg()
                        .bg(Color::Card.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                        .shadow(crate::palette::card_shadow(cx))
                        .child(
                            h_flex()
                                .gap_1p5()
                                .flex_wrap()
                                .items_center()
                                .child(
                                    div()
                                        .id("git-diff-review-location")
                                        .role(Role::Status)
                                        .aria_label(location.to_owned())
                                        .text_xs()
                                        .text_color(if invalid { Color::Error.get(cx) } else { Color::Secondary.get(cx) })
                                        .child(location.to_owned()),
                                )
                                .when(draft.selection.is_some(), |row| {
                                    row.child(
                                        Button::new("git-diff-review-whole-file")
                                            .ghost()
                                            .compact()
                                            .xsmall()
                                            .label(tr("评审整个文件"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.open_diff_review(None, window, cx);
                                            })),
                                    )
                                }),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(gpui_kit::component::input::Input::new(&draft.comment).small()),
                                )
                                .child(
                                    Button::new("git-diff-review-send")
                                        .primary()
                                        .compact()
                                        .xsmall()
                                        .h(px(26.))
                                        .px_2p5()
                                        .rounded_md()
                                        .label(tr("插入到输入框"))
                                        .accessibility_label(tr("插入到输入框"))
                                        .disabled(invalid)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.send_diff_review_to_agent(window, cx);
                                        })),
                                ),
                        ),
                    ),
                )
            })
            .into_any_element()
    }

    fn render_commit_history_panel(&self, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .gap_2()
            .when(self.commits_loading, |col| {
                col.child(
                    div()
                        .p_2()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(tr("正在读取提交历史…")),
                )
            })
            .when(!self.commits_loading && self.commits.is_empty(), |col| {
                col.child(
                    div()
                        .p_3()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("当前仓库尚无提交记录。")),
                )
            })
            .children(self.commits.iter().enumerate().map(|(i, commit)| {
                let oid = commit.oid.clone();
                let is_selected = self.selected_commit_oid.as_deref() == Some(commit.oid.as_str());
                v_flex()
                    .id(("git-commit-row", i))
                    .w_full()
                    .gap_1p5()
                    .p_2p5()
                    .rounded_lg()
                    .bg(if is_selected { Color::AccentSoft.get(cx) } else { Color::Card.get(cx) })
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py(px(0.5))
                                                    .rounded_sm()
                                                    .bg(Color::Inset.get(cx))
                                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                                    .text_size(px(10.5))
                                                    .text_color(Color::Accent.get(cx))
                                                    .child(commit.short_oid.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(Color::Text.get(cx))
                                                    .truncate()
                                                    .child(commit.subject.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(Color::Muted.get(cx))
                                            .child(format!("{} · {}", commit.author, commit.relative_time)),
                                    ),
                            )
                            .child(
                                Button::new(("git-commit-inspect", i))
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_sm()
                                    .label(if is_selected { tr("收起") } else { tr("查看补丁") })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.load_commit_diff(oid.clone(), cx);
                                    })),
                            ),
                    )
                    .when(is_selected && self.selected_commit_loading, |card| {
                        card.child(
                            div()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(tr("正在读取提交差异…")),
                        )
                    })
                    .when(is_selected, |card| {
                        card.when_some(self.selected_commit_diff.as_ref(), |c, diff| {
                            let preview: String = diff.patch.chars().take(12_000).collect();
                            c.child(
                                div()
                                    .id(("git-commit-diff-scroll", i))
                                    .w_full()
                                    .max_h(px(260.))
                                    .overflow_y_scroll()
                                    .p_2()
                                    .rounded_md()
                                    .bg(Color::Inset.get(cx))
                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                    .text_size(px(11.))
                                    .line_height(px(16.))
                                    .text_color(Color::Secondary.get(cx))
                                    .child(if diff.truncated {
                                        format!("{preview}\n…")
                                    } else {
                                        preview
                                    }),
                            )
                        })
                    })
            }))
            .into_any_element()
    }

}
impl Render for GitWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy(cx);
        let current = self.current(cx);
        let segment_variant = |active: bool, cx: &App| {
            ButtonCustomVariant::new(cx)
                .color(if active {
                    Color::Card.get(cx).into()
                } else {
                    gpui::rgba(0).into()
                })
                .foreground(if active {
                    Color::Text.get(cx).into()
                } else {
                    Color::Secondary.get(cx).into()
                })
                .hover(Color::Card.get(cx).opacity(0.65).into())
                .active(Color::Card.get(cx).into())
        };

        let branch_badge = self.status.as_ref().map(|status| {
            (
                status.identity.branch_label(),
                format!(
                    "{}\ncomplete={} · scope={} · submodules={} · config={}",
                    status.identity.description(),
                    status.complete,
                    status.scope,
                    status.submodule_details,
                    status.global_configuration
                ),
            )
        });
        let total_changes = self.status.as_ref().map(|s| s.total).unwrap_or(0);
        let app_expand = self.app.clone();
        let app_close = self.app.clone();

        let header = h_flex()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py_2()
            .when(self.in_drawer, |bar| {
                bar.bg(Color::Sidebar.get(cx))
                    .border_b_1()
                    .border_color(Color::Divider.get(cx))
            })
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(Glyph::GitBranch)
                            .size(px(14.))
                            .text_color(Color::Muted.get(cx))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(self.project_name.clone()),
                    )
                    .when_some(branch_badge, |row, (branch, details)| {
                        row.child(
                            div()
                                .id("git-workspace-identity")
                                .max_w(px(140.))
                                .truncate()
                                .px_2()
                                .py(px(1.))
                                .rounded_full()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(branch)
                                .tooltip(move |window, cx| {
                                    Tooltip::new(details.clone()).build(window, cx)
                                }),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_0p5()
                            .p_0p5()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(
                                Button::new("git-local-panel")
                                    .custom(segment_variant(!self.show_remotes && !self.show_history, cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_md()
                                    .label(if total_changes > 0 {
                                        format!("{} ({total_changes})", tr("变更"))
                                    } else {
                                        tr("变更").to_owned()
                                    })
                                    .selected(!self.show_remotes && !self.show_history)
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.busy(cx) {
                                            return;
                                        }
                                        this.show_remotes = false;
                                        this.show_history = false;
                                        this.refresh(cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("git-history-panel")
                                    .custom(segment_variant(self.show_history, cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_md()
                                    .label(tr("提交历史"))
                                    .selected(self.show_history)
                                    .disabled(busy || self.preview_mock)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.busy(cx) {
                                            return;
                                        }
                                        this.show_remotes = false;
                                        this.show_history = true;
                                        this.load_commit_log(cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("git-remotes-panel")
                                    .custom(segment_variant(self.show_remotes, cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_md()
                                    .label(tr("远端与 PR"))
                                    .selected(self.show_remotes)
                                    .disabled(busy || self.remotes.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.busy(cx) {
                                            return;
                                        }
                                        this.show_remotes = true;
                                        this.show_history = false;
                                        this.reload_diff_on_open = false;
                                        this.clear_file_diff(cx);
                                        this.hide_commit_patch();
                                        if let Some(remotes) = &this.remotes {
                                            remotes.update(cx, |remote, cx| remote.activate(cx));
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        Button::new("refresh-git-workspace")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(24.))
                            .w(px(24.))
                            .rounded_md()
                            .icon(Glyph::RefreshCw)
                            .tooltip(tr("刷新 Git 状态"))
                            .accessibility_label(tr("刷新 Git 状态"))
                            .disabled(!current || self.status_loading || self.index_busy || self.preview_mock)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .when(self.in_drawer, |row| {
                        row.when(!self.preview_mock, |row| {
                            row.child(
                                Button::new("git-drawer-expand")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(24.))
                                    .w(px(24.))
                                    .rounded_md()
                                    .icon(Glyph::Maximize2)
                                    .tooltip(tr("在弹窗中展开"))
                                    .accessibility_label(tr("在弹窗中展开"))
                                    .on_click(move |_, window, cx| {
                                        let _ = app_expand.update(cx, |this, cx| {
                                            this.open_git_workspace_modal(window, cx);
                                        });
                                    }),
                            )
                        })
                        .child(
                            Button::new("git-drawer-close")
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(24.))
                                .w(px(24.))
                                .rounded_md()
                                .icon(Glyph::X)
                                .tooltip(tr("关闭 Git 面板"))
                                .accessibility_label(tr("关闭 Git 面板"))
                                .on_click(move |_, _, cx| {
                                    let _ = app_close.update(cx, |this, cx| {
                                        this.close_git_workspace(cx);
                                    });
                                }),
                        )
                    }),
            );

        let body: AnyElement = if self.show_remotes {
            if let Some(remotes) = &self.remotes {
                v_flex().min_w_0().child(remotes.clone()).into_any_element()
            } else {
                div().text_xs().text_color(Color::Muted.get(cx)).child(tr("预览模式下不连接远端仓库。")).into_any_element()
            }
        } else if self.show_history {
            self.render_commit_history_panel(cx)
        } else {
            let keys = cx.entity();
            let list_height = if self.in_drawer {
                px(164.)
            } else {
                px(180.).min(window.viewport_size().height * 0.25)
            };
            let diff_height = if self.in_drawer {
                px(360.).max(window.viewport_size().height * 0.44)
            } else {
                px(360.).min(window.viewport_size().height * 0.4)
            };
            let selected_change = self.selected_change().filter(|_| current).cloned();

            v_flex()
                .min_w_0()
                .gap_3()
                .child(self.render_branch_stash_toolbar(current, cx))
                .child(self.render_diff_review_and_discard_bar(current, cx))
                .when(!current, |view| {
                    view.child(
                        div()
                            .id("git-context-changed")
                            .role(Role::Status)
                            .p_2p5()
                            .rounded_lg()
                            .bg(Color::ErrorSurface.get(cx))
                            .text_xs()
                            .text_color(Color::Error.get(cx))
                            .child(tr("项目或后台连接已变化，请关闭后重新打开 Git 状态。")),
                    )
                })
                .when(self.status_loading && current, |view| {
                    view.child(
                        div()
                            .p_2()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .child(tr("正在读取 Git 状态…")),
                    )
                })
                .when_some(self.status_error.as_ref(), |view, error| {
                    view.child(
                        div()
                            .id("git-status-error")
                            .role(Role::Status)
                            .p_2p5()
                            .rounded_lg()
                            .bg(Color::ErrorSurface.get(cx))
                            .text_xs()
                            .text_color(Color::Error.get(cx))
                            .child(error.text()),
                    )
                })
                .when_some(self.status.as_ref().filter(|_| current), |view, status| {
                    let accent = Color::Accent.get(cx);
                    let success = Color::Success.get(cx);
                    let error_color = Color::Error.get(cx);
                    let warning_color = rgb(if Theme::global(cx).is_dark() { 0xfbbf24 } else { 0xd97706 });
                    let can_index = current && !self.status_loading && !self.index_busy && !self.index_refresh_required && !self.preview_mock;
                    let selected_can_stage = selected_change.as_ref().is_some_and(|c| c.has_unstaged());
                    let selected_can_unstage = selected_change.as_ref().is_some_and(|c| c.has_staged());
                    let has_any_staged = status.can_preview_commit();

                    view.when(!status.complete || !status.warnings.is_empty(), |view| {
                        view.child(
                            div()
                                .id("git-status-warnings")
                                .role(Role::Status)
                                .p_2p5()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .text_xs()
                                .text_color(Color::Error.get(cx))
                                .child(format!("{}\n{}", tr("Git 状态读取不完整，请查看警告"), status.warnings.join("\n"))),
                        )
                    })
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .rounded_xl()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .overflow_hidden()
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .px_3()
                                    .py_1p5()
                                    .bg(Color::Inset.get(cx))
                                    .border_b_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(tr("改动文件")),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(if status.entries.is_empty() {
                                                        "0".to_owned()
                                                    } else if status.total > status.entries.len() {
                                                        format!("{}–{} / {}", status.offset + 1, status.offset + status.entries.len(), status.total)
                                                    } else {
                                                        status.total.to_string()
                                                    }),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                Button::new("git-stage-selected")
                                                    .outline()
                                                    .xsmall()
                                                    .h(px(22.))
                                                    .px_2()
                                                    .rounded_md()
                                                    .label(tr("暂存"))
                                                    .tooltip(tr("暂存所选文件"))
                                                    .disabled(!can_index || !selected_can_stage)
                                                    .on_click(cx.listener(|this, _, _, cx| this.preview_index(IndexOperation::Stage, cx))),
                                            )
                                            .child(
                                                Button::new("git-unstage-selected")
                                                    .outline()
                                                    .xsmall()
                                                    .h(px(22.))
                                                    .px_2()
                                                    .rounded_md()
                                                    .label(tr("撤销暂存"))
                                                    .tooltip(tr("撤销暂存所选文件"))
                                                    .disabled(!can_index || !selected_can_unstage)
                                                    .on_click(cx.listener(|this, _, _, cx| this.preview_index(IndexOperation::Unstage, cx))),
                                            )
                                            .child(
                                                crate::focus_scroll::Reveal::new(
                                                    "git-commit-preview-reveal",
                                                    &self.body_scroll,
                                                    Button::new("git-commit-preview")
                                                    .track_focus(&self.commit_trigger_focus)
                                                    .primary()
                                                    .xsmall()
                                                    .h(px(22.))
                                                    .px_2()
                                                    .rounded_md()
                                                    .label(tr("提交…"))
                                                    .tooltip(tr("提交已暂存改动"))
                                                    .disabled(!can_index || !has_any_staged)
                                                    .on_click(cx.listener(|this, _, _, cx| this.preview_commit(cx))),
                                                ),
                                            ),
                                    ),
                            )
                            .child(
                                div().h(list_height).flex().min_h_0().child(Viewport::new(
                                    "git-files-viewport",
                                    &self.files_scroll,
                                    v_flex()
                                        .id("git-changed-files")
                                        .role(Role::List)
                                        .aria_label(tr("Git 改动文件"))
                                        .tab_index(0)
                                        .capture_key_down(move |event, _, cx| {
                                            keys.update(cx, |this, cx| this.key(event, cx))
                                        })
                                        .flex_1()
                                        .min_h_0()
                                        .min_w_0()
                                        .overflow_y_scroll()
                                        .track_scroll(&self.files_scroll)
                                        .vertical_scrollbar(&self.files_scroll)
                                        .p_1()
                                        .gap_0p5()
                                        .when(status.entries.is_empty(), |list| {
                                            list.child(
                                                div()
                                                    .p_3()
                                                    .text_xs()
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(if status.total == 0 && status.complete {
                                                        tr("项目范围内没有改动")
                                                    } else {
                                                        tr("当前页没有文件，请刷新或返回上一页")
                                                    }),
                                            )
                                        })
                                        .children(status.entries.iter().enumerate().map(|(index, change)| {
                                            let (letter, tone) = change.badge();
                                            let badge_color = match tone {
                                                "added" => success,
                                                "deleted" => error_color,
                                                "conflict" => error_color,
                                                "renamed" => accent,
                                                _ => warning_color,
                                            };
                                            let selected = index == self.selected;
                                            let file_name = change.file_name().to_owned();
                                            let dir_name = change.dir_name().map(str::to_owned);
                                            let staged = change.has_staged();
                                            let unstaged = change.has_unstaged();
                                            let meta_tip = change.metadata();
                                            let title_label = change.title();

                                            Reveal::new(
                                                ("git-file-reveal", index),
                                                &self.files_scroll,
                                                h_flex()
                                                    .w_full()
                                                    .min_w_0()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        Button::new(("git-changed-file", index))
                                                            .custom(crate::palette::sidebar_item_variant(selected, cx))
                                                            .compact()
                                                            .small()
                                                            .h(px(26.))
                                                            .flex_1()
                                                            .min_w_0()
                                                            .justify_start()
                                                            .px_2()
                                                            .rounded_md()
                                                            .selected(selected)
                                                            .disabled(!current || self.index_busy)
                                                            .accessibility_label(title_label)
                                                            .tooltip(meta_tip)
                                                            .child(
                                                                h_flex()
                                                                    .w_full()
                                                                    .min_w_0()
                                                                    .items_center()
                                                                    .gap_1p5()
                                                                    .child(
                                                                        div()
                                                                            .w(px(14.))
                                                                            .flex_shrink_0()
                                                                            .text_center()
                                                                            .font_family(Theme::global(cx).mono_font_family.clone())
                                                                            .text_size(px(11.))
                                                                            .font_weight(FontWeight::BOLD)
                                                                            .text_color(badge_color)
                                                                            .child(letter),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .flex_shrink_0()
                                                                            .max_w_1_2()
                                                                            .truncate()
                                                                            .text_xs()
                                                                            .font_weight(if selected { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                                                                            .text_color(Color::Text.get(cx))
                                                                            .child(file_name),
                                                                    )
                                                                    .when_some(dir_name, |row, dir| {
                                                                        row.child(
                                                                            div()
                                                                                .flex_1()
                                                                                .min_w_0()
                                                                                .truncate()
                                                                                .text_size(px(11.))
                                                                                .text_color(Color::Muted.get(cx))
                                                                                .child(dir),
                                                                        )
                                                                    })
                                                                    .when(staged, |row| {
                                                                        row.child(
                                                                            div()
                                                                                .w(px(5.))
                                                                                .h(px(5.))
                                                                                .rounded_full()
                                                                                .bg(success)
                                                                                .flex_shrink_0(),
                                                                        )
                                                                    })
                                                                    .when(unstaged && staged, |row| {
                                                                        row.child(
                                                                            div()
                                                                                .w(px(5.))
                                                                                .h(px(5.))
                                                                                .rounded_full()
                                                                                .bg(warning_color)
                                                                                .flex_shrink_0(),
                                                                        )
                                                                    }),
                                                            )
                                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                                this.select(index, true, cx)
                                                            })),
                                                    )
                                                    .when(can_index && (unstaged || staged), |row| {
                                                        let op = if unstaged { IndexOperation::Stage } else { IndexOperation::Unstage };
                                                        let icon = if unstaged { Glyph::Plus } else { Glyph::Minus };
                                                        let tip = if unstaged { tr("暂存") } else { tr("撤销暂存") };
                                                        row.child(
                                                            Button::new(("git-file-quick-op", index))
                                                                .custom(crate::palette::quiet_icon_variant(cx))
                                                                .compact()
                                                                .xsmall()
                                                                .h(px(22.))
                                                                .w(px(22.))
                                                                .rounded_md()
                                                                .icon(icon)
                                                                .tooltip(tip)
                                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                                    this.select(index, false, cx);
                                                                    this.preview_index(op, cx);
                                                                })),
                                                        )
                                                    }),
                                            )
                                        })),
                                )),
                            )
                            .when(self.offset > 0 || status.has_more, |card| {
                                card.child(
                                    h_flex()
                                        .items_center()
                                        .justify_end()
                                        .gap_1()
                                        .px_2p5()
                                        .py_1()
                                        .border_t_1()
                                        .border_color(Color::Divider.get(cx))
                                        .child(
                                            Button::new("git-files-prev")
                                                .ghost()
                                                .xsmall()
                                                .label(tr("上一页"))
                                                .disabled(!current || self.index_busy || self.offset == 0)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    let limit = this.status.as_ref().map(|status| status.limit).unwrap_or(100);
                                                    this.offset = this.offset.saturating_sub(limit);
                                                    this.refresh(cx);
                                                })),
                                        )
                                        .child(
                                            Button::new("git-files-next")
                                                .ghost()
                                                .xsmall()
                                                .label(tr("下一页"))
                                                .disabled(!current || self.index_busy || !status.has_more)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.offset += this.status.as_ref().map(|status| status.limit).unwrap_or(100);
                                                    this.refresh(cx);
                                                })),
                                        ),
                                )
                            }),
                    )
                })
                .when(self.index_busy && current, |view| {
                    view.child(
                        div()
                            .id("git-index-busy")
                            .role(Role::Status)
                            .p_2()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .child(tr("正在处理 Git 操作…")),
                    )
                })
                .when_some(self.index_error.as_ref(), |view, error| {
                    view.child(
                        div()
                            .id("git-index-error")
                            .role(Role::Status)
                            .p_2p5()
                            .rounded_lg()
                            .bg(Color::ErrorSurface.get(cx))
                            .text_xs()
                            .text_color(Color::Error.get(cx))
                            .child(format!(
                                "{}\n[{}] {}",
                                tr("Git 操作失败"),
                                error.code,
                                crate::language::localize_owned_error(&error.message)
                            )),
                    )
                })
                .when(
                    self.index_refresh_required && self.index_notice != Some("Git 索引已更改，请刷新状态后再操作。"),
                    |view| {
                        view.child(
                            div()
                                .id("git-index-refresh-required")
                                .role(Role::Status)
                                .p_2()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .text_xs()
                                .text_color(Color::Error.get(cx))
                                .child(tr("Git 索引可能已更改，请刷新状态后再操作。")),
                        )
                    },
                )
                .when_some(self.index_notice, |view, notice| {
                    view.child(
                        div()
                            .id("git-index-notice")
                            .role(Role::Status)
                            .p_2()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .child(tr(notice)),
                    )
                })
                .when_some(self.index_preview.as_ref().filter(|_| current), |view, pending| {
                    let paths = pending.preview.paths.join("\n");
                    view.child(
                        v_flex()
                            .gap_2()
                            .p_3()
                            .rounded_xl()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(Color::Text.get(cx)).child(pending.preview.operation.confirm_label()))
                            .child(crate::text_reading::selectable_text("git-index-preview-paths", format!("{}:\n{}", tr("受影响的项目内路径"), paths)).text_xs().text_color(Color::Secondary.get(cx)))
                            .when(pending.preview.content_filters, |view| {
                                view.child(div().text_xs().text_color(Color::Error.get(cx)).child(tr("暂存会按仓库配置运行内容过滤器。")))
                            })
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .child(
                                        Button::new("git-index-confirm")
                                            .primary()
                                            .xsmall()
                                            .label(pending.preview.operation.confirm_label())
                                            .disabled(self.index_busy)
                                            .on_click(cx.listener(|this, _, _, cx| this.apply_index(cx))),
                                    )
                                    .child(
                                        Button::new("git-index-cancel")
                                            .ghost()
                                            .xsmall()
                                            .label(tr("取消"))
                                            .disabled(self.index_busy)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.index_generation += 1;
                                                this.index_preview = None;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                })
                .when_some(self.commit_result.as_ref(), |view, result| {
                    let state = if result.committed == json!(true) {
                        if result.index_updated {
                            tr("提交已创建，索引已同步。")
                        } else {
                            tr("提交已创建，但索引未同步；请刷新并检查。")
                        }
                    } else if result.committed == json!(false) && result.applied == json!(false) {
                        tr("提交未创建；请刷新并检查后重试。")
                    } else {
                        tr("提交状态不确定；请刷新并核对 HEAD，勿自动重试。")
                    };
                    let mut tech = format!(
                        "ok={} · committed={} · applied={} · indexUpdated={}",
                        result.ok, result.committed, result.applied, result.index_updated
                    );
                    if let Some(workspace) = &result.workspace {
                        tech.push_str(&format!("\n{}", workspace.description()));
                    }
                    let oid_chip = result
                        .commit_oid
                        .as_deref()
                        .or(result.observed_head_oid.as_deref())
                        .map(|oid| oid.chars().take(7).collect::<String>());
                    let mut extra_lines = Vec::new();
                    if let Some(error) = &result.error {
                        extra_lines.push(format!("[{}] {}", error.code, crate::language::localize_owned_error(&error.message)));
                    }
                    extra_lines.extend(result.warnings.iter().cloned());

                    let tech_tip = tech.clone();
                    let accessible_result = commit_result_label(state, &extra_lines);
                    view.child(
                        v_flex()
                            .id("git-commit-result")
                            .role(Role::Status)
                            .aria_label(accessible_result)
                            .gap_1()
                            .p_2p5()
                            .rounded_lg()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .tooltip(move |window, cx| Tooltip::new(tech_tip.clone()).build(window, cx))
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(Color::Text.get(cx)).child(state))
                                    .when_some(oid_chip, |row, short_oid| {
                                        row.child(
                                            div()
                                                .id("git-commit-oid-badge")
                                                .px_1p5()
                                                .py(px(1.))
                                                .rounded_md()
                                                .bg(Color::Inset.get(cx))
                                                .font_family(Theme::global(cx).mono_font_family.clone())
                                                .text_size(px(11.))
                                                .text_color(Color::Secondary.get(cx))
                                                .child(short_oid)
                                                .tooltip(move |window, cx| Tooltip::new(tech.clone()).build(window, cx)),
                                        )
                                    }),
                            )
                            .when(!extra_lines.is_empty(), |col| {
                                col.child(div().text_xs().text_color(Color::Error.get(cx)).child(extra_lines.join("\n")))
                            }),
                    )
                })
                .when_some(self.commit_preview.as_ref().filter(|_| current), |view, preview| {
                    let paths = preview
                        .changes
                        .iter()
                        .map(|change| {
                            let from = change
                                .original_repository_path
                                .as_ref()
                                .map(|path| format!("{path} → "))
                                .unwrap_or_default();
                            format!(
                                "{}{} · {}{}",
                                from,
                                change.repository_path,
                                change.status,
                                if change.project_path.is_none() { " · outside project" } else { "" }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let outside = preview.outside_project_paths.join("\n");
                    let hooks = if preview.hooks.is_empty() { tr("无").to_owned() } else { preview.hooks.join(", ") };
                    view.child(
                        v_flex()
                            .gap_2()
                            .p_3()
                            .rounded_xl()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(Color::Text.get(cx)).child(tr("完整仓库的已暂存改动")))
                            .child(
                                crate::text_reading::region(
                                    "git-commit-paths-region".into(),
                                    tr("完整仓库的已暂存改动").to_owned(),
                                    &self.commit_scroll,
                                    px(120.),
                                    cx,
                                )
                                .child(crate::text_reading::selectable_text("git-commit-paths", paths)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(format!("{}: {} · {}: {}", tr("原生 Git hooks"), hooks, tr("需要签名"), if preview.signing_required { tr("是") } else { tr("否") })),
                            )
                            .child(
                                Button::new("git-commit-toggle-patch")
                                    .ghost()
                                    .xsmall()
                                    .label(if self.commit_patch.is_some() { tr("隐藏已暂存补丁") } else { tr("查看已暂存补丁") })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.commit_patch.is_some() {
                                            this.hide_commit_patch();
                                            cx.notify();
                                        } else {
                                            this.load_commit_patch(cx);
                                        }
                                    })),
                            )
                            .when_some(self.commit_patch.as_ref(), |view, patch| {
                                view.child(match patch {
                                    CommitPatch::Loading { .. } => {
                                        let message = tr("正在读取已暂存补丁…");
                                        div().id("git-commit-patch-loading").role(Role::Status)
                                            .aria_label(message).text_xs().child(message).into_any_element()
                                    }
                                    CommitPatch::Ready(Err(error)) => {
                                        let message = error.text();
                                        div().id("git-commit-patch-error").role(Role::Status)
                                            .aria_label(message.clone()).text_xs().text_color(Color::Error.get(cx))
                                            .child(message).into_any_element()
                                    }
                                    CommitPatch::Ready(Ok(reader)) => reader.view(
                                        "git-commit-patch",
                                        crate::text_reading::region(
                                            "git-commit-patch-region".into(),
                                            tr("已暂存补丁").to_owned(),
                                            &reader.scroll,
                                            px(110.),
                                            cx,
                                        ),
                                        Self::staged_patch_reader,
                                        cx,
                                    ).into_any_element(),
                                })
                            })
                            .when(preview.hooks_may_modify_index, |view| {
                                view.child(div().text_xs().text_color(Color::Secondary.get(cx)).child(tr("原生 Git hooks 可能修改文件、索引和提交说明；签名使用仓库配置。")))
                            })
                            .when(!preview.outside_project_paths.is_empty(), |view| {
                                view.child(
                                    crate::text_reading::region(
                                        "git-commit-outside-region".into(),
                                        tr("项目范围外的已暂存路径").to_owned(),
                                        &self.outside_scroll,
                                        px(90.),
                                        cx,
                                    )
                                    .child(crate::text_reading::selectable_text("git-commit-outside-paths", format!("{}:\n{}", tr("项目范围外的已暂存路径"), outside))),
                                )
                                .child(
                                    Checkbox::new("git-commit-include-outside")
                                        .label(tr("我确认提交包含项目范围外的路径"))
                                        .checked(self.commit_include_outside)
                                        .disabled(self.index_busy)
                                        .on_change(cx.listener(|this, value, _, cx| {
                                            this.commit_include_outside = *value;
                                            cx.notify();
                                        })),
                                )
                            })
                            .child(Textarea::new(&self.commit_message).disabled(self.index_busy))
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .child(
                                        Button::new("git-commit-confirm")
                                            .primary()
                                            .xsmall()
                                            .label(tr("确认提交"))
                                            .disabled(self.index_busy
                                                || matches!(self.commit_patch, Some(CommitPatch::Loading { .. } | CommitPatch::Ready(Err(_))))
                                                || (!preview.outside_project_paths.is_empty() && !self.commit_include_outside))
                                            .on_click(cx.listener(|this, _, _, cx| this.apply_commit(cx))),
                                    )
                                    .child(
                                        Button::new("git-commit-cancel")
                                            .ghost()
                                            .xsmall()
                                            .label(tr("取消"))
                                            .disabled(self.index_busy)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.index_generation += 1;
                                                this.clear_commit_preview();
                                                this.commit_trigger_focus.focus(window, cx);
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                })
                .when(self.commit_preview.is_none(), |view| {
                    view.child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .rounded_xl()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .overflow_hidden()
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .px_3()
                                    .py_1p5()
                                    .bg(Color::Inset.get(cx))
                                    .border_b_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .items_center()
                                            .gap_1p5()
                                            .child(Icon::new(Glyph::FileDiff).size(px(13.)).text_color(Color::Muted.get(cx)).flex_shrink_0())
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(
                                                        selected_change
                                                            .as_ref()
                                                            .map(|c| c.path.clone())
                                                            .unwrap_or_else(|| tr("文件差异").to_owned()),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_0p5()
                                            .p_0p5()
                                            .rounded_md()
                                            .bg(Color::Card.get(cx))
                                            .border_1()
                                            .border_color(Color::Divider.get(cx))
                                            .children([(Comparison::Unstaged, "未暂存"), (Comparison::Staged, "已暂存"), (Comparison::HeadWorktree, "全部")].into_iter().map(|(comparison, short)| {
                                                Button::new(comparison.wire())
                                                    .custom(segment_variant(self.comparison == comparison, cx))
                                                    .compact()
                                                    .xsmall()
                                                    .h(px(20.))
                                                    .px_1p5()
                                                    .rounded_sm()
                                                    .label(tr(short))
                                                    .tooltip(comparison.label())
                                                    .selected(self.comparison == comparison)
                                                    .disabled(!current || selected_change.is_none() || self.status_loading || self.preview_mock)
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.comparison = comparison;
                                                        this.load_diff(cx);
                                                    }))
                                            })),
                                    ),
                            )
                            .when(self.diff_loading && current, |col| {
                                col.child(div().p_3().text_xs().text_color(Color::Secondary.get(cx)).child(tr("正在读取文件差异…")))
                            })
                            .when_some(self.diff_error.as_ref(), |col, error| {
                                col.child(
                                    h_flex()
                                        .p_3()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(div().id("git-diff-error").role(Role::Status).text_xs().text_color(Color::Error.get(cx)).child(error.text()))
                                        .child(
                                            Button::new("retry-git-diff")
                                                .outline()
                                                .xsmall()
                                                .label(tr("重试文件差异"))
                                                .disabled(!current || self.diff_loading)
                                                .on_click(cx.listener(|this, _, _, cx| this.load_diff(cx))),
                                        ),
                                )
                            })
                            .when_some(self.diff.as_ref().filter(|_| current), |col, diff| {
                                let mut metadata = format!(
                                    "{}: {}\n{}: {}\ncomparison={} · content={} · binary={} · complete={}\n{}\n{}\n{}",
                                    tr("项目内路径"),
                                    diff.path,
                                    tr("仓库内路径"),
                                    diff.repository_path,
                                    diff.comparison,
                                    diff.content,
                                    diff.binary,
                                    diff.complete,
                                    diff.identity.description(),
                                    diff.before.description(&format!("{} · {}", tr("比较前"), diff.before_path.as_deref().unwrap_or("—"))),
                                    diff.after.description(&format!("{} · {}", tr("比较后"), diff.after_path.as_deref().unwrap_or("—"))),
                                );
                                if let Some(stages) = &diff.conflict_stages {
                                    for stage in stages {
                                        metadata.push_str(&format!("\n{}", stage.snapshot.description(&format!("stage {}", stage.stage))));
                                    }
                                }
                                col.when(!diff.complete || !diff.limitations.is_empty(), |view| {
                                    view.child(
                                        div()
                                            .id("git-diff-limitations")
                                            .role(Role::Status)
                                            .px_3()
                                            .py_2()
                                            .bg(Color::ErrorSurface.get(cx))
                                            .text_xs()
                                            .text_color(Color::Error.get(cx))
                                            .child(format!("{}\n{}", tr("此差异包含限制；元数据或冲突阶段不代表合并后的补丁"), diff.limitations.join("\n"))),
                                    )
                                })
                                .when(diff.binary, |view| {
                                    view.child(div().p_3().text_xs().text_color(Color::Secondary.get(cx)).child(tr("二进制内容仅显示元数据，不生成文本补丁")))
                                })
                                .when_some(diff.patch.as_ref(), |view, patch| {
                                    let appearance = cx.global::<crate::appearance::Appearance>();
                                    let code_size = f32::from(appearance.code_font_size());
                                    let code_wrap = appearance.code_word_wrap();
                                    let diff_mode = self.diff_state.read(cx).mode();
                                    let has_patch = !patch.trim().is_empty();
                                    let show_structured = !self.diff_raw && self.diff_has_structured;
                                    view.when(has_patch, |inner| {
                                        inner.child(
                                            h_flex()
                                                .flex_wrap()
                                                .items_center()
                                                .justify_between()
                                                .gap_2()
                                                .px_3()
                                                .py_1p5()
                                                .border_b_1()
                                                .border_color(Color::Divider.get(cx))
                                                .child(
                                                    h_flex()
                                                        .gap_0p5()
                                                        .p_0p5()
                                                        .rounded_md()
                                                        .bg(Color::Inset.get(cx))
                                                        .border_1()
                                                        .border_color(Color::Divider.get(cx))
                                                        .child(
                                                            Button::new("git-diff-unified")
                                                                .custom(segment_variant(!self.diff_raw && self.diff_has_structured && diff_mode == DiffMode::Unified, cx))
                                                                .compact().xsmall().h(px(20.)).px_2().rounded_sm()
                                                                .label(tr("合并视图"))
                                                                .selected(!self.diff_raw && self.diff_has_structured && diff_mode == DiffMode::Unified)
                                                                .disabled(!self.diff_has_structured)
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.diff_raw = false;
                                                                    this.file_patch_reader = None;
                                                                    this.diff_state.update(cx, |s, cx| s.set_mode(DiffMode::Unified, cx));
                                                                    cx.notify();
                                                                })),
                                                        )
                                                        .child(
                                                            Button::new("git-diff-split")
                                                                .custom(segment_variant(!self.diff_raw && self.diff_has_structured && diff_mode == DiffMode::Split, cx))
                                                                .compact().xsmall().h(px(20.)).px_2().rounded_sm()
                                                                .label(tr("并排视图"))
                                                                .selected(!self.diff_raw && self.diff_has_structured && diff_mode == DiffMode::Split)
                                                                .disabled(!self.diff_has_structured)
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.diff_raw = false;
                                                                    this.file_patch_reader = None;
                                                                    this.diff_state.update(cx, |s, cx| s.set_mode(DiffMode::Split, cx));
                                                                    cx.notify();
                                                                })),
                                                        )
                                                        .child(
                                                            Button::new("git-diff-raw")
                                                                .custom(segment_variant(self.diff_raw || !self.diff_has_structured, cx))
                                                                .compact().xsmall().h(px(20.)).px_2().rounded_sm()
                                                                .label(tr("原始补丁"))
                                                                .selected(self.diff_raw || !self.diff_has_structured)
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    if !this.diff_raw {
                                                                        this.file_patch_reader = None;
                                                                        this.diff_raw = true;
                                                                    }
                                                                    cx.notify();
                                                                })),
                                                        )
                                                        .child({
                                                            let inline_mode = crate::diffs_settings::InlineDiffMode::from_inline_unit(
                                                                self.diff_state.read(cx).inline_unit(),
                                                            );
                                                            Button::new("git-diff-inline-unit")
                                                                .custom(segment_variant(!self.diff_raw && self.diff_has_structured && inline_mode != crate::diffs_settings::InlineDiffMode::Off, cx))
                                                                .compact().xsmall().h(px(20.)).px_2().rounded_sm()
                                                                .label(inline_mode.label())
                                                                .tooltip(tr("切换行内局部差异高亮粒度（词级 / 字符级 / 关闭）"))
                                                                .disabled(self.diff_raw || !self.diff_has_structured)
                                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                                    let next = inline_mode.next();
                                                                    this.diff_state.update(cx, |s, cx| {
                                                                        s.set_inline_unit(next.into_inline_unit(), cx);
                                                                    });
                                                                    cx.notify();
                                                                }))
                                                        }),
                                                )
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .gap_1()
                                                        .child(
                                                            Button::new("git-diff-ignore-ws")
                                                                .custom(crate::palette::quiet_icon_variant(cx))
                                                                .compact().xsmall().h(px(22.)).px_2().rounded_md()
                                                                .label(tr("忽略空白"))
                                                                .selected(self.ignore_whitespace)
                                                                .toggled(self.ignore_whitespace)
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.ignore_whitespace = !this.ignore_whitespace;
                                                                    this.sync_diff_state(false, cx);
                                                                    cx.notify();
                                                                })),
                                                        )
                                                        .child(
                                                            div()
                                                                .id("git-diff-metadata")
                                                                .p_1()
                                                                .rounded_md()
                                                                .child(Icon::new(Glyph::Info).size(px(12.)).text_color(Color::Muted.get(cx)))
                                                                .tooltip(move |window, cx| Tooltip::new(metadata.clone()).build(window, cx)),
                                                        ),
                                                ),
                                        )
                                    })
                                    .when(show_structured && current && !self.preview_mock, |view| {
                                        view.child(
                                            div()
                                                .px_2()
                                                .text_xs()
                                                .text_color(Color::Secondary.get(cx))
                                                .child(tr("点击行旁的 + 添加批注。可拖动行号或按住 Shift 点击以选择多行。")),
                                        )
                                    })
                                    .child(if show_structured {
                                        let workspace = cx.weak_entity();
                                        let generation = self.diff_generation;
                                        div()
                                            .h(diff_height)
                                            .min_w_0()
                                            .bg(Color::Card.get(cx))
                                            .overflow_hidden()
                                            .child(
                                                Diff::new(&self.diff_state)
                                                    .soft_wrap(code_wrap)
                                                    .hover_highlight(DiffHoverHighlight::Both)
                                                    .change_indicator(DiffChangeIndicator::Signs)
                                                    .hunk_separator(DiffHunkSeparator::Metadata)
                                                    .when(current && !self.preview_mock, |diff| {
                                                        diff.on_add_annotation(move |range, window, cx| {
                                                            let _ = workspace.update(cx, |this, cx| {
                                                                if this.diff_generation == generation {
                                                                    this.open_diff_review(Some(range.clone()), window, cx);
                                                                }
                                                            });
                                                        })
                                                    })
                                                    .overflow_hidden()
                                                    .border_color(gpui::rgba(0)),
                                            )
                                            .into_any_element()
                                    } else {
                                        let reader = self.file_patch_reader.get_or_insert_with(|| {
                                            let text = if patch.is_empty() {
                                                tr("此比较没有文本内容差异").into()
                                            } else if self.ignore_whitespace && !self.diff_raw {
                                                let filtered = crate::diffs_settings::filter_patch_whitespace(patch);
                                                if filtered.trim().is_empty() {
                                                    tr("忽略空白后没有代码内容差异").into()
                                                } else {
                                                    SharedString::from(filtered)
                                                }
                                            } else {
                                                patch.clone()
                                            };
                                            PatchReader::new(text)
                                        });
                                        let region = crate::text_reading::region(
                                            "git-raw-patch-region".into(),
                                            tr("Git 原始字节补丁").to_owned(),
                                            &reader.scroll,
                                            diff_height,
                                            cx,
                                        )
                                        .font_family(Theme::global(cx).mono_font_family.clone())
                                        .text_size(px(code_size))
                                        .line_height(px((code_size * 1.55).round()))
                                        .when(code_wrap, |v| v.whitespace_normal());
                                        reader.view("git-raw-patch", region, |this| this.file_patch_reader.as_mut(), cx)
                                        .into_any_element()
                                    })
                                })
                                .when(diff.patch.is_none() && !diff.binary, |view| {
                                    view.child(div().p_3().text_xs().text_color(Color::Secondary.get(cx)).child(tr("后台未提供文本补丁；请查看文件类型、对象标识和限制说明")))
                                })
                            }),
                    )
                })
                .into_any_element()
        };

        if self.in_drawer {
            v_flex()
                .size_full()
                .min_w_0()
                .min_h_0()
                .bg(Color::Surface.get(cx))
                .child(header)
                .child(
                    div().flex_1().min_h_0().min_w_0().flex().child(Viewport::new(
                        "git-drawer-viewport",
                        &self.body_scroll,
                        v_flex()
                            .id("git-drawer-body")
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.body_scroll)
                            .vertical_scrollbar(&self.body_scroll)
                            .p_3()
                            .child(body),
                    )),
                )
                .into_any_element()
        } else {
            v_flex()
                .flex_shrink_0()
                .min_w_0()
                .gap_3()
                .child(header)
                .child(body)
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{commit_result_label, Change, CommitPatchResponse, CommitPreview, CommitResult, FileDiff, IndexApplyResponse, IndexOperation, IndexPreview, Status, decode, git_params, valid_write_state};
    #[test]
    fn commit_result_accessible_label_includes_reason_and_warnings() {
        assert_eq!(
            commit_result_label("No commit was created.", &["[INDEX_BUSY] Git index.lock exists".into(), "Inspect the lock".into()]),
            "No commit was created.\n[INDEX_BUSY] Git index.lock exists\nInspect the lock"
        );
        assert_eq!(commit_result_label("Commit created.", &[]), "Commit created.");
    }
    use serde_json::json;
    fn identity() -> serde_json::Value {
        json!({"projectRoot":"/repo/project","repositoryRoot":"/repo","projectPrefix":"project/","linkedWorktree":true,"branch":null,"headOid":"abc","detached":true,"unborn":false})
    }
    #[test]
    fn commit_wire_preserves_outside_paths_and_uncertain_outcomes() {
        let preview: CommitPreview = decode(json!({"ok":true,"preview":{
            "version":"v", "workspace":identity(), "changes":[
                {"repositoryPath":"outside/new", "originalRepositoryPath":"outside/old", "projectPath":null,"status":"R100"}],
            "outsideProjectPaths":["outside/new", "outside/old"],
            "signingRequired":true, "hooks":["pre-commit"], "hooksMayModifyIndex":true
        }}), "preview").unwrap();
        assert_eq!(preview.outside_project_paths, ["outside/new", "outside/old"]);
        assert_eq!(preview.changes[0].original_repository_path.as_deref(), Some("outside/old"));
        assert!(preview.signing_required && preview.hooks_may_modify_index);
        let patch: CommitPatchResponse = decode(json!({"ok":true,"preview":{
            "version":"v", "patch":"diff content"
        }}), "preview").unwrap();
        assert_eq!(patch.version, preview.version);
        assert_eq!(patch.patch, "diff content");
        let result: CommitResult = serde_json::from_value(json!({"ok":false,"committed":"unknown","applied":"unknown",
            "commitOid":null,"observedHeadOid":"abc","indexUpdated":false,"warnings":["inspect"],
            "error":{"code":"COMMIT_STATE_UNCERTAIN","message":"check HEAD"}})).unwrap();
        assert!(valid_write_state(&result.committed) && valid_write_state(&result.applied));
        assert_eq!(result.observed_head_oid.as_deref(), Some("abc"));
        assert_eq!(result.warnings, ["inspect"]);
        assert_eq!(result.error.unwrap().code, "COMMIT_STATE_UNCERTAIN");
        assert!(!valid_write_state(&json!("other")));
        let result: CommitResult = serde_json::from_value(json!({"ok":false,"committed":true,"applied":true,
            "commitOid":"created", "indexUpdated":false,"warnings":[]})).unwrap();
        assert_eq!(result.commit_oid.as_deref(), Some("created"));
        assert!(!result.index_updated);
        assert_eq!(git_params(json!({"workspaceId":"w"}), &Some("s".into()))["sessionId"], "s");
        assert!(git_params(json!({"workspaceId":"w"}), &None).get("sessionId").is_none());
    }
    #[test]
    fn index_wire_preserves_rename_filters_and_applied_failure() {
        let preview: IndexPreview = decode(json!({"ok":true,"preview":{
            "operation":"stage", "version":"version", "workspace":identity(),
            "paths":["old name", "new name"], "files":[], "contentFilters":true
        }}), "preview").unwrap();
        assert_eq!(preview.operation, IndexOperation::Stage);
        assert_eq!(preview.paths, ["old name", "new name"]);
        assert!(preview.content_filters);
        let failure: IndexApplyResponse = serde_json::from_value(json!({
            "ok":false,"error":{"code":"INDEX_UPDATED_REFRESH_FAILED","message":"refresh failed"},"applied":true
        })).unwrap();
        assert!(!failure.ok && failure.applied);
        assert_eq!(failure.error.unwrap().code, "INDEX_UPDATED_REFRESH_FAILED");
        let success: IndexApplyResponse = serde_json::from_value(json!({
            "ok":true, "operation":"unstage", "paths":["a"], "workspace":identity()
        })).unwrap();
        assert!(success.ok && !success.applied);
        assert_eq!(success.operation, Some(IndexOperation::Unstage));
        assert_eq!(success.paths.unwrap(), ["a"]);
    }
    #[test]
    fn commit_preview_uses_repository_count_and_keeps_legacy_status_usable() {
        let mut wire = identity();
        wire.as_object_mut().unwrap().extend(json!({
            "entries":[],"total":102,"offset":0,"limit":100,"hasMore":true,
            "complete":true,"warnings":[],"scope":"project",
            "submoduleDetails":"not-scanned","globalConfiguration":"read-only"
        }).as_object().unwrap().clone());
        let legacy: Status = decode(json!({"ok":true,"status":wire.clone()}), "status").unwrap();
        assert_eq!(legacy.repository_staged_total, None);
        assert!(legacy.can_preview_commit(), "legacy pages defer to authoritative preflight");
        wire["repositoryStagedTotal"] = json!(1);
        let off_page: Status = decode(json!({"ok":true,"status":wire.clone()}), "status").unwrap();
        assert!(off_page.can_preview_commit());
        wire["repositoryStagedTotal"] = json!(0);
        let empty: Status = decode(json!({"ok":true,"status":wire}), "status").unwrap();
        assert!(!empty.can_preview_commit());
        let mut incomplete = empty;
        incomplete.complete = false;
        assert!(incomplete.can_preview_commit(), "incomplete scans defer to preflight");
    }
    #[test]
    fn git_wire_retains_scope_warning_rename_and_conflict_identity() {
        let mut status = identity();
        status.as_object_mut().unwrap().extend(json!({"entries":[{"path":"新\n名.txt","repositoryPath":"project/新\n名.txt","originalPath":"旧名.txt","pathRole":"rename-source","kind":"unmerged","indexStatus":"U","worktreeStatus":"U","conflictStages":[{"stage":2,"mode":"100644","oid":"stage-two"}]}],"total":201,"offset":100,"limit":100,"hasMore":true,"complete":false,"warnings":["warning"],"scope":"project","submoduleDetails":"not-scanned","globalConfiguration":"read-only"}).as_object().unwrap().clone());
        let status: Status = decode(json!({"ok":true,"status":status}), "status").unwrap();
        assert_eq!(status.identity.repository_root, "/repo");
        assert!(status.identity.detached && status.identity.linked_worktree);
        assert!(!status.complete);
        assert_eq!(status.offset, 100);
        assert_eq!(status.total, 201);
        let change: &Change = &status.entries[0];
        assert_eq!(change.path, "新\n名.txt");
        assert!(change.reveal_path().is_none());
        assert_eq!(change.original_path.as_deref(), Some("旧名.txt"));
        assert_eq!(change.conflict_stages.as_ref().unwrap()[0].oid, "stage-two");
        assert!(change.title().contains("\\n"));
        assert!(
            change
                .metadata()
                .contains("stage 2: mode=100644 oid=stage-two")
        );
    }

    #[test]
    fn git_reveal_uses_the_live_destination_of_a_rename() {
        let mut change: Change = serde_json::from_value(json!({
            "path":"nested/destination.txt", "repositoryPath":"project/nested/destination.txt",
            "originalPath":"original.txt", "pathRole":"target", "kind":"rename",
            "indexStatus":"R", "worktreeStatus":".", "worktreeMode":"100644"
        })).unwrap();
        assert_eq!(change.reveal_path(), Some("nested/destination.txt"));
        change.worktree_status = "D".into();
        assert_eq!(change.reveal_path(), None);
        change.worktree_status = ".".into();
        change.worktree_mode = Some("000000".into());
        assert_eq!(change.reveal_path(), None);
    }
    #[test]
    fn git_diff_binary_and_errors_are_not_fabricated_text_patches() {
        let mut diff = identity();
        diff.as_object_mut().unwrap().extend(json!({"path":"asset.bin","repositoryPath":"project/asset.bin","comparison":"staged","before":{"kind":"binary","bytes":2,"mode":"100644","oid":"old"},"after":{"kind":"binary","bytes":3,"mode":"100755","oid":"new"},"binary":true,"complete":true,"limitations":[],"content":"raw"}).as_object().unwrap().clone());
        let diff: FileDiff = decode(json!({"ok":true,"diff":diff}), "diff").unwrap();
        assert!(diff.binary && diff.patch.is_none());
        assert!(diff.before_path.is_none() && diff.after_path.is_none());
        assert_eq!(diff.content, "raw");
        assert_eq!(diff.after.oid.as_deref(), Some("new"));
        let fault = decode::<FileDiff>(
            json!({"ok":false,"error":{"code":"CONCURRENT_CHANGE","message":"Index changed"}}),
            "diff",
        )
        .err()
        .unwrap();
        assert_eq!(fault.code, "CONCURRENT_CHANGE");
        assert!(fault.text().contains("Index changed"));
        assert!(decode::<FileDiff>(json!({"ok":true}), "diff").is_err());
    }
    #[test]
    fn git_unchanged_rename_retains_each_comparisons_actual_paths() {
        for (comparison, before_path) in [
            ("staged", "旧名.txt"),
            ("head-worktree", "旧名.txt"),
            ("unstaged", "新名.txt"),
        ] {
            let mut value = identity();
            value.as_object_mut().unwrap().extend(
                json!({
                    "path":"新名.txt", "repositoryPath":"project/新名.txt", "comparison":comparison,
                    "beforePath":before_path, "afterPath":"新名.txt",
                    "before":{"kind":"text","bytes":4,"text":"same","mode":"100644"},
                    "after":{"kind":"text","bytes":4,"text":"same","mode":"100755"},
                    "patch":"", "binary":false,"complete":true,"limitations":[],"content":"raw"
                })
                .as_object()
                .unwrap()
                .clone(),
            );
            let diff: FileDiff = decode(json!({"ok":true,"diff":value}), "diff").unwrap();
            assert_eq!(diff.before_path.as_deref(), Some(before_path));
            assert_eq!(diff.after_path.as_deref(), Some("新名.txt"));
            assert_eq!(diff.before.kind, "text");
            assert_eq!(diff.after.kind, "text");
            assert_eq!(diff.patch.as_deref(), Some(""));
            assert_ne!(diff.before.mode, diff.after.mode);
        }
    }
}
