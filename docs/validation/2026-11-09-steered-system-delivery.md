# 系统通知进入当前 Codex 回合

## 已修的具体路径

主控还在等待工作流结果时，完成通知原先总是排在当前回合之后；主控先自行读取结果并汇总，随后通知又触发一次汇总。原故障见 `2026-11-09-native-workflow-orchestrator.md`，原生接口/停止边界见 `2026-11-09-codex-steering.md`。

现已在生产 Codex 适配器及共享持久队列接入原生 `turn/steer`：

- 先原子持久化系统通知，再尝试交给当前活跃回合；有排队用户或任务输入时不越过它。
- 按原生 `userMessage` 的 itemId 保存各条投递来源；旧版只有 turnId 的元数据仍能读取。
- 追加响应仅表示接受。收到对应原生消息，并在成功终态后重新读取完整原生历史确认它确实存在，才持久记录完成；队列随后移除。
- 多条不同通知可以进入同一回合；相同正文的重复追加保守地保留普通排队路径，避免缺少厂商 itemId 响应时混淆身份，不静默去重。
- 停止、断线、写盘失败、未消费、追加被拒绝继续保留投递；显式重试不被旧失败 Promise 卡住，也不覆盖同一投递在旧回合中的来源；中断不等待尚未返回的追加RPC超时。只有已经观察到原生终态且收到 -32600，才回到普通投递；不按错误文字吞错。
- 原生消息先于追加响应时，等待可靠来源关联后再发送展示事件，避免同一输入先以 native item ID 显示用户行、又以 Hive delivery ID 增加通知行。

其他后端未宣称支持该能力；它们继续走原队列。回合已经结束、没有可追加回合或有用户输入在排队时，也保留原路径。这不是任意通知的去重/精确一次保证。

## 定向检查

新增 `packages/runtime/checks/codex-steering.ts`，加入 `check:codex`。真实 JSONL 子进程和生产适配器/持久 spool，厂商对端是明确标注的合成 peer，不是模型验收。

覆盖：多通知同一回合、独立来源与冷恢复；native item 早于 RPC 响应；已接受后中断；没有消费消息；仅出现 live item 却未出现在完整原生历史；RPC拒绝后的显式重试；终态竞争；断线；真实目录权限导致元数据写盘失败；无追加响应时停止仍及时返回；已消费后停止再重试保留前后回合来源；不越过排队用户输入；不先发送一行错误的普通用户消息。

临时关闭队列追加接线，同检查在原生回合数断言失败（3，预期1），恢复后通过。日志 `.artifacts/steering-delivery/no-steering-baseline.log`；没有用自己调用的 mock 次数证明产品行为。

实际通过：

```sh
pnpm check
pnpm build:runtime
pnpm --filter @hive-desktop/runtime check:team
pnpm --filter @hive-desktop/runtime check:codex
```

最终 TS/build/Codex 日志在 `.artifacts/steering-delivery/`。没有为了该改动运行全量 suite、Rust/UI 测试或重新打包。

## 真实主控与 Worker 工作流

`.artifacts/workflow-steered-main-final/`：生产 runtime，原始 Codex CLI 0.166.0，两位 Agent 均固定 `openai/gpt-6.1-terra`。

- 工作区 `2c9bc279-bbe1-48af-8b30-78b2e812b17d`；主会话 `0f6f6d4b-dc2a-4dcc-946f-f5bfaada3239`。
- 主控自己读取保存的工作流源码，通过原生 team 工具启动该原文，只有一次 run；验证驱动不提交 Worker report。
- run `1650d754-c4b5-484f-a75d-a77dada6c1fc`；Worker 会话 `eb7bfaf8-5148-4328-879d-f7ca4c85c705`。
- Worker 实际读取 `[5,19,23]`，写出 `{sum:47,count:3}`，读取 messages/seen，显式带产物 report；派单 reported，原生 report 工具 completed/success=true，成员 pending=0；输入原字节保持。
- 主控调用本地 `fs.watch` 等待脚本；驱动观察真实 workflow terminal 后打开自有文件 gate。它只用于确定验证期间主控仍在等待，不是产品的固定等待/兜底，也不伪造结果。
- 主控同一 native turn `01a1bff3-380a-74a1-8a61-802548299a23` 保存用户和系统两条独立原生 item，分别对应两条已完成 delivery。
- 主控确实再次读取结果和输入文件，并通过 team workflow show 检查同一 run；只有一条 final：`MAIN_STEER_OK sum=47 count=3`。
- 完整关闭后台后，用新 runtime 冷读：pending=0，来源及完成 ID 保持。receipt/audit/events/raw metadata 保留。

第一轮 `.artifacts/workflow-steered-main/` 因探针工作流漏写必填 description 被拒绝；主控诚实报告失败，没有实际 run。驱动随后等到验证期限才退出，记录保留；不算产品失败或成功证据。修正探针并加入“主控已结束但没有启动run”检查后才跑上述新工作区。

真实工作流后，另补回调来源排序、写盘异常、未返回RPC的停止和重试保留历史来源，最终合成对端/TS/build验证这些路径；未将它们冒充第二次完整原生窗口走查。

## 真实追加后停止

生产接线构建另执行 `.artifacts/steering-delivery/native-stop-final.mjs`，直接使用生产 Codex 适配器和 DeliveryQueue、原始 CLI/Terra：

- thread `01a1bff9-865e-76d1-935c-c5ceb35240db`。
- 真实模型调用自有动态工具并等待。只观察原始 transport 的真实 steer RPC 应答，不替换参数、结果或对端；确认接受后再执行真实 abort。
- 原始用户投递按现有 Stop 规则丢弃当前项，系统通知 ID `169dac76-5558-4ed6-8d97-7d228ea7b8bc` 仍在原 spool，完成 ID 为空。
- 关闭并建立全新原生 session，通知仍以相同 ID 待处理，原生历史只有最初用户输入；没有把“已接受”当作“已消费”。
- 初次脚本的 TypeBox 文件导入错误发生在创建 session 前，已修正，原失败日志保留；有效执行 exit0，receipt.pass=true。

两个实际工作目录收尾 lsof 无打开引用。工作流及停止驱动正常关闭各自拥有的 runtime/session；没有清理原故障证据或用户会话。

## 交付范围

源码修复和定向/真实后台路径通过。未重验 Mac 原生窗口、Windows、其他厂商的同回合输入，未更新候选0e72137 ZIP或推广current。主控已结束后的晚到通知仍可能另起回合，其他后端仍沿用原队列；不会把本修复说成所有重复答复场景已消除。完整桌面/UI/配置/跨平台发行目标继续开放。
