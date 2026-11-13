# 桌面引导、输入框与协作过程界面层次打磨

2026-11-13，基线 `bf44928`。融合 Beluga（首屏就绪度与步骤卡片）、T3 Code（输入框控制栏后端徽标、建议胶囊与工具行节奏）及 Cindy（协作投递卡片与平静对比度）的桌面细节，不改变既有元素 ID、无障碍标签或焦点滚动契约。

## 改进要点

1. **Beluga 风格首屏 Agent 就绪卡片与三步引导（`welcome.rs`、`welcome/agents.rs`、`palette.rs`）**：
   - 四个 Agent 卡片（内置 Pi、Codex、Claude Code、外部 Pi）升级为 `Color::Card` 圆角面板，补充独立图标徽章与本机检查状态圆点（新增 `Color::Success` 语义色：仅反映本机已存凭据或已检测程序版本，严格保留 `welcome-agent-check-scope` 免责声明，不推导登录有效性或模型可用性）。
   - 三步上手说明（`welcome-step-01` ~ `03`）与项目主页顶部的项目上下文栏（`home-project-folder` / `home-project-name` / `home-project-path`）改为带清晰边框与层次的卡片容器。
2. **T3 Code + Cindy 风格输入框控制栏与起始建议胶囊（`composer_controls.rs`、`view.rs`）**：
   - 修复新建未发送会话处于起始欢迎态（`welcome && session.is_some()`）时输入框区域缺少当前后端标识的断层：在控制栏左侧新增 `composer-backend-badge` 胶囊（图标 + 当前 Agent 名称），并在进入对话态后的顶部后端胶囊中统一展示对应后端图标。
   - 三个项目起始建议按钮（`starter-0` ~ `starter-2`）增加 `Color::Card` 边框胶囊衬底，改善浅色/深色主题可读性；工作区空闲连接态在输入框底部温和提示当前发送快捷键（`SendMode::hint()`）。
3. **结构化工具过程与协作投递/通知卡片（`tool_message.rs`、`conversation_list/view.rs`）**：
   - 工具组折叠头与单条工具行将纯文本三角符统一替换为 `Glyph::ChevronDown` / `Glyph::ChevronRight` 矢量图标，工具组补充 `Glyph::Cpu` 图标与计数胶囊，单条工具名采用紧凑 `Color::Inset` 徽章并按状态区分色彩（`完成` / `失败` / `执行中`）。
   - 成员汇报与派单投递消息（`DeliveryCard`）增加独立 `Color::Card` 边框卡片容器与胶囊态 `report-state`，原文展开区改为内嵌 `Color::Inset` 衬底；会话恢复等 `notice` 消息采用柔和 `Color::Inset` 底色，与普通助手正文清晰分层。
4. **设置页程序检查自有错误映射补齐（`settings/programs.rs`）**：
   - `program_setup_status` 与 `executable_field` 错误读出接入 `localize_owned_error`，确保英文界面下自有提示一致翻译、外部 CLI 原始报错逐字节保留。

## 编译与静态验证

- 词表核对：`1,298` 条唯一键，0 缺失 `tr()` 键；`git diff --check` 通过。
- Rust 单元测试与进程树测试：`cargo test --locked --manifest-path apps/desktop/Cargo.toml`（35 通过）、`--bin process-supervisor-check`（13 通过）。
- 跨平台与发布构建：`cargo check --locked --target x86_64-pc-windows-msvc`（主程序与 `process-supervisor-check` 均通过）、`cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` 通过。
- 接下来随新开发包一并组装入包并执行原生 macOS 窗口中英文/界面核验。
