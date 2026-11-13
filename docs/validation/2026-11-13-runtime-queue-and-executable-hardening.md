# 暂停队列新输入保持、可执行程序解析与元数据原子写清理

2026-11-13，基线 `45a54fe`。当前 `0a388b3` 安装包尚未纳入本次源码修复。

## 实证问题与根因

逐模块审计 `packages/runtime/src/` 并以真实 Hive HTTP / SQLite、原始 Codex `0.167.0` `app-server` 与官方 Claude Agent SDK（本地受控回环/合成 CLI 对端，零厂商推理）复现三处后端逻辑边界：

1. **暂停或投递失败的 `DeliveryQueue` 在补充新消息时被隐式恢复消费**：
   - 桌面契约要求“停止保留后续输入，暂停队列需显式继续”及“投递失败保留原队首与原因，待显式继续重试”。
   - 当会话不存在活动 Hive run 时（例如 Codex 在运行期排队补充消息后点击停止，`abort()` 会终止对应 Hive run 并将队列置于暂停；或任意后端在队列暂停/队首失败后冷重开后台），用户再次通过输入框发送补充消息调用 `chats.prompt()`，会触发 `activateOnce(id, true)` → `lifecycle.start()` → `run.handle.ready()` → `chat.queue.resume()`，以及 `chats.ts` 随后的 `chat.queue.resume()`。
   - 结果：原本暂停的旧后续消息与新补充消息被立即自动连发（`queriesBeforeExplicitResume: 3` 而非 `1`），冷重开的失败队首也被自动重发并清除原失败诊断（`requestsBeforeResume: 3` 而非 `1`）。
   - 同时需注意 Hive 的 `deliverPostStartBacklog` 在 `resolveReady()` 微任务阶段、`/start` HTTP 响应返回前即重放 outbox 积压汇报，因此不能无条件在所有 `prompt()` 期间抑制 `ready()`，否则会影响冷开空闲主控对积压汇报的正常消费。
2. **Claude CLI 可执行路径解析命中同名目录、空 `PATH` 段及 Windows POSIX 包装脚本**：
   - `claude.ts` 原 `resolveClaudeExecutable` 仅调用 `accessSync(path, constants.X_OK)`，在 POSIX 上对权限 `0755` 的同名目录（如工作目录或 `PATH` 中的 `claude/` 目录）直接返回成功，且未过滤空 `PATH` 段（`""` 会回落到当前工作目录相对路径 `'claude'`）。
   - 在 `win32` 分支中，裸命令 `claude` 先尝试无扩展名候选路径，再尝试 `.exe`；当 npm 全局目录同时存在 POSIX `#!/bin/sh` 无扩展名脚本 `claude` 与原生 `claude.exe`（或 `.cmd` 批处理）时，会误选无法由 `child_process.spawn(..., { shell: false })` 直接执行的脚本。
3. **Claude / Codex 会话元数据原子改写失败时遗留 `.tmp` 临时文件**：
   - `claude-metadata.ts` 与 `codex-metadata.ts` 在 `writeFileSync(temporary, ...)` 后执行 `renameSync(temporary, path)`，当重命名失败（如目标路径被目录占据触发 `EISDIR` 或权限拒绝）时未清理临时文件，在 `chats/claude`、`chats/codex` 目录残留 `*.json.<uuid>.tmp`。

## 最小修改

