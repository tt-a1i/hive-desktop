# Claude 写入丢失与退出结算

任务 #12，关联排队输入 #6。修复后台实际完成判断，不改变 Hive 协作协议、厂商审批或全局模型设置。

## 复现与根因

官方 Claude Agent SDK 0.3.317 的 SDKSessionWritesLostMessage 明确区分 lost 和 transcript_session_ids：该通知在最后 result 后、CLI 退出时发送。全局配置或其他会话记录丢失不表示当前会话记录丢失。

原代码统一把通知写入 active.error；同时在原生进程退出后立即通知 Chats 清理。清理会 abort 活动 prompt，可能在成功终端结果已经收到、持久完成标记还未写入时将其取消。真实 Hive/SQLite/队列与官方 SDK 接合法合成 CLI 复现：当前输入/助手已落盘、成功 result 已收到，只有 global_config 丢失，但 completed 为空，当前输入和补充都留在失败队列，明确重试会重复执行第一条。

源码基线被新检查的完成 ID 断言拒绝。旧包通过包内 Node/SDK 跑相同检查，另出现对已退出进程 interrupt 的清理错误，未把它冒认为同一条断言失败。两份原始失败保留。

## 修复

- 只在 lost 包含 transcript 且 transcript_session_ids 明确包含当前会话时，将该回合记为记录持久化失败。其他范围保留已收到且实际可读取的成功输入/助手与原有完成检查，错误说明包含原生范围及 error_code。
- 收到该“退出中”通知即停用当前原生句柄；不等稍后的进程 exit 事件，避免第一条结算后向退出中的进程发第二条。
- 故障立即拒绝尚未收到终端结果的等待，但 owner 的退出清理回调等活动回合结算、清空 active 后再通知，且只通知一次。继续复用既有 pause/cleanup 和冷恢复；不吞掉真实失败，不根据普通聊天结束代替派单汇报。

## 检查

`pnpm --filter @hive-desktop/runtime check:claude-write-loss`：真实 Hive HTTP/SQLite、spool、原生格式会话文件，官方 SDK 连接明确标注的合成 CLI。固定合成元数据 gpt-6.1-terra，无网络/厂商推理，不代表 Claude 厂商支持该模型。

四组范围分别为 global_config、其他会话 transcript、本会话 transcript、成功结果前进程退出。前两组第一条完成且补充保留，后两组第一条未完成且两条保留；冷重启仍暂停且投递 ID 不变。修复外部条件后明确继续，已确认的第一条不重放，失败第一条重新尝试，补充只执行一次，完成 ID 唯一。通过真实 query 内容而非 mock 调用计数核对，并保留原生恢复消息。

校准与修订：初次检查把恢复系统文本中引用的原文计为重复 query，已区分实际用户输入前缀，不删恢复请求或改变生产语义。随后更严格的补充失败断言抓到“通知已到但 exit 未到仍启动下一条”，已在生产通知入口立即停用句柄后重新通过四组。

相关回归：Codex/Claude 两后端队列停止/冷恢复、官方 SDK 与实际 Hive 的许可/提问、team CLI 回调取消通过；原始 Claude CLI 在独立配置目录的真实进程死亡检查通过，无用户输入/模型推理。pnpm check、pnpm build:runtime、git diff --check 通过。不跑无关全量 suite，不改 UI/GPUI 或既有 Hive 账本。

另用包内 Node 22 和既有生产依赖绑定新 dist 跑四组通过；该小候选运行时使用依赖链接，不是正式开发包验收。打包单列，原开发 ZIP 在更新前仍包含旧实现。

证据 `.artifacts/claude-write-loss/`：基线、第一次计数校准、通知/exit 顺序失败、最终四组、包内 Node 候选、旧包失败与相关回归日志。没有真实制造原始 Claude 的磁盘满或配额满，也没有验证 Claude 厂商模型协作、Windows 原生和完整产品目标。
