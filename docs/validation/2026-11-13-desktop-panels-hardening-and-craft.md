# 桌面后台生命周期竞态、次级面板焦点显露与界面层次打磨

2026-11-13，分支 `codex/desktop-foundation`，在 `cd32e9d` 基础上完成对 `apps/desktop/src/` 后台重连生命周期、跨面板本地化遗漏及协作/成员/记忆/角色/工作流次级面板的键盘滚动显露与视觉层次统一。

## 根因与改进范围

### 1. 后台生命周期与重连视图状态（`app/backend.rs`、`sessions.rs`、`collaboration.rs`、`collaboration/tasks.rs`）

- **过期后台启动句柄回收**：`start_backend` 异步完成 `Runtime::launch` 后，若在启动期间 `connection_generation` 已因断线或手动重启推进，原先只覆盖或忽略结果而不主动关闭刚启动的过期进程树。现引入 `Adoption::{Adopted, Closing, Stale}` 分支，当检测到 `this.connection_generation != connection` 时立即对过期 `runtime` 执行 `runtime.shutdown().await`，确保不残留孤立 Node sidecar 进程组。
- **重连视图保留与显式切会话解耦**：`restore_backend_view` 在后台重连恢复工作区时保留 `self.collaboration.visible`（与工作流、成员、设置等顶层面板一致），避免用户在协作页查看派单或编辑任务文档时因短暂重连被强制踢回聊天视图；同时在 `sessions.rs` 的 `open_session` 中补齐 `self.close_collaboration()`，保证用户显式打开任一会话时必定切回对话主视图。
- **断线期间在途任务保存明确反馈**：`reset_collaboration_connection` 重置 `draft.saving = false` 与 `draft.operation = 0` 时，原先静默清除保存中状态而不留下任何提示，用户无法区分“已保存”与“断线未确认”。现写入本地化错误提示 `("连接已中断，任务保存结果尚未确认", "请重新读取或再次保存以核对团队文档。")` 并以 `Role::Alert` 呈现，草稿内容完整保留。

### 2. 次级面板本地化闭合补齐（推进 `#17`）

- **`members/view.rs` 模型目录读取错误**：修复英文下直接输出内部后端代号（如 `pi-rpc`）及中文分支误用未本地化变量的问题，统一映射 `Backend::label()` 展示名并在中英文双分支均接入闭合集 `localize_owned_error`（外部 CLI/SDK 原生错误仍逐字节保留）。
- **设置与导出自有错误**：`settings/remote/view.rs`（`#remote-read-error`）、`settings/pi_accounts.rs`（`runtime_message` 回退分支）与 `conversation_export/view.rs`（导出底部错误提示）统一接入 `localize_owned_error`。
- **词条完整性审计**：`language.rs` 与 `locales/en.json` 新增 `"内置"`、`"连接已中断，任务保存结果尚未确认"` 与 `"请重新读取或再次保存以核对团队文档。"`（闭合词典共 `1,301` 条唯一键，静态与自有错误键覆盖率 `100%`）。

### 3. 窄窗键盘滚动显露与 Beluga / T3 Code / Cindy 融合界面打磨（推进 `#20` 与整体体验）

- **记忆页（`memory.rs`、`memory/view.rs`）与角色模板页（`roles.rs`）补齐 `Viewport` + `Reveal`**：
  - 为 `Memory` 与 `Roles` 增加独立 `ScrollHandle`，接入 `Viewport`、`.track_scroll(...)` 与 `.vertical_scrollbar(...)`，为全部表单输入、筛选胶囊、开关与卡片操作按钮包裹 `Reveal`，消除窄窗/大字号下键盘 Tab 焦点落入视口外盲区的问题。
  - `Memory` 搜索框支持回车（`InputEvent::PressEnter`）直接触发检索，切换工作区时自动重置滚动偏移；`Roles` 点击“编辑”或“新建模板”自动滚回顶部并聚焦名称输入框；补齐全局错误横幅与 `Role::Status` / `Role::Alert` 无障碍语义。
- **协作、成员、记忆、角色与工作流面板视觉统一**（`collaboration/*`、`members/view.rs`、`memory/view.rs`、`roles.rs`、`workflows/*`）：
  - 统一 `960px` 居中阅读容器（`max_w(px(960.)).mx_auto()`）与纵向滚动条呈现。
  - 任务文档、派单列表与详情、团队成员卡片、记忆条目、角色模板、工作流运行/详情/关联派单/日志与定时计划全面升级为 `rounded_xl` `Color::Card` + `Color::Divider` 层次卡片。
  - 统一语义状态胶囊着色（`working` / `running` / `queued` / `submitted` 使用 `Color::Attention`；`idle` / `reported` / `completed` / `active` / `enabled` 使用 `Color::Inset` 配 `Color::Success` 状态圆点；`failed` 使用 `Color::ErrorSurface` + `Color::Error`；`stopped` / `interrupted` / `cancelled` / `deprecated` 使用 `Color::Inset` + `Color::Secondary`）。
  - 全部既有元素 ID、无障碍 `Role` / `aria_label` 与文本可选复制组件保持不变。

## 验证证据

- **词条与格式静态核验**：
  - 全仓 `apps/desktop/src/**/*.rs` 的 `tr(...)`、`OWNED_ERRORS`、`OWNED_PREFIXES` 与面板错误键核对脚本输出 `LOCALE_AUDIT_OK 1301`，`git diff --check` 零空白/格式问题。
- **Rust 单元测试与进程树监督回归**：
  - `cargo test --locked --manifest-path apps/desktop/Cargo.toml`：`35 passed; 0 failed; 1 ignored`（含更新后的 `language_preferences_reopen_and_preserve_invalid_or_failed_targets`）。
  - `cargo test --locked --manifest-path apps/desktop/Cargo.toml --features process-supervisor-check --bin process-supervisor-check`：`13 passed; 0 failed; 1 ignored`。
- **Windows 交叉编译类型检查与 macOS Release 构建**：
  - `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`：`exit 0`，零警告。
  - `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --no-default-features`：`exit 0`，零警告。
  - `cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`：`exit 0`，零警告。

## 边界说明

- 本轮不涉及任何厂商模型推理调用，未修改 `packages/runtime/checks/` 正式测试文件。
- Windows 仅在 macOS 上完成 `x86_64-pc-windows-msvc` 编译期检查，不冒充真实 Windows 硬件 GUI 与通知运行验收；`#17`、`#20`、`#25` 仍保持开放，本批源码改动随即连同 `cd32e9d` 一并打包入最新 macOS ARM64 开发包并执行包内与原生窗口核验。
