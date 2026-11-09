# 新建会话后直接输入

基于 cf62a3a，源码 Mac 原生窗口、Dark/20、约 592×456。current/candidate ZIP 仍为 7ae27ca。

## 复现与修复

实际查看已安装 T3 Code 的新会话页及 Beluga 引导页，继续对照正文输入与底部操作层次。未操作参考应用的项目/会话/配置，也未复制上游代码或素材。

Hive 从项目旁的＋菜单选择 Codex，加载完成后直接输入 NEW_CHAT_DIRECT_INPUT，文本没有进入编辑框；`before.ax.txt` / `before.png` 保留空输入及禁用发送状态。新建请求成功分支未设置已有的输入焦点交接意图。

修复仅在成功打开新记录后设置现有 `focus_project_input`，由既有渲染逻辑等待加载结束、聊天可见和窄窗导航收起，再在下一帧聚焦输入框；切换会话时清除旧意图，避免带到另一会话。不修改 Agent 选择、模型默认、请求发送或持久化语义。

## 最终原生验证

- 项目＋菜单→Codex→等待实际加载完成→不点击编辑框，直接输入 NEW_CHAT_DIRECT_INPUT，正文精确出现，Send 可用。
- 全局 New 菜单→Codex→等待实际加载完成→不点击编辑框，直接输入 GLOBAL_NEW_CHAT_DIRECT_INPUT，正文精确出现，Send 可用。
- 两次均未发送。正常 Cmd+Q exit 0 后重开，实际窗口保留第二条完整草稿；再次 Cmd+Q exit 0。
- 独立 SQLite/索引/草稿断言通过：messages、dispatches、agent_runs、workflow_runs 与原始副本逐行相同，原始聊天索引不变，两个新草稿及旧 REFLOW_UNSENT_AFTER_STOP 精确保留。四条新空记录包含初次复现、一次误用旧 wrapper 二进制复验及最终两条；均无 agentId、codexThreadId、sessionFile，无新模型提示。默认模型名称显示为厂商默认 Sol，仅打开空会话，未对该模型发请求。

证据目录 `.artifacts/composer-layout-cf62a3a/`，最终 `project-direct.*`、`global-direct.*`、`cold.*`、`state-receipt.json`。初次启动错写 runtime entry 为 index.js，关闭后纠正为 main.js；一次构建后 wrapper 仍指向旧 inode，关闭进程后重新链接最终二进制再复验。两者均未作为成功证据。

## 验证范围

Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows MSVC `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`、`git diff --check` 通过。纯 UI 两行修复未新增测试或运行全量后台测试；独立状态检查为真实文件/SQLite 断言。

其他后端新建路径共享同一成功分支，但本轮未各自做原生复演；Windows 原生、中文输入法、完整键盘和 VoiceOver、整套视觉对标仍开放。短窗第三个起始建议需滚动，本轮未重新设计主页或据此宣称 UI/UX 完成。
