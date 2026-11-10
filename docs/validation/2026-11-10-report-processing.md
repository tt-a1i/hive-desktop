# 活动回合追加汇报的处理状态

2026-11-10，[#28](https://github.com/tt-a1i/hive-desktop/issues/28)。

## 问题与修复

真实一对二任务中，已加入主控活动回合的汇报仍显示“已收到 · 待处理”。共享队列的`pendingReports.processing`仅检查index0；原始用户输入占据队头时，通过原生steer处理的报告排在后面，因而始终false。

只把已有`steering.has(item.id)`纳入processing判断，仍要求busy且未暂停。它表示追加处理正在进行，不等同模型已经阅读或持久消费完成；completedReportId仍严格等待原生成功终态及完成凭据。被用户输入阻挡、没有追加能力的后端、暂停/停止及暂停冷恢复保持待处理。无新字段、协议、存储、依赖或审批。

## 定向与真实验证

既有Codex steering检查增加真实拥有的JSONL子进程、生产适配器及磁盘队列的状态断言：两份活动报告true、尚无完成通知；用户阻挡false；停止和暂停冷恢复false。旧getter先实际失败`[false,false] !== [true,true]`，一处修复后system/report共24模式通过。厂商对端是明确合成协议，不是模型验收。

实际运行：`pnpm --filter @hive-desktop/runtime exec tsx checks/codex-steering.ts`、runtime build、`check:team`、`pnpm check`、`git diff --check`通过。团队套件实际运行HTTP/SQLite/team CLI与无推理SDK。日志在`.artifacts/team-two-report-fixed/processing-{red,green,build,team,check}.log`。不重跑无关全量套件，无旧测试删除。

新隔离目录`.artifacts/team-report-processing`从原生聊天框再次完整粘贴768字符同一任务，独立核对草稿字节后只点一次Send。原样Codex0.167.0，三个`gpt-6.1-terra / medium`会话，无全局配置/账号修改：

- 主控`d0f476ae-7abc-4aff-86a2-3551bb8af2b2`；alpha `af49be99-c369-429a-8f6f-77391f5d7e2e`；beta `dfccaa41-7635-49b2-8022-953aa4b0ef9c`。
- 两派单显式reported，正确成员和主控；两证明文件27/26字节精确，无结尾换行。主控独立Python断言完成，唯一最终答复`HIVE_TWO_WORKERS_COMPLETE`。
- 三个native会话各1轮，两个report都在同一主控native turn，有各自来源/完成ID，投递spool空。
- 真实窗口已观察到带成员名的就绪通知及报告“处理回合已完成”。主控较快结束，原生截图未捕获短暂processing状态；该分支仅由上面的实际子进程/队列检查证明，不宣称获得动态截图。初始读取不存在的项目文档失败原样保留。
- Cmd+Q和所属启动会话exit0；独立源码后台仅`chats.open`三会话，不resume/不prompt。文件、台账、原生轮数、来源/完成ID逐字段保持，pending0，无新增推理；`baseline.json == after-cold.json`。

当前1e68306 current/candidate/Applications尚不含此后续getter修复及3a41948通知成员名修复。Windows原生、其他后端/混合组合及完整UI体验继续按#1验收；不把本次真实完成状态等同完整动态/可访问性矩阵。
