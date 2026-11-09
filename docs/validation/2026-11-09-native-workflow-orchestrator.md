# 桌面主控亲自编写并运行工作流

2026-11-09，源码运行时基线1831e1d，原始Codex CLI 0.166.0，真实模型仅openai/gpt-6.1-terra。Mac原生开发窗口连接源码runtime，本轮不是便携候选包实证。

## 实际链路

通过桌面聊天框粘贴并发送完整请求；驱动仅准备独立workspace、输入与固定Codex/Terra角色，没有调用workflow run或替Worker汇报。主控读取workflow guide，自己保存workflow-source.ts，经原生team工具运行完全相同的源码。工作流await一位OwnedNativeWorkflowCoder的agent()汇报。

- run：f9277a87-a6e5-45a4-ad0d-1ea1de9e442e，completed。
- dispatch：0d63c944-72c2-469f-a90b-9e99b312a514，reported。
- Worker通过真实messages取得required_seen_seq=0，以--seen 0及--artifact显式report；原生工具completed/success=true。
- Worker自己读取samples=[2,17,23,41]，写入sum=83、count=4；input原始字节保持。
- 主控独立读取input与result核验，答复WORKFLOW_MAIN_OK sum=83 count=4。主控没有调用report/spawn，没有新增计划。
- 独立生产runtime冷读回相同run/dispatch、两会话原生工具历史、模型及产物；主控busy=false、pending=0。
- 初次原生窗口PID99728正常Cmd+Q，exit0；展示修复后第二次窗口PID3918同样exit0。

可复核证据位于`.artifacts/workflow-orchestrator/`：request.txt、seed.mjs、location.json、project/、state/、audit.mjs、receipt.json、recovery-snapshots.json，以及两次原生进程/退出/日志。初次audit错误地要求原生输入与用户原文完全相等；Hive会附加内部reminder，已改为完整用户正文前缀核对并通过，校准记录单独保留，未为断言修改生产逻辑。

## 实际发现与展示修复

工作流完成通知经Hive原有writeSystemMessageToAgent投递。在主控已主动轮询并答复的情况下，队列随后再执行结束通知，因而本次有两份相同结果的final答复；种子恢复也曾触发一次空任务答复。这些现象保留，不宣称无重复回合。

原生聊天还把通知整个内部envelope直接展开。现改为：可信presentation明确标记system、且没有独立显示摘要（displayText等于原始正文）时，默认仅显示团队通知和查看原文按钮。完整正文继续保留在原有展开、搜索与复制路径；不解析文本标签判定来源，不改变模型输入/业务投递。派单、汇报、用户输入及独立摘要继续走原显示逻辑。

实际新构建窗口验证：折叠时无envelope正文；点击查看原文显示同一run ID、结果、日志及完整reminder；再次收起恢复紧凑通知，切换成员可恢复messages与report工具状态。Mac构建、Windows MSVC类型检查及git diff --check通过。纯展示修复不新增测试，不重复全量suite。Windows原生界面未验收，候选ZIP未纳入此修复。

## 尚未完成

Worker原生report已确认成功，但临时成员被回收时原派单仍保留在暂停投递队列；重开成员会显示待处理消息1和继续处理。未点击继续，避免再次执行已经汇报的责任。这是实际队列结算缺口，不能以工作流completed掩盖，后续须修复并验证已汇报派单与未处理补充消息的边界。

异步结束通知必须保留唤醒能力；本轮没有以删除通知或假标记回合完成消除重复答复。没有证明所有混合后端、Windows、任意中断工具恢复或exactly-once，也没有完成完整产品目标。
