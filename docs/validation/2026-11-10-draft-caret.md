# 草稿切换光标与冷启动聚焦

基线 `1d06db8`，末尾选区修复 `09904eb`；解锁后已补本页列出的 Mac 原生复验，并进一步修复冷启动恢复会话没有初始键盘焦点的问题。完整产品及平台矩阵仍未完成。

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

## 解锁后的原生复验

重新建立第二个自有项目，项目输入 `TRANSFER_BASE` → 选择 Codex → 直接键入 `_AFTER_CREATE`，实际成为 `TRANSFER_BASE_AFTER_CREATE`，原生交接后没有前插，见 `transfer-fixed.ax.txt`。

实际粘贴 `中文草稿 · 🐝\n第二行：继续完善 Hive`，切到原会话再切回，追加 `_UNICODE_TAIL` 位于完整 Unicode 正文末尾。Cmd+Left、Right 两次后收起侧栏触发重绘，再输入 `_MID_`，精确出现在“第二”和“行”之间，证明同 owner 重绘没有把手动位置送回末尾。见 `unicode-mid-fixed.ax.txt`。

正常退出后，中文/emoji/换行/中间插入完整恢复，见 `unicode-cold.ax.txt`。但该旧构建直接打字与三次 Tab 没有进入输入框；设置返回、模型面板关闭也不能建立此前不存在的输入焦点。这不作为末尾追加通过证据。自动恢复会话的调用方现在在 `open_session` 后复用 `focus_project_input` 意图，沿既有“读取完成且聊天可见 → 下一帧聚焦”路径运行；显式切到其他会话仍会清除旧意图，无新全局焦点状态或延迟时间。

重新构建并完整重开后，不点击输入框直接键入 `_COLD_OK`，实际追加到已恢复中文正文末尾，见 `cold-focus-fixed.ax.txt`。把光标再次移到“第二”后，Cmd+, → Escape → 输入 `_RETURN_`，精确保持该中间位置，见 `settings-return-mid.ax.txt`。约592×506窄窗收起侧栏后追加 `_NARROW_`，仍在手动位置，见 `narrow-mid.ax.txt` / PNG；这是可见输入区的局部窄窗验证，不是整个小窗口首次键盘导航矩阵。

四次修复构建窗口均正常 Cmd+Q exit 0。最终 `check-cold.mjs` 用独立真实 Hive/SQLite/runtime 冷读验证：两份精确草稿、两条会话/两个工作区；messages/dispatches/dispatch_messages/agent_runs/workflow_runs 全为零；两会话 history 为空、pending 0 / busy false，均显式选择 Terra，应用保存的原始 Codex 路径保持。全程无发送、厂商推理或全局配置修改。`cold-receipt.json` 与 `cold-check.log` 保留结果。

冷启动聚焦修订的 Mac build、Windows target check 和 diff check 均通过，日志为 `build-cold-focus.log` / `windows-cold-focus.log`。没有新增/修改测试，未跑全量 suite。

## 未扩大为通过的结果

首次解锁后对原草稿点击并尝试 `_COLD_TAIL`，结果为空，未得到追加文字。根因尚未确定；原错误草稿已在 `before-receipt.json` 保留，随后重新输入 ASCII 草稿并正常退出建立可重复冷读基线。没有把这次尝试计为冷恢复追加成功，也不宣称所有鼠标/剪贴板异常都已解决。后续上述确定步骤各自核对实际文本；中文使用粘贴，不表示中文输入法组合态已验收。

证据根 `.artifacts/entry-flow-1d06db8/`，所有本轮原生句柄及独立冷读均已终止。current/candidate 仍为 `6edac4c` 开发包，未纳入本修改；Windows 原生、完整输入法/辅助技术及完整目标保持未完成。
