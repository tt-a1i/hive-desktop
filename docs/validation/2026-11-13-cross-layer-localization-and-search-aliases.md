# 跨层文案本地化、动态回退与双语会话搜索索引

2026-11-13，基线 `271082f`（推进 `#17` 跨层文案完整性）。当前 `0a388b3` 安装包尚未纳入本次源码修改。

## 实证问题与根因

逐模块审计 `apps/desktop/src/` 与 `packages/runtime/src/` 后定位五类跨层中英文显示与检索断层：

1. **会话搜索对默认未绑定会话与绑定角色缺少英文/双语检索词**：
   - 桌面界面对 `!custom_title && agent_id.is_none() && title == "新会话"` 的未发送记录在英文下显示 `New conversation`，但 `packages/runtime/src/chats.ts` 的 `Chats.search` 仅索引落盘 `'新会话'`，英文输入 `New conversation` 返回 0 条；同时绑定 `agentId` 的会话副标题在桌面展示角色（`主控` / `Orchestrator`、`工作流` / `Workflow`、`成员` / `Worker`），但搜索索引未纳入角色词。
2. **会话行与搜索结果角色标签不对称**：
   - `apps/desktop/src/navigation/session_row.rs` 仅区分 `:orchestrator` 与其他 `agent_id`，将 `:__workflow__` 会话误标为 `成员` / `Worker`；`apps/desktop/src/sessions/search.rs` 的搜索面板副标题、悬浮提示与无障碍标签未携带绑定角色。
3. **打开历史时固化的回退提示无法随语言即时切换，且缺来源提示可被误引用**：
   - `apps/desktop/src/conversation.rs` 在解析快照时将缺来源提示（`此消息缺少有效的来源记录。展开查看原文。`）与默认恢复提示（`已恢复会话，继续处理未完成的工作。`）直接写入 `Message.body`，切换语言后旧会话不随之更新，且缺来源提示的回退句可能被引用入草稿；`apps/desktop/src/failures.rs` 对空错误文本回退 `Agent 回复未完成。` 同样在构造期固化。
4. **部分自有跨层错误、适配器标签与英文标点遗漏**：
   - `apps/desktop/src/usage/labels.rs` 未翻译 `"内置 Pi 原生会话"` 与 `"外部 Pi 原生会话"`；`apps/desktop/src/reasoning_picker/view.rs` 未对推理档位恢复提示及自有错误做英文映射；桌面侧 `transport.rs` / `runtime_location.rs` / `file_viewer.rs` / `pi_models` / `chats.ts` 等自有中文错误在英文界面原样拼接中文；`navigation.rs`、`artifacts.rs`、`interactions/claude.rs`、`model_picker.rs`、`sessions.rs` 存在英文模式残留全角标点或未过 `tr("项目")` 的回退名。

## 最小修改

- **运行时双语搜索词（`packages/runtime/src/chats.ts`）**：
  - `Chats.search` 仅在 `!hit.session.customTitle && !hit.session.agentId && hit.session.title === '新会话'` 时补充 `'New conversation'`（用户显式重命名为 `'新会话'` 或首条真实输入为 `'新会话'` 的已绑定会话不误匹配）；当 `hit.session.agentId` 非空时按 `:orchestrator` / `:__workflow__` / 其他成员补充双语角色词（`'主控 Orchestrator'`、`'工作流 Workflow'`、`'成员 Worker'`），全程只读索引与落盘文件，零 Agent 启动。
- **渲染期动态回退与原文保护（`apps/desktop/src/`）**：
  - `app.rs` 为 `DeliveryCard` 增加 `missing_provenance: bool` 并新增 `Message::display_body(&self) -> &str`；`conversation.rs` 对缺来源投递保留空 `body`（复制仍取完整 `d.raw`，引用在 `missing_provenance` 且无正文时不把提示句拼入草稿），展示、会话内查找（`conversation_find.rs`）与导出（`conversation_export/snapshot.rs`）统一读取动态本地化的 `display_body()`；默认恢复摘要在 `body.is_empty()` 时按当前语言动态渲染。
  - `failures.rs` 新增 `Diagnostic::display_message(&self) -> &str`，仅在 `stop_reason == "error"` 且原始错误为空时动态回退 `tr("Agent 回复未完成。")`，保留非空原始错误逐字展示与复制。
- **自有错误闭合映射与跨层细节补齐**：
  - `language.rs` 新增闭合集 `localize_owned_error(error: &str) -> String`：中文模式零开销直返；英文模式仅精确匹配 Hive / 桌面自有固定文案、精确前缀与 4 条自有模板，未命中的外部 CLI / SDK / OS / 用户路径或错误逐字节保留（同时保持 `transport.rs` 与 `runtime_location.rs` 零 UI 依赖）。
  - 接入状态栏、用量、终端、文件查看器、设置、Pi 模型编辑、远程连接、命令面板、推理档位选择器、工作流、外观、聊天快捷键、角色、记忆、成员、协作及会话列表/搜索的错误展示。
  - 修正 `navigation/session_row.rs` 的 `:__workflow__` 角色标签并在 `sessions/search.rs` 补齐角色副标题与无障碍描述；修正英文标点连接符并将 `locales/en.json` 扩展至 1,298 条唯一键。

## 验证证据与边界

- **词表与静态核对**：词表唯一键检查通过（`1,298` 个唯一键、0 重复键、0 缺失 `tr()` 键、英文值 0 残留 CJK 字符）；`git diff --check`、`pnpm check`、`pnpm build:runtime` 通过。
- **双语搜索与真实冷副本核对**：
  - `pnpm --filter @hive-desktop/runtime exec tsx checks/session-search.ts`（`SESSION_SEARCH_OK`）。
  - `.artifacts/localization-20261113/verify-alias-search.mjs`（`ALIAS_SEARCH_OK`）：验证未绑定默认会话命中 `New conversation` 及后端组合词、显式重命名 `'新会话'` 不误匹配 `New conversation`、冷态绑定记录双语角色检索（`Orchestrator`/`主控`、`Workflow`/`工作流`、`Worker`/`成员`），以及 [d43f55e 真实 Terra 三会话](2026-11-12-latest-terra-team.md) 临时副本的 `Orchestrator`（1 条）与 `Worker`（2 条：`proof-alpha`、`proof-beta`）冷检索，全程 `events === 0`（零 Agent 启动、零网络/厂商请求）。
- **Rust 单元与跨平台编译检查**：
  - `cargo test --locked --manifest-path apps/desktop/Cargo.toml`（35 通过，含新增 `localize_owned_error` 闭合集与外部错误逐字保留断言）。
  - `cargo test --locked --manifest-path apps/desktop/Cargo.toml --no-default-features --features process-supervisor-check --bin process-supervisor-check`（13 通过）。
  - `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` 与 `--bin process-supervisor-check --target x86_64-pc-windows-msvc` 通过。
- **边界声明**：本批未关闭 `#17`（仍待原生窗口全表面中英文切换、字号档位、VoiceOver 与真实 Windows 硬件核验），未增改删 `packages/runtime/checks/` 正式测试文件，待随下一开发包一并组装入包并执行原生窗口抽查。
