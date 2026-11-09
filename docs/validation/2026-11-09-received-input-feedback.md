# 已接收输入在原生消息到达前保持可见

2026-11-09，源码开发窗口验证；不扩大当前候选 ZIP 的验收范围。

## 问题与修复

后台已持久接受输入、编辑框清空后，厂商原生 userMessage 可能稍后才到。旧界面排除了 processing 的待处理项，此时只剩运行指示，刚发送的正文短暂消失。

`queued_inputs.rs` 现按 delivery ID 是否已经出现在聊天消息中决定临时卡片显示，处理中的输入同样保留。状态区分“已接收，正在处理”、运行中排队、暂停等待继续，附件名与状态共存；原生消息出现后移除临时卡片。英文文案及卡片正文/状态的可访问名称同步补齐。后台投递和执行语义未变。

## 验证

- 旧二进制 + 独立本地 Codex JSONL 对端：故意暂缓 userMessage 事件，真实原生发送后正文消失，证据 `.artifacts/input-feedback/baseline/blank-processing.png`。
- 修复后二进制：中文第一条显示处理、第二条显示等待；复制第一条并粘贴回编辑框核对原文，随后替换为第二条发送。释放第一条原生消息但保持回合运行，第一张临时卡片消失，第二张仍保留；完成第一回合后第二条转为处理。
- 最新含可访问名称的二进制：英文 `AX_FEEDBACK_004 · Received; processing`、中文 `中文处理反馈_005 · 已接收，正在处理` 和 `中文等待反馈_006 · 已入队，等待当前回复结束` 均由实际 AX 树读取。完成后仅保留正常聊天消息，临时卡片移除。
- 英文退出、中文冷启动读取既有历史并继续发送，均正常 Cmd+Q，退出码 0。独立读盘核对 6 个 completed 回合、6 个不同原生用户 item ID、投递 spool 为空数组。证据和摘要在 `.artifacts/input-feedback/receipt.json`、`final/ax-zh-processing-waiting.txt` 及同目录截图。
- `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` 通过。
- `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop` 通过。
- `git diff --check` 通过。

这是 UI 变更，未新增或修改测试，也未重复运行后台全量测试。协议对端是明确标识的本地合成 fixture，无厂商模型推理，不把它算作真实 Codex 模型验收。此次未重跑附件、完整 VoiceOver 导航或 Windows 原生界面矩阵；Windows 编译检查不代替实际平台运行。candidate 仍为 768d085，current 未推广，完整产品目标与 #6 继续开放。
