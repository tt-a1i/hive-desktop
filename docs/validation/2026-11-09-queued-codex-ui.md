# 真实 Codex 原生排队与恢复额外唤醒修复

2026-11-09。源目录 runtime + 最新 GPUI 原生应用，原始 Codex CLI0.166.0，所有真实推理固定 openai/gpt-6.1-terra。没有 Magpie、认证复制、模型 fallback 或全局默认变更。本轮没有重新打包。

## 真实发现

初轮从原生输入框发送只读 sleep60，忙碌中排入一条补充。第二条粘贴遇到系统剪贴板竞争，实际输入了另一段草稿；检查后没有发送，保留本地未发送草稿，历史中不存在该内容。停止后，第一条补充原 ID 保留，完整退出/冷重开仍暂停且草稿保持；显式继续才执行。

原生界面随后出现两次同一完成标记。生产元数据证明：第一回合对应唯一用户补充，第二回合来自独立 origin=recovery 投递。冷恢复把“继续未完成工作”追加到了已有投递之后，用户需求完成后又唤醒了一次模型。此处不是用户输入被重复入队，也不能以用户历史只一条掩盖额外推理。

初轮 sleep 的最终原生记录为约59.9秒、退出0；停止发生在其结束附近，不作为命令提前终止的证据。初轮完整现场及剪贴板冲突保留在 `.artifacts/queued-ui-live/`，不上传草稿正文。

## 最小修复与回归

在 Chats.start 的公共 deliver 入口中，仅当 recovery 到达的是空闲且已有待处理投递的队列，复用既有 addContext 路径，将当前协议事实持久保存并交给下一次实际投递，不再追加恢复唤醒；空队列保持原恢复行为。没有改 Hive 协议、SQLite schema、用户输入来源、队列顺序或审批，也没有添加新的队列账本。

先修改既有真实 SDK/Hive HTTP/SQLite 回归要求，旧实现得到4次请求而预期3次，确实失败；修复后通过。断言不仅核对少一次请求，也核对首条补充请求包含原补充内容及 Current member profile 恢复事实。再扩展空队列冷重开检查，确认它仍独立产生恢复请求。该检查的第四次请求现在用于这个独立空队列场景，不能与旧冗余请求混淆。

内置 Pi、原始外部 Pi 的回环 HTTP/SQLite检查、Codex/Claude JSONL合成对端的排队检查、structured-runtime 与 team-runtime 检查均通过。回环和合成对端不冒称厂商真实模型。

## 修复后真实原生闭环

新隔离项目和空 Terra 会话。使用原生 AX setValue 核对输入后点击发送，避免再次依赖共享剪贴板；没有脚本代发 prompt。

1. 原样 CLI 执行只读 sleep300，原生投影确认 inProgress，真实进程存在。忙碌期间从桌面排入两条补充，spool 为当前项+两条待处理项。
2. 点击停止，当前回合中断，两条补充原 ID 与顺序保持。完整 Cmd+Q exit0；冷重开仍暂停，未新增完成回合。
3. 点击“继续处理”，真实模型按顺序分别回复 FIXED_QUEUE_ONE_OK / FIXED_QUEUE_TWO_OK。各用户投递只一条，无 recovery 投递/额外恢复答案；元数据保留当前恢复协议事实，原始 Codex thread ID 不变，队列为空。全程3个完成回合：初始中断+两条补充。
4. 再次正常 Cmd+Q exit0。没有创建成员或修改项目文件。未发送的初轮无关草稿保持在独立的初轮数据中。

断言脚本 `.artifacts/queued-ui-fixed/verify.py` 核对文件 spool、原始 native delivery/turn/item、模型、顺序答案和退出收据。源数据/日志/前后投影、最终 receipt 同目录；原生点击和截图在本聊天工具记录。

## 另一个未修的停止边界

修复后测试提供了明确反例：点击停止时，原始 Codex sleep300 仍在运行，投影为 hiveInterrupted=true / inProgress，PID23018仍存在，直到完整应用退出才消失。不能将“停止回复”当作工具进程已被杀停。此问题在 [#19](https://github.com/tt-a1i/hive-desktop/issues/19) 独立跟踪，下一步核查原生 app-server 的进程控制，不在本次恢复合并中偷偷加入其他进程终止策略。

## 实际命令与限制

通过 runtime build / TypeScript check、`tsx checks/queued-user-inputs.ts`、`queued-user-inputs-external-pi.ts`、`queued-user-inputs-peers.ts`、`structured-runtime.ts`、`team-runtime.ts` 及 git diff --check。命令通过 Node22.23.3的隔离 PATH 执行；日志 `/tmp/hive-queue-recovery-{red,green,external-pi,peers,structured,team,build,types}.log`。

本轮改公共恢复投递接缝及现有定向检查，未新增测试文件。风险匹配的跨后端定向检查和真实 Codex 原生闭环已执行，没有重复无关全量。Windows 原生、完整界面/混合后端矩阵及正式发行未完成；当前候选 ZIP 仍为17d8ce7，不包含本轮 runtime 修复。完整目标继续。
