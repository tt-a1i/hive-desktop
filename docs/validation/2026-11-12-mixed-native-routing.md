# 四后端路由矩阵与设置错误原因

2026-11-12。基于c96c1dc；矩阵运行时生产runtime与645c9a5包一致。随后仅修改`packages/runtime/src/chats.ts`的共享设置错误消息，保留原生原因，不改暂停、清理、重建、队列或协议字段。

## 四种后端的16个角色组合

现场与可重跑脚本：`.artifacts/mixed-native-20261112/check.mjs`、`receipt.json`、`state-evidence/`。后台使用真实Hive HTTP/SQLite、Pi SDK、外部Pi 1.4.0 RPC、Codex 0.167.0 app-server及Claude Code 2.1.319/官方SDK。无面板操作、无mock或合成模型回复。

四种后端分别担任主控，每个主控创建四种后端的Worker，共16组合（12个混合组合）。会话真实初始化并写入隐藏上下文后，暂停投递队列，再通过真实team CLI按成员名派单、读取required_seen_seq并显式汇报。Worker端收到正确dispatch来源、任务正文和派单ID；报告进入对应主控的pendingReports，成员身份、正文、dispatchId正确，没有落到Worker会话。

每组完整关闭后台再重开主控，四条待处理汇报及完成ID保持；重新打开各Worker确认原后端、pending=0、无主控报告。独立只读SQLite复核全部16条派单reported、成员ID与报告正文匹配，16条report_outbox均投递给所属workspace的orchestrator。关闭后lsof确认本轮临时目录无打开文件；四组关闭状态另归档在state-evidence。

这是路由、原生控制和持久化验证，**不是16次模型协作**。所有队列暂停，Pi使用独立的本机拒绝端点/模型目录（收到模型请求0），Codex和Claude只走原生控制；没有assistant_delta/tool_start事件。没有让Claude改用Terra，也没用其他厂商推理绕过用户模型限制。真实Terra协作仍由已有独立记录证明，Pi当前OAuth失效及Claude真实推理未验收的边界不变。

准备检查时两次脚本问题保留原始记录：独立空Pi目录没有OpenAI模型；team report成功输出为空，不能按JSON解析。检查改用显式本机控制目录，并只解析send/messages的JSON；没有改产品语义去迎合脚本。

## 设置失败的具体原因

上述空目录的选择失败暴露：原生错误作为Error.cause保留，但共享changeNativeSettings只把通用“无法保存原生会话设置”发给桌面。用户看不到Unknown Pi model等原生原因，无法判断如何修复。

现在共享包装改为“原生会话设置更新失败……原因：<原生错误>”，继续按原路径暂停并释放句柄，供重新读取后重试；不添加异常字符串分类、fallback或新的恢复分支。作用于共享的模型/推理/原生设置变更入口。

`settings-wire.mjs`分别启动645c9a5旧包与新源码的生产JSONL入口、真实HTTP/SQLite/Pi SDK。旧响应只有通用提示，新响应带`Unknown Pi model: owned-settings/missing-model`；保留两个原始RPC响应，不以mock回调或源码字符串替代验证。两者均独立验证：拒绝后原模型/原生身份/pending/会话文件字节保持，重新读取后可选择另一个有效模型，模型请求0、正常退出0。保留原有保守暂停语义；本次没有把无效选择改成无条件复用旧句柄。

## 验证与交付边界

- `pnpm check`、`pnpm build:runtime`、`git diff --check`通过。
- `node .artifacts/mixed-native-20261112/check.mjs`通过16组合；矩阵运行在错误消息改动前，路由逻辑没有变，不为一行诊断文字重复整个矩阵。
- `node .artifacts/mixed-native-20261112/settings-wire.mjs`通过旧/新真实入口对照及原生状态/文件检查。
- `pnpm exec tsx checks/pi-reasoning.ts`、`checks/pi-context-persistence.ts`通过，覆盖真实文件写入失败、句柄退役、重试/冷恢复与内外Pi。检查从packages/runtime运行，范围沿原脚本；本机阻断/拒绝端点不是厂商推理。
- `HIVE_CODEX_COMMAND=/Users/tsk/.npm-global/bin/codex pnpm exec tsx checks/external-reasoning.ts`通过，原始Codex控制及官方Claude SDK/合成协议对端。首次默认PATH运行遇到另一个CLI不支持`--listen`；保留工具输出摘录，随后显式指向0.167.0，未修改测试或产品适配器。

没有新增、修改或删除正式测试。生产改动仅一行错误消息，未运行无关全量suite、Rust构建或重复面板操作。Applications/current/candidate仍为645c9a5；新诊断尚未入包，下一次代码批量刷新时纳入。Windows原生、真实混合后端推理及完整产品验收继续开放。
