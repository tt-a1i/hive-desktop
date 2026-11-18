use crate::language::tr;
use crate::{
    app::{HiveApp, SettingsSection},
    keybindings::Target as KeyTarget,
    palette::Color,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};

struct Entry {
    section: Option<SettingsSection>,
    anchor: Option<&'static str>,
    keybinding: Option<KeyTarget>,
    title: &'static str,
    description: &'static str,
    keywords: &'static str,
}

const ENTRIES: &[Entry] = &[
    Entry {
        section: Some(SettingsSection::General),
        anchor: None,
        keybinding: None,
        title: "通用",
        description: "默认打开工具、侧边栏排序、忙碌发送方式与确认保护",
        keywords: "general default editor open target sort pin time busy send steer queue confirm quit 通用 打开 排序 时间 确认 退出 发送",
    },
    Entry {
        section: Some(SettingsSection::General),
        anchor: Some("general-open-target"),
        keybinding: None,
        title: "通用 · 默认打开工具",
        description: "选择标题栏打开项目时默认使用的外部编辑器、终端或文件管理器",
        keywords: "open target finder explorer vscode cursor zed terminal ghostty 默认 打开 编辑器 终端 访达 资源管理器",
    },
    Entry {
        section: Some(SettingsSection::General),
        anchor: Some("general-project-sort"),
        keybinding: None,
        title: "通用 · 侧边栏项目排序",
        description: "按添加顺序、最近活动或名称字母排列左侧工作区",
        keywords: "sidebar project sort added recent alphabetical 侧边栏 项目 排序 最近 字母",
    },
    Entry {
        section: Some(SettingsSection::General),
        anchor: Some("general-pin-active"),
        keybinding: None,
        title: "通用 · 活跃会话置顶",
        description: "优先将正在执行或待回应的会话排在项目顶部",
        keywords: "pin active running session top 活跃 进行中 置顶 侧边栏",
    },
    Entry {
        section: Some(SettingsSection::General),
        anchor: Some("general-time-format"),
        keybinding: None,
        title: "通用 · 会话时间戳格式",
        description: "在相对时间、24 小时制与 12 小时制间切换",
        keywords: "timestamp time format relative 24h 12h clock 时间戳 相对时间 24小时 12小时",
    },
    Entry {
        section: Some(SettingsSection::General),
        anchor: Some("general-busy-send"),
        keybinding: None,
        title: "通用 · 会话忙碌时发送方式",
        description: "选择排队等待、优先插队引导或等待当前回复完成",
        keywords: "busy send queue steer block follow up 忙碌 发送 排队 插队 引导 禁止",
    },
    Entry {
        section: Some(SettingsSection::General),
        anchor: Some("general-confirmations"),
        keybinding: None,
        title: "通用 · 操作与退出确认",
        description: "设置归档、取消置顶、撤回待发消息及退出应用前的二次确认",
        keywords: "confirm archive unpin cancel queue quit exit 确认 归档 置顶 撤回 退出",
    },
    Entry {
        section: Some(SettingsSection::Appearance),
        anchor: None,
        keybinding: None,
        title: "外观",
        description: "主题、界面字号、对话宽度、代码与终端排版、语言",
        keywords: "theme appearance light dark font size chat width code terminal wrap language locale english chinese 主题 外观 字号 宽度 字体 换行 语言",
    },
    Entry {
        section: Some(SettingsSection::Appearance),
        anchor: Some("appearance-chat-width"),
        keybinding: None,
        title: "外观 · 对话列宽",
        description: "在标准、舒适与宽屏间切换主对话与输入框最大宽度",
        keywords: "chat width column standard comfortable wide 对话 宽度 列宽 标准 舒适 宽屏",
    },
    Entry {
        section: Some(SettingsSection::Appearance),
        anchor: Some("appearance-code-typography"),
        keybinding: None,
        title: "外观 · 代码与文件预览排版",
        description: "配置代码等宽字体、字号与自动换行",
        keywords: "code font family size word wrap file preview diff 代码 字体 字号 等宽 自动换行 文件预览",
    },
    Entry {
        section: Some(SettingsSection::Appearance),
        anchor: Some("appearance-terminal-typography"),
        keybinding: None,
        title: "外观 · 工作区终端排版",
        description: "独立设置内置工作区终端字体与字号并实时生效",
        keywords: "terminal font family size 终端 字体 字号 等宽",
    },
    Entry {
        section: Some(SettingsSection::Chat),
        anchor: None,
        keybinding: None,
        title: "聊天",
        description: "发送快捷键、工具活动与思考展示",
        keywords: "enter shift cmd command thinking tools activity group completed 聊天 发送 换行 工具 思考",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: None,
        title: "快捷键",
        description: "搜索、录制与恢复全局快捷键，保存后立即生效",
        keywords: "keybinding shortcut keyboard record custom disable reset live 快捷键 键盘 自定义 录制 停用 恢复",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::NewSession),
        title: "快捷键 · 新建会话",
        description: "快速在当前工作区发起新对话",
        keywords: "shortcut new session cmd-n ctrl-n 快捷键 新建会话",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::Search),
        title: "快捷键 · 搜索会话",
        description: "打开全局会话搜索弹窗",
        keywords: "shortcut search sessions cmd-k ctrl-k 快捷键 搜索会话",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::Find),
        title: "快捷键 · 查找会话内容",
        description: "在当前会话中搜索内容",
        keywords: "shortcut find conversation cmd-f ctrl-f 快捷键 会话内查找",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::PreviousSession),
        title: "快捷键 · 上一个会话",
        description: "按侧边栏顺序切换到上一条会话",
        keywords: "shortcut previous session cycle 快捷键 上一个会话",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::NextSession),
        title: "快捷键 · 下一个会话",
        description: "按侧边栏顺序切换到下一条会话",
        keywords: "shortcut next session cycle 快捷键 下一个会话",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::ToggleSidebar),
        title: "快捷键 · 切换侧边栏",
        description: "展开或收起左侧导航栏",
        keywords: "shortcut toggle sidebar cmd-b ctrl-b 快捷键 侧边栏",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::ToggleTerminal),
        title: "快捷键 · 切换终端面板",
        description: "打开或收起当前工作区的终端视图",
        keywords: "shortcut toggle terminal 快捷键 终端面板",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::OpenGitWorkspace),
        title: "快捷键 · 打开 Git 工作区",
        description: "查看工作区 Git 变更与提交状态",
        keywords: "shortcut git workspace diff status 快捷键 git 工作区",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::SearchProjectFiles),
        title: "快捷键 · 搜索项目文件",
        description: "打开文件选择器并插入引用",
        keywords: "shortcut project files picker cmd-p ctrl-p 快捷键 搜索文件",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::ChooseModel),
        title: "快捷键 · 选择模型",
        description: "展开或关闭模型选择器",
        keywords: "shortcut choose model picker 快捷键 选择模型",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::ToggleMode),
        title: "快捷键 · 切换会话模式",
        description: "在当前 Agent 的会话模式间切换",
        keywords: "shortcut toggle mode plan code 快捷键 切换模式",
    },
    Entry {
        section: Some(SettingsSection::Keybindings),
        anchor: None,
        keybinding: Some(KeyTarget::Preferences),
        title: "快捷键 · 打开设置",
        description: "打开应用设置弹窗",
        keywords: "shortcut preferences settings cmd-, ctrl-, 快捷键 打开设置",
    },
    Entry {
        section: Some(SettingsSection::Notifications),
        anchor: None,
        keybinding: None,
        title: "通知",
        description: "本机系统提醒与 Webhook 通知地址",
        keywords: "notification system local desktop webhook url 提醒 通知 地址 本机 系统",
    },
    Entry {
        section: Some(SettingsSection::PiAccounts),
        anchor: None,
        keybinding: None,
        title: "内置 Pi",
        description: "连接提供商、订阅登录与 API Key",
        keywords: "api key apikey token auth oauth login openai anthropic 认证 密钥 登录 账号 凭据 订阅",
    },
    Entry {
        section: Some(SettingsSection::Models),
        anchor: None,
        keybinding: None,
        title: "模型与接口",
        description: "自定义接口、模型覆盖、上下文与能力",
        keywords: "model provider base url endpoint api context vision reasoning 模型 接口 提供商 上下文 图片 思考 继承 覆盖",
    },
    Entry {
        section: Some(SettingsSection::Profiles),
        anchor: None,
        keybinding: None,
        title: "Agent 配置",
        description: "管理启动配置、环境变量和历史版本",
        keywords: "agent profile profiles driver command args env revision 配置 启动 版本",
    },
    Entry {
        section: Some(SettingsSection::Programs),
        anchor: None,
        keybinding: None,
        title: "外部程序",
        description: "外部 Pi、Codex 与 Claude Code 的路径和版本检查",
        keywords: "pi codex claude code cli path executable version 终端 可执行 路径 版本 程序 检查",
    },
    Entry {
        section: Some(SettingsSection::Diffs),
        anchor: None,
        keybinding: None,
        title: "Diff 与 PR",
        description: "默认差异视图、空白过滤与 Pull Request 创建偏好",
        keywords: "diff unified split collapse whitespace pr pull request draft base branch 差异 分栏 折叠 空白",
    },
    Entry {
        section: Some(SettingsSection::Diffs),
        anchor: Some("diffs-view-mode"),
        keybinding: None,
        title: "Diff 与 PR · 默认视图与空白",
        description: "设置默认统一或双栏 Diff、多文件折叠与忽略空白变更",
        keywords: "diff mode unified split multi file collapse whitespace 差异视图 双栏 统一 折叠 忽略空白",
    },
    Entry {
        section: Some(SettingsSection::Diffs),
        anchor: Some("diffs-pr-defaults"),
        keybinding: None,
        title: "Diff 与 PR · Pull Request 默认值",
        description: "预设创建 PR 时是否默认草稿及目标分支",
        keywords: "pr pull request draft default base branch main 草稿 目标分支",
    },
    Entry {
        section: Some(SettingsSection::ProjectScripts),
        anchor: None,
        keybinding: None,
        title: "项目脚本",
        description: "保存项目命令，手动运行并查看后台执行记录与输出",
        keywords: "project scripts command run stop output logs 项目 脚本 命令 运行 停止 输出 日志",
    },
    Entry {
        section: Some(SettingsSection::Team),
        anchor: None,
        keybinding: None,
        title: "团队",
        description: "自动组队引导",
        keywords: "team worker orchestrator autostaff 主控 成员 协作 自动组队",
    },
    Entry {
        section: None,
        anchor: None,
        keybinding: None,
        title: "角色模板",
        description: "成员职责与角色提示词",
        keywords: "role template prompt 角色 模板 成员 职责 提示词",
    },
    Entry {
        section: Some(SettingsSection::Archive),
        anchor: None,
        keybinding: None,
        title: "归档会话",
        description: "搜索、恢复并打开已归档的历史会话",
        keywords: "archive archived sessions restore unarchive history 归档 已归档 恢复 历史会话",
    },
    Entry {
        section: Some(SettingsSection::About),
        anchor: None,
        keybinding: None,
        title: "关于与维护",
        description: "版本、后台诊断、临时缓存清理与开源许可",
        keywords: "about version build revision runtime node hive diagnostics maintenance clean logs tmp welcome guide license check update download 版本 构建 诊断 清理 维护 引导 开源许可 检查 更新 下载",
    },
    Entry {
        section: Some(SettingsSection::About),
        anchor: Some("about-maintenance"),
        keybinding: None,
        title: "关于与维护 · 数据目录与缓存清理",
        description: "在文件管理器中定位数据目录，扫描并清理临时日志与缓存文件",
        keywords: "data dir reveal logs tmp cache clean scan maintenance 数据目录 缓存 临时 日志 清理",
    },
    Entry {
        section: Some(SettingsSection::About),
        anchor: Some("about-guide-licenses"),
        keybinding: None,
        title: "关于与维护 · 欢迎引导与开源许可",
        description: "重新查看欢迎引导页或浏览随附第三方开源许可证",
        keywords: "welcome guide open source licenses notice 欢迎 引导 开源 许可",
    },
];

