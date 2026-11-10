# 命名空会话的标题与欢迎页

2026-11-10，源码基线 b90c18d，修复仅涉及原生显示条件；Node/runtime、协议和存储不变。

## 问题与修复

把尚未发送消息的独立会话重命名后，界面仍因 agentId 为空而显示项目欢迎页，隐藏会话标题及重命名入口。侧栏收起时无法看见正在编辑哪条会话。现在只有尚未绑定 Agent 且没有自定义标题的空会话进入欢迎页；命名会话沿用正常对话标题和既有操作。没有新增面板或会话状态。同时本机提醒的“正在阅读会话”判断复用已有 viewing_conversation，替代完全相同的七项面板判断。

## 实际验证

- 自有 Hive Notification Audit 数据中的“保留草稿 B”尚无 Agent、无历史但 customTitle=true。修复前实际显示 What would you like to work on；empty-title-before.png/AX 留证。
- 修复后 Mac 原生 English/light/20px、约593×507窄窗、收起侧栏，显示“保留草稿 B”及重命名入口；打开重命名初始值正确，Escape 取消，原名称与草稿不变。empty-title-fixed.png。
- 通过界面新建未命名 Codex 空会话，仍显示欢迎页。选择 Terra 后未发送消息，返回 B，标题及 NOTIFICATION_OFF_KEEP_B · 保留未发送草稿 精确保留。unnamed-welcome-terra.png、empty-title-roundtrip.ax.txt。新会话最初继承本机 Sol 元数据，仅作显示，发送前已选择 Terra，没有 Sol 推理或修改全局默认。
- 退出窗口后用021c162包内 Node/runtime 独立冷读：三条会话，B名称与 customTitle=true、新会话未设置可选 customTitle 字段且模型为 Terra，两条空会话历史均空、pending=0、busy=false，B草稿逐字匹配、本机通知=false。empty-title-cold-receipt.json。第一次冷检查误把可选字段缺省断言为显式false；核对 chat-index.ts 的可选契约后只修检查，不改生产存储。
- Mac cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop 和 Windows x86_64-pc-windows-msvc cargo check 均通过，日志 build-empty-title.log / windows-empty-title.log。git diff --check 通过。纯UI条件与既有helper复用，没有新增/修改测试，未跑全量；Windows为类型检查，非原生验收。

证据目录 .artifacts/local-notifications-9e61f41。此段标题走查无新增模型回合；该目录前一阶段的一次真实 Terra 回合另见本机提醒记录。当前安装版、ZIP与DMG仍021c162，未含此修复。本项不能代替完整UI、通知或Windows验收。
