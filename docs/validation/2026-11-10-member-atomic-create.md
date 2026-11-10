# 成员与后端配置一起提交

2026-11-10；源码基线08337f1，当前开发包ecad294尚未纳入本修复。

## 真实缺陷

桌面原先先POST /workers，再POST /agents/:id/config。真实自有SQLite在后一步写入launch config失败时，前一步成员已持久化；界面虽报错，列表留下无后端成员，原名称再次提交得到409。`.artifacts/member-atomic-08337f1/before.mjs` 用真实Hive HTTP和SQLite BEFORE INSERT触发器复现，`before-receipt.json`保留残留对象和冲突。无模型调用；不是推测性并发问题。

## 复用已有修复路径

Hive 2.2.1已提供addWorkerWithLaunch：成员和launch配置处在既有数据库事务中；失败时同时清理内存。桌面现在用一个本地JSONL请求team.member.create调用该方法，删除前端两步创建。只补窄store类型边界，不复制协作账本或事务实现、不改依赖包/数据库schema、不创建临时命令预设。

runtime验证名字、worker角色、四种已支持后端及可选职责的类型，并遵守既有workspace-removal保护；名字规范化、唯一性等仍由Hive处理。默认职责继续调用Hive自己的语言解析与角色文案，显式职责原文保留。创建保持stopped，不启动Agent或模型。已有成功后清空提交字段/失败保留输入继续生效。

## 验证

新增 `packages/runtime/checks/member-creation.ts`；入口 `pnpm --filter @hive-desktop/runtime check:members`。通过实际production dist/main.js JSONL、真实Hive HTTP与独立SQLite连接验证：

- 非法后端/角色/职责类型/空名拒绝且零成员；四种后端分别创建为stopped，命令hive:pi/codex/pi-rpc/claude正确，默认英文职责等于同workspace真实HTTP原生创建结果。
- SQLite拒绝后端配置写入时，HTTP列表与失败前完全相等、数据库无目标成员；移除触发器后原名称/两行职责成功重试，只多一位完整成员。重复名称与不存在workspace均拒绝且原列表不变。
- 五位目标成员对应精确五条launch配置，args均空。原生workspace另有orchestrator默认配置，测试明确排除其已知身份，不把它算成新增成员配置。零messages/dispatches/dispatch_messages/agent_runs/workflow_runs。
- 完整runtime退出并冷启动，成员对象/身份完全一致、进程正常退出。最终Node22.23.3通过；receipt路径记录在 `.artifacts/member-atomic-08337f1/check.log`。没有mock fetch/SQLite/PTY或模型推理。

最初检查误将JSONL结构化error当字符串，以及把原生orchestrator默认配置计入成员数；分别核对实际协议/schema后校准断言，没有为断言修改产品路径。首轮类型声明缺AppStateRecord.key，已对齐真实Hive返回结构。保留失败尝试，不声称首轮全绿。

Mac原生开发应用另用独立native-state、English/Dark20：填写Atomic native member与两行职责、键盘选择Codex并提交，真实数据库触发错误。窗口显示错误且输入保留；独立SQLite此刻确认原15成员、目标0，移除自有触发器后直接Return以原内容重试成功，第16位stopped成员出现，正常CmdQ/exit0。独立真实HTTP/SQLite冷读：原15完整对象不变、仅多1位Codex coder、职责精确、空args、零执行。证据native-failure.ax.txt、native-failure-db.json、native-retry.ax.txt与cold-receipt.json。

通过runtime build/typecheck、既有check:hive真实HTTP/SQLite、Mac cargo build、Windows x86_64-pc-windows-msvc cargo check和git diff --check。风险为成员创建与存储事务调用，运行对应真回归；不跑无关全量suite。原生只复演Codex失败重试，其余后端为无推理JSONL配置验证；Windows原生、新包、全厂商真实模型及完整产品目标继续开放。
