# 外部Pi取消失败时回收会话并保留投递

源码基线 `6c24b6b`。本轮只修改外部Pi适配器的失败生命周期与对应检查，不改变Hive协作协议、权限、模型配置或正常取消语义。current/candidate仍是6148801开发包，未纳入本批后台修复。

## 已复现的问题

外部Pi已有JSONL `clear_queue` / `abort` 请求。活动投递期间任一请求明确返回失败，原abort实现直接抛错，没有结束原投递或关闭所拥有的CLI；父层已暂停队列，却无法确认活动执行结束。真实JSONL子进程的定向检查先失败于“停止返回后投递仍未settle”，见`.artifacts/pi-abort-6c24b6b/before.log`。该对端是合成协议进程，不是厂商模型。

第一版回收修复能结束进程，但同一次故障从onClose与prompt finally上报两次，真实断言得到2而非1，见`after-first.log`。没有删除或放宽这个断言。

## 根因修复

- 取消请求失败时保留首个错误，调用已有transport.close并等待所属进程回收；finally拒绝原terminal并等待run.done，使停止返回前原投递已结束。失败继续向调用者抛出，不伪造成功。
- 现有failure路径集中通知：初始化完成、活动投递结束后，仅上报一次；主动dispose不产生额外意外退出。bridge/onClose保留首个原因，清理时的“transport closing”不覆盖原取消失败。
- 不删除待处理投递、不写完成凭据，不改正常abort→原终态等待路径，不增加超时等待、后备执行器或新的进程管理抽象。

## 最终检查范围

新增`packages/runtime/checks/external-pi-abort-rejection.ts`并纳入原`check:external-pi`。实际子进程、正式JSONL适配器、真实Hive HTTP/SQLite：

1. clear_queue拒绝、abort拒绝：停止返回前prompt结束、旧PID已退出、故障回调恰一次、completedDeliveryIds为空。
2. abort期间对端exit7：同样释放活动投递/进程，只通知一次。
3. 正常abort对照：取消投递结束，连接仍存活可由原dispose清理，没有意外故障回调。
4. 两种拒绝分别穿透桌面runtime：真实用户输入→活动投递→停止失败，旧运行status=error、旧令牌撤销、旧PID退出；重新打开与完整runtime关闭/重开后pending仍1、busyfalse、原投递ID和原文保留，未生成完成凭据。SQLite仅一条原始user_input，原agent_run仍error，错误事件保留原取消诊断。

定向最终检查在包内Node22.23.3下运行，所有自有对端PID均确认退出；最终收据`receipt.json`、`process-receipt.json`及`final-exit.log`。测试中一次夹具漏建sessionFile导致真实恢复保护拒绝重开，随后在夹具自身写原生会话头，不为产品增加fallback；另一次错误地要求显式Stop写per-delivery失败记录，检查现有chat.aborted语义后改为验证真实错误事件，未更改产品Stop/队列语义。失败日志保留。

实际运行与结果：

- `pnpm --filter @hive-desktop/runtime check:external-pi` 首轮七项exit0。最终源码复跑前四项通过，但原有external-pi-runtime在首次get_state的30秒期限失败（index仍sessionFile null、agent_runs为空，尚未进入本轮取消路径）；保留`regression-final.log`及原failure.json。该检查单独复核exit0，实际派单/seen/report及冷恢复通过，见`native-runtime-recheck.log`；余下生命周期检查单独执行，新增取消四模式及两项HTTP/SQLite恢复在最终源码单独exit0。初始化超时未复现，原因未证实，不声称已修；未延长超时、修改旧测试或加入重试。
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-interrupt-rejection.ts`（实际Codex JSONL合成对端、Hive/SQLite，未改Codex）。
- `pnpm --filter @hive-desktop/runtime check`、`pnpm build:runtime`、`git diff --check`。

无新增模型请求，不以合成对端声称真实Pi厂商取消失败已发生。未跑全量suite；纯后台改动未重复无变化Rust窗口/Windows编译，Windows原生及新包尚待。所有断言围绕进程、实际投递/令牌/存储和恢复状态，不检查源码字符串或自造mock调用次数。完整目标继续开放。
