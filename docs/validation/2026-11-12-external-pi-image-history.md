# 外部 Pi 合规图片历史与增量读取

2026-11-12，基线 a26ab5a。当前 7b7940c 安装包尚未纳入本次源码修复。

## 实证问题与根因

`prompt-images.ts` 允许单张图片 base64 最多 5,000,000 字符、单次合计最多 20,000,000 字符（最多 10 张）。使用真实 `SessionManager`、原始 `/Users/tsk/Library/pnpm/pi` 1.4.0、真实 Hive HTTP / SQLite 上传链路与本地协议对端（无厂商推理）核验发现两处真实冲突：

1. **累计历史整表回传击穿单行上限并锁死会话恢复**：`external-pi.ts` 的 `refresh()` 在初始化、每次 `message_end`、补充上下文及完成结算时均无参调用 `get_entries`，令上游 `rpc-mode` 将整段会话历史拼入单行 stdout JSONL。每轮仅附带 1 张合规 PNG（3,241,780 字符 < 5 MB）时，第 3 轮累计达 9.72 MB 即触发 `External Pi JSONL line exceeds limit`；且由于图片已落盘至原生 JSONL，后续 `chats.open` 与重启冷恢复均在同一 `get_entries` 处抛错，导致该会话无法再次打开。
2. **单轮多图事件帧上限低于输入契约，且大行缓冲重复拷贝**：上游 `rpc-mode` 对单轮用户输入输出的 `message_start`、`message_end` 与 `agent_end` 事件均包含当轮全部图片（但 `agent_end` 仅含本轮新增消息，不跨轮累积）。当单次请求携带合规多图（如 6 张共 19,450,680 字符 < 20 MB）时，单条原生事件帧约 19.48 MB，超过 `external-pi-transport.ts` 原 8 MiB 单行限制；同时原 `Buffer.concat` 在每个管道分片上重复拼接并从头扫描换行。

## 最小修改

- `packages/runtime/src/agents/external-pi.ts`：冷启动按 1 MiB 分块只读解析已落盘 `state.sessionFile` 作为初始条目种子（不写回或改动原生会话文件），后续 `refresh()` 通过原生 `get_entries({ since })` 仅拉取新增条目及最新 `leafId`；当原生扩展或命令切换新会话返回 `Entry not found:` 时重新读取 `get_state` 与新会话文件。空会话保持无参 `get_entries` 兼容。
- `packages/runtime/src/agents/external-pi-transport.ts`：保持严格有界内存保护，将单行上限对齐单轮合规图片包络至 32 MiB，并改为分片数组按需在换行处一次性拼接，消除大帧读取时的平方级缓冲拷贝，未改变图片契约或引入新抽象。

## 前后真实对照

驱动与收据位于 `.artifacts/external-pi-image-history-20261112/check.mjs`、`baseline-receipt.json` 和 `after-receipt.json`：

| 场景 | 修复前（a26ab5a） | 修复后 |
| --- | --- | --- |
| SessionManager 12 轮合规图片冷读（累计 38,901,360 字符 > 32 MiB）及后续文本回合 | 启动即报 `External Pi JSONL line exceeds limit` | 12 张原图 SHA-256 与完成回执全部一致，第 13 轮正常完成 |
| DesktopRuntime + Hive 上传连发 4 轮单图（累计 12.97 MB）+ bash 工具 + 冷重开 | 第 3 轮失败，且重开 `chats.open` 同报超限无法打开 | 5 轮全部 `pending: 0`，bash 产物一致，冷重开 4 张原图保持且零新请求 |
| 单轮 6 张合规图片（合计 19,450,680 字符 ≤ 20 MB）及冷重开 | 单轮事件帧即报超限失败 | 请求含全部 6 张原图，完成结算与冷重开一致 |
| 非用户来源 `dispatch` 投递 3 张合规图片（9.73 MB） | 报超限失败 | 完成回执与 3 张原图 SHA-256 一致 |
| 扩展命令调用 `ctx.newSession()` 切换会话后继续对话 | 通过 | `Entry not found` 分支重载新会话并通过 |
| 受控子进程输出 > 32 MiB（33 MiB）异常单行 | 拒绝并报 `External Pi JSONL line exceeds limit` | 同样立即拒绝并报同一有界错误 |

第 1 项累计体积（38.90 MB）高于新单行上限（32 MiB），证明跨轮历史通过只读文件种子与增量 `since` 解决，而非依赖放大单行阈值。

## 回归与验证边界

通过 `pnpm check`、`pnpm build:runtime`、`git diff --check`，以及以下已有检查（日志保存在同批 `.artifacts/external-pi-image-history-20261112/`）：

```sh
pnpm --filter @hive-desktop/runtime check:external-pi
pnpm --filter @hive-desktop/runtime check:images-native-pi
pnpm --filter @hive-desktop/runtime exec tsx checks/pi-external-persistence.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/queued-user-inputs-external-pi.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-interactions-transport.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/pi-user-input.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/pi-history.ts
```

本批使用本地协议回环对端验证原始 Pi CLI / SDK / Hive SQLite 数据流，不计作真实厂商推理或面板走查；正式测试未作增改，尚未纳入 7b7940c 安装包。
