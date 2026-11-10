# Codex 双成员汇报的同回合处理

2026-11-10；关联 [#27](https://github.com/tt-a1i/hive-desktop/issues/27)、完整目标 #1。

## 真实问题与改动

在实际安装的0080304开发包中，用户从原生聊天框一次发送一对二任务。原样Codex 0.167.0、三个`gpt-6.1-terra / medium`会话，两名Worker分别创建独立证明文件并`team report`，主控读取汇报后独立运行只读Python字节断言并完成唯一可见最终答复。两份汇报在主控首次结束前约82/77秒已投递，但各自仍排到一个新的空模型回合：主控3轮、Worker各1轮，总5轮。额外两轮约11.5秒，无可见最终消息。这是已读汇报的重复模型开销，不能把任务文件通过等同于完整链路无问题。

根因：共享队列的原生`steer`准入仅包含`system`，正常`report`仍逐项`turn/start`。仅将已有准入扩大至`system | report`，复用原生追加、来源关联和完成凭据，不新增等待、存储或按正文去重。不处理会继续在这一正常协作场景产生额外模型工作。

- 仍先原子持久化，只有原生接受、实际消息与成功终态的持久凭据齐备才移除投递。
- 不越过已排队的用户/任务；停止、写盘失败、断线、拒绝和未消费继续保留或显式重试。
- 回合结束后到达的汇报继续正常启动后续回合；不支持原生追加的后端保持原队列行为。
- 显式`team report`结算派单与模型消费是两件事；不以厂商回合结束代替任务汇报，也不把投递完成冒称用户已读确认。

## 定向检查

扩展既有`checks/codex-steering.ts`，在系统通知与成员汇报两种来源下各跑12种模式：正常双条、消息先于ack、停止、消费后停止、无ack、未消费、未持久化、拒绝、终态竞争、断线、真实元数据写盘失败和用户排队顺序。使用实际拥有的JSONL子进程、生产适配器、磁盘队列/元数据；厂商对端是合成协议，不是模型验收。另核对汇报的dispatch/worker/artifacts及完成事件、冷恢复。

先只改检查，旧准入实际失败`3 !== 1`；扩大准入后24组通过。命令实际exit0：

```sh
pnpm --filter @hive-desktop/runtime exec tsx checks/codex-steering.ts
pnpm --filter @hive-desktop/runtime build
pnpm --filter @hive-desktop/runtime check:team
pnpm --filter @hive-desktop/runtime check:codex
pnpm check
git diff --check
```

团队检查包含真实Hive HTTP/SQLite/team CLI和无推理的Pi SDK；与合成厂商检查分别记录。无Rust代码、协议schema、依赖或旧测试删除，不跑无关全量套件；跨后端所有组合与Windows原生不在本轮覆盖范围。

## 原始安装包证据

隔离目录`.artifacts/team-two-0080304`：原生聊天框粘贴完整768字符任务，发送前独立核对草稿字节且只点一次Send；无脚本代发提示。主控要求先创建两名再派两项，独立文件/显式汇报/只读核验；不修改用户配置或父项目，不访问网络。

- workspace `07dd490a-7c4d-413e-8fc0-59e689206a49`。
- main `e341b946-8a83-428a-bdbe-2624a2938790`；alpha `0e1963cd-1e66-45aa-835b-ab8b7a915e14`；beta `60c81ca5-db8d-49c0-ba92-fb85880115e1`。
- 两派单reported，目标成员与主控正确；outbox各delivered，notification_state仍pending/read_at null，不能称已确认阅读。
- alpha精确`HIVE_DESKTOP_TERRA_ALPHA_OK`（27字节）、beta精确`HIVE_DESKTOP_TERRA_BETA_OK`（26字节），都没有结尾换行。两个执行窗口实际重叠。
- 原生会话确认三个模型均Terra/medium，主控第一轮唯一final与两轮null final原样保留。
- Mac实际打开主控与两名成员，工具/失败/汇报正文和关联入口可见，截图与AX单独保存；初始读取不存在的`docs/implementation-plan.md`失败保留，后续恢复完成，不算产品fallback。
- Cmd+Q及所属启动会话exit0；独立包内后台`chats.open`三会话，不resume/不prompt，pending0、投递ID/完成凭据保持。`baseline.json`与`after-cold.json`逐字段相同，原生轮数未增。

## 修复源码的真实复验

最终答复、原生轮数、界面和完整冷恢复均已核对。使用新隔离目录`.artifacts/team-two-report-fixed`，当前原生开发程序加修复后的源码runtime、0080304内Node。任务文本与原始包完全相同，原生框发送一次；原样Codex、Terra/medium，不改变全局模型/认证。此项尚未更新安装包。


- workspace `4adecd3b-444c-457c-be39-19669dae9582`；main `0d13c471-20be-4cc6-9340-acbfdd6dcfab`，alpha `48f2e221-b2f7-4e28-a6c5-427c730fbcad`，beta `db5ceecc-7c91-4040-9af4-e5139864901e`。
- 主控12:53:39.505Z开始、12:54:42.207Z完成；两份显式汇报在12:54:23.076Z和12:54:27.150Z提交，均加入同一个主控native turn，dispatch/worker名称与ID保持。
- 主控只读Python断言在12:54:37.763Z调用，晚于两份汇报；实际输出`Both byte assertions passed`。两文件精确内容与基线一致；两派单reported，outbox正确投递。
- 三个原生会话各1轮，全是Terra/medium；主控唯一final包含`HIVE_TWO_WORKERS_COMPLETE`，两个report delivery均在同一原生turn有实际item及持久完成ID。投递spool全空。与基线总5轮相比，本例总3轮；不是跨运行延迟或成本基准。
- 实际Mac查看两份汇报的“处理回合已完成”，主控唯一答复、派单详情→正确成员→另一成员均可读；截图/AX保存。最初AX点到视口边缘裁切的汇报操作没有跳转，滚动到完整按钮后跳转成功，不声称离屏按钮点击已通过。
- Cmd+Q、所属原生启动会话exit0；独立源码后台仅`chats.open`三会话，pending0，dispatch/outbox/文件/原生轮数/投递及完成ID逐字段保持，`baseline.json == after-cold.json`。未resume/未prompt/未新增模型回合。

实际检查脚本、原生JSONL路径索引、`baseline.json`、`after-cold.json`、`cold-snapshots.json`、原生截图和AX保留在各自隔离证据目录。初始文件读取失败保留；没有修改父项目、全局认证或模型默认。

## 范围与剩余项

本项源码修复及真实Codex场景通过；0080304 current/candidate/Applications仍保留旧准入，尚未更新。欢迎页连接摘要亦为待入包源码。晚到报告应触发后续回合，不把所有报告强行并入已结束回合。其他后端的真实账号/厂商/平台矩阵、系统通知授权、Windows原生和完整UI体验仍按完整目标继续。
