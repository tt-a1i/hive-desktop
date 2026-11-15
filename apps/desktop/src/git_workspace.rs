mod remotes;
use crate::{
    app::HiveApp,
    focus_scroll::{Reveal, Viewport},
    language::tr,
    palette::Color,
    transport::Runtime,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonCustomVariant, ButtonVariants},
        checkbox::Checkbox,
        diff::{
            Diff, DiffChangeIndicator, DiffFile, DiffHoverHighlight, DiffHunkSeparator, DiffMode,
            DiffState,
        },
        input::{Textarea, TextareaState},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

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
    fn description(&self) -> String {
        let branch = if self.unborn {
            format!(
                "{} · {}",
                self.branch.as_deref().unwrap_or("HEAD"),
                tr("尚无提交")
            )
        } else if self.detached {
            tr("分离 HEAD").to_owned()
        } else {
            self.branch.clone().unwrap_or_else(|| "HEAD".to_owned())
        };
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
    complete: bool,
    warnings: Vec<String>,
    scope: String,
    submodule_details: String,
    global_configuration: String,
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
    patch: Option<String>,
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
    patch: String,
    outside_project_paths: Vec<String>,
    signing_required: bool,
    hooks: Vec<String>,
    hooks_may_modify_index: bool,
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
fn decode<T: serde::de::DeserializeOwned>(value: Value, field: &str) -> Result<T, Fault> {
    if value["ok"] == true {
        serde_json::from_value(value[field].clone()).map_err(|error| Fault {
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
struct GitWorkspace {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    workspace: String,
    session: Option<String>,
    project_name: String,
    connection: u64,
    open_request: u64,
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
    files_scroll: ScrollHandle,
    diff_scroll: ScrollHandle,
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
    commit_include_outside: bool,
    commit_result: Option<CommitResult>,
    commit_scroll: ScrollHandle,
    outside_scroll: ScrollHandle,
    commit_patch_scroll: ScrollHandle,
    commit_patch_visible: bool,
    remotes: Entity<remotes::RemotePanel>,
    show_remotes: bool,
}
impl HiveApp {
    pub(crate) fn open_git_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected || self.closing || window.has_active_dialog(cx) {
            return;
        }
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let project_name = self
            .workspaces
            .iter()
            .find(|project| project.id == workspace)
            .map(|project| project.name.clone())
            .unwrap_or_else(|| workspace.clone());
        let session = self.session.as_ref().filter(|session| session.workspace_id == workspace).map(|session| session.id.clone());
        let app = cx.weak_entity();
        let connection = self.connection_generation;
        let open_request = self.open_request;
        let state = cx.new(|cx| {
            if let Some(parent) = app.upgrade() {
                cx.observe(&parent, |this: &mut GitWorkspace, _, cx| {
                    if !this.current(cx) {
                        this.index_generation += 1;
                        this.index_preview = None;
                        this.commit_preview = None;
                        if this.index_busy { this.index_refresh_required = true; }
                    }
                    cx.notify();
                }).detach();
            }
            let remotes = cx.new(|cx| remotes::RemotePanel::new(app.clone(), runtime.clone(), workspace.clone(), session.clone(), connection, open_request, window, cx));
            cx.observe(&remotes, |_, _, cx| cx.notify()).detach();
            let state = GitWorkspace {
                app,
                runtime,
                workspace,
                session,
                project_name,
                connection,
                open_request,
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
                files_scroll: ScrollHandle::new(),
                diff_scroll: ScrollHandle::new(),
                diff_state: cx.new(|cx| {
                    DiffState::new([], cx).with_mode(
                        crate::diffs_settings::current(cx)
                            .default_diff_mode
                            .into_diff_mode(),
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
                commit_message: cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 5).placeholder(tr("提交说明"))),
                commit_include_outside: false,
                commit_result: None,
                commit_scroll: ScrollHandle::new(),
                outside_scroll: ScrollHandle::new(),
                commit_patch_scroll: ScrollHandle::new(),
                commit_patch_visible: false,
                remotes,
                show_remotes: false,
            };
            // The parent HiveApp is still mutably borrowed by this action.
            // Validate it only after that update completes.
            let pending = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = pending.update(cx, |state, cx| state.refresh(cx));
            });
            state
        });
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
}
impl GitWorkspace {
    fn busy(&self, cx: &App) -> bool { self.index_busy || self.remotes.read(cx).busy }
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|app| {
            let app = app.read(cx);
            app.connected
                && !app.closing
                && app.workspace.as_ref() == Some(&self.workspace)
                && app.connection_generation == self.connection
                && app.open_request == self.open_request
                && app.session.as_ref().filter(|session| session.workspace_id == self.workspace).map(|session| &session.id) == self.session.as_ref()
                && app.runtime.as_ref().is_some_and(|runtime| Arc::ptr_eq(runtime, &self.runtime))
        })
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy(cx) || !self.current(cx) { return; }
        self.remotes.update(cx, |remote, cx| remote.invalidate(cx));
        self.index_generation += 1;
        self.index_preview = None;
        self.commit_preview = None;
        self.commit_include_outside = false;
        self.commit_patch_visible = false;
        self.status_generation += 1;
        self.diff_generation += 1;
        let generation = self.status_generation;
        let offset = self.offset;
        let selected_path = self.selected_change().map(|change| change.path.clone());
        self.status = None;
        self.status_error = None;
        self.status_loading = true;
        self.diff = None;
        self.diff_has_structured = false;
        self.diff_state.update(cx, |s, cx| s.set_files([], cx));
        self.diff_error = None;
        self.diff_loading = false;
        self.files_scroll = ScrollHandle::new();
        self.diff_scroll = ScrollHandle::new();
        cx.notify();
        let runtime = self.runtime.clone();
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
        self.commit_preview = None;
        self.commit_include_outside = false;
        self.index_error = None;
        self.index_notice = None;
        self.diff_generation += 1;
        self.diff = None;
        self.diff_has_structured = false;
        self.diff_state.update(cx, |s, cx| s.set_files([], cx));
        self.diff_error = None;
        self.diff_loading = false;
        self.files_scroll.scroll_to_item(index);
        cx.notify();
        if load {
            self.load_diff(cx);
        }
    }
    fn load_diff(&mut self, cx: &mut Context<Self>) {
        if !self.current(cx) || self.status_loading {
            return;
        }
        let Some(path) = self.selected_change().map(|change| change.path.clone()) else {
            return;
        };
        self.diff_generation += 1;
        let generation = self.diff_generation;
        self.diff = None;
        self.diff_has_structured = false;
        self.diff_state.update(cx, |s, cx| s.set_files([], cx));
        self.diff_error = None;
        self.diff_loading = true;
        self.diff_scroll = ScrollHandle::new();
        cx.notify();
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        let comparison = self.comparison.wire();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "git.diff",
                    git_params(json!({"workspaceId":workspace,"path":path,"comparison":comparison}), &session),
                )
                .await
                .map_err(Fault::transport)
                .and_then(|value| decode::<FileDiff>(value, "diff"));
            let _ = this.update(cx, |this, cx| {
                if this.diff_generation != generation {
                    return;
                }
                this.diff_loading = false;
                if !this.current(cx) {
                    cx.notify();
                    return;
                }
                match result {
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
            crate::diffs_settings::filter_patch_whitespace(patch)
        } else {
            patch.to_owned()
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
        let Some(path) = self.selected_change().map(|change| change.path.clone()) else { return; };
        self.index_generation += 1;
        let generation = self.index_generation;
        self.index_busy = true;
        self.index_preview = None;
        self.commit_preview = None;
        self.commit_include_outside = false;
        self.index_error = None;
        self.index_notice = None;
        let runtime = self.runtime.clone();
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
        let Some(pending) = self.index_preview.take() else { return; };
        if self.selected_change().map(|change| &change.path) != Some(&pending.selected_path) { return; }
        self.index_generation += 1;
        let generation = self.index_generation;
        self.index_busy = true;
        self.index_error = None;
        let runtime = self.runtime.clone();
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
        self.index_generation += 1;
        let generation = self.index_generation;
        self.index_busy = true;
        self.index_preview = None;
        self.commit_preview = None;
        self.commit_include_outside = false;
        self.commit_patch_visible = false;
        self.commit_result = None;
        self.index_error = None;
        self.index_notice = None;
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        let session = self.session.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this.update(cx, |this, cx| {
                let valid = this.index_generation == generation && this.current(cx);
                if !valid { this.index_busy = false; cx.notify(); }
                valid
            }).ok() != Some(true) { return; }
            let result = runtime.request("git.commit.preview", git_params(json!({"workspaceId":workspace}), &session)).await
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
    fn apply_commit(&mut self, cx: &mut Context<Self>) {
        if !self.current(cx) || self.busy(cx) || self.status_loading || self.index_refresh_required { return; }
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
        self.commit_preview = None;
        self.index_generation += 1;
        let generation = self.index_generation;
        self.index_busy = true;
        self.index_error = None;
        self.commit_result = None;
        let runtime = self.runtime.clone();
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
}
impl Render for GitWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy(cx);
        let tabs = h_flex().flex_wrap().gap_2()
            .child(Button::new("git-local-panel").outline().small().label(tr("本地 Git"))
                .selected(!self.show_remotes).disabled(busy)
                .on_click(cx.listener(|this, _, _, cx| { if this.busy(cx) { return; } this.show_remotes = false; this.refresh(cx); cx.notify(); })))
            .child(Button::new("git-remotes-panel").outline().small().label(tr("远端与 PR"))
                .selected(self.show_remotes).disabled(busy)
                .on_click(cx.listener(|this, _, _, cx| { if this.busy(cx) { return; } this.show_remotes = true; this.remotes.update(cx, |remote, cx| remote.activate(cx)); cx.notify(); })));
        if self.show_remotes {
            return v_flex().flex_shrink_0().min_w_0().gap_2().child(tabs).child(self.remotes.clone()).into_any_element();
        }
        let current = self.current(cx);
        let keys = cx.entity();
        let list_height = px(180.).min(window.viewport_size().height * 0.25);
        let diff_height = px(360.).min(window.viewport_size().height * 0.4);
        let selected_metadata = self
            .selected_change()
            .filter(|_| current)
            .map(Change::metadata);
        v_flex().flex_shrink_0().min_w_0().gap_2().child(tabs)
            .child(h_flex().min_w_0().items_center().gap_2()
                .child(div().flex_1().min_w_0().truncate().child(self.project_name.clone()))
                .child(Button::new("refresh-git-workspace").outline().small().label(tr("刷新 Git 状态")).disabled(!current || self.status_loading || self.index_busy)
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)))))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("项目范围内的 Git 状态；不扫描子模块内部文件或远端领先落后信息。")))
            .when(!current, |view| view.child(div().id("git-context-changed").role(Role::Status).text_sm().text_color(Color::Error.get(cx))
                .child(tr("项目或后台连接已变化，请关闭后重新打开 Git 状态。"))))
            .when(self.status_loading && current, |view| view.child(div().text_sm().child(tr("正在读取 Git 状态…"))))
            .when_some(self.status_error.as_ref(), |view, error| view.child(div().id("git-status-error").role(Role::Status)
                .text_sm().text_color(Color::Error.get(cx)).child(error.text())))
            .when_some(self.status.as_ref().filter(|_| current), |view, status| {
                view.child(crate::text_reading::selectable_text("git-workspace-identity", status.identity.description()))
                    .child(div().text_xs().child(format!("complete={} · scope={} · submoduleDetails={} · globalConfiguration={}", status.complete, status.scope, status.submodule_details, status.global_configuration)))
                    .when(!status.complete || !status.warnings.is_empty(), |view| view.child(div().id("git-status-warnings").role(Role::Status).text_sm().text_color(Color::Error.get(cx))
                        .child(format!("{}\n{}", tr("Git 状态读取不完整，请查看警告"), status.warnings.join("\n")))))
                    .child(div().h(list_height).flex().min_h_0().child(Viewport::new("git-files-viewport", &self.files_scroll,
                        v_flex().id("git-changed-files").role(Role::List).aria_label(tr("Git 改动文件")).tab_index(0)
                            .capture_key_down(move |event, _, cx| keys.update(cx, |this, cx| this.key(event, cx)))
                            .flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(&self.files_scroll).vertical_scrollbar(&self.files_scroll)
                            .when(status.entries.is_empty(), |list| list.child(div().p_2().text_sm().child(if status.total == 0 && status.complete { tr("项目范围内没有改动") } else { tr("当前页没有文件，请刷新或返回上一页") })))
                            .children(status.entries.iter().enumerate().map(|(index, change)| Reveal::new(("git-file-reveal", index), &self.files_scroll,
                                Button::new(("git-changed-file", index)).ghost().compact().w_full().min_w_0().justify_start().selected(index == self.selected)
                                    .disabled(!current || self.index_busy).accessibility_label(change.title()).tooltip(change.metadata())
                                    .child(div().min_w_0().truncate().text_sm().child(change.title()))
                                    .on_click(cx.listener(move |this, _, _, cx| this.select(index, true, cx)))))))))
                    .child(h_flex().items_center().justify_between().gap_2()
                        .child(div().text_xs().child(format!("{}–{} / {}", if status.entries.is_empty() {0} else {status.offset + 1}, status.offset + status.entries.len(), status.total)))
                        .child(h_flex().gap_1()
                            .child(Button::new("git-files-prev").outline().small().label(tr("上一页")).disabled(!current || self.index_busy || self.offset == 0)
                                .on_click(cx.listener(|this, _, _, cx| { let limit = this.status.as_ref().map(|status| status.limit).unwrap_or(100); this.offset = this.offset.saturating_sub(limit); this.refresh(cx); })))
                            .child(Button::new("git-files-next").outline().small().label(tr("下一页")).disabled(!current || self.index_busy || !status.has_more)
                                .on_click(cx.listener(|this, _, _, cx| { this.offset += this.status.as_ref().map(|status| status.limit).unwrap_or(100); this.refresh(cx); })))))
            })
            .child(h_flex().flex_wrap().gap_1()
                .child(Button::new("git-stage-selected").outline().small().label(tr("暂存所选文件"))
                    .disabled(!current || self.status_loading || self.index_busy || self.index_refresh_required || self.selected_change().is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.preview_index(IndexOperation::Stage, cx))))
                .child(Button::new("git-unstage-selected").outline().small().label(tr("撤销暂存所选文件"))
                    .disabled(!current || self.status_loading || self.index_busy || self.index_refresh_required || self.selected_change().is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.preview_index(IndexOperation::Unstage, cx))))
                .child(Button::new("git-commit-preview").outline().small().label(tr("提交已暂存改动"))
                    .disabled(!current || self.status_loading || self.index_busy || self.index_refresh_required || self.status.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.preview_commit(cx)))))
            .when(self.index_busy && current, |view| view.child(div().id("git-index-busy").role(Role::Status).text_sm().child(tr("正在处理 Git 操作…"))))
            .when_some(self.index_error.as_ref(), |view, error| view.child(div().id("git-index-error").role(Role::Status).text_sm().text_color(Color::Error.get(cx))
                .child(format!("{}\n[{}] {}", tr("Git 操作失败"), error.code, crate::language::localize_owned_error(&error.message)))))
            .when(self.index_refresh_required && self.index_notice != Some("Git 索引已更改，请刷新状态后再操作。"), |view| view.child(div().id("git-index-refresh-required").role(Role::Status).text_sm().text_color(Color::Error.get(cx))
                .child(tr("Git 索引可能已更改，请刷新状态后再操作。"))))
            .when_some(self.index_notice, |view, notice| view.child(div().id("git-index-notice").role(Role::Status).text_sm().child(tr(notice))))
            .when_some(self.index_preview.as_ref().filter(|_| current), |view, pending| {
                let paths = pending.preview.paths.iter().map(|path| path.escape_debug().to_string()).collect::<Vec<_>>().join("\n");
                view.child(v_flex().gap_1().p_2().rounded_lg().bg(Color::Inset.get(cx))
                    .child(div().text_sm().child(pending.preview.operation.confirm_label()))
                    .child(crate::text_reading::selectable_text("git-index-preview-paths", format!("{}:\n{}", tr("受影响的项目内路径"), paths)))
                    .when(pending.preview.content_filters, |view| view.child(div().text_sm().text_color(Color::Error.get(cx)).child(tr("暂存会按仓库配置运行内容过滤器。"))))
                    .child(h_flex().gap_1()
                        .child(Button::new("git-index-confirm").outline().small().label(pending.preview.operation.confirm_label()).disabled(self.index_busy)
                            .on_click(cx.listener(|this, _, _, cx| this.apply_index(cx))))
                        .child(Button::new("git-index-cancel").ghost().small().label(tr("取消")).disabled(self.index_busy)
                            .on_click(cx.listener(|this, _, _, cx| { this.index_generation += 1; this.index_preview = None; cx.notify(); })))))
            })
            .when_some(self.commit_result.as_ref(), |view, result| {
                let state = if result.committed == json!(true) {
                    if result.index_updated { tr("提交已创建，索引已同步。") } else { tr("提交已创建，但索引未同步；请刷新并检查。") }
                } else if result.committed == json!(false) && result.applied == json!(false) {
                    tr("提交未创建；请刷新并检查后重试。")
                } else { tr("提交状态不确定；请刷新并核对 HEAD，勿自动重试。") };
                let mut details = format!("{}\nok={} · committed={} · applied={} · indexUpdated={}", state, result.ok, result.committed, result.applied, result.index_updated);
                if result.committed == json!(true) || result.commit_oid.is_some() {
                    details.push_str(&format!("\ncommitOid: {}", result.commit_oid.as_deref().unwrap_or("—")));
                }
                if result.committed == json!("unknown") || result.observed_head_oid.is_some() {
                    details.push_str(&format!("\nobservedHeadOid: {}", result.observed_head_oid.as_deref().unwrap_or("—")));
                }
                if let Some(workspace) = &result.workspace { details.push_str(&format!("\n{}", workspace.description())); }
                if let Some(error) = &result.error { details.push_str(&format!("\n[{}] {}", error.code, error.message)); }
                for warning in &result.warnings { details.push_str(&format!("\n{warning}")); }
                view.child(div().id("git-commit-result").role(Role::Status).text_sm().child(details))
            })
            .when_some(self.commit_preview.as_ref().filter(|_| current), |view, preview| {
                let paths = preview.changes.iter().map(|change| {
                    let from = change.original_repository_path.as_ref().map(|path| format!("{} → ", path.escape_debug())).unwrap_or_default();
                    format!("{}{} · {}{}", from, change.repository_path.escape_debug(), change.status,
                        if change.project_path.is_none() { " · outside project" } else { "" })
                }).collect::<Vec<_>>().join("\n");
                let outside = preview.outside_project_paths.iter().map(|path| path.escape_debug().to_string()).collect::<Vec<_>>().join("\n");
                let hooks = if preview.hooks.is_empty() { tr("无").to_owned() } else { preview.hooks.join(", ") };
                view.child(v_flex().gap_1().p_2().rounded_lg().bg(Color::Inset.get(cx))
                    .child(div().text_sm().child(tr("完整仓库的已暂存改动")))
                    .child(crate::text_reading::region("git-commit-paths-region".into(), tr("完整仓库的已暂存改动").to_owned(), &self.commit_scroll, px(130.), cx)
                        .child(crate::text_reading::selectable_text("git-commit-paths", paths)))
                    .child(div().text_xs().child(format!("{}: {} · {}: {}", tr("原生 Git hooks"), hooks, tr("需要签名"), if preview.signing_required { tr("是") } else { tr("否") })))
                    .child(Button::new("git-commit-toggle-patch").ghost().small().label(if self.commit_patch_visible { tr("隐藏已暂存补丁") } else { tr("查看已暂存补丁") })
                        .on_click(cx.listener(|this, _, _, cx| { this.commit_patch_visible = !this.commit_patch_visible; cx.notify(); })))
                    .when(self.commit_patch_visible, |view| view.child(crate::text_reading::region("git-commit-patch-region".into(), tr("已暂存补丁").to_owned(), &self.commit_patch_scroll, px(110.), cx)
                        .child(crate::text_reading::selectable_text("git-commit-patch", preview.patch.clone()))))
                    .when(preview.hooks_may_modify_index, |view| view.child(div().text_xs().child(tr("原生 Git hooks 可能修改文件、索引和提交说明；签名使用仓库配置。"))))
                    .when(!preview.outside_project_paths.is_empty(), |view| view
                        .child(crate::text_reading::region("git-commit-outside-region".into(), tr("项目范围外的已暂存路径").to_owned(), &self.outside_scroll, px(90.), cx)
                            .child(crate::text_reading::selectable_text("git-commit-outside-paths", format!("{}:\n{}", tr("项目范围外的已暂存路径"), outside))))
                        .child(Checkbox::new("git-commit-include-outside").label(tr("我确认提交包含项目范围外的路径"))
                            .checked(self.commit_include_outside).disabled(self.index_busy)
                            .on_change(cx.listener(|this, value, _, cx| { this.commit_include_outside = *value; cx.notify(); }))))
                    .child(Textarea::new(&self.commit_message).disabled(self.index_busy))
                    .child(h_flex().gap_1()
                        .child(Button::new("git-commit-confirm").outline().small().label(tr("确认提交"))
                            .disabled(self.index_busy || (!preview.outside_project_paths.is_empty() && !self.commit_include_outside))
                            .on_click(cx.listener(|this, _, _, cx| this.apply_commit(cx))))
                        .child(Button::new("git-commit-cancel").ghost().small().label(tr("取消")).disabled(self.index_busy)
                            .on_click(cx.listener(|this, _, _, cx| { this.index_generation += 1; this.commit_preview = None; this.commit_include_outside = false; cx.notify(); })))))
            })
            .when_some(selected_metadata, |view, metadata| view.child(crate::text_reading::selectable_text("git-selected-file-metadata", metadata)))
            .child(h_flex().flex_wrap().gap_1().children([Comparison::Unstaged, Comparison::Staged, Comparison::HeadWorktree].into_iter().map(|comparison|
                Button::new(comparison.wire()).outline().small().label(comparison.label()).selected(self.comparison == comparison)
                    .disabled(!current || self.selected_change().is_none() || self.status_loading)
                    .on_click(cx.listener(move |this, _, _, cx| { this.comparison = comparison; this.load_diff(cx); })))))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("↑↓ 选择文件 · Enter 查看差异；补丁只比较原始字节，不运行 Git 内容转换或外部差异程序。")))
            .when(self.diff_loading && current, |view| view.child(div().text_sm().child(tr("正在读取文件差异…"))))
            .when_some(self.diff_error.as_ref(), |view, error| view.child(v_flex().gap_1()
                .child(div().id("git-diff-error").role(Role::Status).text_sm().text_color(Color::Error.get(cx)).child(error.text()))
                .child(Button::new("retry-git-diff").outline().small().label(tr("重试文件差异")).disabled(!current || self.diff_loading).on_click(cx.listener(|this, _, _, cx| this.load_diff(cx))))))
            .when_some(self.diff.as_ref().filter(|_| current && self.commit_preview.is_none()), |view, diff| {
                let mut metadata = format!("{}: {}\n{}: {}\ncomparison={} · content={} · binary={} · complete={}\n{}\n{}\n{}",
                    tr("项目内路径"), diff.path, tr("仓库内路径"), diff.repository_path, diff.comparison, diff.content, diff.binary, diff.complete,
                    diff.identity.description(),
                    diff.before.description(&format!("{} · {}", tr("比较前"), diff.before_path.as_deref().unwrap_or("—"))),
                    diff.after.description(&format!("{} · {}", tr("比较后"), diff.after_path.as_deref().unwrap_or("—"))));
                if let Some(stages) = &diff.conflict_stages { for stage in stages { metadata.push_str(&format!("\n{}", stage.snapshot.description(&format!("stage {}", stage.stage)))); } }
                view.child(crate::text_reading::selectable_text("git-diff-metadata", metadata))
                    .when(!diff.complete || !diff.limitations.is_empty(), |view| view.child(div().id("git-diff-limitations").role(Role::Status).text_sm().text_color(Color::Error.get(cx))
                        .child(format!("{}\n{}", tr("此差异包含限制；元数据或冲突阶段不代表合并后的补丁"), diff.limitations.join("\n")))))
                    .when(diff.binary, |view| view.child(div().text_sm().child(tr("二进制内容仅显示元数据，不生成文本补丁"))))
                    .when_some(diff.patch.as_ref(), |view, patch| {
                        let appearance = cx.global::<crate::appearance::Appearance>();
                        let code_size = f32::from(appearance.code_font_size());
                        let code_wrap = appearance.code_word_wrap();
                        let diff_mode = self.diff_state.read(cx).mode();
                        let has_patch = !patch.trim().is_empty();
                        let show_structured = !self.diff_raw && self.diff_has_structured;
                        let segment_variant = |cx: &App| {
                            ButtonCustomVariant::new(cx)
                                .color(gpui::rgba(0).into())
                                .foreground(Color::Text.get(cx).into())
                                .hover(Color::Card.get(cx).opacity(0.65).into())
                                .active(Color::Card.get(cx).into())
                        };
                        let raw_display = if patch.is_empty() {
                            tr("此比较没有文本内容差异").to_owned()
                        } else if self.ignore_whitespace && !self.diff_raw {
                            let filtered = crate::diffs_settings::filter_patch_whitespace(patch);
                            if filtered.trim().is_empty() {
                                tr("忽略空白后没有代码内容差异").to_owned()
                            } else {
                                filtered
                            }
                        } else {
                            patch.clone()
                        };
                        view.child(
                            v_flex()
                                .min_w_0()
                                .gap_2()
                                .when(has_patch, |col| {
                                    col.child(
                                        h_flex()
                                            .flex_wrap()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .child(
                                                h_flex()
                                                    .gap_0p5()
                                                    .p_0p5()
                                                    .rounded_lg()
                                                    .bg(Color::Inset.get(cx))
                                                    .border_1()
                                                    .border_color(Color::Divider.get(cx))
                                                    .child(
                                                        Button::new("git-diff-unified")
                                                            .custom(segment_variant(cx))
                                                            .compact()
                                                            .h(px(24.))
                                                            .px_2()
                                                            .rounded_md()
                                                            .label(tr("合并视图"))
                                                            .selected(!self.diff_raw && self.diff_has_structured && diff_mode == DiffMode::Unified)
                                                            .disabled(!self.diff_has_structured)
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.diff_raw = false;
                                                                this.diff_state.update(cx, |s, cx| s.set_mode(DiffMode::Unified, cx));
                                                                cx.notify();
                                                            })),
                                                    )
                                                    .child(
                                                        Button::new("git-diff-split")
                                                            .custom(segment_variant(cx))
                                                            .compact()
                                                            .h(px(24.))
                                                            .px_2()
                                                            .rounded_md()
                                                            .label(tr("并排视图"))
                                                            .selected(!self.diff_raw && self.diff_has_structured && diff_mode == DiffMode::Split)
                                                            .disabled(!self.diff_has_structured)
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.diff_raw = false;
                                                                this.diff_state.update(cx, |s, cx| s.set_mode(DiffMode::Split, cx));
                                                                cx.notify();
                                                            })),
                                                    )
                                                    .child(
                                                        Button::new("git-diff-raw")
                                                            .custom(segment_variant(cx))
                                                            .compact()
                                                            .h(px(24.))
                                                            .px_2()
                                                            .rounded_md()
                                                            .label(tr("原始补丁"))
                                                            .selected(self.diff_raw || !self.diff_has_structured)
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.diff_raw = true;
                                                                cx.notify();
                                                            })),
                                                    ),
                                            )
                                            .child(
                                                Button::new("git-diff-ignore-ws")
                                                    .custom(crate::palette::quiet_icon_variant(cx))
                                                    .compact()
                                                    .h(px(24.))
                                                    .px_2()
                                                    .rounded_md()
                                                    .label(tr("忽略空白"))
                                                    .selected(self.ignore_whitespace)
                                                    .toggled(self.ignore_whitespace)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.ignore_whitespace = !this.ignore_whitespace;
                                                        this.sync_diff_state(false, cx);
                                                        cx.notify();
                                                    })),
                                            ),
                                    )
                                })
                                .child(if show_structured {
                                    div()
                                        .h(diff_height)
                                        .min_w_0()
                                        .rounded_md()
                                        .bg(Color::Card.get(cx))
                                        .overflow_hidden()
                                        .child(
                                            Diff::new(&self.diff_state)
                                                .soft_wrap(code_wrap)
                                                .hover_highlight(DiffHoverHighlight::Both)
                                                .change_indicator(DiffChangeIndicator::Signs)
                                                .hunk_separator(DiffHunkSeparator::Metadata)
                                                .rounded_md()
                                                .overflow_hidden()
                                                .border_color(Color::Divider.get(cx)),
                                        )
                                        .into_any_element()
                                } else {
                                    crate::text_reading::region(
                                        "git-raw-patch-region".into(),
                                        tr("Git 原始字节补丁").to_owned(),
                                        &self.diff_scroll,
                                        diff_height,
                                        cx,
                                    )
                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                    .text_size(px(code_size))
                                    .line_height(px((code_size * 1.55).round()))
                                    .when(code_wrap, |v| v.whitespace_normal())
                                    .child(crate::text_reading::selectable_text(
                                        "git-raw-patch",
                                        raw_display,
                                    ))
                                    .into_any_element()
                                }),
                        )
                    })
                    .when(diff.patch.is_none() && !diff.binary, |view| view.child(div().text_sm().child(tr("后台未提供文本补丁；请查看文件类型、对象标识和限制说明"))))
            }).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Change, CommitPreview, CommitResult, FileDiff, IndexApplyResponse, IndexOperation, IndexPreview, Status, decode, git_params, valid_write_state};
    use serde_json::json;
    fn identity() -> serde_json::Value {
        json!({"projectRoot":"/repo/project","repositoryRoot":"/repo","projectPrefix":"project/","linkedWorktree":true,"branch":null,"headOid":"abc","detached":true,"unborn":false})
    }
    #[test]
    fn commit_wire_preserves_outside_paths_and_uncertain_outcomes() {
        let preview: CommitPreview = decode(json!({"ok":true,"preview":{
            "version":"v", "workspace":identity(), "changes":[
                {"repositoryPath":"outside/new", "originalRepositoryPath":"outside/old", "projectPath":null,"status":"R100"}],
            "patch":"diff content", "outsideProjectPaths":["outside/new", "outside/old"],
            "signingRequired":true, "hooks":["pre-commit"], "hooksMayModifyIndex":true
        }}), "preview").unwrap();
        assert_eq!(preview.outside_project_paths, ["outside/new", "outside/old"]);
        assert_eq!(preview.changes[0].original_repository_path.as_deref(), Some("outside/old"));
        assert!(preview.signing_required && preview.hooks_may_modify_index);
        assert_eq!(preview.patch, "diff content");
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
