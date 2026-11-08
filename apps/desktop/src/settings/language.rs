use crate::{
    app::HiveApp,
    language::{self, Language, Locale, tr},
    palette::Color,
};
use gpui_kit::{
    component::{button::Button, *},
    *,
};

impl HiveApp {
    pub(super) fn render_language_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = cx.global::<Language>().locale;
        v_flex()
            .gap_2()
            .pb_4()
            .border_b_1()
            .border_color(Color::Divider.get(cx))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("界面语言")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(Color::Secondary.get(cx))
                    .child(tr(
                        "切换应用界面语言，不翻译会话正文、工具结果或用户配置内容。",
                    )),
            )
            .child(
                h_flex().gap_2().children(
                    [
                        (Locale::Zh, "中文", "language-zh"),
                        (Locale::En, "English", "language-en"),
                    ]
                    .into_iter()
                    .map(|(locale, label, id)| {
                        Button::new(id)
                            .rounded_full()
                            .label(label)
                            .accessibility_label(label)
                            .selected(selected == locale)
                            .toggled(selected == locale)
                            .disabled(!language::available(cx))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                language::choose(locale, cx);
                                cx.notify();
                            }))
                    }),
                ),
            )
            .children(language::message(cx).map(|message| div().text_sm().child(message)))
    }
    pub(crate) fn localize_placeholders(&self, window: &mut Window, cx: &mut App) {
        self.memory.localize_placeholder(window, cx);
        for (input, label) in [
            (&self.settings_search, "搜索设置，如模型、通知、快捷键…"),
            (&self.session_name, "会话名称"),
            (&self.folder, "项目文件夹的绝对路径"),
            (&self.member_name, "成员名称"),
            (&self.member_rename, "新名称"),
            (&self.role_name, "模板名称"),
            (&self.model_search, "搜索模型、提供商…"),
            (&self.member_search, "搜索成员、职责、角色或 Agent…"),
            (&self.member_model_search, "搜索提供商或模型"),
            (&self.pi_command, "留空时使用 PATH"),
            (&self.codex_command, "留空时使用 PATH"),
            (&self.claude_command, "留空时使用 PATH"),
        ] {
            input.update(cx, |input, cx| input.set_placeholder(tr(label), window, cx));
        }
        for (input, label) in [
            (&self.member_description, "职责说明（可选）"),
            (&self.role_description, "职责说明"),
        ] {
            input.update(cx, |input, cx| input.set_placeholder(tr(label), window, cx));
        }
    }
}
