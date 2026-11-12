# 工作流完成通知的 Unicode 截断

2026-11-12。源码修复，当前 a744821 包尚未包含本次改动与 f81ceb4 超时原因修复。

实际工作流返回长字符串/JSON，或 Worker 提交包含 emoji 的汇报时，完成通知用 UTF-16 length/slice 截取 4000/197 单元，会留下孤立代理项，或把未达 200 码点的汇报错误截短。当前 JSONL sender 已能保护原生读取，但不会修复进入主控投递队列的模型上下文；因此需要修生产通知生成，而不是依赖传输层替换。

既有 Hive 2.2.1 补丁只修改 runtime-store-workflows.js 两处摘要计数/截断为 Unicode 码点。保留 4000 码点结果上限、200 码点汇报阈值及 197+省略号、原转义、不可信证据提示、原始运行结果/派单正文和通知投递方式。剩余字符数也使用同一计数。锁文件只改变 patch hash，无依赖版本升级，未新增或修改正式测试。

## 实证

`.artifacts/workflow-notice-unicode-20261112/check.mjs` 使用真实 runtime、Hive HTTP/SQLite、脚本 Worker、Pi 会话和真实 team CLI 的 messages/seen，再通过真实 HTTP report。Worker 端是明确的受控 Codex 原生协议进程；报告由验证驱动提交，不冒称模型输出。主控队列暂停，模型配置指向自有 loopback 端点，厂商推理和本地模型请求均为 0。

五个案例：3999 个 ASCII 后接 emoji 的长结果、3000 个 emoji 的未超限结果、JSON 结果边界、196 个 ASCII 后接 emoji 的超长汇报、150 个 emoji 的未超限汇报。真实完成通知直接从生产队列读取并保留在 evidence JSON；不是拿已被 JSONL sender 替换过的文本当作生产修复证据。

旧版五例中四条实际通知包含孤立代理项；3000 emoji 结果另被错误截短。修复后五例所有通知都 well formed；完整结果摘要/剩余码点数及精确派单摘要逐字断言，短 emoji 案例不截短，转义保持。所有运行 completed；HTTP 完整结果与 report_text 原文精确相等。

关闭并完整重开 runtime 后，每条完整运行记录、已持久化通知原文保持；每个案例对应一条完成通知，受控 Worker 子进程均退出。此样例不证明所有通知路径恰好一次，也未执行模型消费通知。

通过 `pnpm check`、`pnpm build:runtime`、`pnpm install --frozen-lockfile --offline`、`git diff --check`，以及以下现有定向检查：

- `pnpm --filter @hive-desktop/runtime exec tsx checks/team-runtime.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/reported-dispatch-queue.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/structured-runtime.ts`

不跑全量测试：本次范围是两个生产通知格式化分支，用真实工作流、CLI/HTTP 报告、持久化队列、冷读与相关现有回归验证。没有面板操作、新模型请求、协议或存储语义改动。旧通知原文不重写；下一包必须按新补丁重新生产部署。Pi 登录继续搁置，真实推理仅 Terra；账号/系统通知/Windows 真机与完整产品目标继续开放。
