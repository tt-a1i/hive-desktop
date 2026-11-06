# 外部 Pi RPC 桥接进度

已实现独立进程 JSONL transport 与显式加载的 Hive 扩展，尚未接入 Chats、原生 UI 或团队主控/Worker 闭环。

验证版本是本机 `/Users/tsk/Library/pnpm/pi` 1.4.0，协议依据为其安装包的 `docs/rpc.md`、`rpc-commands.md` 与 `extensions.md`。`HIVE_PI_COMMAND` 可显式选定可执行文件；不安装或改写外部 Pi 的全局配置。

## 已验证范围

- `checks/external-pi-transport.ts` 通过实际 CLI 验证 JSONL Unicode、并发响应、空会话中的隐藏上下文和自定义消息持久化、同一 session ID 重开。`.artifacts/external-pi/transport.json` 是本机证据。
- `.artifacts/external-pi/live.json` 记录一次原生 `openai/gpt-6.1-terra` 模型回合：隐藏上下文中的随机标记确实到达模型，原始 delivery 与来源元数据保留，等待 `agent_settled` 后继续。本次没有实际调用 team CLI，不能称为团队协作测试。
- 之后的独立审查发现旧 complete 命令可能用 B 的成功回复确认失败或中止的 A，也可能误用废弃分支。已改为 `{deliveryId, assistantEntryId}`，校验当前分支、精确终止 assistant、成功结束和中间不存在其他输入。
- `checks/external-pi-completion.ts` 使用真实 Pi SessionManager 与真实外部 RPC，验证 error/aborted A 后的成功 B、废弃分支、sibling assistant、后续用户输入及不在当前分支的 delivery。审查者独立复跑通过；这些检查构造历史，不调用模型，不能冒充实际模型失败场景。

## 接入约束

RPC 的 prompt `handled` 只表示扩展命令被处理，不代表执行结束。Host 需等待实际消息和 `agent_settled`，核对失败/中止，再从 `get_entries` 的 `leafId` 沿 `parentId` 找到该回合的真实 assistant entry ID，调用 complete。不能使用展示 ID、数组下标或别的回合的最后一条回复。

扩展使用每个子进程独立的命令与 team 工具名，名称可从 transport 取得；后续隐藏指引必须提供真实工具名。team 仍调用已有 Hive CLI，不新增协作账本。用户原有扩展保留；child-only HIVE 环境变量避免修改父进程，但不是对同进程用户扩展的秘密隔离。

首次 custom-only 会话通过公开 header/entries 与 `switchSession` 写盘和重新载入，不伪造用户消息。未实现的扩展对话请求会取消并产生显式事件，接入层必须把这些错误显示出来。关闭进程、取消执行与完成派单仍是不同语义。

当前下一步是实现统一会话 handle、接入现有持久队列和运行生命周期，再验证真实主控/Worker、取消恢复、UI 与目标平台。原型单回合证据不代替这些验收。
