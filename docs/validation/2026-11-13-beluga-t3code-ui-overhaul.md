# Beluga 引导页 + T3 Code 主窗口/侧栏/会话流/浮动输入框 UI/UX 重构验证（2026-11-13）

## 1. 目标与已确认的产品范围

围绕用户最新确认的 UI/UX 第一优先级决策（详见 [设计复刻与融合记录](../research/2026-11-13-beluga-t3code-ui-overhaul.md)）：

1. **Beluga 风格首次引导与本地 Agent 就绪面板**（`welcome.rs`, `welcome/agents.rs`）：
   - 顶部沉浸式可拖拽栏（侧栏收起时内嵌展开开关，无双栏割裂）；
   - 居中 `52px` 圆角品牌徽章 + `H I V E` 宽字距标识、`36px` 紧凑主标题与单 Agent 工作流副标题；
   - 双胶囊主次按钮（`打开项目文件夹` / `先配置 Agent`）与单卡四行分组式本地 Agent 环境就绪面板（内置 Pi、Codex、Claude Code、外部 Pi 状态圆点、徽标与配置入口，保留本机版本/账号检查免责声明）。
2. **T3 Code 风格主窗口骨架、沉浸式标题栏、左侧栏、会话流与浮动输入框**：
   - **macOS 无缝交通灯标题栏**（`chrome.rs`, `main.rs`）：启用 `appears_transparent`、`traffic_light_position(15, 16)` 与 `app_owns_titlebar_drag`，消除 macOS 原生双栏割裂与顶栏按钮双击歧义延迟；Windows 保持原生标题栏与窗口标题契约（`Hive`）。
   - **紧凑工程侧栏**（`navigation.rs`, `navigation/session_row.rs`, `navigation/settings.rs`）：`256px` 栏宽、内嵌交通灯顶栏、`新建会话` + 搜索 + 更多菜单、`28px` 项目分组头与 `29px` 单行会话条目 + `6px` 语义状态圆点（运行中/待回应/未读/报错）。
   - **纯净对话流与单行 WorkLog 工作过程**（`conversation_list.rs`, `conversation_list/view.rs`, `thinking/view.rs`, `tool_message.rs`）：`768px` 居中阅读栏宽、右对齐无边框 `rounded-2xl` 用户消息气泡、无头像纯净助手 Markdown 正文、底部轻量复制/引用操作，以及 `26–28px` 单行可折叠思考与工具执行行（按工具语义自动匹配文件/搜索/终端/编辑图标，展开呈现 `Color::Inset` 详情面板与只读 Diff/预览入口）。
   - **空项目居中起草页与 `rounded-2xl` 悬浮输入框**（`view.rs`, `composer_controls.rs`）：合并冗余顶栏与后端状态条，左下整合 `+` 附件、`/` 命令与后端胶囊徽标，右下整合模型/推理选择器、原生用量与 `30px` 圆形 `ArrowUp` 发送按钮；空项目居中展示项目胶囊、主标题与单 Agent 开发快捷提示词（`了解这个项目`、`审查未提交的改动`、`定位并修复问题`）。
   - **冷调中性浅色 / 曜石石墨深色主题同步**（`palette.rs`, `appearance.rs`, `settings/*`）：同步 `Color` 语义表面与 `gpui-kit` 全局 `Theme` 令牌（圆角、悬停、选中、边框与焦点环），设置页右栏接入 `46px` 顶栏搜索与拖拽支持。
3. **多 Agent 编排 UI 暂缓（后端零破坏）**：
   - 日常单 Agent 新工作区默认隐藏 `团队成员` / `协作概览` 侧栏入口、移除欢迎页三步团队卡与空项目团队快捷按钮；
   - 底层 Hive Runtime、RPC 协议、冷启动恢复与既有工作区成员/派单上下文自动感知（`has_active_team_context`）完整保留。

## 2. 验证结果

- **双语词条闭合检查**：新增 12 条静态界面词条写入 `apps/desktop/src/locales/en.json`（共 1,313 条键），全量扫描 `apps/desktop/src/**/*.rs` 全部 `tr(...)` 调用确认缺失键为 `0`。
- **Rust 原生编译与单元测试**：
  - `cargo check --offline --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`：`0` 错误、`0` 警告。
  - `cargo test --offline --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`：`36 passed; 0 failed; 1 ignored`。
  - `cargo test --offline --manifest-path apps/desktop/Cargo.toml --no-default-features --features process-supervisor-check --bin process-supervisor-check`：`13 passed; 0 failed; 1 ignored`。
- **Windows `x86_64-pc-windows-msvc` 交叉编译与自动化脚本对齐**：
  - `cargo check --offline --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`：通过。
  - `cargo check --offline --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --no-default-features --features process-supervisor-check --bin process-supervisor-check`：通过。
  - `scripts/capture-windows-preview.ps1`：补齐 `选择会话模型与推理强度` 无障碍名称匹配与旧名回退，保持全部既有 UIA 自动化选择器契约。

## 3. 边界说明

- 本次重构聚焦原生桌面 UI/UX 视觉层次、交互动线与单 Agent 默认体验收敛，未修改 `packages/runtime/` 协作与会话后端实现，无新厂商模型推理调用。
- 多 Agent 之间的互动与编排专属 UI 按已确认产品结论暂缓设计；真实 Windows 硬件渲染截图与 macOS 系统通知送达授权（`#25`）继续单列跟踪。
