# 已删除成员的历史操作不再重建启动配置

2026-11-10，基线7eccc10。继续核对删除后的聊天/恢复，发现真实后台缺陷，修复范围限于桌面的启动配置写入接缝。

## 真实问题与根因

成员创建并启动一次后已有持久Pi会话（只有启动上下文，无模型回复）。通过Hive正常DELETE删除该成员，成员和launch配置先正确消失，历史也仍能读取。但在旧聊天调用prompt或resume时，Chats.activateOnce直接调用agent runtime的configureAgentLaunch：它只写launch cache/SQLite，绕过Hive store已有getAgent校验。随后正常HTTP start因成员不存在失败，此前写回的孤立配置却留在内存和DB。同样的直接写入也用于旧会话模型变更。

`.artifacts/deleted-member-launch/` 保留复现与日志。原开发包db6fb20运行新回归时，删除后配置不存在的前置检查通过，失败发送之后断言实际得到hive:pi配置而非undefined（regression-baseline.log、before-terra.log）。这是已复现的普通旧会话操作，不是假设并发条件。

## 最小修复

HiveHost仅转发现有Hive store.configureAgentLaunch并沿用关闭保护，补其窄类型声明；Chats激活与模型变更两处统一走该入口。校验、错误和实际配置写盘仍由Hive原有逻辑处理，不复制校验、不修改依赖/数据库schema、不另写成员生命周期或事务框架。

删除后的历史保留可读，发送/恢复仍按已有成员不存在语义拒绝；现在拒绝后不再重建配置。没有把旧历史重新绑定为主控，或自动创建替代成员。

## 验证

新增 `packages/runtime/checks/deleted-member-session.ts`，并纳入现有check:members命令。它使用真Hive HTTP/SQLite、真Pi SDK JSONL与独立配置，原样历史加载/完整runtime重开；不用mock fetch/store/agent runtime。

- 创建两个真实成员并启动无推理会话，显式选择隔离注册的openai/gpt-6.1-terra能力元数据。
- 删除目标后分别执行旧会话prompt、resume和跨provider的同ID Terra模型变更，三者明确拒绝；每次同时查真实launch cache和SQLite，目标配置始终不存在。
- 原历史非空并在操作和完整冷恢复后逐项相同。另一成员仍能正常保存模型配置；冷恢复保留该provider与原成员ID，团队只剩此人。
- 本机HTTP模型对端仅用于检测意外请求，整个过程请求数为0；没有厂商推理、用户凭据或全局默认值修改。
- 原db6fb20包执行同一新回归先红；修复源码通过（regression-baseline.log / regression-final.log）。setModel拒绝会保留原有native session settings failed诊断，未吞掉错误。
- 现有member-creation、team-runtime、empty-session-recovery、user-input-recovery四组定向真集成全部exit0。
- pnpm check、runtime build、git diff --check通过。无Rust/UI变更，不重复Mac/Windows编译或无关全量suite，既有测试未删改。

首个手动探针没有显式launch模型且SDK默认回落到其他模型元数据，随后显式Terra又因隔离目录未注册该ID而失败；两者没有发起推理，日志保留。最终原包红/源码绿和提交回归均显式注册/选择同一Terra能力元数据、零网络推理，不将这些元数据测试写成真实Terra模型验收。首次新回归调用了不存在的chats.stop，仅修正检查脚本并重跑，没有为检查修改生产API。

## 边界

本次解决已删除成员操作留下孤立launch配置；不承诺恢复已删除成员的执行能力、跨存储模型变更的原子性、所有在途删除时序或新完整UI验收。历史保留按既有行为。当前ZIP/DMG/Applications仍db6fb20，尚未纳入本轮后台修复及前两轮UI语义更新，完整目标继续开放。
