# Pi 扩展诊断与投递失败分离

2026-11-12。基线 71f0f39，源码修复，尚未纳入当前 ff2aa70 安装包。

## 真实问题

内置 Pi 的扩展在 agent_end / agent_settled 钩子抛异常时，原生 SDK 1.4.0 将异常通过 extensionRunner.onError 报为诊断并继续正常结算。桌面适配器却把所有 onError 都写入 active.finalError。若诊断发生在成功 assistant message_end 之后，已执行工具且已完成回复的投递被桌面留在失败队列；用户显式继续会重投原输入，重复执行工具。

本地受控 Responses 流实际让 Pi 执行 bash 追加文件，再返回成功回复。agent_settled 扩展抛错后，旧版 pending 1、无完成回执；显式继续后原工具把文件从 x 写为 xx，两次均有成功回复。这是自有夹具中的真实文件副作用，不是对厂商模型概率或所有工具的推测。

## 修改

仅修改 packages/runtime/src/agents/pi.ts 的一个条件：只有 SDK 明确标识 event=command 的扩展命令失败才通过 onError 将该次输入标为失败。SDK 的命令处理会捕获 handler 异常、emitError(command)，然后仍返回 handled；必须保留这个失败判据。普通事件钩子的错误继续原样发送到现有诊断 UI，不把它变成投递失败。

模型 stopReason=error、SDK promise 的实际 reject、取消、会话持久化错误沿原有失败路径，不改记录或队列协议。没有移除错误展示、吞文件错误、自动重试、新 store/抽象或依赖变化。SDK 行为依据本机固定 1.4.0 的 extensions/runner.js emit 与 agent-session.js _tryExecuteExtensionCommand / _emitAgentSettled 实现。

## 六组实际 SDK / HTTP / SQLite 检查

证据 `.artifacts/pi-extension-diagnostics-20261112/`。check.mjs 使用真实 Pi SDK、资源加载器、原生工作区扩展、Hive HTTP/SQLite、投递 spool 与真实 bash 工具；仅自有 loopback Responses 接口返回受控事件，未调用厂商模型。

1. agent_settled 诊断：旧版实际文件写入两次；修复后仅一次，完成回执存在、pending 0、无 deliveryFailure，在同一已空闲会话显式 resume 不新增请求。诊断事件仍被观察到。
2. agent_end 诊断：旧版 pending 1；修复后成功回复结算 pending 0，诊断仍可见。
3. context 诊断：SDK 继续使用正常上下文并回复，前后均 pending 0，诊断保留。
4. 注册扩展命令真实抛错：保留原输入与 Owned command failure，pending 1；没有发起模型请求。
5. 本地接口真实返回 HTTP 400：保留原模型拒绝原因与输入，pending 1。
6. 真实 bash exit 7：原生 tool_end 标记 isError，随后 SDK 正常生成成功回复，投递结算 pending 0；工具失败没有被误判为整个 Agent 退出。

六个会话完整关闭、重开后，JSON 可序列化历史、队列 pending、失败详情及完成 delivery IDs 与保存前相等；冷读取没有发起额外请求。前后合计本地接口请求分别为 9 / 7，差额是第一组旧版的重复工具回合，不推广为性能结论或通用 exactly-once 保证。扩展错误仍沿已有 PiEvent.error 显示，未修改 UI 协议或原生源码。

首次 before 的功能断言均通过，但冷历史 deepEqual 因内存对象存在 details/usage=undefined、JSONL 重读省略这些键而失败。夹具改为按实际 JSON 协议值比较并重新运行 baseline；原失败日志保留，未改生产历史语义。有效对照为 baseline / after。

## 已执行

- `pnpm check`、`pnpm build:runtime`、`git diff --check`
- 上述本地断言式复现
- `pnpm --filter @hive-desktop/runtime exec tsx checks/pi-user-input.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/delivery-failures.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/pi-context-persistence.ts`

全部通过。现有回归保留内置/原始外部 Pi 的技能/模板/命令/变换/取消语义、真实输入和附件重试，以及真实只读 JSONL 写入拒绝后的失败/恢复隔离。本批不修改外部 Pi 实现，不把其原有回归扩大为新事件矩阵验证。

未新增、修改或删除正式测试，未跑全量测试：改动集中于一个 SDK 错误归属条件，使用真实 SDK/工具/文件副作用/HTTP/SQLite/冷恢复与相关现有回归验证。无面板操作、厂商推理、真实凭据或用户全局配置修改。既有错误队列不自动删除或补写成功回执，保留原输入供用户核对。Pi 登录继续搁置，真实推理仍仅 Terra；当前安装包和 Windows/完整产品验收边界保持。
