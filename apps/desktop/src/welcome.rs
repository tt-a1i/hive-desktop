use crate::{app::{HiveApp, SettingsSection}, focus_scroll::Reveal, language::tr, palette::Color};
use gpui_kit::{component::{button::{Button, ButtonVariants}, menu::{DropdownMenu, PopupMenuItem}, scroll::ScrollableElement, *}, prelude::FluentBuilder, *};
use gpui_kit::assets::IconName as Glyph;

impl HiveApp {
    pub(crate) fn render_project_context(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let project = self.workspaces.iter().find(|project| Some(&project.id) == self.workspace.as_ref());
        h_flex().w_full().min_w_0().max_w(px(crate::conversation_list::CHAT_COLUMN_WIDTH)).mx_auto().gap_2()
            .when_some(project, |row, project| {
                let path = std::path::PathBuf::from(&project.path);
                let identity = format!("{} · {}", project.name, project.path);
                row.child(Button::new("home-project-folder").ghost().compact().icon(Glyph::FolderOpen)
                    .accessibility_label(format!("{}: {identity}", tr("在文件管理器中打开项目")))
                    .tooltip(project.path.clone()).on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&path))))
                    .child(v_flex().min_w_0().flex_1().gap_1()
                        .child(div().id("home-project-name").role(Role::Label).aria_label(project.name.clone())
                            .text_sm().font_weight(FontWeight::SEMIBOLD).truncate().child(project.name.clone()))
                        .child(div().id("home-project-path").role(Role::Label).aria_label(project.path.clone())
                            .text_xs().text_color(Color::Muted.get(cx)).truncate().child(project.path.clone())))
            })
    }

    /// The first project uses the same picker and configuration flows as daily use.
    /// Opening this page never creates a session or starts an agent.
    pub(crate) fn render_welcome(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let weak = cx.weak_entity();
        let primary_focus = self.connected.then(|| window.use_keyed_state("welcome-primary-focus", cx, |window, cx| {
            let focus = cx.focus_handle();
            let initial = focus.clone();
            window.on_next_frame(move |window, cx| initial.focus(window, cx));
            focus
        }).read(cx).clone());
        div().flex().flex_1().min_w_0().min_h_0().h_full().child(crate::focus_scroll::Viewport::new("welcome-viewport", &self.welcome_scroll, div().id("welcome-page").flex_1().min_w_0().min_h_0().h_full().overflow_y_scroll().track_scroll(&self.welcome_scroll).vertical_scrollbar(&self.welcome_scroll)
            .child(
                v_flex().flex_shrink_0().w_full().max_w(px(640.)).mx_auto().gap_6().px_5().py_6()
                    .child(v_flex().flex_shrink_0().gap_3()
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(Color::Muted.get(cx)).child("HIVE"))
                        .child(div().id("welcome-title").role(Role::Label).aria_label(tr("你的项目，你的 Agent。一起推进。"))
                            .text_size(px(36.)).line_height(px(46.)).font_weight(FontWeight::SEMIBOLD)
                            .child(tr("你的项目，你的 Agent。一起推进。")))
                        .child(div().id("welcome-description").role(Role::Label).aria_label(tr("选择熟悉的 Agent 担任主控，在同一个地方交流、查看工具过程，与团队成员协作。")).text_base().text_color(Color::Secondary.get(cx))
                            .child(tr("选择熟悉的 Agent 担任主控，在同一个地方交流、查看工具过程，与团队成员协作。"))))
                    .when(self.error.is_some(), |view| view.child(self.render_error_banner(cx)))
                    .child(v_flex().flex_shrink_0().gap_3()
                        .child(Reveal::new("welcome-actions-reveal", &self.welcome_scroll, h_flex().flex_shrink_0().gap_2().flex_wrap()
                            .child(Button::new("welcome-open-project").primary().label(tr("打开项目文件夹"))
                                .when_some(primary_focus, |button, focus| button.track_focus(&focus))
                                .disabled(!self.connected).on_click(cx.listener(|this, _, window, cx| this.choose_workspace_folder(window, cx))))
                            .child(Button::new("welcome-configure").ghost().label(tr("先配置 Agent")).dropdown_caret(true)
                                .accessibility_label(tr("选择 Agent 配置入口"))
                                .dropdown_menu(move |mut menu, _, _| {
                                    for (section, label) in [
                                        (SettingsSection::PiAccounts, tr("内置 Pi 账号")),
                                        (SettingsSection::Models, tr("模型与接口")),
                                        (SettingsSection::Programs, tr("外部 Agent 程序")),
                                    ] {
                                        let weak = weak.clone();
                                        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                                            let _ = weak.update(cx, |this, cx| { this.settings_section = section; this.open_settings(window, cx); });
                                        }));
                                    }
                                    menu
                                }))))
                        .child(div().text_xs().text_color(Color::Muted.get(cx)).child(if self.connected {
                            tr("打开项目后再选择 Agent；不会自动发送消息或启动成员。")
                        } else { tr("后台暂不可用 · 可在窗口顶部重新启动后台") })))
                    .child(v_flex().flex_shrink_0().gap_4().pt_5().border_t_1().border_color(Color::Divider.get(cx))
                        .children([
                            ("01", tr("打开项目"), tr("选择本地文件夹，已有项目和会话会留在侧栏。")),
                            ("02", tr("选择你的主控"), tr("内置 Pi、Claude Code、Codex 或外部 Pi，使用各自的模型与账号。")),
                            ("03", tr("和团队一起工作"), tr("从主会话安排任务，随时查看成员的对话、执行过程与汇报。")),
                        ].into_iter().map(|(number, title, description)| h_flex().flex_shrink_0().items_start().gap_4()
                            .child(div().text_xs().pt_1().text_color(Color::Muted.get(cx)).child(number))
                            .child(v_flex().flex_shrink_0().id(SharedString::from(format!("welcome-step-{number}"))).role(Role::Label).aria_label(format!("{number} · {title} · {description}")).flex_1().min_w_0().gap_1()
                                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(title))
                                .child(div().text_sm().text_color(Color::Secondary.get(cx)).child(description)))))
                        .child(Reveal::new("welcome-programs-reveal", &self.welcome_scroll, Button::new("welcome-programs").ghost().label(tr("检查外部 Agent 程序"))
                            .on_click(cx.listener(|this, _, window, cx| { this.settings_section = SettingsSection::Programs; this.open_settings(window, cx); })))))
            )))
    }
}
