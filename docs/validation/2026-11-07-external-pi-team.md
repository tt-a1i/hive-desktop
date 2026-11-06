# 外部 Pi 团队接入验证

2026-11-07，macOS，Pi 1.4.0，Hive 2.2.1。`pi-rpc` 会话与 `hive:pi-rpc` 成员使用现有 Hive HTTP、SQLite、team CLI 和持久投递队列，不另建协作账本。外部进程保留自身认证和扩展，逐次执行前取得当前 run 的绑定凭据；重开时添加当前子进程的唯一工具名称。

## 实际验证

- `checks/external-pi-runtime.ts`：真实 HTTP/SQLite/RPC/CLI，暂停模型执行；验证成员创建、后端与 Terra 模型继承、派单、seen/report、待投递恢复和当前工具指引。证据 `.artifacts/external-pi-runtime-no-model-passed/receipt.json`。
- `checks/external-pi-runtime-lifecycle.ts`：杀死本次拥有的外部 Pi 子进程，验证空闲与待投递场景的 run/token 终止、重新建立 handle、保留待投递 ID，并拒绝跨后端复用旧会话。证据 `.artifacts/external-pi-lifecycle-hr161n/receipt.json`。早期检查因 Pi 改写 process title 而找错 PID 匹配条件，未作为有效结果；失败记录保留。
- `checks/external-pi-runtime-live.ts --live`：两个真实外部 Pi 会话完成主控创建 Worker、派单、Worker 写入精确 18 字节 `HIVE_TEAM_TERRA_OK`、显式 seen/report、主控独立读文件并核验。派单状态为 `reported`，`delivered_at` 为空，不声称已交付确认。整体关闭、重开后，两名成员原生 session ID 与持久 source entry ID 保留。
- `checks/external-pi-runtime-resume-live.ts --live <proof-directory>`：重开后的真实主控使用新工具名与新绑定调用 team list，继续在原会话工作。

真实团队及恢复收据在 `.artifacts/external-pi-team-live-Zzqy1d/` 的 `receipt.json`、`resume-receipt.json` 和事件文件中。`model-audit.json` 逐条核验主控 12 条、Worker 9 条，以及恢复后主控合计 15 条 assistant 消息均为原生 `openai/gpt-6.1-terra`。相同 provider/model 断言已加入后续检查脚本。未改用户全局默认模型；不经过 Magpie。

真实运行发现 `team send` 不支持 `--stdin`，已纠正桌面隐藏指引及工具说明：send 正文作为单个位置参数，只有支持该选项的命令才使用 stdin。

## 边界与复核

独立审查复跑真实 RPC 的完成标记与会话检查；原先跨 delivery/分支误确认的 P1 保持修复。实时和最终展示 ID 在同一 handle 内一致，跨进程使用持久 entry ID；toolResult 使用原生 toolCallId，避免重复工具行。

本收据不代表完整桌面交付。成员管理与原生界面、Windows 外部 CLI 发现、分发包另行验证；扩展交互对话尚无完整桌面响应器。测试按新增 RPC/队列/生命周期边界执行，没有重复全量测试或将构造的错误历史当真实模型错误。
