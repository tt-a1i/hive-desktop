# 成员未结派单数与窄窗操作

2026-11-09，基线7944ab0，源码开发窗口；未更新78721c7候选ZIP。

## 问题与改动

用户查看同时承担多项任务的成员时，原卡片只有“工作中”，无法判断每位成员尚有多少责任。Hive团队接口已有`pending_task_count`，现按原字段反序列化并呈现“工作中 · N 项未结派单”。英文正确区分dispatch/dispatches；0项保留原状态，不伪造完成、不把模型回合结束当汇报。成员分组与状态提供AX语义。

英文1184×1000实测发现角色文字被操作按钮挤到单词内换行；保留身份区最小宽度，窄窗将计数和操作整体放在下一行，避免停止按钮单独掉行。复用原会话、停止、菜单处理器。

## 实际验证

证据目录`.artifacts/member-workload-7944ab0/`，隔离数据/项目/Agent配置；原生GPUI窗口→注册team工具→真实Hive HTTP/SQLite/SDK和子进程。厂商侧为本地可控JSONL peer，无网络/模型推理，不计作真实Codex/Terra验证。

- 第一轮实际聊天启动双Worker工作流，每人1项；追加`team send Stop-a`后Stop-a=2、Stop-b=1，中文宽窗截图`two-one-zh.png`及AX文本保存。
- 中文窄窗、工作中筛选、打开Stop-a成员会话/返回通过。设置切换English后AX分别读到`Working · 2 open dispatches`和`Working · 1 open dispatch`。
- 英文窄窗保留了问题版/中间版截图；最终版`narrow-en-final.png`、AX文本：身份完整，整组计数/Conversation/Stop/菜单可见。此处仅覆盖1184×1000和默认字号，未声称任意字号宽度均验收。
- 第一轮实际详情Stop workflow取消两任务并删除临时成员；最后一轮点击成员Stop触发工作流失败后其余临时成员一并回收，工作中筛选显示0，Stopped筛选显示保留的Available reviewer且未配置后端的Start禁用。没有将成员Stop说成工作流Stop或成功汇报。
- 三轮native PID 8983、65675、84882均Cmd+Q后exit0。`final-ledger.json`独立读取9项真实dispatch均cancelled，仅非临时reviewer仍在workers；所有owned native/peer PID已不在进程表，历史保留。
- 第二轮在运行中直接Cmd+Q，真实账本显示run failed，原因`runtime closing`，所有派单cancelled。记录为后续生命周期问题；没有写成正常完成或已解决。

## 检查与边界

`cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`通过；`cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`通过；最终输出分别在`mac-build-final.log`、`windows-check-final.log`。`git diff --check`通过。纯显示与布局改动，无后端语义变化，不新增UI测试、未跑后端全套。Windows为编译检查，未做原生Windows/完整VoiceOver/所有状态矩阵验收。候选包及完整目标保持未完成。
