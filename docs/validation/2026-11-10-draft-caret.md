# 草稿切换光标：复现与待复验修复

基线 `1d06db8`；**修复后原生操作尚未验收**，本记录不表示该交互已完成。Mac 在重开修复构建后锁屏，已请求用户解锁，保留实际运行句柄供继续验证。

## 已复现

全新隔离状态中，从欢迎页配置 Codex，直接键入原始 CLI 路径，检测到 0.167.0；返回显示未保存状态，再保存后欢迎页去掉未保存限定。系统文件夹选择器打开自有项目，直接键入 `ENTRY_DRAFT_KEEP`，再从主控菜单选择 Codex。

新会话精确保留草稿；但继续键入 `_AFTER_OPEN` 后，实际输入变成 `_AFTER_OPENENTRY_DRAFT_KEEP`，新增文字插在开头。正常退出 exit 0 后，独立文件断言确认该错误顺序已原样持久化。见 `.artifacts/entry-flow-1d06db8/before.ax.txt`、`before-receipt.json`。

该流程没有发送消息。后台只读取原始 Codex 配置/模型元数据，显示其原有默认模型；未启动模型推理，也未更改用户全局模型或认证。

## 根因与修改

`view.rs` 在草稿 owner 切换时用现有 `InputState::set_value` 安装正文。gpui-base 0.9.0 的该 API 明确将多行输入的选区重置为 `0..0`，故新会话自动聚焦后继续输入会前插。

安装草稿后复用现有 `set_selected_range`，按正文的 UTF-8 字节长度折叠选区到末尾。它不主动获取焦点。分支只在草稿 owner 改变时运行，不在普通重绘时移动正在编辑的选区；文本、附件、草稿存储及后端协议没有改动。未引入依赖、定时器或第二套光标状态。

## 已完成的检查

- `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` exit 0。
- LLVM PATH 下 `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop` exit 0；不是 Windows 原生验证。
- 包内 Node 22 / tsx 运行已有 `checks/empty-session-recovery.ts --codex` exit 0：真实 Hive HTTP/SQLite 与原始 Codex，固定 Terra 元数据、队列暂停，无模型请求；空会话保持 idle，已有用户队列与冷 spool 保留。该检查不测 GUI 光标。
- `git diff --check` 通过。纯 UI 修改没有新增/修改测试，未跑全量 suite。

## 下一步原生验收

解锁后继续同一修复构建，验证冷启动已有草稿后追加文字位于末尾；重新从项目草稿创建会话，核对中文多行及末尾追加；手动移光标到正文中间，触发同 owner 重绘/配置返回，确认不被重置；退出后独立冷读核对精确草稿、零新投递与原配置保留。

证据根 `.artifacts/entry-flow-1d06db8/`。当前运行句柄记录于 `validation-pending.json`，是否运行仍以实际工具句柄为准。current/candidate 仍为 `6edac4c` 开发包，未纳入本修改；Windows 原生及完整目标保持未完成。
