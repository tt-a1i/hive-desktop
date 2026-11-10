# Claude中断被拒绝后的进程回收

源码基线`a4c4341`。实际场景：Claude官方SDK的interrupt明确返回错误，桌面停止调用已经返回，但所属CLI尚未退出。与外部Pi前轮缺口分别验证，不根据相似代码直接假定有问题。

## 复现与修复

复用现有`claude-reasoning-peer.cjs`，仅增加默认不启用的rejectInterrupt控制和启动PID日志；真实官方SDK、真实子进程，合成JSONL对端不接模型。最初等待SIGTERM事件的夹具假设不成立，保留`before.log`，不把它当作产品缺陷；随后直接检查公开abort返回后的PID，`before-owned-exit.log`真实失败于PID仍存在。

原实现catch调用`running.close()`并结束投递，但没有等待SDK所属子进程的close事件。现先沿原fail/投递终态通知释放活动投递，再await已有`close()`。复用现有进程退出等待与强制关闭预算，没有新增管理器、固定等待或新的超时。错误继续抛给原调用者，正常中断路径不改。

顺序有实际含义：原故障通知需在dispose之前发生，才能让Hive更新失败运行、撤销令牌并保留队列；先直接dispose可能抑制该通知。定向检查同时验证故障通知恰一次和父层状态。

## 最终穿透检查

新增`checks/claude-abort-rejection.ts`并纳入`check:claude`：

- 官方SDK直接会话：等待对端实际收到用户输入，原生interrupt拒绝；abort返回前prompt结束、旧PID已退出、只发一次意外退出通知、完成凭据为空。
- 真实Hive HTTP/SQLite：活动用户输入后追加第二条输入；失败Stop结束旧进程，原run为error、令牌撤销，pending两项原ID保留，均不记完成。重新打开及完整关闭/冷启动仍busyfalse、runId空、pending2且顺序/原文相同；冷打开没有新query。
- 显式继续后，第一条重试、第二条第一次处理，两个原投递ID各完成一次，最终pending0。按实际已接受的队列text（包括Hive系统提醒）核对原生输入；用户显示正文独立核对，不移除Hive提醒来迎合测试。
- SQLite仅有原先两条user_input；原Hive另有合法system_recovery_summary，不将它误算成重复用户输入或修改生产语义。初次过窄的原文/全消息断言失败日志保留，修正的是检查范围。

最终在Node22.23.3下exit0，所有自有对端PID均确认退出；证据`.artifacts/claude-abort-a4c4341/receipt.json`、`process-receipt.json`、`wire.jsonl`、`final-verified.log`。SDK/合成CLI使用Terra名称做协议控制，不宣称Claude真实提供该模型。

## 回归与边界

以下实际exit0：新定向检查、`pnpm --filter @hive-desktop/runtime check:claude-write-loss`（四类写盘/退出与恢复）、`tsx checks/queued-user-inputs-peers.ts`（Codex/Claude正常停止、跟进附件、冷恢复与继续）、`tsx checks/claude-session-lifecycle.ts`（真实原始Claude进程死亡，无模型请求）、`pnpm build:runtime`、`git diff --check`。

未跑全量suite：改动限于Claude中断拒绝后的关闭等待，风险匹配为上述真实SDK/进程/HTTP/SQLite/恢复检查。没有Rust界面更改，未重做原生GUI/Windows或厂商推理。真实Claude厂商协作仍未验收，current/candidate保持6148801开发包，尚未纳入外部Pi与本批Claude修复；完整目标继续开放。
