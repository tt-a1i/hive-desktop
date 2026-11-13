# 会话停止清理竞态、Codex 账号探测取消与显示日志原子写防护

2026-11-13，基线 `884dfd4`。当前 `9e6a44a` 安装包尚未纳入本次源码修复。

## 实证问题与根因

逐文件审计 `packages/runtime/src/` 剩余模块，并以真实 Hive HTTP / SQLite 与本地受控 JSONL / Node 子进程对端（零厂商模型推理）复现四处后端并发与资源清理边界：

1. **`Chats.stopLive()` 未挂载 `chat.cleanup` 导致并发请求命中已释放会话**：
   - `chats.ts` 原 `stopLive(id, chat)` 在入口检查 `if (chat.cleanup) return chat.cleanup;`，却从未给 `chat.cleanup` 赋值，且在 `this.live.delete(id)` 之前异步等待 `abort({ settleTools: true })`、`queue.pause()` 与 `handle.dispose()`。
   - 结果：当会话正在停止或释放期间（例如 Codex `abort()`、主会话切换退役或工作区停止成员），并发到达的 `chat.open`、`chat.reasoning`、`chat.usage`、`chat.commands` 或 `chat.prompt` 调用 `require(id)` 时不会等待清理结束，而是直接拿到正在销毁的 `LiveChat` 实例并抛出 `"Codex session is disposed"`（`before-receipt.json`：`cleanupSetDuringStop: false`，`reasoningDuringDisposeOk: false`）。
   - 同时，`chats.abort(id)` 在 Codex 分支于 `await this.stopLive(id, chat)` 返回后的下一微任务才调用 `chat.onExit?.()`，使并发等待 `chat.cleanup` 的调用方在微任务窗口内仍可能看到尚未结算的旧 `runId`。
2. **`stopAgentRun()` 在 `handle.abort({ settleTools: true })` 抛错且传输层未自发断开时泄漏子进程、令牌与运行状态**：
   - 原 `stopLive()` 将 `await chat.handle.abort({ settleTools: true })` 置于 `try ... finally` 之外。当运行因自有错误拒绝 `abort` 且底层连接未主动关闭（例如适配器记录 `run.error` 后传输层仍存活）时，`stopLive()` 直接抛错跳过 `handle.dispose()`、`this.live.delete(id)` 与 `onExit`。
   - 结果：`waitForRunExit` 超时返回 `false`，`agent_runs.status` 卡在 `'running'`，Agent 鉴权令牌未吊销，子进程存活泄漏（`childDead: false`），且 `this.live` 残留坏会话记录（`liveCleared: false`）。
3. **`probeCodexAccount()` 在 `initialize` 握手挂起时无法响应 `AbortSignal` 立即取消**：
   - `codex-account.ts` 仅通过 `starting.then(value => value.close())` 注册取消回调；若目标可执行文件不返回 JSON-RPC `initialize` 响应，`CodexTransport.start()` 需等待完整 `5,000 ms` 请求超时后才能进入 `then` 分支关闭进程，拖慢设置面板切换或应用退出（实测取消耗时 `5,007 ms`）。
4. **`CodexProjection.save()` 原子重命名失败时遗留 `.tmp` 临时文件，以及 `team-tool` 子进程启动失败遗留 `abort` 监听器**：
   - `codex-projection.ts` 在 `writeFileSync(temporary, ...)` 后执行 `renameSync(temporary, this.file)` 未包裹 `finally` 清理，重命名失败（如 `EISDIR`）时会在 `chats/codex/` 残留 `*.display.json.<uuid>.tmp` 且不推进内存状态；`team-tool.ts` 在 `child` 触发 `'error'` 时未移除 `AbortSignal` 监听器。

## 最小修改

- `packages/runtime/src/chats.ts`、`packages/runtime/src/structured-runtime-types.ts`、`packages/runtime/src/structured-runtime-lifecycle.ts`：
  - 为 `StructuredSessionHandle.stop(error?: unknown)` 透传停止错误原因至 `stopLive(id, chat, exitError)`，并在 `stopLive()` 同步赋值返回 `chat.cleanup` Promise，使 `require(id)`、`respondInteraction()`、`failLive()` 与重复 `stopLive()` 严格串行化或去重。
  - 在 `stopLive()` 内以 `Promise.allSettled([chat.handle.abort({ settleTools: true }), paused])` 捕获中止异常、记录 `chat.failure` 并去重发送单次 `error` 事件（通过 `LiveChat.lastErrorMessage` 避免对回合中已广播的同一错误消息重复推送）；无论 `abort` 是否抛错，均在 `finally` 中保证执行 `await chat.handle.dispose()`、按实例身份清理 `this.live`，并在 `dispose()` 确认成功后、清理 Promise 结算前同步触发单次 `onExit?.(exitError ?? abortError)`（若 `dispose()` 自身失败则保留未退出运行状态，不谎报已回收）。同步移除 `chats.abort(id)` 中 Codex 分支的尾随 `chat.onExit?.()`。
