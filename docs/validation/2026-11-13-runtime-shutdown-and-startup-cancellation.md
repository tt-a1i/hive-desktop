# 运行时关闭隔离、启动期握手即时取消与开发态远程命令路径

2026-11-13，基线 `844b1bb`。当前 `4b3135d` 安装包尚未纳入本次源码修复。

## 实证问题与根因

逐模块审计 `packages/runtime/src/` 剩余运行时与传输层模块，并以真实 Hive HTTP / SQLite 与本地受控挂起 CLI 子进程对端（零厂商模型推理）复现四处后端关闭与启动取消边界：

1. **`AgentModels.close()` 与 `Chats.close()` 无法取消在途 `Codex`、`External Pi`、`Claude` 启动期握手**：
   - `agent-models.ts` 在 `close()` 中仅 `Promise.allSettled([...this.active])` 等待在途目录拉取自然结束，`chats.ts` 在 `close()` 中完全未跟踪或中断 `this.opening` 正在打开的原生会话。
   - 当用户配置的 `codex`、`pi`（`pi-rpc`）或 `claude` 可执行文件在启动期（`initialize` / `get_state` / `initializationResult` / `supportedModels` / `mcpServerStatus`）挂起且不响应输入流 EOF 时，应用关闭会被阻塞直至 30 秒 RPC / SDK 超时（`before.json`：`agentModelsClose.outcome: "stalled"`，`chatsClose.outcome: "stalled"`）。
   - 同时需严格区分工作区移除与进程退出：`AgentModels.removeWorkspace()` 与 `Chats.removeWorkspace()` 仍按契约等待在途句柄自然排空并在启动失败时拒绝移除，不误触发退出取消信号。
2. **`Chats.close()` 与 `TerminalManager` 多连接关闭使用 `Promise.all()` 存在早退未排空与错误丢失**：
   - 原 `Chats.close()` 执行 `await Promise.all([...this.live.values()].map(chat => chat.handle.dispose()))`。一旦首个会话快速拒绝，整个 `chats.close()` 立即返回并抛错，而其余较慢会话的 `dispose()` 仍在后台运行尚未退出（`before.json`：`siblingFinishedAtReturn: false`），破坏退出确定性。
   - `terminal.ts` 的 `stop()`、`detachWorkspace()` 与 `close()` 同样以 `Promise.all()` 关闭多路终端连接，首个失败会掩盖后续连接的关闭结果。
3. **`createDesktopRuntime().close()` 缺少子系统隔离且 `finally` 覆盖前序异常**：
   - 原 `desktop-runtime.ts` 在单层 `try ... finally` 中串行执行 `settings.close()`，随后在 `finally` 中直接 `await Promise.all([piModelConfig.close(), remote.close(), piAuth.close(), terminal.close(), agentModels.close()])`。
   - 一旦 `piAuth.close()` 抛错，不仅跳过后续阶段，还会使同阶段的异常提前中断并覆盖 `try` 块中 `settings.close()` 的原始错误（`before.json`：`terminalClosed: false`，`agentModelsClosed: false`，`subErrors: ["piauth-close-fault"]`）。
4. **`remote-commands.ts` 在源码态（`tsx`）解析不存在的 `src/remote-cli.js`**：
   - `remoteCommands()` 硬编码 `new URL('./remote-cli.js', import.meta.url)`；在 `dist/` 构建产物下存在，但在 `src/` 源码直接运行时指向不存在的 `packages/runtime/src/remote-cli.js`（`before.json`：`srcExists: false`）。

## 最小修改

- `packages/runtime/src/agents/codex-transport.ts`、`packages/runtime/src/agents/codex.ts`：
  - `CodexTransport.close(graceMs?)` 支持按次覆盖关闭宽限期，启动期 `AbortSignal` 触发时立即执行 `close(200)`；`CodexSessionOptions` 新增可选 `signal?: AbortSignal`，透传至 `CodexTransport.start()` 并在 `initialize` 之后的线程创建/恢复、目录读取与历史装载阶段全程监听取消并立即关闭底层传输。
- `packages/runtime/src/agents/external-pi-transport.ts`、`packages/runtime/src/agents/external-pi.ts`：
  - `ExternalPiTransport.start()` 支持可选 `signal?: AbortSignal` 与 `close(graceMs?)`，启动期取消时立即调用 `close(200)`；`createExternalPiSession()` 在 `get_state`、`get_commands`、`get_available_models` 等启动握手期间绑定取消监听并在中断时立即回收子进程。
