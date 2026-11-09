# Codex 同回合通知的原生验证

## 为什么验证

实际桌面主控工作流出现“主控自己读结果并汇总，随后完成通知再次要求汇总”的两条回复，见 `2026-11-09-native-workflow-orchestrator.md`。现有队列把通知排在当前用户回合之后，无法交给仍在运行的同一回合。

官方 [app-server 文档](https://learn.chatgpt.com/docs/app-server#steer-an-active-turn) 支持 `turn/steer`，要求匹配 `expectedTurnId`；本轮进一步确认实际 CLI 的通知、持久化和停止行为，以确定安全的投递接入边界。

## 实际执行

源码基线 `afe8d2a`，原始 `codex-cli 0.166.0`，固定 `openai/gpt-6.1-terra`。使用项目生产 JSONL transport，模型直接通过本机 Codex 认证运行；没有 Pi 中转、替代模型、全局设置修改或直接 API 凭据写入。

新增手动探针 `experiments/codex-steering/probe.ts`，不进入默认测试。注册自有动态工具 `hold_for_result`，真实模型调用后让该工具等待；本进程在这个确定边界追加通知。工具返回的31/3是实验固定值，不是真 Hive 工作流/Worker 产物。

### 正常完成

`.artifacts/codex-steering/live/`：

- thread `01a1bfe6-d7b3-7ec0-a36b-04c5eda65b6f`。
- turn `01a1bfe6-e1b7-7672-95bf-2731a59dd0bc`。
- 只产生一个 `turn/started`、一个持久回合。
- 原始输入与追加通知为两个不同 native userMessage item，ID 分别 `01a1bfe6-eee1-71e3-90ca-da038e395743`、`01a1bfe6-fad9-7ad1-93e2-ec00214fa0af`。
- 仅一条 final：`STEER_RESULT_OK sum=31 count=3`，原生状态 completed。
- 错误 turn ID、完成后追加均返回 `CodexRpcError`，code 都是 `-32600`；错误代码本身不足以区分触发原因，不能按这个通用代码直接吞错重投。
- 关闭第一连接后，由 `.artifacts/codex-steering/cold-original.mjs` 建立全新原生连接，恢复同一线程，完整 native items 与原历史逐项一致，无 late 输入。

### 已接受后停止

`.artifacts/codex-steering/interrupted/`：

- thread `01a1bfe7-d7e5-7fb3-b65c-b274bd0524d0`。
- turn `01a1bfe7-e2b3-7fb0-b8d8-83b29d04d823`。
- `turn/steer` 成功返回相同 turnId，随后在释放动态工具前执行 `turn/interrupt`。
- 原生状态 interrupted，无 final。
- 持久历史只有最初的一个 userMessage，追加通知未出现；完全新建连接冷恢复后仍只有这一条。
- 错误 ID、回合结束后请求仍明确拒绝。

这是实际观察到的允许行为，不据此推断每次停止都丢弃追加输入；但它已经否定“RPC成功就可以删除Hive投递”的实现。

四个独立 app-server 进程（正常、正常冷恢复、中断、中断冷恢复）均有 rootExited/closed、exitCode=0 的生命周期诊断；任务收尾另按具体PID确认不再存活。原始 trace/history/receipt 保留，不额外生成应用或ZIP。

## 对实现的约束

1. Hive 必须先持久化通知，追加成功只代表原生接受请求，不能当作投递完成。
2. 同一回合可有多个原生 userMessage，当前按 turnId 只关联一条 delivery 的展示元数据必须改为具体 native item 关联，避免把追加通知渲染成原用户输入。
3. 已关联消息且回合确实 completed 才能结算对应投递；interrupted、断线、未见消息的追加输入继续留在原持久队列，由已有暂停/重试路径处理。
4. 结束与追加竞争时，只有确认原生终态后才能回到正常新回合投递；其他错误继续显式失败，不能凭 `-32600` 或错误文字吞掉。
5. 保留排队用户输入的顺序；这次没有授权把所有消息无条件插队、也不要求其他厂商后端假装有同一种接口。

## 范围

本轮增加可复现原生实验和实现证据，未修改生产投递行为。工作流完成后的重复回复仍未修复，未将单个厂商的探针冒充完整协作验收；现有候选包、Windows、UI、其他后端与完整目标状态保持各自待验收范围。接下来需把原生 item 关联与持久队列结算一起实现并做真实工作流回归。