fn matches(entry: &Entry, query: &str) -> bool {
    let text = format!(
        "{} {} {} {} {}",
        entry.title,
        entry.description,
        entry.keywords,
        tr(entry.title),
        tr(entry.description)
    )
    .to_lowercase();
    query.split_whitespace().all(|word| text.contains(word))
}

fn filtered_indices(query: &str) -> Vec<usize> {
    ENTRIES
        .iter()
        .enumerate()
        .filter(|(_, entry)| matches(entry, query))
        .map(|(index, _)| index)
        .collect()
}

impl HiveApp {
    pub fn select_settings_section(
        &mut self,
        section: SettingsSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences_search_open = false;
        self.settings_search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.settings_search_selected = 0;
        self.settings_section = section;
        if section == SettingsSection::Profiles {
            self.load_profiles(cx);
        }
        if section == SettingsSection::Remote {
            self.refresh_remote(cx);
        }
        if section == SettingsSection::Models {
            self.load_pi_models(cx);
        }
        if section == SettingsSection::PiAccounts {
            self.load_pi_accounts(window.window_handle(), cx);
        }
        if section == SettingsSection::Archive {
            self.load_archived_settings(false, cx);
        }
        if !self.settings_panel {
            self.open_settings(window, cx);
        }
        self.preferences_focus.focus(window, cx);
        cx.notify();
    }

