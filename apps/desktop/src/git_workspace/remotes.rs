mod model;
use super::{Fault, git_params};
use crate::{app::HiveApp, language::tr, palette::Color, transport::Runtime};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use model::*;
use serde_json::json;
use std::sync::Arc;

struct PushReview {
    preview: PushPreview,
    generation: RequestStamp,
}
struct PrReview {
    preview: PrPreview,
    generation: RequestStamp,
    title: String,
    body: String,
    draft: bool,
}
struct PushOutcome {
    result: PushResult,
    expected_oid: String,
    context_valid: bool,
}
struct PrOutcome {
    result: PrResult,
    repository: Repository,
    operation: &'static str,
    expected_number: Option<u64>,
    context_valid: bool,
}
pub(super) struct RemotePanel {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    workspace: String,
    session: Option<String>,
    connection: u64,
    open_request: u64,
    list: Option<RemoteList>,
    remote: Option<String>,
    branch: Entity<InputState>,
    head: Entity<InputState>,
    base: Entity<InputState>,
    title: Entity<InputState>,
    body: Entity<TextareaState>,
    draft: bool,
    defaults_set: bool,
    generation: u64,
    pub busy: bool,
    writing: bool,
    push: Option<PushReview>,
    pr: Option<PrReview>,
    push_result: Option<PushOutcome>,
    pr_result: Option<PrOutcome>,
    error: Option<Fault>,
    uncertain: bool,
    commits_scroll: ScrollHandle,
    review_scroll: ScrollHandle,
}
impl RemotePanel {
    pub fn new(
        app: WeakEntity<HiveApp>,
        runtime: Arc<Runtime>,
        workspace: String,
        session: Option<String>,
        connection: u64,
        open_request: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let default_base = crate::diffs_settings::current(cx).pr_default_base;
        let default_draft = crate::diffs_settings::current(cx).pr_default_draft;
        let branch = cx.new(|cx| InputState::new(window, cx));
        let head = cx.new(|cx| InputState::new(window, cx));
        let base = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value(default_base, window, cx);
            input
        });
        let title = cx.new(|cx| InputState::new(window, cx));
        let body = cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 8));
        for input in [&branch, &head, &base, &title] {
            cx.subscribe(input, |this: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.invalidate(cx);
                }
            })
            .detach();
        }
        cx.subscribe(&body, |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.invalidate(cx);
            }
        })
        .detach();
        if let Some(parent) = app.upgrade() {
            cx.observe(&parent, |this: &mut Self, _, cx| {
                if !this.current(cx) {
                    this.invalidate(cx);
                }
                cx.notify();
            })
            .detach();
        }
        Self {
            app,
            runtime,
            workspace,
            session,
            connection,
            open_request,
            list: None,
            remote: None,
            branch,
            head,
            base,
            title,
            body,
            draft: default_draft,
            defaults_set: false,
            generation: 0,
            busy: false,
            writing: false,
            push: None,
            pr: None,
            push_result: None,
            pr_result: None,
            error: None,
            uncertain: false,
            commits_scroll: ScrollHandle::new(),
            review_scroll: ScrollHandle::new(),
        }
    }
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|app| {
            let app = app.read(cx);
            app.connected
                && !app.closing
                && app.workspace.as_ref() == Some(&self.workspace)
                && app.connection_generation == self.connection
                && app.open_request == self.open_request
                && app
                    .session
                    .as_ref()
                    .filter(|session| session.workspace_id == self.workspace)
                    .map(|session| &session.id)
                    == self.session.as_ref()
                && app
                    .runtime
                    .as_ref()
                    .is_some_and(|runtime| Arc::ptr_eq(runtime, &self.runtime))
        })
    }
    pub fn invalidate(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        cx.notify();
    }
    pub fn activate(&mut self, cx: &mut Context<Self>) {
        if self.list.is_none() && !self.busy {
            self.load(cx);
        }
    }
    fn params(&self, value: serde_json::Value) -> serde_json::Value {
        git_params(value, &self.session)
    }
    fn ready(&self, cx: &App) -> bool {
        !self.busy && self.current(cx)
    }
    fn push_fresh(&self, cx: &App) -> bool {
        self.current(cx)
            && self.push.as_ref().is_some_and(|review| {
                review
                    .generation
                    .matches(self.open_request, self.generation)
            })
    }
    fn pr_fresh(&self, cx: &App) -> bool {
        self.current(cx)
            && self.pr.as_ref().is_some_and(|review| {
                review
                    .generation
                    .matches(self.open_request, self.generation)
            })
    }
    fn before_request(&mut self, generation: RequestStamp, cx: &mut Context<Self>) -> bool {
        if generation.matches(self.open_request, self.generation) && self.current(cx) {
            return true;
        }
        self.busy = false;
        self.writing = false;
        cx.notify();
        false
    }
    fn load(&mut self, cx: &mut Context<Self>) {
        if !self.ready(cx) {
            return;
        }
        self.invalidate(cx);
        self.busy = true;
        self.error = None;
        let generation = RequestStamp::new(self.open_request, self.generation);
        let params = self.params(json!({"workspaceId":self.workspace}));
        let runtime = self.runtime.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| this.before_request(generation, cx))
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request("git.remotes", params)
                .await
                .map_err(Fault::transport)
                .and_then(|value| envelope::<RemoteList>(value, None));
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if !this.current(cx) || !generation.matches(this.open_request, this.generation) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(list) => {
                        if this.remote.as_ref().is_some_and(|name| {
                            !list.remotes.iter().any(|remote| &remote.name == name)
                        }) {
                            this.remote = None;
                        }
                        this.list = Some(list);
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn set_defaults(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.defaults_set {
            return;
        }
        let Some(branch) = self
            .list
            .as_ref()
            .and_then(|list| list.workspace.identity.branch.clone())
        else {
            return;
        };
        self.defaults_set = true;
        let editor = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = editor.update(cx, |this, cx| {
                if this.branch.read(cx).value().is_empty() {
                    this.branch
                        .update(cx, |input, cx| input.set_value(branch.clone(), window, cx));
                }
                if this.head.read(cx).value().is_empty() {
                    this.head
                        .update(cx, |input, cx| input.set_value(branch, window, cx));
                }
                if this.base.read(cx).value().trim().is_empty() {
                    let default_base = crate::diffs_settings::current(cx).pr_default_base;
                    this.base
                        .update(cx, |input, cx| input.set_value(default_base, window, cx));
                }
                cx.notify();
            });
        });
    }
    fn preview_push(&mut self, cx: &mut Context<Self>) {
        if !self.ready(cx) {
            return;
        }
        let Some(remote) = self.remote.clone() else {
            return;
        };
        let branch = self.branch.read(cx).value().to_string();
        if branch.is_empty() {
            return;
        }
        self.invalidate(cx);
        let generation = RequestStamp::new(self.open_request, self.generation);
        self.busy = true;
        self.error = None;
        let runtime = self.runtime.clone();
        let params =
            self.params(json!({"workspaceId":self.workspace,"remote":remote,"branch":branch}));
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| this.before_request(generation, cx))
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request("git.push.preview", params)
                .await
                .map_err(Fault::transport)
                .and_then(|value| envelope::<PushPreview>(value, Some("preview")));
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if !this.current(cx) || !generation.matches(this.open_request, this.generation) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(preview) => {
                        this.push = Some(PushReview {
                            preview,
                            generation,
                        });
                        this.push_result = None;
                        this.uncertain = false;
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn preview_pr(&mut self, cx: &mut Context<Self>) {
        if !self.ready(cx) {
            return;
        }
        let Some(remote) = self.remote.clone() else {
            return;
        };
        let head = self.head.read(cx).value().to_string();
        let base = self.base.read(cx).value().to_string();
        if head.is_empty() || base.is_empty() {
            return;
        }
        self.invalidate(cx);
        let generation = RequestStamp::new(self.open_request, self.generation);
        self.busy = true;
        self.error = None;
        let runtime = self.runtime.clone();
        let title = self.title.read(cx).value().to_string();
        let body = self.body.read(cx).value().to_string();
        let draft = self.draft;
        let params = self.params(
            json!({"workspaceId":self.workspace,"remote":remote,"base":base,"headBranch":head}),
        );
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| this.before_request(generation, cx))
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request("git.pr.preview", params)
                .await
                .map_err(Fault::transport)
                .and_then(|value| envelope::<PrPreview>(value, Some("preview")));
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if !this.current(cx) || !generation.matches(this.open_request, this.generation) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(preview) => {
                        this.pr = Some(PrReview {
                            preview,
                            generation,
                            title,
                            body,
                            draft,
                        });
                        this.pr_result = None;
                        this.uncertain = false;
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn push(&mut self, cx: &mut Context<Self>) {
        if !self.ready(cx) || !self.push_fresh(cx) {
            return;
        }
        let Some(review) = &self.push else {
            return;
        };
        if !review.preview.allowed() {
            return;
        }
        let preview = review.preview.clone();
        let params=self.params(json!({"workspaceId":self.workspace,"remote":preview.remote,"branch":preview.branch,"expectedVersion":preview.version}));
        self.invalidate(cx);
        let generation = RequestStamp::new(self.open_request, self.generation);
        self.busy = true;
        self.writing = true;
        self.error = None;
        let runtime = self.runtime.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| this.before_request(generation, cx))
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request("git.push.apply", params)
                .await
                .map_err(Fault::transport)
                .and_then(|value| write_result::<PushResult>(value, "pushed"));
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.writing = false;
                let valid =
                    this.current(cx) && generation.matches(this.open_request, this.generation);
                this.uncertain = true;
                match result {
                    Ok(result) => {
                        this.uncertain = result.pushed != false
                            && (!result.confirmed(&preview.head_oid) || !valid);
                        this.push_result = Some(PushOutcome {
                            result,
                            expected_oid: preview.head_oid,
                            context_valid: valid,
                        });
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn write_pr(&mut self, cx: &mut Context<Self>) {
        if !self.ready(cx) || !self.pr_fresh(cx) {
            return;
        }
        let Some(review) = &self.pr else {
            return;
        };
        if !review.preview.allowed() || !valid_pr_text(&review.title, &review.body) {
            return;
        }
        let preview = review.preview.clone();
        let operation = if preview.existing.is_empty() {
            "create"
        } else {
            "update"
        };
        let expected_number = preview.existing.first().map(|pr| pr.number);
        let mut params=self.params(json!({"workspaceId":self.workspace,"remote":preview.remote,"base":preview.base,"headBranch":preview.head_branch,
            "expectedVersion":preview.version,"title":review.title,"body":review.body}));
        if operation == "create" {
            params["draft"] = json!(review.draft);
        } else {
            params["number"] = json!(preview.existing[0].number);
        }
        self.invalidate(cx);
        let generation = RequestStamp::new(self.open_request, self.generation);
        self.busy = true;
        self.writing = true;
        self.error = None;
        let runtime = self.runtime.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if this
                .update(cx, |this, cx| this.before_request(generation, cx))
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request(
                    if operation == "create" {
                        "git.pr.create"
                    } else {
                        "git.pr.update"
                    },
                    params,
                )
                .await
                .map_err(Fault::transport)
                .and_then(|value| write_result::<PrResult>(value, "applied"));
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.writing = false;
                let valid =
                    this.current(cx) && generation.matches(this.open_request, this.generation);
                this.uncertain = true;
                match result {
                    Ok(result) => {
                        let confirmed =
                            result.confirmed(&preview.repository, operation, expected_number)
                                && valid;
                        this.uncertain = result.applied != false && !confirmed;
                        this.pr_result = Some(PrOutcome {
                            result,
                            repository: preview.repository,
                            operation,
                            expected_number,
                            context_valid: valid,
                        });
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
fn valid_pr_text(title: &str, body: &str) -> bool {
    !title.trim().is_empty()
        && title.encode_utf16().count() <= 500
        && !title.chars().any(char::is_control)
        && !body.contains('\0')
        && body.len() <= 1024 * 1024
}
fn fault(fault: &Fault) -> String {
    format!(
        "[{}] {}",
        fault.code,
        crate::language::localize_owned_error(&fault.message)
    )
}
fn oid(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("—")
}
impl Render for RemotePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.set_defaults(window, cx);
        let current = self.current(cx);
        let enabled = self.ready(cx);
        let push_fresh = self.push_fresh(cx);
        let pr_fresh = self.pr_fresh(cx);
        let updating = self
            .pr
            .as_ref()
            .is_some_and(|review| review.preview.existing.len() == 1);
        v_flex().flex_shrink_0().min_w_0().gap_3()
            .child(div().text_sm().child(tr("远端列表只读取本地配置。只有点击联网预览、确认推送或确认 PR 写入才会访问网络。")))
            .when(!current,|view|view.child(div().id("remote-context-changed").role(Role::Status).text_sm().text_color(Color::Error.get(cx)).child(tr("项目、会话或后台连接已变化；旧预览不能再使用，请关闭后重新打开。"))))
            .when(self.busy,|view|view.child(div().id("remote-busy").role(Role::Status).text_sm().child(if self.writing{tr("正在执行远端写操作，请等待结果，勿重复提交。")}else{tr("正在读取预览，请等待…")})))
            .when(self.uncertain,|view|view.child(div().id("remote-outcome-uncertain").role(Role::Status).text_sm().text_color(Color::Error.get(cx)).child(tr("操作结果需要核对。请检查远端或 GitHub；保留的预览仅供检查，重新预览后才可再次提交。"))))
            .when_some(self.error.as_ref(),|view,error|view.child(div().id("remote-error").role(Role::Status).text_sm().text_color(Color::Error.get(cx)).child(fault(error))))
            .child(Button::new("load-git-remotes").outline().small().label(tr("刷新本地远端列表")).disabled(!enabled).on_click(cx.listener(|this,_,_,cx|this.load(cx))))
            .when_some(self.list.as_ref(),|view,list|view
                .child(crate::text_reading::selectable_text("remote-workspace",list.workspace.description()))
                .when(list.remotes.is_empty(),|view|view.child(div().text_sm().child(tr("项目没有配置远端；请用原生 Git 配置后刷新。"))))
                .children(list.remotes.iter().enumerate().map(|(index,remote)|{let name=remote.name.clone();Button::new(("select-git-remote",index)).outline().small().h_auto()
                    .label(format!("{} · {}",remote.name,remote.url.as_deref().or(remote.error.as_deref()).unwrap_or("—"))).selected(self.remote.as_ref()==Some(&remote.name))
                    .disabled(!enabled||remote.url.is_none()||remote.error.is_some()).on_click(cx.listener(move|this,_,_,cx|{this.remote=Some(name.clone());this.invalidate(cx);} ))})))
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("推送当前提交")))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("远端推送目标分支")))
            .child(Input::new(&self.branch).aria_label(tr("远端推送目标分支")).disabled(self.busy))
            .child(Button::new("preview-git-push").outline().small().label(tr("联网预览推送")).disabled(!enabled||self.remote.is_none()||self.branch.read(cx).value().is_empty()).on_click(cx.listener(|this,_,_,cx|this.preview_push(cx))))
            .when_some(self.push.as_ref(),|view,review|{let preview=&review.preview;let commits=preview.commits.iter().map(|commit|format!("{} {}",commit.oid,commit.subject)).collect::<Vec<_>>().join("\n");view
                .child(crate::text_reading::selectable_text("git-push-review",format!("{}\nremote: {}\nURL: {}\nbranch: {}\nHEAD: {}\nremote OID: {}\n{}: {}",preview.workspace.description(),preview.remote,preview.remote_url,preview.branch,preview.head_oid,oid(&preview.remote_oid),tr("推送关系"),preview.relation.label())))
                .child(div().text_sm().child(tr("原生 Git hooks 可能执行；原生 tracking refs 可能更新。不会自动 fetch、提交或设置 upstream。")))
                .child(div().text_xs().child(format!("{}: {} · {}",tr("已列出提交数"),preview.commits.len(),if preview.commits_complete{tr("提交列表完整")}else{tr("提交列表不完整")})))
                .when(!preview.commits_complete||!preview.warnings.is_empty(),|view|view.child(div().id("push-incomplete").role(Role::Status).text_sm().text_color(Color::Error.get(cx)).child(format!("{}\n{}",tr("提交列表不完整；请核对预览限制。"),preview.warnings.join("\n")))))
                .when(!commits.is_empty(),|view|view.child(crate::text_reading::region("push-commits".into(),tr("预计推送提交").to_owned(),&self.commits_scroll,px(180.),cx).child(crate::text_reading::selectable_text("push-commits-text",commits))))
                .when(!preview.allowed(),|view|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(tr("此预览不允许推送。分叉或缺少远端历史时，请先用原生 Git fetch 并自行处理，再重新预览；不会提供强制推送。"))))
                .when(!push_fresh,|view|view.child(div().text_sm().child(tr("预览已失效；请重新联网预览后再确认。"))))
                .child(Button::new("confirm-git-push").primary().label(tr("确认推送预览中的固定提交")).disabled(!enabled||!push_fresh||!preview.allowed()).on_click(cx.listener(|this,_,_,cx|this.push(cx))))})
            .when_some(self.push_result.as_ref(),|view,outcome|view.child(div().id("git-push-outcome").role(Role::Status).text_sm().child(format!("{}\nHEAD: {}\nremote OID: {}{}",if outcome.result.pushed==false{tr("推送未执行，请核对错误并重新预览。")}else if outcome.context_valid&&outcome.result.confirmed(&outcome.expected_oid){tr("推送已确认")}else{tr("推送结果未确认，请检查远端。")},outcome.result.head_oid,oid(&outcome.result.remote_oid),outcome.result.error.as_ref().map(|error|format!("\n{}",fault(error))).unwrap_or_default()))))
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("GitHub PR（仅同仓库 github.com）")))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("PR head 分支")))
            .child(Input::new(&self.head).aria_label(tr("PR head 分支")).disabled(self.busy))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("PR base 分支（显式输入，不推断上游）")))
            .child(Input::new(&self.base).aria_label(tr("PR base 分支（显式输入，不推断上游）")).disabled(self.busy))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("PR 标题")))
            .child(Input::new(&self.title).aria_label(tr("PR 标题")).disabled(self.busy))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("PR 正文")))
            .child(Textarea::new(&self.body).aria_label(tr("PR 正文")).disabled(self.busy))
            .child(Checkbox::new("create-draft-pr").label(tr("新建为草稿 PR（仅创建时适用）")).checked(self.draft).disabled(self.busy||updating).on_click(cx.listener(|this,checked:&bool,_,cx|{this.draft=*checked;this.invalidate(cx);})))
            .child(div().text_xs().text_color(Color::Muted.get(cx)).child(tr("任何输入改变都会使预览失效；请先填写标题、正文与草稿选项，再联网预览并核对完整内容。")))
            .child(Button::new("preview-github-pr").outline().small().label(tr("联网预览 PR")).disabled(!enabled||self.remote.is_none()||self.head.read(cx).value().is_empty()||self.base.read(cx).value().is_empty()).on_click(cx.listener(|this,_,_,cx|this.preview_pr(cx))))
            .when_some(self.pr.as_ref(),|view,review|{let preview=&review.preview;view
                .child(crate::text_reading::selectable_text("github-pr-review",format!("{}\nrepository: {}\nremote: {}\nURL: {}\nhead: {}\nbase: {}\nHEAD: {}\nremote OID: {}\nbase OID: {}\n{}",preview.workspace.description(),preview.repository.url,preview.remote,preview.remote_url,preview.head_branch,preview.base,preview.head_oid,oid(&preview.remote_oid),preview.base_oid,if preview.pushed{tr("所选 head 已包含当前提交")}else{tr("所选 head 尚未包含当前提交")})))
                .when(!preview.pushed,|view|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(tr("当前固定提交尚未推送到所选 head 分支，不能创建或更新 PR。"))))
                .when(!preview.allowed(),|view|view.child(div().text_sm().text_color(Color::Error.get(cx)).child(tr("PR 预览不满足同仓库、唯一 PR 或 head 提交验证要求；请先核对 GitHub。"))))
                .children(preview.existing.iter().map(|pr|{let url=verified_pr_url(&pr.url,&preview.repository,pr.number);let title=pr.title.clone();let body=pr.body.clone();v_flex().gap_1()
                    .child(crate::text_reading::selectable_text("existing-pr",format!("PR #{} · {}\n{}\n{}",pr.number,pr.url,pr.title,pr.body)))
                    .child(Button::new("open-existing-pr").outline().small().label(tr("打开已验证的 PR 地址")).disabled(url.is_none()).on_click(move|_,_,cx|{if let Some(url)=&url{cx.open_url(url);}}))
                    .child(Button::new("load-existing-pr-text").outline().small().label(tr("用此 PR 内容填写表单（需重新预览）")).disabled(!enabled).on_click(cx.listener(move|this,_,window,cx|{this.title.update(cx,|input,cx|input.set_value(title.clone(),window,cx));this.body.update(cx,|input,cx|input.set_value(body.clone(),window,cx));this.invalidate(cx);}))) }))
                .child(crate::text_reading::region("pr-write-review".into(),tr("将提交的 PR 完整内容").to_owned(),&self.review_scroll,px(240.),cx).child(crate::text_reading::selectable_text("pr-write-review-text",format!("{}\n{}: {}\n\n{}",if preview.existing.is_empty(){if review.draft{tr("创建草稿 PR")}else{tr("创建可审阅 PR")}.to_owned()}else{tr("更新保留现有 PR 的草稿状态，不提交 draft 字段。").to_owned()},tr("PR 标题"),review.title,review.body))))
                .when(!pr_fresh,|view|view.child(div().text_sm().child(tr("预览已失效；请重新联网预览后再确认。"))))
                .child(Button::new("confirm-github-pr").primary().label(if preview.existing.is_empty(){tr("确认创建预览中的 PR")}else{tr("确认更新此编号的 PR")}).disabled(!enabled||!pr_fresh||!preview.allowed()||!valid_pr_text(&review.title,&review.body)).on_click(cx.listener(|this,_,_,cx|this.write_pr(cx))))})
            .when_some(self.pr_result.as_ref(),|view,outcome|{let url=outcome.result.url.as_ref().zip(outcome.result.number).filter(|(_,number)|outcome.expected_number.is_none_or(|expected|expected==*number)).and_then(|(url,number)|verified_pr_url(url,&outcome.repository,number));let confirmed=outcome.context_valid&&outcome.result.confirmed(&outcome.repository,outcome.operation,outcome.expected_number);view
                .child(div().id("github-pr-outcome").role(Role::Status).text_sm().child(format!("{} · {} · #{}{}",if outcome.result.applied==false{tr("PR 写入未执行，请核对错误并重新预览。")}else if confirmed{tr("PR 写入已确认")}else{tr("PR 写入结果未确认，请核对 GitHub。")},outcome.result.operation,outcome.result.number.map(|number|number.to_string()).unwrap_or_else(||"—".into()),outcome.result.error.as_ref().map(|error|format!("\n{}",fault(error))).unwrap_or_default())))
                .child(Button::new("open-pr-outcome").outline().small().label(tr("在 GitHub 核对此 PR")).disabled(url.is_none()).on_click(move|_,_,cx|{if let Some(url)=&url{cx.open_url(url);}}))})
    }
}
