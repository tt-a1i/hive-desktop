# 真实暂停队列冷恢复与继续操作

基于 8b1f97d 源码开发二进制，本轮 UI 修复尚未进入 7ae27ca current/candidate ZIP。完整目标开放。

## 后台定向复核

使用开发包中的 Node 22，经 `packages/runtime/node_modules/tsx/dist/cli.mjs` 执行既有检查，均 exit 0。日志目录 `.artifacts/queue-audit-8b1f97d/`：

- `queued-user-inputs.ts`：真实内置 Pi SDK / Hive HTTP / SQLite，回环 HTTP 持有当前请求，补充文字及文件排队、无效附件不干扰原请求、停止保留、冷启动暂停、继续后按序唯一消费。
- `queued-user-inputs-external-pi.ts`：相同契约，经原始 `/Users/tsk/Library/pnpm/pi` RPC 执行；显式 Node 22 PATH，不改全局设置。
- `queued-user-inputs-peers.ts`：生产 Codex 适配器 / 官方 Claude SDK，加独立合成 CLI 子进程；检查相同停止、附件、队列身份与唯一消费契约。这不是厂商模型验证。
- `codex-interrupt-rejection.ts`：合成 JSONL 对端模拟中断被拒、晚到完成、启动响应前取消及子进程退出，真实 Hive/SQLite 检查原 pending ID 冷恢复。

以上不发厂商模型请求。本轮未改测试或后台代码，未运行全量 runtime suite。

## 真实 Codex / Terra 桌面闭环

证据目录 `.artifacts/queue-resume-live-8b1f97d/`。全新隔离项目、原始 Codex CLI 0.167.0、gpt-6.1-terra、Medium。

1. 实际窗口 Send 一条运行 120 秒 Node 命令的请求，实际看到工具 In progress。
2. 实际输入并 Queue 请求 A、B。两卡显示等待当前回复，未作为原回合 steering。
3. 实际 Stop，原工具显示 Interrupted；A、B 均保留等待继续。输入未发送草稿 COLD_QUEUE_UNSENT_DRAFT，Cmd+Q 正常退出。
4. 独立 runtime 冷读 pending 2 / busy false，两项 delivery ID、原文及顺序保留，completed 为空。实际再次打开仍暂停，草稿完整；未自行执行。
5. 修复下述底栏后，从约 592×456、Dark/20 窗口实际点击固定底栏 Resume processing。真实模型依次回复 COLD_QUEUE_A_OK、COLD_QUEUE_B_OK，均只出现一次。
6. 正常退出后独立冷读 pending 0 / busy false，两项 completed ID 与暂停时完全一致。历史只有原来的三条用户输入、一个已中断 commandExecution，未重跑原工具、未出现原请求的最终标记 QUEUE_RESUME_FIRST_DONE。未发送草稿精确保留。
7. 实际窗口再冷启动，看到 A、B 回复及草稿；再退出后的独立冷读 history 完全相同。最终两个修复后二进制进程退出 0。见 `state-receipt.json`、`check-state.py`、`final-cold.*`。

本条真实验证使用纯文字排队，附件边界来自上述 SDK/回环或协议对端检查，不扩大为本轮真实 Codex 附件恢复结论。停止工具原始 checkpoint 仍标 inProgress，Hive 持久化 hiveInterrupted 并归一 interrupted；本轮未独立记录工具 PID 提前退出，相关 PID 实证见上一轮运行中输入记录。

## 原生发现与修复

暂停时，旧 Resume processing 位于队列卡片下方的滚动内容内；在上述窄窗被卡片挤到下方，而固定底栏仍给出禁用 Send。`compact-paused.png` 保留原始复现。

现在保持原有可继续条件、连接/加载禁用条件及 `resume` handler，把暂停时主操作放到固定底栏，替代禁用 Send；运行中仍为 Stop / Queue，空闲正常为 Send。未新增自动继续、重试、后台规则或状态存储。删除旧滚动区域的重复操作及计数行，队列卡片原文与计数继续保留。

相同真实待处理队列、相同窄窗下 `fixed-paused.png` 可见主按钮，实际点击完成上述双回复。宽窗口最终冷恢复中恢复 Send。Mac build、Windows MSVC target check、`git diff --check` 通过；纯 UI 改动未新增测试，后台契约由本节与上述真实集成检查验证。

## 尚未完成

队列卡片与输入区存在嵌套滚动，窄窗下草稿未直接显露；队列收起后还观察到消息列表滚动位置不理想，首次截图只见最后消息的操作，需继续检查自适应高度及阅读位置，未据本轮宣称整套布局完成。完整键盘/VoiceOver/中文输入法、其他后端同流程原生复演、Windows 原生与包内验证均保留待验。
