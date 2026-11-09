# Mac 候选包的真实 Codex 桌面协作

2026-11-09。在 ZIP SHA256 `006a8e689f020d505a5ac0be24765139f4ab0b345d247d8cc90ac2f429a0828e` 的实际应用中，从原生聊天框发送需求。原始 Codex CLI 0.166.0，主控及唯一 Worker 均为 `openai/gpt-6.1-terra`，没有模型 fallback、Magpie 或后台 Pi 转述。

## 执行与可观察结果

使用新的隔离项目与数据目录。包内 Node 导入包内 runtime，通过现有 API 创建空工作区和 Codex 会话，明确设置 Terra，再保存导航；准备阶段历史为0，无推理。随后启动解压应用，移除开发 Node/runtime 环境覆盖；进程确认实际使用包内 Node 与 dist/main.js，CLI 路径仅在本轮环境中指定，不改变用户全局默认、认证或权限。

在实际原生聊天框粘贴并点击发送：创建一名 packaged-worker，继承后端与模型，在项目内写入指定文件、核验、显式汇报，主控独立读取后返回完成标记。没有由脚本代发 prompt，也没有构造聊天消息。

- 主控原生 team 工具创建唯一 Worker 并派单；成员页可进入真实 Worker 会话，显示 Terra、派单原文和工具过程。
- Worker 创建 `packaged-team-proof.txt`，内容精确为 `HIVE_PACKAGED_TEAM_TERRA_OK`，27字节，无换行。主控没有代写。
- 派单 `ac987aa4-eca6-448f-a87e-5f81f80f7b03` 为 reported，带显式 report 与 seen=0；当前任务没有协作消息，不能外推到非零 seen 冲突处理。
- report_outbox 的目标为正确主控，delivered_at 有值。主控在 reported_at 之后执行独立 Python read_bytes 校验，真实输出 match: True，然后回复 `HIVE_PACKAGED_TEAM_COMPLETE`；原生界面和关联派单详情均可看到结果。
- Worker 的 shell 中 cat 项目说明文件失败，因为隔离项目没有 README/docs；后续创建及 Python 断言成功，整个命令退出0。完整输出和最终说明保留，没有吞异常或为了测试添加生产 fallback。

## 完整退出与冷启动

第一次 Cmd+Q，原生进程15974退出0，包内后台15977和原始 CLI 15983均消失。重新启动同一解压应用17517，后台17521，打开主控及成员，原生窗口恢复完成标记、汇报、工具详情与 Terra。

断言两条原始 Codex thread ID 保持；主控30条、Worker10条展示条目的 turn/item ID及 completedTurns 不变，单一派单和单一 report_outbox 原样保持。没有重发任务或增加完成回合。再次 Cmd+Q退出0，原生及后台/两个CLI进程均消失。

report_outbox 的 notification_state 仍为 pending，read_at 为空；不把 delivered_at、模型核验或界面“处理回合已完成”等同于显式读确认。原回合结束后还有例行 status/report 投递回合，不能把本次结果宣称为 exactly-once 或无额外模型开销。

## 证据、清理与范围

`.artifacts/packaged-team/` 保留 seed、前后快照、真实SQLite/会话投影、原生RPC日志、文件、进程/退出及最终 receipt。`capture.py before-close` / `capture.py reopened` 检查真实文件、派单、汇报目标、主控读取时序及原始身份/条目；准备脚本没有 prompt。原生点击/截图保留在本聊天工具记录。

正常退出、进程消失及 lsof 无引用之后，仅删除本轮 extracted 副本。唯一候选 ZIP 及 current ZIP 未替换；候选收据追加本次真实模型记录，之前无推理的合成历史检查仍单独保留其范围。

本轮没有产品或测试代码改动；运行定向真实包/模型/原生界面检查和 git diff --check，未重跑无关全量测试。结果只覆盖本次 Mac Codex 单 Worker 成功及正常退出/恢复，不代表在途强杀、跨后端所有组合、完整 UI、Pi 失效认证、Claude 真实模型、Windows 或正式安装已验收。完整目标保持进行中。
