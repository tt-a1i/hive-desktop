# 欢迎页保留 Codex 连接与账号结果（#26）

2026-11-10，基线9e40695/安装包0080304。本次只有原生显示与消息复用，没有Node/runtime、协议、依赖、凭据或持久化结构改动。

## 实际缺口

在已安装0080304中，用自有零项目目录，从欢迎页Codex进入设置，原样0.167.0实际检查得到“已连接 Codex，检测到 ChatGPT 账号记录”。Escape返回，Codex卡片却只有“尚未检查程序版本”，刚完成的另一项检查没有回到首次配置流程。before-welcome.png/AX保留实证；这不是版本文案本身错误，而是另一类已有结果缺席。

现有Codex卡片增加独立连接/账号行，与版本行分开。设置页和欢迎页共用同一消息映射和输入过期判断；未保存路径明确标注，保存后去掉未保存标签。账号记录不升级为模型可用，卡片下的适用范围同步说明版本/连接/本机记录都不验证登录有效性或模型访问。未知/进行中仍明确显示；重开/重连沿现有状态清空，不持久化ready或自动探测，也不自动选择主控。

## Mac 实际验证

证据目录.artifacts/welcome-account-9e40695，原生程序使用本次debug源码，后台与Node使用0080304包内版本。Pi目录是本轮空目录；仅在同一终端会话下检查外部Codex，无模型回合或登录操作。

- 冷开初始分别显示“尚未检查程序版本”“尚未检查连接与账号”。真实原样Codex账号检查发起后立即Escape回欢迎，实际看到进行中，随后同页显示ChatGPT账号记录；版本仍未检查。fixed-connected.png/AX。
- 改成真实不存在的自有路径，未重查就返回：旧连接结果消失，明确要求重新检查连接与账号。再实际检查，欢迎显示未保存路径及失败，不保留旧账号结论。stale-welcome.ax.txt、failure-welcome.ax.txt。
- 输入前轮独立空配置的原样0.167.0脚本，分别做版本与账号检查；返回欢迎，显示两条独立结果：未保存路径的0.167.0，以及未保存路径的已连接但未检测到账号。没有执行登录；unsaved-signed-out.ax.txt。
- 中文/light/20px、约593×507窄窗，收起侧栏，Tab自动显露Codex配置按钮，手动小幅滚动可读完整两项状态；Return直达该程序输入。沿现有Tab顺序到Save、Return实际保存，返回欢迎，两条未保存标签均移除，仍保留正确的版本/未登录结果；saved-signed-out.ax.txt、zh20-unsaved.png。
- 切English返回同页，版本/未登录/范围文案即时翻译，20px窄窗长文换行，键盘聚焦配置按钮且两条结果可读。en20-saved.png/AX。
- 完整Cmd+Q再开，同一保存路径仍在，而欢迎的版本/连接账号均恢复未检查；进入Codex设置读取到隔离脚本的App settings、Save禁用，没有复用旧已连接结论。cold-untested.ax.txt、cold-settings.ax.txt。
- 基线安装版、源码验证窗口、完整重开窗口均Cmd+Q并exec退出0，开发壳不再运行。

## 独立数据与检查

退出后使用包内Node/runtime重新读取真实设置与SQLite：隔离脚本路径保存，Pi/Claude路径仍null，语言en、light/20；workspaces/messages/agent_runs/dispatches/workflow_runs全部0。receipt.json。没有创建聊天、Agent run、模型执行或凭据。

最终Mac cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop、Windows MSVC cargo check及git diff --check通过。首次编译因新用when_some缺FluentBuilder导入失败，补准确导入后两平台检查通过，初始失败日志保留。纯UI消息映射/既有状态复用，没有新增、修改或删除测试，没有重跑未变后台或全量suite。

本轮使用固定.artifacts/development/Hive Desktop Development.app开发壳，退出后才链接最新debug程序，后续可复用该位置，避免每轮另存一份验证程序。此壳表示最新开发源码，不是冻结安装包；不覆盖此前验证壳或0080304包。

## 边界

这是已有配置流程的反馈补齐；API Key/其他原生账号类型、环境覆盖的前端组合、VoiceOver、完整主题字号/Windows原生及模型可用性仍未由本次验收。当前ZIP/DMG/Applications仍0080304，不含本次欢迎页摘要。#26、通知授权相关#25与完整目标保持开放。
