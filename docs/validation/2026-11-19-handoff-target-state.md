# 接续状态刷新与事件顺序

发送、失败或一轮结束后，接续窗口通过 `chat.handoffTargetState` 读取投递、队列、模型和身份状态。窗口继续显示首次完整打开时读到的消息、历史警告和冻结来源。正常打开会话和手动重新打开继续读取完整历史。既有 `chat.open(includeLiveHistory:false)` 的冷态合同保持原样。[后端状态入口](../../packages/runtime/src/chats.ts) · [桌面合并规则](../../apps/desktop/src/history_navigation.rs)

GPT-6.2 Sol 复现了旧接口先捕获队列与交互、再等待冻结来源读取的时序。序号 6 的完成事件已到，旧请求仍返回序号 5 的忙碌队列和已处理交互。桌面现在核对请求顺序和已接收事件序号。状态响应不会推进正文事件的读取位置。运行态目标暂时变成只读时，状态请求继续报告真实的排队、活动和原有交互。

## 响应大小

用同一份含 1,505 条记录的隔离冷 ACP 会话，从旧应用后台和本轮后台读取状态。旧刷新返回 6,364,661 字节，本轮返回 8,767 字节，计入 JSONL 帧。随后完整打开仍返回 1,505 条记录和原消息 ID。适配器协议日志保持原样，两次后台都正常退出。已完成交付的状态检查跳过用于显示的历史。冻结摘要、身份、工作树及队列仍须校验，未确认投递按原规则核验原生回执。

## 验收

- 生产 JSONL 与真实 Hive HTTP/SQLite 回归覆盖活跃回合、待发输入、冷重开、目标配置禁用、冻结摘要损坏、显示历史损坏和过期窗口请求。冷队列编辑、撤回和附件保留也已验证。测试使用隔离 Git 工作树和受控 ACP 对端。[接续运行时](../../packages/runtime/checks/session-handoff-runtime.ts) · [冷队列编辑与撤回](../../packages/runtime/checks/handoff-queue-retraction.ts)
- macOS 新旧解包应用分别打开接续会话，暂时移走隔离工作树后从原生窗口发送。旧版走 `chat.open`，新版走 `chat.handoffTargetState`。新版显示只读状态，原草稿和冻结来源仍可见，ACP 未收到请求。恢复工作树并重开窗口后，原草稿完成一次交付，状态为 `consumed`。各窗口以状态 0 退出，Node 后台均已结束。
- Rust 125 项通过，1 项辅助入口按设计忽略。运行时类型检查和 Windows MSVC 交叉检查通过。
- 解包应用的 Node 与生产后台通过同一组接续 JSONL 回归，以及实际 PTY、Hive 工作区和后台关闭冒烟检查。刷新帧大小在解包后重复测得一致。

本地 ZIP 使用 `666e39d`。应用在压缩前和解包后通过严格签名检查，归档逐项核对 28,956 个文件、29 个链接和 2,710 个目录。证据位于 `.artifacts/handoff-target-state-hm0WX6/`，包括 `wire-byte-receipt.json`、新旧窗口日志与收据、`source-handoff-receipt.json`、`packaged-handoff-receipt.json` 和 `archive-manifest.json`。ZIP SHA-256 为 `eec3f6eaa91094655de431ff2d696080ba7cb4e4d0fd09fb6a2a30243c6ce9fb`。

当前 Windows 原生更新、故障回退和 HTML 冷开仍待实机验证。实际托管 GitHub 写入和真实 1Password 授权也仍需独立证据。[Windows 与 GitHub](2026-11-19-windows-native-update-and-pr.md) · [密码库](2026-11-19-pi-vault.md)
