# 工作流取消：移除临时成员回收时的多余转发

2026-11-12。源码修复；当前安装包仍为 ff2aa70，尚未包含本批改动。

## 根因与修改

工作流取消先把派单写为 cancelled 并减少 pending，再由 awaiter 拒绝 agent() 的等待。执行器随即进入 finally，删除自己创建的 ephemeral Worker 并停止进程。与此同时，装配层还 fire-and-forget 转发一个取消提示；结构化执行器要异步等待输入队列，到达时临时成员已经退出，产生 workflow.cancelDispatch.forward PtyInactiveError。

这是两个已存在的生命周期步骤互相重叠，不是账本取消失败。ff2aa70 包内实证已经确认进程退出、pending 0 和正确原因；本轮追踪了所有 workflow dispatch 创建者、runner/DAG 取消调用与 ephemeral Worker finally 回收。此路径没有常驻成员复用。

既有 Hive 补丁删除这次工作流专用转发及其两层 catch，保留两行所有权说明。生产模块净减少 10 行；没有增加错误过滤、固定等待、重试、store 或抽象。取消账本、awaiter 拒绝、临时成员回收、超时原因、关闭排空保持。普通 team cancel 是另一条原有路径，继续给常驻成员发送取消提示。

## 实证

证据 `.artifacts/workflow-cancel-forward-20261112/`：

- 修改前 source 与 ff2aa70 包内 runtime-store-workflows.js 的 SHA256 一致（c113a449e93b696ba75160d58be4a7c65cf30c04e09bcfcd8c9bc070fe6ffd7b）。包内 timeout.log 留有实际转发到已退出结构化成员的错误。
- check.mjs 使用真实 Hive HTTP/SQLite、脚本 Worker 和真实 node-pty。自有 Node 脚本在实际 PTY 收到任务正文后，分别经 HTTP 手动停止和 3000ms 时限停止；核对 cancelled、精确取消原因、进程 ESRCH、成员移除及完整冷恢复后的运行/派单相等。baseline-final 和 after 均通过，不靠移除提示来改变停止机制。
- 同一检查另启动一个常驻内置 Pi 成员，暂停其队列，通过真实 HTTP team send / cancel。正常取消提示以原 dispatch ID、reason 进入生产队列，关闭/冷读后原 item 逐对象保持；常驻成员仍在团队中，临时成员不在。主控及该成员使用自有 loopback 模型配置，实际模型请求数 0。
- structured-check.mjs 用真实 runtime/HTTP/SQLite/Pi、脚本 Worker 和受控 Codex 原生协议进程跑七条运行：CPU 超时、手动停止、嵌套父/子超时、正常结果、真实失败、等待成员超时。原因/取消记录保持，实际接收任务的 Worker 进程退出，冷 pending 0，七条完整记录重开保持。对端不冒充厂商模型。
- after 的结构化流程与现有 workflow-shutdown 检查中，该条错误日志为 0；这项日志比较之外，以上真实状态、输入、进程和冷恢复断言才是停止仍正确的依据。

夹具初次 before 使用了错误的 HTTP 字段 worker_name，服务正确返回 Missing to；修为实际契约 to。下一次误要求普通 PTY 具有结构化聊天记录，改为核对实际运行/派单与团队冷状态。这两次失败日志保留，没有为夹具改生产协议或增加 fallback；有效前置基线为 baseline-final，修复验证为 after。

通过命令：

- `pnpm check`、`pnpm build:runtime`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/workflow-shutdown.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/team-runtime.ts`
- 本地真实 PTY 与结构化复现（见上）、`pnpm install --frozen-lockfile --offline`、`git diff --check`

未新增、修改或删除正式测试；未跑全量测试：变更仅删除工作流临时成员销毁时的重复转发，已跑相应真实执行/取消/关闭与团队回归。锁文件只更新 patch hash，没有依赖版本升级。下一包需重新部署生产依赖，当前 ff2aa70 安装包保持。没有面板操作、新模型推理或用户全局配置变动；Pi 登录继续搁置，真实推理只用 Terra，Windows 真机与完整产品目标仍开放。
