# 真实Codex工作流与汇报回收修复

2026-11-09，#21，属于#13与完整目标#1。Mac源码runtime验证，未进入当前2b3c07e候选包。

## 发现的问题

已有工作流检查使用确定性脚本，或由驱动提交report。此次通过生产API启动真实workflow `agent()`，自定义角色选择hive:codex和原始CLI0.166.0，明确openai/gpt-6.1-terra。模型自己读取input.json、计算、写workflow-result.json、读required_seen_seq并显式team report；驱动没有代提交report。

初轮工作流completed、dispatch reported、文件sum=31/count=3正确，但原生dynamicToolCall报告failed/success=false、空contentItems。原探针只核对业务结果曾报pass，补查native历史才发现缺口；其原始收据与日志保留，当前baseline receipt显式标为失败，不把业务完成当作工具成功。

根因：Hive workflow awaitReport在汇报提交后立即finally deleteWorker；桌面stopLive随即abort工具signal和interrupt。team CLI仍在等report HTTP返回、或Codex尚未确认工具结果，就被成员回收中断。没有修改Hive协议、SQLite或用UI覆盖native失败状态。

## 修复

成员回收使用可选settleTools；Codex跟踪实际主机工具callId，收到原生item/completed时才确认结果。回收等待这些确认、原生turn终止或1500ms上限中的先到者，然后仍按原路径interrupt/dispose。上限沿用owned CLI关闭预算；不是固定sleep，也不等待模型写最终答复，不伪造成功。超时仍保留实际native取消/失败，断线错误继续传播。

显式用户停止仍调用无参数abort，立即发出工具取消。其他后端保持原行为，本次只证明Codex路径；不把这一检查扩张为所有SDK的回收保证。

## 验证与证据范围

- 新增checks/codex-tool-retirement.ts，真实JSONL子进程的合成native对端，初轮先红：failed != completed。修复后四分支通过：回收先收到成功工具结果再interrupt；用户Stop的AbortSignal立即生效；无native确认有界退出且不显示成功；断线实际拒绝、通知异常退出，未伪造工具完成。加入check:codex默认定向套件。此检查不冒充HTTP集成或厂商模型。
- `pnpm build`通过TypeScript生产构建。最终`pnpm run check:codex`（含新增检查）与`tsx checks/team-runtime.ts`通过，后者走真实Hive HTTP/SQLite/team CLI/队列，不执行模型。`git diff --check`通过。非全量suite：变化集中于Codex主机工具回收和共享stopLive选项；原协议/store/schema/原生UI没有改动。
- 实际原始Codex/Terra重跑：工作流/派单/文件正确，report原生status=completed、success=true。第二轮探针仍因错误要求CLI成功时必须有非空stdout而exit1；源码team.js的report成功正常无输出。保留原失败receipt/run.log，校准断言另存，不冒充这次原始命令exit0。
- 校准后在全新隔离目录再跑第三轮，实际完整命令exit0，run ddd2aa30-fbf5-431a-a354-b5ca84fd1c44 completed、唯一dispatch reported、文件sum=31/count=3、输入原文保持、worker为Codex/Terra、原生report status completed/success true、空文本输出符合原CLI，runtime正常close。这是本轮最终真实模型通过收据。
- 第二轮数据在完整新runtime恢复后，workflow结果、派单、同一Codex线程和report原生成功保持，独立恢复进程exit0；没有再调用模型。实际桌面源程序显示工作流完成、2条日志、关联汇报；预览真实产物sum/count，往返成员会话看到6项工具完成、report明确“完成”，再返回工作流。GUI正常Cmd+Q exit0，没有GUI chat.prompt；待处理恢复消息保持暂停，没有点击继续。
- 另用原始Codex/Terra运行既有codex-stop-live.ts --live：已yield后台命令被Stop真正回收，另一连接存活，原线程/两条队列ID保持，补充各回答一次，脚本exit0。没有因自然回收等待而削弱直接Stop。

实际模型只用openai/gpt-6.1-terra；driver启动工作流，主控队列暂停，因此不宣称模型主控编写了这次工作流。模型Worker的工具、产物和汇报为真实执行，没有合成厂商回复、切换模型、改全局认证/默认或新权限审批。临时角色已清除，保留工作区、会话和证据。

## 收据

- `.artifacts/workflow-terra/`：修复前真实native失败、original-workflow-only-receipt、baseline-native-report-failure、确定性红色日志。
- `.artifacts/workflow-terra-fixed/`：第二次真实运行及错误的非空stdout断言、单列corrected-native-assertions、恢复/UI记录、最终JSONL与Codex/团队回归日志。
- `.artifacts/workflow-terra-confirmed/receipt.json`：校准后全新真实执行，pass/nativeReportAcknowledged/runtimeClosed均true，run.log命令exit0。
- `.artifacts/codex-stop-live-UoCIMx/receipt.json`：本轮真实Stop回归。

源码Rust未改，不重复Mac/Windows cargo构建。当前candidate仍2b3c07e，尚不包含本次runtime修复，不能据旧候选通过宣称#21包内已验收。Windows原生、其他后端模型回收、工作流删除最终按钮/跨连接时序/大历史及正式发行仍待；#13与完整目标继续。
