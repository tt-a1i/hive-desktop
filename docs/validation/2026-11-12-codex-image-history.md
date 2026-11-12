# Codex 合规图片历史、分页冷恢复与单行包络

2026-11-12，基线 f8a9460。当前 7b7940c 安装包尚未纳入本次源码修复。

## 实证问题与根因

`prompt-images.ts` 允许单张图片 base64 最多 5,000,000 字符、单次合计最多 20,000,000 字符（最多 10 张）。使用原始 `/Users/tsk/.npm-global/bin/codex` 0.167.0（`app-server --listen stdio://`）、隔离 `CODEX_HOME` 本地回环 Responses 对端（零厂商推理）及真实 Hive HTTP / SQLite 上传链路核验发现三处真实冲突：

1. **单轮合规多图事件帧超过原 8 MiB 上限且重复拷贝缓冲**：Codex `app-server` 对 `userMessage` 输出的 `item/started` 与 `item/completed` 原生通知直接内联全部 `data:image/...;base64,...`。当单次请求附带合规多图（如 6 张共 19,450,680 字符 ≤ 20 MB，或 `dispatch` 3 张共 9.73 MB）时，单条 JSONL 帧超过 `codex-transport.ts` 原 8 MiB 上限并立即触发 `Codex JSONL line exceeds limit`；同时原 stdout 读取在每个管道分片上执行 `Buffer.concat` 并从头扫描换行。
2. **分页历史单页 `limit: 100` 将多轮图片拼入同一 RPC 响应行**：`codex.ts` 的 `recoverHistory()` 以 `limit: 100` 请求 `thread/turns/list`（`itemsView: 'full'`），使单页响应携带多达 100 轮完整图片字节。每轮仅附带 1 张合规 PNG（3,241,780 字符 < 5 MB）时，第 3 轮（累计 9.72 MB）即击穿原 8 MiB 上限；即便单行上限提至 32 MiB，10 轮以上（如 12 轮共 38.90 MB）仍会在回合结束 `hydrate()` 与冷重开时超限失败。
3. **冷重开 `thread/resume` 默认内联整段 `turns` 历史**：上游 Codex 0.167.0 `ThreadResumeParams` 默认在 `thread/resume` 响应内返回全部 `thread.turns`。当会话累计图片超过 32 MiB（12 轮共 38,901,360 字符）时，未传 `excludeTurns: true` 的 `thread/resume` 在进入分页读取前即因单行超限导致会话无法再次打开。

## 最小修改

- `packages/runtime/src/agents/codex-transport.ts`：保持严格有界内存保护，将单行上限对齐单轮合规图片包络至 32 MiB，并改为分片数组仅扫描新增分片、在换行处一次性拼接，消除大帧读取时的平方级缓冲拷贝。
- `packages/runtime/src/agents/codex.ts`：
  - `resume()` 显式传入 `excludeTurns: true`，避免分页线程在 `thread/resume` 单行响应中重复内联整段历史；若命中已落盘 `legacy` 线程且 `turns` 为空，则回退 `thread/read({ includeTurns: true })` 保持旧格式兼容。
  - `recoverHistory()` 将 `thread/turns/list` 页大小改为 `limit: 1`，并在同会话内记录已终态且无 `inProgress` 后台工具项的前缀回合与游标，使后续回合 `hydrate()` 仅增量拉取尾部回合；若游标失效或重叠则自动回退从头按 `limit: 1` 分页读取。

## 前后真实对照

驱动与收据位于 `.artifacts/codex-image-history-20261112/check.mjs`、`baseline-receipt.json`、`after-receipt.json` 与 `cold-team-parity-receipt.json`：

| 场景 | 修复前（f8a9460） | 修复后 |
| --- | --- | --- |
| 单轮 6 张合规图片（合计 19,450,680 字符 ≤ 20 MB）及冷重开 | 首轮即报 `Codex JSONL line exceeds limit` | 6 张原图 SHA-256、完成回执与冷重开一致，零额外请求 |
| DesktopRuntime + Hive 上传连发 4 轮单图（累计 12.97 MB）+ 文本续聊 + 冷重开 | 第 3 轮失败，且重开 `chats.open` 同报超限锁死 | 5 轮全部 `pending: 0`，冷重开 4 张原图保持且零额外请求 |
| 12 轮单图累计 38,901,360 字符（> 32 MiB）+ 第 13 轮动态工具调用 + 冷重开 | 第 3 轮即报超限失败（仅调大单行上限仍会在冷重开 `thread/resume` 超限） | 13 轮全部完成，动态工具执行 1 次，冷重开（372 ms）12 张原图与工具记录完整保持 |
| 非用户来源 `dispatch` 投递 3 张合规图片（9.73 MB） | 报 `Codex JSONL line exceeds limit` | 投影为 `hive-delivery`（`origin: dispatch`），3 张原图与回执一致 |
| 受控子进程输出 > 32 MiB（33 MiB）异常单行 | 拒绝并报 `Codex JSONL line exceeds limit` | 同样立即拒绝并报同一有界错误 |

## 回归与验证边界

通过 `pnpm check`、`pnpm build:runtime`、`git diff --check`、d43f55e 真实 Terra 三会话副本冷读一致性检查（`cold-team-parity.mjs`），以及以下已有检查（日志保存在 `.artifacts/codex-image-history-20261112/`）：

```sh
pnpm --filter @hive-desktop/runtime check:codex
pnpm --filter @hive-desktop/runtime check:images
pnpm --filter @hive-desktop/runtime check:queued-inputs-peers
pnpm --filter @hive-desktop/runtime check:usage
pnpm --filter @hive-desktop/runtime check:external-reasoning
pnpm --filter @hive-desktop/runtime check:delivery-failures
```

本批使用本地回环 Responses 对端验证原始 Codex 0.167.0 `app-server` / Hive SQLite 数据流，不计作真实厂商推理或面板走查；正式测试未作增改，尚未纳入安装包。
