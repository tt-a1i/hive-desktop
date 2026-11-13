use crate::focus_scroll::{Reveal, Viewport};
use crate::language::{english, tr};
use crate::app::HiveApp;
use crate::palette::Color;
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
        scroll::ScrollableElement,
    },
    *,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Deserialize)]
pub struct RoleTemplate {
    id: String,
    name: String,
    role_type: String,
    description: String,
    default_command: String,
    default_args: Vec<String>,
    default_env: Value,
    is_builtin: bool,
}
pub struct Roles {
    pub visible: bool,
    pub scroll: ScrollHandle,
    items: Vec<RoleTemplate>,
    loading: bool,
    error: Option<RoleError>,
    notice: Option<&'static str>,
    generation: u64,
    editing: Option<RoleTemplate>,
    deleting: Option<String>,
    role_type: &'static str,
}
enum RoleError { Message(&'static str), Load(String), Mutation(String) }
impl RoleError {
    fn text(&self) -> String {
        match self {
            Self::Message(key) => tr(key).to_owned(),
            Self::Load(error) => format!("{}: {}", tr("读取角色模板失败"), crate::language::localize_owned_error(error)),
            Self::Mutation(error) => format!("{}: {}", tr("角色模板操作失败"), crate::language::localize_owned_error(error)),
        }
    }
}
impl Roles {
    pub fn new() -> Self {
        Self {
            visible: false,
            scroll: ScrollHandle::new(),
            items: vec![],
            loading: false,
            error: None,
            notice: None,
            generation: 0,
            editing: None,
            deleting: None,
            role_type: "custom",
        }
    }
}
pub(crate) fn role_label(role: &str) -> &str {
    match role {
        "coder" => tr("开发"),
        "reviewer" => tr("审查"),
        "tester" => tr("测试"),
        "custom" => tr("自定义"),
        "orchestrator" => tr("主控"),
        _ => role,
    }
}
fn role_key(role: &str) -> &'static str {
    match role {
        "coder" => "coder",
        "reviewer" => "reviewer",
        "tester" => "tester",
        "orchestrator" => "orchestrator",
        _ => "custom",
    }
}
impl HiveApp {
    pub fn close_roles(&mut self) {
        self.roles.visible = false;
        self.roles.generation += 1;
        self.roles.loading = false;
    }
    pub fn open_roles(&mut self, cx: &mut Context<Self>) {
        self.close_settings();
        self.roles.visible = true;
        self.load_roles(cx);
    }
    pub fn load_roles(&mut self, cx: &mut Context<Self>) {
        let Some(runtime) = self.runtime.clone() else {
            self.roles.error = Some(RoleError::Message("后台尚未连接"));
            cx.notify();
            return;
        };
        self.roles.generation += 1;
        let generation = self.roles.generation;
        let workspace = self.workspace.clone();
        self.roles.loading = true;
        self.roles.error = None;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "hive.request",
                    json!({"method":"GET","path":"/api/settings/role-templates"}),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                if this.roles.generation != generation
                    || this.workspace != workspace
                    || (!this.roles.visible && !this.member_panel)
                {
                    return;
                }
                this.roles.loading = false;
                match result.and_then(|value| {
                    serde_json::from_value::<Vec<RoleTemplate>>(value).map_err(|e| e.to_string())
                }) {
                    Ok(items) => this.roles.items = items,
                    Err(e) => this.roles.error = Some(RoleError::Load(e)),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn save_role(&mut self, cx: &mut Context<Self>) {
        if !self.connected {
            return;
        }
        let name = self.role_name.read(cx).value().trim().to_string();
        let description = self.role_description.read(cx).value().to_string();
        if name.is_empty() {
            self.roles.error = Some(RoleError::Message("请输入模板名称"));
            cx.notify();
            return;
        }
        let existing = self.roles.editing.clone();
        let body = json!({"name":name,"role_type":self.roles.role_type,"description":description,
            "default_command":existing.as_ref().map(|t| t.default_command.as_str()).unwrap_or(""),
            "default_args":existing.as_ref().map(|t| t.default_args.clone()).unwrap_or_default(),
            "default_env":existing.as_ref().map(|t| t.default_env.clone()).unwrap_or_else(|| json!({}))});
        let (method, path) = existing
            .map(|t| ("PATCH", format!("/api/settings/role-templates/{}", t.id)))
            .unwrap_or(("POST", "/api/settings/role-templates".into()));
        self.role_mutation(method, path, Some(body), cx);
    }
    fn role_mutation(
        &mut self,
        method: &'static str,
        path: String,
        body: Option<Value>,
        cx: &mut Context<Self>,
    ) {
        if !self.connected {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            self.roles.error = Some(RoleError::Message("后台尚未连接"));
            cx.notify();
            return;
        };
        self.roles.generation += 1;
        let generation = self.roles.generation;
        let workspace = self.workspace.clone();
        self.roles.loading = true;
        self.roles.error = None;
        self.roles.notice = None;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "hive.request",
                    json!({"method":method,"path":path,"body":body}),
                )
                .await;
            let _ =
                this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return;
                    }
                    if this.roles.generation != generation
                        || this.workspace != workspace
                        || !this.roles.visible
                    {
                        return;
                    }
                    this.roles.loading = false;
                    match result {
                        Ok(value) => {
                            if method == "DELETE" {
                                if this.roles.editing.as_ref().is_some_and(|item| {
                                    this.roles.deleting.as_ref() == Some(&item.id)
                                }) {
                                    this.roles.editing = None;
                                }
                            } else if let Ok(saved) = serde_json::from_value::<RoleTemplate>(value)
                            {
                                this.roles.editing = Some(saved);
                            }
                            this.roles.deleting = None;
                            this.roles.notice = Some(
                                if method == "DELETE" {
                                    "模板已删除"
                                } else {
                                    "模板已保存"
                                }
                                .into(),
                            );
                            this.load_roles(cx);
                        }
                        Err(e) => this.roles.error = Some(RoleError::Mutation(e)),
                    }
                    cx.notify();
                });
        })
        .detach();
    }
    pub fn render_roles(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let scroll = &self.roles.scroll;
        v_flex().flex_1().min_w_0().min_h_0().h_full().gap_4().px_5().py_6()
            .child(v_flex().w_full().max_w(px(960.)).mx_auto().gap_1()
                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(tr("角色模板")))
                .child(div().text_sm().text_color(Color::Secondary.get(cx)).child(tr("保存常用职责。创建成员时可选择模板，仍需确认后创建。"))))
            .when_some(self.roles.error.as_ref().map(|error| error.text()), |view, error| view.child(div().w_full().max_w(px(960.)).mx_auto().id("roles-error").role(Role::Alert).aria_label(error.clone()).p_3().rounded_lg().bg(Color::ErrorSurface.get(cx)).text_sm().text_color(Color::Error.get(cx)).child(error)))
            .when_some(self.roles.notice, |view, notice| view.child(div().w_full().max_w(px(960.)).mx_auto().id("roles-notice").role(Role::Status).aria_label(tr(notice)).text_sm().text_color(Color::Secondary.get(cx)).child(tr(notice))))
            .child(Viewport::new("roles-viewport", scroll, div().id("roles-scroll").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(scroll).vertical_scrollbar(scroll).p_1().child(v_flex().w_full().max_w(px(960.)).mx_auto().flex_shrink_0().gap_4()
                .child(v_flex().gap_3().p_4().rounded_xl().bg(Color::Card.get(cx)).border_1().border_color(Color::Divider.get(cx))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(if self.roles.editing.is_some() { tr("编辑模板") } else { tr("新建模板") }))
                    .child(Reveal::new("roles-name-reveal", scroll, Input::new(&self.role_name).aria_label(tr("角色模板名称"))))
                    .child(Reveal::new("roles-description-reveal", scroll, Textarea::new(&self.role_description).aria_label(tr("角色模板职责说明"))))
                    .child(Reveal::new("roles-types-reveal", scroll, h_flex().flex_wrap().gap_1().children([("orchestrator", tr("主控")), ("coder", tr("开发")), ("reviewer", tr("审查")), ("tester", tr("测试")), ("custom", tr("自定义"))].into_iter().enumerate().map(|(i, (role, label))| Button::new(("role-type", i)).ghost().compact().rounded_full().label(label).selected(self.roles.role_type == role).on_click(cx.listener(move |this, _, _, cx| { this.roles.role_type = role; cx.notify(); }))))))
                    .child(Reveal::new("roles-save-reveal", scroll, h_flex().flex_wrap().gap_2()
                        .child(Button::new("save-role").primary().rounded_full().label(if self.roles.editing.is_some() { tr("保存修改") } else { tr("创建模板") }).disabled(!self.connected || self.roles.loading).on_click(cx.listener(|this, _, _, cx| this.save_role(cx))))
                        .child(Button::new("new-role-form").ghost().rounded_full().label(tr("新建空白模板")).disabled(!self.connected || self.roles.loading).on_click(cx.listener(|this, _, window, cx| { this.roles.editing = None; this.roles.role_type = "custom"; this.role_name.update(cx, |state, cx| state.set_value("", window, cx)); this.role_description.update(cx, |state, cx| state.set_value("", window, cx)); let focus = this.role_name.read(cx).focus_handle(cx); window.on_next_frame(move |window, cx| focus.focus(window, cx)); cx.notify(); }))))))
                .child(Reveal::new("roles-refresh-reveal", scroll, h_flex().items_center().justify_between().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(if self.roles.loading { tr("正在读取或保存…") } else { tr("已有模板") })).child(Button::new("refresh-roles").ghost().compact().icon(Glyph::RefreshCw).label(tr("刷新")).disabled(!self.connected || self.roles.loading).on_click(cx.listener(|this, _, _, cx| this.load_roles(cx))))))
                .when(!self.roles.loading && self.roles.items.is_empty(), |view| view.child(div().p_4().rounded_xl().bg(Color::Card.get(cx)).border_1().border_color(Color::Divider.get(cx)).text_sm().text_color(Color::Muted.get(cx)).child(tr("暂无模板"))))
                .children(self.roles.items.iter().enumerate().map(|(i, template)| {
                    let item = template.clone(); let delete = template.id.clone(); let deleting = self.roles.deleting.as_ref() == Some(&template.id);
                    v_flex().gap_2().p_4().rounded_xl().bg(Color::Card.get(cx)).border_1().border_color(Color::Divider.get(cx))
                        .child(h_flex().flex_wrap().items_center().gap_2()
                            .child(div().flex_1().min_w_0().text_sm().font_weight(FontWeight::SEMIBOLD).child(template.name.clone()))
                            .child(div().rounded_full().bg(Color::Inset.get(cx)).px(px(9.)).py(px(2.)).text_xs().text_color(Color::Secondary.get(cx)).child(role_label(&template.role_type).to_owned()))
                            .when(template.is_builtin, |row| row.child(div().rounded_full().bg(Color::Inset.get(cx)).px(px(9.)).py(px(2.)).text_xs().text_color(Color::Muted.get(cx)).child(tr("内置")))))
                        .child(div().text_sm().text_color(Color::Secondary.get(cx)).child(template.description.clone()))
                        .when(!template.is_builtin, |view| view.child(Reveal::new(("roles-actions-reveal", i), scroll, h_flex().flex_wrap().gap_2()
                            .child(Button::new(("edit-role", i)).ghost().compact().rounded_full().label(tr("编辑")).disabled(!self.connected || self.roles.loading).on_click(cx.listener(move |this, _, window, cx| { this.roles.role_type = role_key(&item.role_type); this.role_name.update(cx, |state, cx| state.set_value(item.name.clone(), window, cx)); this.role_description.update(cx, |state, cx| state.set_value(item.description.clone(), window, cx)); this.roles.editing = Some(item.clone()); this.roles.deleting = None; this.roles.scroll.set_offset(point(px(0.), px(0.))); let focus = this.role_name.read(cx).focus_handle(cx); window.on_next_frame(move |window, cx| focus.focus(window, cx)); cx.notify(); })))
                            .child(Button::new(("delete-role", i)).ghost().compact().rounded_full().label(if deleting { if english() { format!("Confirm deletion of “{}”", template.name) } else { format!("确认删除「{}」", template.name) } } else { tr("删除").into() }).disabled(!self.connected || self.roles.loading).on_click(cx.listener(move |this, _, _, cx| { if this.roles.deleting.as_ref() == Some(&delete) { this.role_mutation("DELETE", format!("/api/settings/role-templates/{delete}"), None, cx); } else { this.roles.deleting = Some(delete.clone()); cx.notify(); } })))
                            .when(deleting, |view| view.child(Button::new(("cancel-delete-role", i)).ghost().compact().rounded_full().label(tr("取消")).accessibility_label(if english() { format!("Cancel deletion of role template {}", template.name) } else { format!("取消删除角色模板 {}", template.name) }).disabled(!self.connected || self.roles.loading).on_click(cx.listener(|this, _, _, cx| { this.roles.deleting = None; cx.notify(); })))))))
                })))))
    }
    pub fn render_member_templates(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_1()
            .child(
                Button::new("toggle-member-templates")
                    .label(if self.member_templates_open {
                        tr("收起角色模板")
                    } else {
                        tr("选择角色模板")
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.member_templates_open = !this.member_templates_open;
                        cx.notify();
                    })),
            )
            .when(self.member_templates_open, |view| {
                view.child(div().text_xs().child(tr("选择模板后可继续编辑职责")))
            })
            .when(self.roles.loading, |view| {
                view.child(div().text_xs().child(tr("正在读取模板…")))
            })
            .when_some(self.roles.error.as_ref().map(|error| error.text()), |view, error| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
                .child(
                    Button::new("retry-member-roles")
                        .label(tr("重试读取模板"))
                        .on_click(cx.listener(|this, _, _, cx| this.load_roles(cx))),
                )
            })
            .children(
                self.roles
                    .items
                    .iter()
                    .filter(|template| {
                        self.member_templates_open && template.role_type != "orchestrator"
                    })
                    .enumerate()
                    .map(|(i, template)| {
                        let item = template.clone();
                        Button::new(("apply-role-template", i))
                            .label(format!(
                                "{} · {}",
                                template.name,
                                role_label(&template.role_type)
                            ))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.member_role = role_key(&item.role_type);
                                this.member_description.update(cx, |state, cx| {
                                    state.set_value(item.description.clone(), window, cx)
                                });
                                this.member_templates_open = false;
                                cx.notify();
                            }))
                    }),
            )
    }
}
