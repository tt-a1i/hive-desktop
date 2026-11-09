# 已汇报派单的投递队列结算

2026-11-09，修复[#22](https://github.com/tt-a1i/hive-desktop/issues/22)。基于649de4f；真实模型仍仅原始Codex CLI 0.166.0 / openai/gpt-6.1-terra。

## 根因与改动

临时Worker的显式report先提交Hive账本，随后成员回收中断尚未结束的原生回合。#21确保report工具得到真实确认，但原生回合被中断，投递spool按原规则保留原派单。于是责任reported、投递却仍可继续执行。

复用固定Hive2.2.1现有store.getDispatch，不新建账本或协议。桌面在加载暂停队列、以及成员回收已中断并等待投递idle后核对账本：仅移除presentation明确为dispatch、dispatchId对应当前workspace/接收成员、且账本状态为reported的投递。其他种类与未汇报责任保持。队列修改仍先原子写盘、再改内存，并要求paused/idle。停止后的资源释放不因核对失败而跳过；Hive已关闭后的最终Chats.close不再读关闭的账本。

这只是传输队列与显式汇报账本的协调，不伪造原生回合completed或覆盖工具失败。原始模型/tool/投递历史完整保留。没有以聊天结束推断责任完成，也没有改变内置Pi权限或用户认证。

## 验证

- 新增`packages/runtime/checks/reported-dispatch-queue.ts`并加入check:team：真实Hive HTTP、SQLite、Pi SDK、team CLI，不发模型请求。验证已汇报派单在实际成员停止后移除；未汇报派单、被拒绝的report所对应责任、后续用户输入、report类投递、其他接收成员的投递均保留；模拟旧版残留spool，生产runtime重开时移除同一已汇报派单，其他项逐项不变。
- 将Chats暂时换回649de4f版本运行同一检查，实际pending=4而期望3，exit1；finally恢复修复源码。baseline-check.log保留。最终修复版本exit0，final-check.log保留。最初检查助手错误地把report的人类提示当JSON解析，修改检查助手后继续；生产行为未为此改动。
- `pnpm check`、`pnpm --filter @hive-desktop/runtime build`、`check:team`和`check:codex`通过。Codex定向含直接停止、拒绝/断线、原生汇报确认、缺确认超时与持久队列恢复；合成JSONL与真实模型证据分别记录。未跑全量suite：本次是队列核对与既有store接缝，不改schema/协议或各厂商接入实现。Rust无改动，不重复原生构建或Windows类型检查。

## 原始Codex/Terra实证

`.artifacts/reported-dispatch-terra/`包含run.mjs、run.log、receipt.json、真实events/state/project、recover.mjs及recovery-receipt.json。

- run：ce4d8ba6-21aa-4661-a4fc-4feb5398ced7，completed。
- dispatch：fcc83273-aad2-43a4-a3fc-b0e138b469ff，reported。
- Worker自己读取samples=[7,11,13]、写sum=31/count=3、通过原生team report显式汇报并带产物；驱动只通过生产API启动workflow，没有代report。
- 等待真实成员run退出，打开同一成员时busy=false、pending=0；原生report工具completed/success=true。随后独立runtime冷打开仍pending=0、同一Codex thread、工作流/派单与report成功记录保持；未resume/prompt。
- 本次驱动启动workflow；主控从桌面亲自编写/启动的实证保留在[前轮记录](2026-11-09-native-workflow-orchestrator.md)，不将两轮拼成同一个运行。

## 旧实际故障数据与原生窗口

复制前轮`.artifacts/workflow-orchestrator/state`到独立`.artifacts/reported-dispatch-legacy/state`，原故障现场不修改；before.json记录原spool哈希与条数。原dispatch 0d63c944-72c2-469f-a90b-9e99b312a514在副本冷读时pending由1降为0，原始report原生成功状态及thread保持。

实际开发窗口使用这个副本：同一compute-and-report成员不再出现待处理消息1/继续处理，展开工作过程仍显示messages和report完成。没有发送新消息或点击继续；原来的README缺文件失败工具记录仍保留。Cmd+Q后PID7653正常exit0，见legacy-ui日志与退出收据。这是源码runtime原生Mac验证，未冒充候选包或Windows原生验收。

## 范围与剩余项

#22所述残留派单问题已修复。异步workflow结束通知在主控已主动核验后仍可能触发第二份答复，本次不关闭通知或假标记已处理来掩盖；该体验项继续跟踪。其他后端共享队列逻辑经过上述实际Pi协议检查，但不宣称已跑所有后端真实模型/混合组合。候选ZIP仍7c28580，未纳入本轮及649de4f展示改动；完整产品目标、Windows和正式发行均未完成。
