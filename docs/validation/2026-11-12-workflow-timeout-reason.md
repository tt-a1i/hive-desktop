# 工作流时限停止原因

2026-11-12。源码修复，尚未纳入当前 a744821 安装包。

## 真实问题

工作流达到 meta.maxDurationMs 后，runner 会停止执行并记录明确的超时日志，但沿用手动 stop 的固定原因 `Stopped by user`。独立纯计算、嵌套父子流程和等待 Worker 的流程都能复现；重启后仍保留错误原因。用户和主控因此无法从运行详情/取消记录/完成通知区分超时与人为停止。

既有 Hive 2.2.1 补丁只调整 workflow-runner.js：私有停止函数接受原因，时限定时器传入 `Workflow exceeded maxDurationMs (Nms)`，递归子运行与派单取消沿用该原因，终态收尾保留已记录原因。公开 stopRun 签名、手动停止原因、stopped 状态、时限数值与清理机制保持，没有新 store/路由/抽象。锁文件只更新 patch hash，无依赖版本升级。

## 实际验证

证据目录 `.artifacts/workflow-timeout-reason-20261112/`。check.mjs 是本地断言式复现，不是新增正式测试。使用真实桌面 runtime、HTTP/SQLite、脚本 Worker 和内置 Pi 会话；主控队列暂停，模型配置仅指向自有 loopback 端点，实际请求数 0。Codex Worker 使用明确的本地原生协议对端，仅证明接入与清理，不冒充厂商模型推理。

- baseline 首先在源码修复前确认纯计算 250ms 超时、嵌套父流程 500ms 超时都被写为用户停止。初次 before 实验在独立失败样例的错误展示前缀断言上失败（实际为 `Error: Owned workflow failure`）；修正夹具预期后重新运行，未修改产品错误语义，原失败日志保留。
- 完整 worker-baseline 显式加载当前 a744821 包的旧 runtime；after 加载修复后源码 runtime。两边执行同一七条运行矩阵：CPU 超时、手动停止、父流程超时及其子流程、正常返回 7、真实抛错、等待 Worker 超时。
- 新版三种超时分别保存 250/500/2000ms 原因，父子原因一致；旧版均为用户停止。手动停止仍为 `Stopped by user`；完成结果 7 和真实失败原文保持。
- Worker 对端实际接收任务后触发 2000ms 时限。真实 workflow dispatch 为 cancelled，report_text 与运行的超时原因精确相等。进程退出被核对，完整 runtime 重启后该成员 pending 为 0。
- 两版七条运行的完整 HTTP 记录在各自关闭/重开后逐对象相等。分别检查已持久化的四条相关主控完成通知，每个 run_id 恰好一条且 error 行与运行原因完全一致；不据此声称所有通知场景都有恰好一次保证或模型已消费通知。
- 原、新两版停止日志都出现既有 `workflow.cancelDispatch.forward PtyInactiveError`，发生在取消通知尝试转交已退出的成员时。日志未吞掉或伪装通过；本批断言验证了取消账本、进程清理和冷 pending，没有扩大范围改动此提示。

已运行并通过：

- `pnpm check`、`pnpm build:runtime`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/workflow-shutdown.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/workflow-removal-lifecycle.ts`
- 本地 before/baseline/after 复现与持久化通知断言（首次夹具失败如上说明）
- `pnpm install --frozen-lockfile --offline`、`git diff --check`

两个现有回归覆盖真实双 Worker、嵌套与纯计算关闭、终态/队列恢复，以及实际脚本 Worker 的待完成 host call 所有权释放。本批未新增、修改或删除正式测试，没有运行全量测试：改动集中在已有停止路径的原因传递，采用实际工作流/派单/进程/冷恢复和相关现有回归验证。

当前安装包仍为 a744821，下一包需按新 patch hash 重新生产部署，不能沿用旧冻结树。旧错误历史不重写。没有面板操作或新模型推理；Pi 登录继续搁置，真实模型仅 Terra；Windows 真机与完整产品验收继续开放。

后续已纳入 ff2aa70 开发包，具体包内验证见[工作流开发包](2026-11-12-workflow-reliability-package.md)。上文待入包描述为源码阶段状态。
