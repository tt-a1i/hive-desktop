# Codex 后台命令的停止与会话恢复

2026-11-09，修复 [#19](https://github.com/tt-a1i/hive-desktop/issues/19)。原样 Codex CLI0.166.0，所有真实模型请求固定 openai/gpt-6.1-terra。Mac实际进程与原生窗口已验收，本轮未重打候选ZIP。

## 根因与修复

`turn/interrupt` 会结束当前模型回合，但已让出执行结果、获得 session_id 的后台命令可能仍存活。前台命令可以被正常中断：初轮前台probe确实退出，不能将它当作后台场景已覆盖。原始CLI生成的schema中，`command/exec/terminate`明确属于客户端`command/exec`创建的进程，不是模型工具命令的通用PID终止接口；没有臆用该接口或按进程名全局kill。

桌面已经为每个Codex聊天独占一个app-server。当前停止入口在原生中断完成、队列暂停且仅丢弃当前投递之后，复用已有stopLive关闭该聊天的连接/进程，并通知Hive结束该运行与撤销token。后续按需打开同一持久线程。其他聊天有自己的进程，保持连接；待处理用户输入与team投递仍持久保留，需显式继续。

只改变Codex停止路径，没有重写会话、改变模型、认证、权限、schema或协议，不自行新增进程树扫描/终止算法。已有Unix关闭/Windows Job所有权接缝继续承担清理；Windows实际运行仍待验收。

## 真实回归：先失败再通过

新增显式`--live`的`checks/codex-stop-live.ts`，真实Hive HTTP/SQLite、原样CLI及Terra：

- 自有Python probe记录PID，等待300秒；要求native exec让出running session，再由实际assistant消息确认进入后台等待。检查真实PID存活，排入两条补充。
- 旧实现在后台场景两次明确失败：Stop返回后PID仍活，最终清理应用才退出。收据保留R2LCi2/QeAsrR，没有把这些失败改为允许继续运行。初轮mRPF2b只覆盖前台，命令退出但新生命周期断言失败；它不证明后台问题。第二个旧实现检查在核对上一结果前已被多启动一次，保留该额外尝试，不算独立新增覆盖。
- 修复后j0G61F通过：停止前PID5945存活，返回前已退出；线程ID不变、两条原投递ID/顺序保持、无活动run/token。另一个工作区Codex handle保持同一对象并能完成原生reasoning读取，无异常退出；显式继续后两条消息各回答一次。
- 最终源码检查另补会话模型断言，仍只允许Terra；最终sJJ8M3通过，PID11781在继续前退出，另一连接及两条回答断言通过。

这不是mock调用次数或源码字符串断言。失败/成功均保留真实事件、SQLite、原生元数据和收据；不宣称对任意主动脱离app-server所有权的守护进程都有效。

## 原生按钮与状态提示

最新runtime的实际Mac窗口，用AX设置并核对输入后点击发送；后台sleep300实际存在且输出UI_BACKGROUND_READY，排入补充后点击停止。进程表确认旧app-server8185与sleep8702消失，应用8098仍运行，原线程及待处理ID保留；继续后只得到STOP_UI_QUEUE_OK，无重跑或额外recovery回合，正常Cmd+Q退出0。

同时发现旧“已停止当前回复”提示在后续回答完成后仍显示。当前会话收到实际busy状态时，仅清除此旧停止提示，保留其他状态/诊断；也覆盖发送入口已经乐观标忙的情况，不依赖busy边沿。

重新构建原生程序后，第二个独立窗口再走后台命令/排队/停止/继续：旧app-server10509与sleep10751消失，应用10495仍运行；原线程、Terra、投递ID保持，回答STOP_STATUS_UI_OK后队列为空，截图中输入区不再有旧停止提示。正常Cmd+Q退出0。

证据`.artifacts/codex-stop-ui/`及`codex-stop-status-ui/`的running/stopped/done快照与进程表、原生/RPC日志和退出收据；原生点击/截图在本聊天工具记录。第一轮UI用于定位提示残留，第二轮用于验证提示修复，不混为一轮。

## 实际检查与边界

通过runtime build、TypeScript check、已有`codex-interrupt-rejection.ts`、`queued-user-inputs-peers.ts`、`team-runtime.ts`；前者含拒绝/退出/早期取消及真实Hive状态回归，后两者的合成对端不冒认为真实厂商。真实CLI live check、Mac生产构建、Windows MSVC目标类型检查和diff检查通过；日志`/tmp/hive-codex-stop-*.log`。没有运行无关全量测试。

显式真实检查命令为 `HIVE_CODEX_COMMAND=/Users/tsk/.npm-global/bin/codex pnpm --filter @hive-desktop/runtime exec tsx checks/codex-stop-live.ts --live`；必须使用Node22.19+，该命令会发起Terra推理。

原生程序对提示guard的最后调整只去掉busy边沿前提，保留实际busy+旧停止键条件；最终Mac/Windows构建复核。当前源码变更为Codex停止清理、停止提示复位及一份显式live检查，无旧测试删除。

当前候选ZIP仍为17d8ce7，不包含本轮修复。原生Windows、混合后端完整矩阵、独立逃逸进程、停止期间所有写盘/OS故障及正式发行未获实证，完整目标继续；不把局部成功当完整交付。

## 后续候选包

上述源码修复已纳入dd0230a候选并取得包内验证，见[候选实证](2026-11-09-stop-candidate.md)。本页原先未打包的边界描述属于当时记录，原始结果保持。
