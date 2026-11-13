# 桌面端闭合错误本地化闭环、会话资源窄窗显露与交互/预览表面打磨

2026-11-13，基线 `2abce78`。当前 `4b3135d` 安装包尚未纳入本次桌面端源码更新。

## 改进范围与问题定位

全量扫描 `packages/runtime/src/` 与 `apps/desktop/src/` 跨边界提示文案与剩余弹窗/交互面板（推进 `#14`、`#16`、`#17`、`#20`），修复四处本地化与窄窗交互缺口并统一界面视觉层次：

1. **运行时跨边界自有诊断包装的闭合本地化（保留外部原文逐字节不变）**：
   - `packages/runtime/src/` 中剩余四类自有错误包装此前在英文界面下仍原样显示中文：`pi-auth.ts` 的 `"登录失败"`、`codex-transport.ts` 的 `"Codex 初始化失败（<cmd>）。请在「设置 → 外部程序」核对可执行文件。"` 首行引导、`claude.ts` 的 `"Claude 无法保存当前会话记录（<code>）"` / `"Claude 报告其他写入丢失（<code>，<lost>）；当前会话未报告记录丢失。"`，以及 `chats.ts` 队列诊断落盘失败包装 `"处理失败：<err>\n失败原因无法保存；原消息仍在待处理队列。<detail>"`。
   - `language.rs` 在 `localize_owned_error` 闭合集中精确匹配上述自有包装（同时补充至 `settings/pi_accounts.rs` 的 `runtime_message`），`failures.rs` 的 `Diagnostic::display_message()` 与 `details()` 接入闭合转换，`conversation_find.rs` 同时匹配展示文本与原始字段；非 Hive 自有的 OS / SDK / CLI 原始错误行（含多行堆栈、路径与 CJK 字符）继续逐字节原样保留，不改导出快照测试契约。
2. **图片查看器自有错误本地化与视口层次**：
   - `image_viewer.rs` 此前以 `format!("{}: {error}", tr(key))` 直接拼接错误原因，导致英文界面下 `"图片内容无法解码"`、`"不支持的图片格式。"`、`"保存路径无效"` 未经 `localize_owned_error` 转换；现补齐闭合转换并为 `#image-viewport` 增加圆角卡片边框。
3. **会话资源浏览弹窗（`#14` / `#20`）窄窗与矮窗键盘焦点自动显露**：
   - `command_picker/resources/view.rs` 的 `#session-resource-results` 原缺少 `.vertical_scrollbar(&self.scroll)` 与 `Viewport` + `Reveal` 焦点几何联动，在矮窗或窄窗下通过 `Tab` 浏览后续资源的“补入草稿 / 复制命令 / 查看来源文件 / 复制路径 / 定位文件”按钮时不会自动滚入视口；同时资源路径原为不可选普通文本。
   - 现接入 `Viewport::new("session-resource-viewport", &self.scroll, ...)`、纵向滚动条与每张卡片的 `Reveal::new(("resource-card-reveal", index), &self.scroll, ...)`，将资源路径升级为等宽可选文本（`selectable_text`），并统一为 `Color::Card` 圆角卡片、类型胶囊与胶囊操作按钮，既有元素 ID 与无障碍标签保持不变。
4. **三后端交互卡片、文件预览、Markdown 导出与工作区终端视觉统一**：
   - 融合 Beluga、T3 Code 与 Cindy 的卡片与代码阅读体验：Codex（`interactions/view.rs`）、Claude（`interactions/claude.rs`）与外部 Pi（`interactions/pi.rs`）交互卡片统一 `rounded_xl` 边框层次，命令、文件 Diff 与原生工具 JSON 输入升级为带细边框的等宽可选文本块（保留 `interaction-diff`、`claude-input-field`、`claude-native-input` 等全部原有元素 ID），并对齐中英文冒号标点；
   - `file_viewer.rs` 的 `#file-preview-path` 内置等宽可选路径文本，`conversation_export/view.rs` 为导出开关增加 `Color::Card` 工具条并以等宽字体展示 Markdown 预览，`terminal/view.rs` 统一终端连接胶囊与视口边框层次。

## 验证证据与边界

- **本地化闭合审计**：`LOCALE_AUDIT_OK { uniqueKeys: 1301, trSites: 1194 }`，全部 `tr(...)`、`OWNED_ERRORS` 与 `OWNED_PREFIXES` 100% 命中 `apps/desktop/src/locales/en.json`。
- **Rust 单元与进程监督测试**：
  - `cargo test --locked --manifest-path apps/desktop/Cargo.toml`：35 项测试全部通过（含新增 `language.rs` 闭合包装翻译与原始多行诊断逐字节保留断言，以及 `conversation_export` 快照测试）。
  - `cargo test --locked --manifest-path apps/desktop/Cargo.toml --features process-supervisor-check --bin process-supervisor-check`：13 项进程树监督与退出测试全部通过。
- **跨平台与构建检查**：
  - `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`（默认特性与 `--no-default-features`）均 0 警告通过。
  - `cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` 与 `git diff --check` 通过。

接下来将连同 `2abce78` 组装最新 macOS ARM64 开发包，执行包内四组正式门禁、启动取消与关闭隔离验证、真实 Terra 冷读与原生中英文窗口核验，并同步更新 `/Applications/Hive Desktop.app`。
