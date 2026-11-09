# 冷恢复后直接发送，不额外启动恢复回合

## 问题与修改

已有真实历史、没有其他待处理投递的会话，应用重开后直接发送新消息。原链路先调用Hive start恢复会话，恢复通知立即入队并唤醒模型，随后才提交用户输入，形成两个回合；此前“空会话不唤醒”的修复只覆盖没有实际对话的情况。

本轮在 `Chats.activateOnce` 标记这次启动是否由显式用户输入触发。该启动期间收到的恢复通知沿用现有上下文保存路径，实际用户投递负责唤醒；启动HTTP成功或失败都在finally清除标记。没有更改Hive协议、协作账本、真实汇报/outbox或Agent原生回合语义。

单独点击继续/恢复仍使用原恢复唤醒；已有排队任务及真实异步消息不被删除或当作完成。该修复不承诺手动恢复已开始、或另有真实消息排队时都只有一个回合。

## 真集成检查

新增 `packages/runtime/checks/user-input-recovery.ts`，进入 `check:team`：真实Hive HTTP/SQLite、Pi SDK持久JSONL；仅在检查内暂停模型队列，不替换业务服务或fetch。

- 冷加载有原生历史的会话后直接prompt：原生产代码pending=2，期望1，断言失败；修复后只有一条user投递，恢复文本确实在索引和SDK上下文。
- 手动resume仍保留正常recovery投递。
- 在自有SQLite上加BEFORE INSERT agent_runs故障触发器，实际启动失败后没有接受用户消息；移除触发器，再手动resume仍正常唤醒，验证标记没有泄漏。触发器仅在临时验证数据库，finally删除。
- 完整关闭重开后，各分支的原投递类型和数量保持。

Pi的历史由原生SessionManager写入，检查不称作Pi真实模型推理。

实际通过：

```sh
pnpm check
pnpm build:runtime
pnpm --filter @hive-desktop/runtime check:team
pnpm --filter @hive-desktop/runtime check:codex
pnpm --filter @hive-desktop/runtime exec tsx checks/queued-user-inputs-peers.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-runtime-lifecycle.ts
node --check scripts/check-packaged-dispatches.mjs
```

后两组分别为生产Codex适配器/官方Claude SDK与合成CLI的真实Hive/SQLite排队检查，以及实际外部Pi RPC进程退出/恢复检查，均无厂商推理。新检查已加入后续包gate，但本轮没有对旧候选执行它或生成新ZIP。

## 原生窗口与真实Codex/Terra

`.artifacts/user-input-recovery/live/` 复制自有工作流验收数据，并通过原始CLI fork单独线程，原始候选包的工作流会话不作为写入目标。

- 原thread `01a1c009-24d0-7531-907b-4d25df5de36b`，验证fork `01a1c015-6200-7082-8be6-9b4ab605a77e`，继承上一轮一个完整真实工作流回合。
- 正式GPUI开发窗口读取后，实际点击输入框、粘贴并点击发送：`Reply exactly COLD_INPUT_OK. Do not use tools or change any files, models, or global settings.`
- 原始Codex CLI 0.166.0 / openai/gpt-6.1-terra实际回复一次 `COLD_INPUT_OK`。本轮新请求没有使用工具或产生第二条恢复答复。
- 持久来源为原user/system加一个新user，共三条完成delivery；native turn总数从继承的1变为2，没有新增recovery投递。恢复事实仍保存于application context，未变成另一个模型回合。
- Cmd+Q正常exit0，PID55314随后不存在。实际send trace正向记录initialize/chat.open，只有一次chat.prompt、没有chat.resume；这是用户输入经原生窗口进入的验证，不是API驱动冒称GUI。
- 应用退出后以全新runtime冷读：pending0、三条完成ID、来源和旧47/3与新COLD_INPUT_OK两个final保持，恢复上下文仍在。没有再次发送模型请求。

[实际原生回复](assets/user-input-recovery/native-reply.png)

基线失败日志、集成/回归日志、fork/准备/原生启动与退出、原生截图及冷恢复receipt保存在 `.artifacts/user-input-recovery/`。

## 范围

本轮只改共享后台输入启动顺序，没有Rust/UI源修改、权限审批或新依赖；复用已验证的原生程序，未重跑Rust/全量suite/Windows。真实模型只跑原始Codex/Terra，Claude模型和Pi重新登录的缺口仍保留。

当前候选仍为768d085；该源码修复尚未进入ZIP。完整多平台产品与UI矩阵继续开放。