- `packages/runtime/src/agents/claude-input.ts`、`packages/runtime/src/agents/claude.ts`：
  - `claudeDeadline()` 支持可选 `signal?: AbortSignal` 即时拒绝等待而无需耗尽 30 秒定时器；`createClaudeSession()` 新增可选 `signal?: AbortSignal`，在启动握手取消时同步调用 `controller.abort(...)`、`child?.kill()` 与 `close(200)` 立即回收未完成初始化的原生进程。
- `packages/runtime/src/agent-models.ts`、`packages/runtime/src/chats.ts`：
  - 分别维护仅在 `close()` 时触发的 `closeAbort`（不影响 `removeWorkspace` 的排空契约），向 `codex`、`claude`、`pi-rpc` 启动流程传入 `signal: this.closeAbort.signal`；`Chats.close()` 同时等待所有 `opening` 与 `live` 会话以 `Promise.allSettled()` 完整排空，若存在异常则抛出单个原错误或 `AggregateError`。
- `packages/runtime/src/terminal.ts`、`packages/runtime/src/desktop-runtime.ts`：
  - `TerminalManager` 提取 `closeConnections()` 以 `Promise.allSettled()` 排空连接并聚合异常；`desktop-runtime.ts` 的 `close()` 改为三阶段 `Promise.allSettled()`（第一阶段并发关闭 `settings`、`piModelConfig`、`remote`、`piAuth`、`terminal`、`agentModels`；第二阶段关闭 `hive`；第三阶段关闭 `chats`），确保任一子系统报错均不跳过其余子系统关闭且保留全部错误原因。
- `packages/runtime/src/remote-commands.ts`：
  - `resolveRemoteCli()` 优先使用同目录 `./remote-cli.js`（已编译包），在 `src/` 源码运行态回退至已构建的 `../dist/remote-cli.js` 或源码 `./remote-cli.ts`。

## 前后真实对照

复现脚本与收据保存在 `.artifacts/runtime-shutdown-and-startup-cancel-20261113/repro.mjs`、`before.json`、`after.json` 与 `cold-team-parity-receipt.json`：

| 场景 | 修复前（844b1bb） | 修复后 |
| --- | --- | --- |
| `src/remote-commands.ts` 解析远程 CLI 入口脚本 | 指向不存在的 `src/remote-cli.js`（`srcExists: false`） | 解析到存在的 `dist/remote-cli.js`（`srcExists: true`，`distExists: true`） |
| `codex` + `pi-rpc` + `claude` 三路目录查询挂起时调用 `agentModels.close()` | 2.5 秒窗口内卡死（`outcome: "stalled"`，需等 30 秒超时） | `204 ms` 完成关闭（`outcome: "closed"`），三路挂起子进程全部回收（`ESRCH`） |
| 三路会话打开挂起 + 一路快速抛错与一路慢速释放并存时调用 `chats.close()` | `outcome: "stalled"`，且快错导致慢速兄弟会话未在返回前完成释放（`siblingFinishedAtReturn: false`） | `384 ms` 排空全部 `opening` 与 `live` 会话后精准拒绝原错误，`siblingFinishedAtReturn: true`，三路挂起子进程全部回收（`ESRCH`） |
| `settings.close()` 与 `piAuth.close()` 同时抛错时调用 `runtime.close()` | `terminalClosed: false`，`agentModelsClosed: false`，`settings-close-fault` 被覆盖丢失 | `terminalClosed: true`，`agentModelsClosed: true`，抛出 `AggregateError(["settings-close-fault", "piauth-close-fault"])` |

## 回归与验证边界

通过 `pnpm check`、`pnpm build:runtime`、`git diff --check`、[d43f55e 真实 Terra 三会话副本冷读](2026-11-12-latest-terra-team.md)（`cold-team-parity.mjs`，3 会话 40 条历史逐字段一致），以及以下已有零推理回归检查：

```sh
pnpm --filter @hive-desktop/runtime check:runtime
pnpm --filter @hive-desktop/runtime check:workspace-removal
pnpm --filter @hive-desktop/runtime check:remote-settings
pnpm --filter @hive-desktop/runtime check:terminal
pnpm --filter @hive-desktop/runtime check:codex
pnpm --filter @hive-desktop/runtime check:codex-account
pnpm --filter @hive-desktop/runtime check:workflow-shutdown
PI_OFFLINE=1 pnpm --filter @hive-desktop/runtime check:external-pi
pnpm --filter @hive-desktop/runtime check:claude
```

本批无厂商模型推理、无正式测试增删改、无原生面板走查，待随桌面端改进一并纳入下一开发包。
