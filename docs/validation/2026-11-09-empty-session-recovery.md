# 空会话恢复不额外启动模型

## 场景与改动

真实桌面主控工作流验证中，只有启动说明、尚未收到任务的会话在重新启动时先回答了一次“没有任务”。来源见 `2026-11-09-native-workflow-orchestrator.md`；它与工作流结束通知导致的第二次结果回复是两条不同路径。

`Chats.start().deliver()` 原来只会把“已有排队任务”的恢复通知并入上下文；空队列始终再排一个模型回合。现在当后端历史为空，或仅含已知 Hive 启动/接入上下文时，也保存恢复上下文而不额外唤醒。历史包含用户、助手、工具、hive-delivery 或未知扩展消息时，继续沿用原恢复唤醒；排队任务、派单账本及原生回合完成语义保持原有路径。

恢复上下文仍写入聊天索引并交给后端 `addContext`；下一次实际任务会读取它。这里只改变原生回合启动条件，没有按通知正文猜测任务完成，也没有删除系统通知。

## 验证

新增 `packages/runtime/checks/empty-session-recovery.ts`，默认进入 `check:team`：

- 真 Hive HTTP/SQLite、Pi SDK/JSONL，不替换协作服务或存储。
- 分别建立独立工作区：纯启动上下文、排队用户任务、原生用户历史、系统来源 hive-delivery、未知扩展历史。
- 正常停止、关闭后台、重开并通过真实启动/恢复链路验证：空会话 pending=0，恢复文本确实持久化；用户任务仍是原一条；后三种历史各保留正常 recovery 投递。
- 再冷开后台读取 spool，确认结果保持。
- 仅在检查内阻止队列执行，不发模型请求；不把手工写入的原生历史称作真实模型协作。
- 原生产代码运行同检查在空会话断言失败（pending=1，预期0），修改后通过。初版检查把有排队任务的聊天与其他用例放在同一工作区，被真实切换保护挡住，已改为独立工作区；该次是检查装配失败，不算产品复现。

另以 `--codex` 运行相同空会话/排队任务/冷恢复路径，连接本机原始 `codex-cli 0.166.0`，会话显式选择 `openai/gpt-6.1-terra`，原生控制链路通过。未请求模型，也未把 Pi 手工历史用例冒充 Codex 历史覆盖。

实际命令：

```sh
pnpm check
pnpm build:runtime
pnpm --filter @hive-desktop/runtime check:team
HIVE_CODEX_COMMAND=/Users/tsk/.npm-global/bin/codex pnpm --filter @hive-desktop/runtime exec tsx checks/empty-session-recovery.ts --codex
pnpm --filter @hive-desktop/runtime exec tsx checks/pi-context-persistence.ts
```

均通过。最后一项另验证真实只读 JSONL 写入失败的暂停、释放、重新打开、上下文恢复；不依赖吞错成功。主要命令日志保留在 `.artifacts/empty-session-recovery/`。运行 Node 22.23.3，显式使用本机短路径 Node 启动 pnpm/tsx。

## 工作流重复回复仍需继续

官方 [Codex app-server：Steer an active turn](https://learn.chatgpt.com/docs/app-server#steer-an-active-turn) 支持 `turn/steer`，必须带匹配的 `expectedTurnId`，无活跃回合会失败，也不会新发 `turn/started`。

当前适配器的持久来源元数据按 turnId 关联一个 delivery，队列按单个 prompt 完成后出队；若把工作流通知直接 steer 到当前回合，必须同步处理多个投递的原生消息关联、停止/冷恢复和接受/完成确认，不能仅把 RPC 改掉。本轮没有启用 steer，没有静默丢弃结束通知，工作流完成后的重复结果回复仍未修复。

## 边界

这是源码后台修复；没有修改 Rust UI、打包 ZIP、用户全局认证或模型默认。未重新执行模型、Mac 原生窗口或 Windows 原生交互；未跑全量套件，范围限于恢复投递和上下文持久化的定向检查。候选包仍为0e72137，尚未包含本轮修复；完整产品目标继续开放。