    fn open_settings_match(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.preferences_search_open = false;
        let entry = &ENTRIES[index];
        if let Some(section) = entry.section {
            self.select_settings_section(section, window, cx);
            if let Some(anchor) = entry.anchor {
                self.highlight_settings_anchor(anchor, cx);
            } else {
                self.settings_highlight_anchor = None;
            }
            if let Some(target) = entry.keybinding {
                self.keybinding_settings
                    .update(cx, |editor, cx| editor.focus_target(target, window, cx));
            }
        } else {
            self.settings_search
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.settings_search_selected = 0;
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
        let matches = filtered_indices(&query);
        if matches.is_empty() {
            return;
        }
        let selected = self.settings_search_selected.min(matches.len() - 1);
        self.open_settings_match(matches[selected], window, cx);
    }

    pub(crate) fn step_settings_search(&mut self, delta: isize, cx: &mut Context<Self>) {
        let query = self.settings_search.read(cx).value().trim().to_lowercase();
        if query.is_empty() {
            return;
        }
        let count = filtered_indices(&query).len();
        if count == 0 {
            return;
        }
        self.settings_search_selected =
            (self.settings_search_selected as isize + delta).rem_euclid(count as isize) as usize;
        cx.notify();
    }

    pub(crate) fn render_settings_search(
        &self,
        _compact: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let has_query = !self.settings_search.read(cx).value().is_empty();
        h_flex()
            .w_full()
            .min_w_0()
            .h(px(48.))
            .flex_shrink_0()
            .px_3()
            .gap_1()
            .items_center()
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                let query = this.settings_search.read(cx).value().trim().to_string();
                if query.is_empty() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "down" if event.keystroke.modifiers.number_of_modifiers() == 0 => {
                        this.step_settings_search(1, cx);
                        cx.stop_propagation();
                    }
                    "up" if event.keystroke.modifiers.number_of_modifiers() == 0 => {
                        this.step_settings_search(-1, cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.settings_search)
                        .small()
                        .aria_label(tr("搜索设置")),
                ),
            )
            .when(has_query, |row| {
                row.child(
                    Button::new("clear-settings-search")
                        .ghost()
                        .compact()
                        .h(px(24.))
                        .w(px(24.))
                        .icon(Glyph::X)
                        .tooltip(tr("清除设置搜索"))
                        .accessibility_label(tr("清除设置搜索"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.settings_search.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                            this.settings_search_selected = 0;
                            cx.notify();
                        })),
                )
            })
    }

    pub(crate) fn render_settings_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.settings_search.read(cx).value().trim().to_lowercase();
        let indices = filtered_indices(&query);
        let selected_pos = if indices.is_empty() {
            0
        } else {
            self.settings_search_selected.min(indices.len() - 1)
        };
        let scroll = &self.settings_search_scroll;
        crate::focus_scroll::Viewport::new(
            "settings-search-viewport",
            scroll,
            v_flex()
                .id("settings-search-results")
                .flex_1()
                .min_h_0()
                .min_w_0()
                .w_full()
                .px_2()
                .pb_3()
                .gap_1()
                .overflow_y_scroll()
                .track_scroll(scroll)
                .vertical_scrollbar(scroll)
                .when(indices.is_empty(), |view| {
                    view.child(
                        div()
                            .px_2()
                            .py_3()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("没有找到匹配的设置或快捷键")),
                    )
                })
                .children(indices.into_iter().enumerate().map(|(pos, index)| {
                    let entry = &ENTRIES[index];
                    let active = pos == selected_pos;
                    crate::focus_scroll::Reveal::new(
                        ("settings-search-reveal", index),
                        scroll,
                        Button::new(("settings-search-result", index))
                            .custom(crate::palette::sidebar_item_variant(active, cx))
                            .w_full()
                            .h_auto()
                            .px_2()
                            .py_2()
                            .rounded_md()
                            .accessibility_label(format!(
                                "{} · {}",
                                tr(entry.title),
                                tr(entry.description)
                            ))
                            .child(
                                v_flex()
                                    .w_full()
                                    .min_w_0()
                                    .items_start()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_left()
                                            .text_color(Color::Text.get(cx))
                                            .child(tr(entry.title)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_left()
                                            .whitespace_normal()
                                            .text_color(Color::Muted.get(cx))
                                            .child(tr(entry.description)),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_settings_match(index, window, cx)
                            })),
                    )
                })),
        )
    }
}