- `packages/runtime/src/chats.ts`：新增 `keepingPaused` 集合，仅在 `prompt()` 进入 `activateOnce(id, true)` 前检测到队列处于非忙碌且已有待处理项（`!opened.queue.busy && opened.queue.pendingCount > 0`，即已暂停或队首失败）时标记保持暂停；`ready()` 与 `prompt()` 仅在未命中该标记时调用 `queue.resume()`，并在 `finally` 中释放标记。空闲会话（`pendingCount === 0`）的正常发送、冷开主控的 outbox 汇报重放及显式 `chats.resume(id)` 保持不变。
- `packages/runtime/src/agents/claude.ts`：`resolveClaudeExecutable` 过滤空 `PATH` 段、统一 `resolve()` 候选路径、在 `accessSync(..., X_OK)` 前校验 `statSync(path).isFile()`；`win32` 对无扩展名 `PATH` 候选仅探测 `${path}.exe`，对显式路径要求 `.exe` 后缀（拒绝无扩展名 POSIX 脚本与 `.cmd`/`.bat`）。
- `packages/runtime/src/agents/claude-metadata.ts`、`packages/runtime/src/agents/codex-metadata.ts`：临时文件写入补充 `encoding: 'utf8', flush: true`，并用 `try { ... } finally { rmSync(temporary, { force: true }); }` 确保原子替换成功或失败均不残留 `.tmp` 文件。

## 前后真实对照

驱动与收据保存在 `.artifacts/backend-hardening-20261113/verify.mjs`、`before-receipt.json`、`after-receipt.json` 与 `cold-team-parity-receipt.json`：

| 场景 | 修复前（45a54fe） | 修复后 |
| --- | --- | --- |
| Codex 运行中排队补充消息 B → `abort()` 停止 → 再发补充消息 C → 显式 `resume()` | 发送 C 时立即自动触发 B 与 C（`queriesBeforeExplicitResume: 3`，暂停态丢失） | 发送 C 后 `queriesBeforeExplicitResume: 1`，队列保持 `["Follow-up B before stop", "Follow-up C while paused"]` 且 `busy: false`；显式 `resume()` 后按序完整消费 |
| 队首投递失败保留 → 冷重开后台 → `prompt()` 追加补充消息 → 显式 `resume()` | 冷重开后发送补充消息即自动重发失败队首与新消息（`requestsBeforeResume: 3`，队首失败诊断被清零） | `requestsBeforeResume: 1`，队首 `failure.message` 与两条待处理消息完整保持；显式 `resume()` 后按序成功投递 |
| `PATH` 含空段与同名 `0755` 目录、`win32` 同目录含无扩展名脚本/`claude.cmd`/`claude.exe` | 误返回工作目录相对 `'claude'` 或同名目录路径 | 跳过空段与目录并命中真实可执行文件；拒绝直接目录路径、`win32` 无扩展名脚本与 `.cmd`，精确命中 `claude.exe` |
| `claude-metadata` / `codex-metadata` 目标路径 `EISDIR` 原子重命名失败 | 抛错后残留 2 个 `*.json.<uuid>.tmp` 文件 | 原错误照常抛出，`leakedTmp: []`（零残留临时文件） |

## 回归与验证边界

通过 `pnpm check`、`pnpm build:runtime`、`git diff --check`、[d43f55e 真实 Terra 三会话副本冷读](2026-11-12-latest-terra-team.md)（`cold-team-parity.mjs`，3 会话 40 条历史逐字段一致），以及以下已有检查：

```sh
OPENAI_API_KEY=paused-queue-no-network pnpm --filter @hive-desktop/runtime check:runtime
OPENAI_API_KEY=paused-queue-no-network pnpm --filter @hive-desktop/runtime check:team
OPENAI_API_KEY=paused-queue-no-network pnpm --filter @hive-desktop/runtime exec tsx checks/executable-probe.ts
OPENAI_API_KEY=paused-queue-no-network pnpm --filter @hive-desktop/runtime exec tsx checks/queued-user-inputs.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/queued-user-inputs-peers.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/delivery-failures.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/claude-session.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/claude-runtime.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/codex-session.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/codex-runtime.ts
```

（`OPENAI_API_KEY=paused-queue-no-network` 仅满足 `PI_OFFLINE=1` 暂停队列检查的模型可用性前置判断，全程零网络/厂商模型请求。）未增改删正式测试，未进行面板走查或真实 Windows 机器执行，尚未纳入当前 `0a388b3` 安装包。