- `packages/runtime/src/agents/codex-transport.ts`、`packages/runtime/src/codex-account.ts`：
  - `CodexTransport.start()` 支持可选 `signal?: AbortSignal`，在 `initialize` 请求期间监听取消并立即调用 `transport.close()`，结束后移除监听器；`probeCodexAccount()` 传入 `{ shutdownGraceMs: 200, signal }`。
- `packages/runtime/src/agents/codex-projection.ts`：
  - `CodexProjection.save()` 用 `try { ... } finally { rmSync(temporary, { force: true }); }` 包裹临时文件写入与原子重命名（保持流式投影原有写入参数，不额外引入每次增量的同步刷盘开销）。
- `packages/runtime/src/team-tool.ts`：
  - `child.once('error', ...)` 在拒绝前移除 `signal` 上的 `abort` 监听。

## 前后真实对照

复现脚本与收据保存在 `.artifacts/runtime-stop-and-probe-hardening-20261113/repro.mjs`、`before-receipt.json`、`after-receipt.json` 与 `cold-team-parity-receipt.json`：

| 场景 | 修复前（884dfd4） | 修复后 |
| --- | --- | --- |
| `stopLive` 释放期间并发调用 `chats.open` 与 `chats.reasoning` | `cleanupSetDuringStop: false`，`open` 返回过期 `"unset"`，`reasoning` 抛 `"Codex session is disposed"` | `cleanupSetDuringStop: true`，等待旧会话释放与 `onExit` 完成后重开新句柄，`openDuringDisposeRunId: null`，`reasoningDuringDisposeOk: true` |
| `stopAgentRun` 遇到 `abort` 抛错且传输层未自发断开 | `exitedInTime: false`，`runStatus: "running"`，令牌未吊销，`childDead: false`，`liveCleared: false` | `exitedInTime: true`，SQLite `runStatus: "error"`，令牌已吊销，子进程已回收（`ESRCH`），`liveCleared: true` |
| `stopAgentRun` 遇到 `turn/interrupt` 协议拒绝及冷重开 | 正常退出但需核对错误不重复广播 | `errorCount: 1`，SQLite `runStatus: "error"`，冷重开精确保留 `reopenedPending: 1` |
| `probeCodexAccount` 在 `initialize` 挂起 `150 ms` 时触发 `AbortController.abort()` | 阻塞至 `5,007 ms` 请求超时才返回 `cancelled` | `468 ms`（首次实测 `855 ms`，均含 `200 ms` 优雅关闭窗口）即返回 `cancelled` 并回收子进程（`ESRCH`） |
| `CodexProjection.save()` 目标路径 `EISDIR` 重命名失败 | 残留 `*.display.json.<uuid>.tmp` 临时文件 | 原错误照常抛出，`leftoverTmp: []`，内存投影保持不变 |

## 回归与验证边界

通过 `pnpm check`、`pnpm build:runtime`、`git diff --check`、[d43f55e 真实 Terra 三会话副本冷读](2026-11-12-latest-terra-team.md)（`cold-team-parity.mjs`，3 会话 40 条历史逐字段一致），以及以下已有回归检查：

```sh
pnpm --filter @hive-desktop/runtime check:codex
pnpm --filter @hive-desktop/runtime check:codex-account
pnpm --filter @hive-desktop/runtime check:workflow-shutdown
pnpm --filter @hive-desktop/runtime check:runtime
OPENAI_API_KEY=paused-queue-no-network pnpm --filter @hive-desktop/runtime check:team
OPENAI_API_KEY=paused-queue-no-network pnpm --filter @hive-desktop/runtime check:programs
pnpm --filter @hive-desktop/runtime exec tsx checks/claude-abort-rejection.ts
pnpm --filter @hive-desktop/runtime check:workspace-removal
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-transport.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-completion.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-session.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-session-lifecycle.ts
PI_OFFLINE=1 pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-runtime.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-runtime-lifecycle.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-abort-rejection.ts
```

（注：`external-pi-runtime.ts` 在未设 `PI_OFFLINE=1` 的受限网络下首次运行因外部 Pi CLI 在线启动检查触发 `get_state` 30 秒超时，与 [外部 Pi 诊断记录](2026-11-12-external-pi-diagnostics.md) 的默认环境观察一致；传入 `PI_OFFLINE=1` 后 5 秒内 `exit 0` 通过，未改动正式检查或外部 Pi 默认联网语义。）本批无厂商模型推理、无正式测试增删改、无原生面板走查，待随桌面端改进一并纳入下一开发包。
