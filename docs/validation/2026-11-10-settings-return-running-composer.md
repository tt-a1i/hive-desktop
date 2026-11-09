# 配置返回焦点与运行中输入卡片

本轮基于 3b824af 源码开发二进制；current/candidate ZIP 仍为 7ae27ca，未包含本轮及上一轮输入卡片修改。完整产品目标继续开放。

## 配置返回焦点

真实 Mac 窗口复现：从失败消息的内置 Pi 账号按钮进入设置，Escape 返回后，Tab/Return 未能打开相邻模型入口。仅延后一帧恢复焦点仍失败：按钮所在子树被移除后，默认元素状态中的 FocusHandle 已被替换。

修复由应用持有账号、模型、外部程序三个修复按钮的稳定 FocusHandle，并在设置关闭后的下一帧恢复焦点。沿用现有配置路由，不重发请求或修改认证。

最终原生检查：账号→Escape→Tab/Return 打开模型；约 592×450 窗口中模型→Escape→ShiftTab/Return 打开账号；输入框 Cmd+,→Escape 后可直接追加 x，再删除，恢复精确原草稿。外部程序使用相同稳定句柄方案，本轮未独立复演其返回路径。

证据在 `.artifacts/settings-return-3b824af/`。保留初始失败及仅延后一帧的失败记录，最终证据为 `stable-accounts-to-models.*`、`stable-models-to-accounts-compact.*`、`editor-return.*`。独立真实 runtime/SDK 冷读断言通过：原失败请求/待处理 ID、附件字节、精确草稿与配置哈希不变，pending 仍为原来的 1；新增提示及厂商推理均为 0。见 `pi-receipt.json`。

## 实际 Codex 排队与停止

使用原始 Codex CLI 0.167.0，模型 gpt-6.1-terra、Medium；隔离的新项目与会话，三个实际用户请求。证据在 `.artifacts/running-composer-3b824af/`。

- 约 592×450、Dark/20 窗口中发送一次 45 秒 Node 工具请求。运行时输入完整七行补充文字并点击 Queue，输入框清空、原文进入等待卡。真实 SQLite 接收时间在工具开始与完成之间，最终助手回复依次为 REFLOW_FIRST_OK、REFLOW_QUEUED_OK，工具退出 0、输出 REFLOW_TOOL_OK。
- 第三次请求启动自有项目内 120 秒 Node 工具。确认真实 PID 63006 在运行后，在输入框留下 REFLOW_UNSENT_AFTER_STOP，并点击 Stop。界面显示 Interrupted；独立 ps 检查确认子进程在自然截止时间前消失，未产生 REFLOW_STOP_FINISHED 助手回复。
- 停止操作实际窗口约 822×700。尝试缩窗时 CUA 命中圆角失败，此前输入已成功；未重复发送，不将本次 Stop 扩大为 450 高度覆盖。另一次自动化粘贴曾插入旧剪贴板内容，发送前已检查并替换，未提交错误内容。
- 正常退出及冷启动后，原生窗口保留排队结果、已中断工具和精确未发送草稿，无 Stop、可发送；两个原生进程正常退出 0。独立冷读 pending 0、busy false，三个用户输入及两个助手回复精确匹配，无恢复后新增推理。

停止工具的厂商原始 checkpoint 仍为 inProgress、aggregatedOutput 为 null；Hive 保留 hiveInterrupted 标记并归一为 interrupted。这里的停止依据包括真实子进程退出，不宣称厂商返回了 completed 结果或部分 stdout 已持久化。`queue-timing-receipt.json` 与 `cold-receipt.json` 均通过。

## 验证边界

Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows MSVC 目标 `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop` 和 `git diff --check` 通过。纯 UI 焦点修改未增加 UI 测试、未跑全量 runtime suite；运行中行为使用上述真实 CLI/HTTP/SQLite/冷恢复检查。

本轮不改变后台排队/停止语义。Windows 原生、其他后端同等运行流程、全部修复入口返回、输入法和完整 VoiceOver 矩阵仍待验证；不据此宣称整套 UI/UX 或应用交付完成。
