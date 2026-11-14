use crate::{
    app::HiveApp,
    focus_scroll::{Reveal, Viewport},
    language::tr,
    palette::Color,
    transport::Runtime,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
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
    project_name: String,
    connection: u64,
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
        let app = cx.weak_entity();
        let connection = self.connection_generation;
        let state = cx.new(|cx| {
            if let Some(parent) = app.upgrade() {
                cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            }
            let mut state = GitWorkspace {
                app,
                runtime,
                workspace,
                project_name,
                connection,
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
            };
            state.refresh(cx);
            state
        });
        let width = (window.viewport_size().width - px(40.)).min(px(1000.));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(tr("Git 状态与差异"))
                .w(width)
                .on_ok(|_, _, _| false)
                .child(state.clone())
        });
    }
}
impl GitWorkspace {
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|app| {
            let app = app.read(cx);
            app.connected
                && !app.closing
                && app.workspace.as_ref() == Some(&self.workspace)
                && app.connection_generation == self.connection
        })
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.status_generation += 1;
        self.diff_generation += 1;
        let generation = self.status_generation;
        let offset = self.offset;
        let selected_path = self.selected_change().map(|change| change.path.clone());
        self.status = None;
        self.status_error = None;
        self.status_loading = true;
        self.diff = None;
        self.diff_error = None;
        self.diff_loading = false;
        self.files_scroll = ScrollHandle::new();
        self.diff_scroll = ScrollHandle::new();
        cx.notify();
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
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
                    json!({"workspaceId":workspace,"offset":offset,"limit":100}),
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
            || !self.current(cx)
            || self
                .status
                .as_ref()
                .is_none_or(|status| index >= status.entries.len())
        {
            return;
        }
        self.selected = index;
        self.diff_generation += 1;
        self.diff = None;
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
        self.diff_error = None;
        self.diff_loading = true;
        self.diff_scroll = ScrollHandle::new();
        cx.notify();
        let runtime = self.runtime.clone();
        let workspace = self.workspace.clone();
        let comparison = self.comparison.wire();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "git.diff",
                    json!({"workspaceId":workspace,"path":path,"comparison":comparison}),
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
                    Ok(diff) => this.diff = Some(diff),
                    Err(error) => this.diff_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
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
        let current = self.current(cx);
        let keys = cx.entity();
        let list_height = px(180.).min(window.viewport_size().height * 0.25);
        let diff_height = px(360.).min(window.viewport_size().height * 0.4);
        let selected_metadata = self
            .selected_change()
            .filter(|_| current)
            .map(Change::metadata);
        v_flex().flex_shrink_0().min_w_0().gap_2()
            .child(h_flex().min_w_0().items_center().gap_2()
                .child(div().flex_1().min_w_0().truncate().child(self.project_name.clone()))
                .child(Button::new("refresh-git-workspace").outline().small().label(tr("刷新 Git 状态")).disabled(!current || self.status_loading)
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)))))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("只读项目范围；不扫描子模块内部文件或远端领先落后信息。")))
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
                                    .disabled(!current).accessibility_label(change.title()).tooltip(change.metadata())
                                    .child(div().min_w_0().truncate().text_sm().child(change.title()))
                                    .on_click(cx.listener(move |this, _, _, cx| this.select(index, true, cx)))))))))
                    .child(h_flex().items_center().justify_between().gap_2()
                        .child(div().text_xs().child(format!("{}–{} / {}", if status.entries.is_empty() {0} else {status.offset + 1}, status.offset + status.entries.len(), status.total)))
                        .child(h_flex().gap_1()
                            .child(Button::new("git-files-prev").outline().small().label(tr("上一页")).disabled(!current || self.offset == 0)
                                .on_click(cx.listener(|this, _, _, cx| { let limit = this.status.as_ref().map(|status| status.limit).unwrap_or(100); this.offset = this.offset.saturating_sub(limit); this.refresh(cx); })))
                            .child(Button::new("git-files-next").outline().small().label(tr("下一页")).disabled(!current || !status.has_more)
                                .on_click(cx.listener(|this, _, _, cx| { this.offset += this.status.as_ref().map(|status| status.limit).unwrap_or(100); this.refresh(cx); })))))
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
            .when_some(self.diff.as_ref().filter(|_| current), |view, diff| {
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
                        view.child(crate::text_reading::region("git-raw-patch-region".into(), tr("Git 原始字节补丁").to_owned(), &self.diff_scroll, diff_height, cx)
                            .font_family(Theme::global(cx).mono_font_family.clone())
                            .child(crate::text_reading::selectable_text("git-raw-patch", if patch.is_empty() { tr("此比较没有文本内容差异").to_owned() } else { patch.clone() })))
                    })
                    .when(diff.patch.is_none() && !diff.binary, |view| view.child(div().text_sm().child(tr("后台未提供文本补丁；请查看文件类型、对象标识和限制说明"))))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{Change, FileDiff, Status, decode};
    use serde_json::json;
    fn identity() -> serde_json::Value {
        json!({"projectRoot":"/repo/project","repositoryRoot":"/repo","projectPrefix":"project/","linkedWorktree":true,"branch":null,"headOid":"abc","detached":true,"unborn":false})
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
