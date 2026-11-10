# 协作概览：先看到派单

源码基线 36cbc97，本轮为纯原生 UI 改动。当前开发包仍为 6edac4c，未纳入本轮。

## 真实问题

- 场景：用户打开已有协作，任务文档有 40 行，想查看成员的派单状态和汇报。原界面先完整展开文档，约 592×506 窗口首屏被文档占满。
- 可观察变化：任务入口仍在顶部，只读正文默认收起，同一窗口可直接看到负责人、Reported、汇报摘要和 1 artifact；显式展开可阅读，编辑与恢复草稿自动显示。
- 不改的影响：每次查看协作进度仍需滚过完整任务文档；任务越长，派单越难找到。

## 改动

复用既有按钮、编辑器、焦点滚动和任务保存逻辑，仅新增一个暂存展开状态；切换工作区重置，创建/恢复编辑器时展开。加载、空文档及错误反馈仍显示；不新增后台状态或接口。展开正文增加 Label 文本语义，收起时正文不进入可访问树。补齐中英文展开/收起标签。

## 实际验证

证据 `.artifacts/collaboration-overview-36cbc97/`。复用隔离的真实 Codex/Terra 晚到 Worker 汇报历史：两个会话、一条 reported 派单、原产物 `{sum:37,count:3}`，已有 40 行任务文档。本轮没有新模型提示、启动成员或改派单。

1. 同一英文 Dark/16 窄窗，`before-visible.png` 是原来长文档占满页面；`after-collapsed.png` 可直接看到真实派单与汇报摘要。初次 `before.png` 实际是导航遮挡，已保留但不算有效对照。
2. 最终版本初始收起，AX 无任务正文；展开后 AX 可读正文前缀，再收起后消失。AX 工具将长字符串截至约 512 字符，本轮不据此声称完整 40 行屏幕阅读器朗读已通过。正文文件逐字节验证见后续检查。
3. 从收起状态点击 Edit tasks，编辑器直接获得输入；保留本地测试草稿 UNSENT_OVERVIEW_DRAFT 后 Cmd+Q，独立检查确认精确草稿已写入、团队任务文件未变。
4. 冷启动打开协作，编辑器自动恢复原测试草稿，不被默认收起隐藏。输入框 Tab 三次显露 Cancel 的焦点轮廓，Space 取消；测试草稿移除，原团队文档仍保留。
5. 收起只读正文后直接打开同一派单，详情中原始汇报、成员会话入口和 result.json 均保留；实际 Preview 看到 `{sum:37,count:3}`，Escape 返回，Cmd+Q 正常退出。最终两次原生进程均 exit 0。

一次粘贴与用户剪贴板冲突，实际插入了其他内容。立即检查后用 ASCII 键入替换为明确测试标记，未点击 Save tasks；不将该次粘贴记为成功或归因于产品。原任务文档最终校验未变。

## 状态检查与边界

独立真实 runtime / 原始 Codex 冷读：两个原线程、pending 0 / busy false，主控原 WORK_STARTED / LATE_REPORT_OK 37/3、reported 账本及精确产物均保留。SQLite messages、dispatches、dispatch_messages、agent_runs、workflow_runs 与原副本逐行相同，聊天索引、成员未发送草稿、原项目文件哈希和 1,912 字符任务文档不变。取消后的任务草稿不存在。见 `check-cold.mjs`、`draft-persisted-receipt.json`、`state-receipt.json`。

最终 Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows MSVC `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`、英文 JSON 目录及 `git diff --check` 通过。未新增/删除测试、未跑全量后台 suite；纯 UI 使用真实历史、原生操作与状态断言验证。

本轮是一个历史 Worker 的概览切片，不是新多 Worker 实时协作验收。中文原生第二轮、跨工作区展开状态矩阵、完整辅助技术/输入法、Windows 原生、新开发包及整体 UI/UX 对标继续开放。
