use crate::language::tr;
use crate::{
    app::{HiveApp, SettingsSection},
    palette::Color,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        *,
    },
    prelude::FluentBuilder,
    *,
};

struct Entry {
    section: Option<SettingsSection>,
    title: &'static str,
    description: &'static str,
    keywords: &'static str,
}
const ENTRIES: &[Entry] = &[
    Entry {
        section: Some(SettingsSection::Appearance),
        title: "外观",
        description: "主题、界面字号、语言与阅读预览",
        keywords: "theme appearance light dark font size text language locale english chinese 中文 英文 语言 主题 颜色 字体 字号 大小 阅读 缩放",
    },
    Entry {
        section: Some(SettingsSection::Chat),
        title: "聊天",
        description: "消息发送快捷键与输入方式",
        keywords: "enter shift cmd command 快捷键 发送 换行 键盘",
    },
    Entry {
        section: Some(SettingsSection::PiAccounts),
        title: "内置 Pi",
        description: "连接提供商、订阅登录与 API Key",
        keywords: "api key apikey token auth oauth login openai anthropic 认证 密钥 登录 账号 凭据 订阅",
    },
    Entry {
        section: Some(SettingsSection::Models),
        title: "模型与接口",
        description: "自定义接口、模型覆盖、上下文与能力",
        keywords: "model provider base url endpoint api context vision reasoning 模型 接口 提供商 上下文 图片 思考 继承 覆盖",
    },
    Entry {
        section: Some(SettingsSection::Team),
        title: "团队",
        description: "自动组队引导",
        keywords: "team worker orchestrator autostaff 主控 成员 协作 自动组队",
    },
    Entry {
        section: Some(SettingsSection::Remote),
        title: "远程接入",
        description: "账号连接、设备配对与访问记录",
        keywords: "remote pairing device gateway audit diagnostics support 远程 手机 配对 设备 网关 安全短码 审计 诊断 故障",
    },
    Entry {
        section: Some(SettingsSection::Notifications),
        title: "通知",
        description: "本机系统提醒与 Webhook 通知地址",
        keywords: "notification system local desktop webhook url 提醒 通知 地址 本机 系统",
    },
    Entry {
        section: Some(SettingsSection::Programs),
        title: "外部程序",
        description: "外部 Pi、Codex 与 Claude Code 的路径和版本检查",
        keywords: "pi codex claude code cli path executable version 终端 可执行 路径 版本 程序 检查",
    },
    Entry {
        section: None,
        title: "角色模板",
        description: "成员职责与角色提示词",
        keywords: "role template prompt 角色 模板 成员 职责 提示词",
    },
];
fn matches(entry: &Entry, query: &str) -> bool {
    let text = format!("{} {} {} {} {}", entry.title, entry.description, entry.keywords, tr(entry.title), tr(entry.description)).to_lowercase();
    query.split_whitespace().all(|word| text.contains(word))
}
impl HiveApp {
    pub fn select_settings_section(
        &mut self,
        section: SettingsSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.settings_section = section;
        if section == SettingsSection::Remote { self.refresh_remote(cx); }
        if section == SettingsSection::Models {
            self.load_pi_models(cx);
        }
        if section == SettingsSection::PiAccounts {
            self.load_pi_accounts(window.window_handle(), cx);
        }
        if !self.settings_panel {
            self.open_settings(window, cx);
        }
        self.preferences_focus.focus(window, cx);
        cx.notify();
    }
    fn open_settings_match(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(section) = ENTRIES[index].section {
            self.select_settings_section(section, window, cx);
        } else {
            self.settings_search
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.open_roles(cx);
            self.preferences_focus.focus(window, cx);
            cx.notify();
        }
    }
    pub fn open_first_settings_match(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.settings_search.read(cx).value().trim().to_lowercase();
        if query.is_empty() {
            return;
        }
        if let Some(index) = ENTRIES.iter().position(|entry| matches(entry, &query)) {
            self.open_settings_match(index, window, cx);
        }
    }
    pub(super) fn render_settings_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .max_w(px(960.))
            .mx_auto()
            .px_5()
            .pt_4()
            .gap_2()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&self.settings_search).aria_label(tr("搜索设置"))),
            )
            .when(!self.settings_search.read(cx).value().is_empty(), |row| {
                row.child(
                    Button::new("clear-settings-search")
                        .ghost()
                        .compact()
                        .label(tr("清除"))
                        .accessibility_label(tr("清除设置搜索"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.settings_search.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                            cx.notify();
                        })),
                )
            })
    }
    pub(super) fn render_settings_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.settings_search.read(cx).value().trim().to_lowercase();
        let entries: Vec<_> = ENTRIES
            .iter()
            .enumerate()
            .filter(|(_, entry)| matches(entry, &query))
            .collect();
        v_flex()
            .id("settings-search-results")
            .flex_1()
            .min_h_0()
            .w_full()
            .max_w(px(960.))
            .mx_auto()
            .px_5()
            .py_6()
            .gap_3()
            .overflow_y_scroll()
            .child(div().text_xs().text_color(Color::Secondary.get(cx)).child(
                if entries.is_empty() {
                    tr("没有找到设置页面").to_string()
                } else {
                    if crate::language::english() { format!("{} settings pages · Enter opens the first", entries.len()) } else { format!("找到 {} 个设置页面 · Enter 打开首项", entries.len()) }
                },
            ))
            .when(entries.is_empty(), |view| {
                view.child(
                    div()
                        .py_6()
                        .text_sm()
                        .text_color(Color::Secondary.get(cx))
                        .child(tr("没有匹配的设置。换个关键词，或清除搜索浏览全部设置。")),
                )
            })
            .children(entries.into_iter().map(|(index, entry)| {
                Button::new(("settings-search-result", index))
                    .ghost()
                    .w_full()
                    .h_auto()
                    .justify_start()
                    .p_4()
                    .rounded_lg()
                    .accessibility_label(format!("{} · {}", tr(entry.title), tr(entry.description)))
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .items_start()
                            .text_left()
                            .gap_1()
                            .child(div().text_sm().child(tr(entry.title)))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr(entry.description)),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_settings_match(index, window, cx)
                    }))
            }))
    }
}
