# 外部 Pi 扩展诊断与重复执行

2026-11-12，基线 f4e6981。当前 7b7940c 安装包尚未纳入本次源码修复。

## 实际问题与根因

检查先前内置 Pi 扩展诊断修复的相邻路径，发现外部 Pi 的 `onEvent` 仍将所有 `extension_error` 记入 `run.error`。原生 Pi 对普通扩展钩子报错后继续执行并可成功回复，桌面却在完成判定前抛出该诊断，保留原投递；用户显式继续会再次消费同一输入及工具。

使用原始 `/Users/tsk/Library/pnpm/pi` 1.4.0、显式加载的自有扩展、隔离模型/配置、真实 Hive HTTP / SQLite 与真实 bash 复现。模型协议由本地 HTTP 服务提供固定工具/文本响应，实际原生 SDK 和 CLI 执行工具；不是 mock PTY、假会话或厂商推理。

`agent_settled` 钩子抛出自有诊断时，原工具先执行 `printf x >> tool-proof.txt`，文件为 x；旧适配器留下 pending 1 和失败详情。显式 resume 后，文件变为 xx，本地协议请求从 2 增至 4。这是实际重复副作用，不以错误文案或代码扫描代替行为证明。

## 最小修改

仅调整 `packages/runtime/src/agents/external-pi.ts` 的分类条件：普通扩展钩子诊断继续发送 error 事件，但不覆盖投递结果。原生 command 失败和 `hive_extension_ui_unsupported` 仍阻止完成；真正的 RPC/bridge 拒绝、模型失败、取消、持久化错误和终态校验仍走原有失败路径。没有增加重试、fallback、新依赖或审批。

## 前后真实对照

`.artifacts/external-pi-diagnostics-20261112/check.mjs` 使用同一驱动和六组隔离项目，保留 baseline-verified / after 收据：

| 场景 | 修复前 | 修复后 |
| --- | --- | --- |
| settled 钩子诊断，成功 bash/回复 | pending 1，显式继续重复写入 xx | pending 0，文件 x，显式继续无新请求或写入 |
| agent_end 钩子诊断，成功回复 | pending 1 | pending 0，有完成回执，诊断仍可见 |
| context 钩子诊断，原生继续成功 | pending 1 | pending 0，有完成回执，诊断仍可见 |
| 扩展命令抛错 | pending 1，无完成 ID、零模型请求 | 同前 |
| 本地模型实际 HTTP 400 | pending 1，无完成 ID | 同前 |
| bash exit 7 后原生继续成功回复 | 工具 isError，投递正常完成 | 同前 |

两批分别 9 / 7 次本地协议请求，差异来自旧版显式继续造成的两次重复请求，不是模型性能测量。各批关闭整个 runtime 后重开，队列、完成 IDs、失败详情和其余完整历史逐字段一致，无新请求。只排除每个原生进程重开时正常追加的 `hive-desktop-external` 隐藏上下文，因为其中注册工具名随进程更新；未过滤用户、助手、工具或派单来源。

## 现有回归与异常记录

通过 `pnpm check`、`pnpm build:runtime`、`git diff --check`，以及以下现有检查：

```sh
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-completion.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/queued-user-inputs-external-pi.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/pi-external-persistence.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-interactions-transport.ts
```

分别覆盖真实分支/终态身份、运行中后续原文与附件/停止/冷恢复、真实只读 JSONL 写入失败及原投递显式重试、受控 JSONL 进程的等待/重复响应/生命周期。最后一项明确使用合成协议对端，不能替代真实 Pi 结论。

`external-pi-interactions-native.ts` 首次在默认本机配置下，前八个对话/取消及 timeout/abort 路径通过，但启动期断言收到 get_state 超时，未算整项通过。随后使用自有隔离 `PI_CODING_AGENT_DIR`、本地端口 9 的模型定义（不访问厂商）重跑原检查，全部 exit 0：select/confirm/input/editor 各确认与取消、超时/停止、启动期与目录读取的有界失败诊断。未修改检查源码或生产语义来让断言通过。

另以同一隔离启动扩展对照旧安装包 transport 和新源码，两者都正确返回“启动期对话无法回答”诊断。首次默认配置超时的根因未确定，原日志与 `startup-compare.json` 均保留；不能据隔离通过宣称默认环境超时已被解决，也不能归因于本批错误分类修改。

首次驱动还修正了设置 API 必须传全三个 executable 字段、冷历史比较需单列上述正常新增进程上下文的验证预期；失败日志保留。正式测试未新增、修改或删除，未跑全量 suite：本批只改扩展诊断分类，以六组真实 CLI/工具/HTTP/SQLite 前后对照及相关已有回归验证。无面板操作、厂商调用或真实凭据变化；Terra 仅为本地协议模型定义，未计作真实 Terra 推理。Pi 登录继续暂缓，完整目标、默认环境异常、Windows 原生与新包验收继续开放。
