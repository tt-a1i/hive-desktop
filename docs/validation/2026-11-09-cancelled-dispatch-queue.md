# 取消派单在停止与冷恢复时清理过期投递

2026-11-09。

## 实证与根因

实际Hive工作流并行创建两位结构化Codex成员；可控本地JSONL对端把两条原生回合保持在执行中，真实子进程、Hive HTTP/SQLite、workflow agent调用及取消路径均正常运行，没有模型推理。通过真实stop端点停止后，run为stopped、两dispatch为cancelled、两子进程均退出，但旧代码每位成员的原派单仍pending1，冷恢复也保留。

原有 `pruneReportedDispatches` 只认reported，遗漏同样终态的cancelled。此问题令已取消任务继续显示为待处理；普通仍保留成员的队列还可能在后续继续时再次执行过期任务。此轮没有把“已退出”或“原生回合中断”自行当作任务完成。

## 修复

复用原有停止/冷加载的队列协调位置，将判据扩为：Hive真实账本中派单属于该workspace/该成员，状态reported或cancelled。只删除origin=dispatch的对应投递；不新增账本、不改变任务状态、不删除聊天历史，也不把取消标成成功完成。仍只在暂停且队列空闲后协调，不主动删运行中的输入。

定向真实Pi SDK + team CLI + Hive HTTP/SQLite回归先失败（停止后5条，期待4条），修复后通过。扩充原有 `checks/reported-dispatch-queue.ts`，覆盖reported/cancelled、取消通知保留、未结派单、后续user/report投递、其他成员队列及两种旧spool冷加载。没有新增mock链或生产测试分支。

## 验证及限制

- `.artifacts/two-worker-stop/` 保存第一次缺description的fixture拒绝、修正后的旧代码故障数据、先红后绿日志与修复后的双成员停止记录；初次检查脚本错误不是产品故障。
- 修复后重新创建并停止两位实际Node协议子进程：两派单cancelled、进程退出、两成员pending0；重建runtime后线程身份和pending0保持。合成厂商协议不冒认真模型。
- 用旧故障state启动源码原生应用：两位历史成员正文仍在，没有过期继续处理提示；关联派单详情实际显示已取消及Stopped by user。旧spool冷加载后均为空，Cmd+Q退出0，PID82701。
- `pnpm --filter @hive-desktop/runtime exec tsx checks/reported-dispatch-queue.ts`、`pnpm --filter @hive-desktop/runtime check:team`、`pnpm check`、`pnpm build:runtime`、Codex定向回归及diff检查通过（日志均在上述目录）。
- 停止过程中仍观察到Hive取消转发遇到已退出成员的409日志；终态、账本取消、进程清理和本次队列修复均独立确认，不吞掉或隐藏该日志，也不把转发成功当成通过前提。

改动仅为共享Node协调判据及其回归；未重跑全量测试、Rust构建或新增Windows原生验收，未改变GPUI二进制。当前候选仍06ee28b，本修复尚未入ZIP。真实厂商模型取消、所有混合后端及完整产品仍待，目标保持开放。
