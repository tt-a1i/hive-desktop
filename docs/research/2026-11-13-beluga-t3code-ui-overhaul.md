# Beluga + T3 Code UI/UX 复刻与融合记录（2026-11-13）

## 1. 背景与确认的产品决策

- **UI/UX 分工方案（黄金组合）**：
  - **首次进入引导与本地 Agent 就绪面板**：复刻 [kayraucklnc/beluga](https://github.com/kayraucklnc/beluga)（MIT，`302d4680f79dd4f5ea714d62cc66148d6092df0e`）的居中大气品牌头图、紧凑双主次按钮与分组式本地 Agent 环境就绪卡片。
  - **主窗口骨架、沉浸式标题栏、左侧工作区/会话导航、会话流、工具/思考过程折叠与浮动输入框**：复刻 [pingdotgg/t3code](https://github.com/pingdotgg/t3code)（MIT，`c7a3f02e3851923ced8ebd000db79f641e11b038`）的工程极简视觉体系。
- **多 Agent 编排交互策略**：
  - 按已确认结论，**暂缓在日常单 Agent 工作流中突出展示多 Agent 编排与团队入口**（移除欢迎页三步团队说明卡、移除空项目首页 `团队成员` 快捷图标与 `拆分团队任务` 引导词；左侧栏在未启用成员/派单的普通单 Agent 工作区默认隐藏 `团队成员` / `协作概览` 底部入口）。
  - **后端零破坏**：完整保留底层 Hive Runtime、RPC 协议、成员与派单冷启动恢复、会话归属校验及既有自动化验证契约；当工作区已存在成员会话、未读汇报或冷恢复派单时，侧栏自动提供紧凑协作入口。

## 2. 关键视觉与交互落地映射

| 模块 | 参考来源 | Hive Desktop 原生 GPUI 实现文件 | 核心设计要点 |
| --- | --- | --- | --- |
| 色彩与组件令牌 | Beluga `tokens.css` + T3 Code `index.css` | `apps/desktop/src/palette.rs`, `apps/desktop/src/appearance.rs` | 冷调中性浅色 Zinc（`#ffffff` / `#f6f6f8` / `#3b63f6`）与深色 Obsidian Graphite（`#16161a` / `#111114` / `#5b7cfa`），同步 `gpui-kit` 全局 `Theme` 圆角、悬停与选中态令牌 |
| 沉浸式标题栏与拖拽 | T3 Code `WorkspacePageHeader.tsx` | `apps/desktop/src/chrome.rs`, `apps/desktop/src/main.rs` | macOS 启用 `appears_transparent` + `traffic_light_position(15, 16)` + `app_owns_titlebar_drag`，消除原生双栏割裂与双击歧义延迟；Windows 保持原生标题栏兼容 |
| 首次引导与 Agent 就绪 | Beluga `Onboarding.tsx`, `AgentsStep.tsx` | `apps/desktop/src/welcome.rs`, `apps/desktop/src/welcome/agents.rs` | `52px` 圆角品牌徽章 + `H I V E` 字标、居中大标题、胶囊主次操作按钮、单卡四行分组式 Agent 环境检测（内置 Pi / Codex / Claude Code / 外部 Pi） |
| 左侧工作区与会话树 | T3 Code `SidebarProjectFolderHeader.tsx`, `SidebarThreadRow.tsx` | `apps/desktop/src/navigation.rs`, `apps/desktop/src/navigation/session_row.rs`, `apps/desktop/src/navigation/settings.rs` | `256px` 紧凑侧栏、交通灯内嵌顶栏、`新建会话` + 搜索 + 更多菜单、`28px` 项目分组头与 `29px` 单行会话条目 + `6px` 语义状态圆点 |
| 消息流与工作过程日志 | T3 Code `TimelineAssistantRows.tsx`, `WorkLog.tsx`, `TimelineWorkEntryRow.tsx` | `apps/desktop/src/conversation_list.rs`, `apps/desktop/src/conversation_list/view.rs`, `apps/desktop/src/thinking/view.rs`, `apps/desktop/src/tool_message.rs` | `768px` 阅读栏宽、右对齐无边框气泡用户消息、无头像纯净助手 Markdown 正文、`26–28px` 单行可展开 `WorkLog` 思考与工具执行行（按工具语义匹配图标） |
| 浮动输入框与空项目首页 | T3 Code `ChatComposer.tsx`, `ComposerControlsBar.tsx` | `apps/desktop/src/view.rs`, `apps/desktop/src/composer_controls.rs` | `rounded-2xl` 悬浮输入卡、左下 `+` 附件与后端胶囊、右下模型/推理选择器 + 用量 + `30px` 圆形 `ArrowUp` 发送按钮；空项目居中展示项目胶囊、主标题与单 Agent 开发快捷提示词 |

## 3. 开源许可与致谢说明

- **Beluga** (`https://github.com/kayraucklnc/beluga`, commit `302d4680f79dd4f5ea714d62cc66148d6092df0e`) — MIT License, Copyright (c) Kayra Uckilinc.
- **T3 Code** (`https://github.com/pingdotgg/t3code`, commit `c7a3f02e3851923ced8ebd000db79f641e11b038`) — MIT License, Copyright (c) Ping Labs / T3 Tools.
- Hive Desktop 使用 Rust + GPUI 原生独立实现全部视图与交互组件，未直接打包上述仓库的 TypeScript/CSS 源码文件。
