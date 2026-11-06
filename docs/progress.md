# Hive Desktop 进度

更新：2026-11-07。

- [完整产品目标 #1](https://github.com/tt-a1i/hive-desktop/issues/1)：开放，涵盖 macOS / Windows 桌面、统一 Agent 聊天及现有 Hive 团队协作接入。完整应用尚未完成。
- [基础阶段 #2](https://github.com/tt-a1i/hive-desktop/issues/2)：开放，目标是实际 Hive Node sidecar、持久内置 Pi 会话和 GPUI 真实聊天链路。它是 #1 的子任务，完成后仍需集成结构化 Hive 派单与汇报及外部 Agent，不能据此声称团队协作已完成。

正式应用代码位于 `apps/desktop` 与 `packages/runtime`。macOS 已跑通实际工作区、Pi 会话、模型选择、真实 write/read 工具及最终回复；真实模型固定使用 Pi 原生 `openai/gpt-6.1-terra`，不经过 Magpie。详细范围见 [基础阶段收据](validation/2026-11-07-foundation.md)。

当前下一步是把结构化会话接入同一个 Hive 团队执行、派单和汇报链路；已有控制器接口仍绑定 Codex 通知，需要扩展传输接缝，不能通过冒用身份或另建账本绕过。随后完成外部 Agent、配置体验和两端安装包。

每天 09:00 的当前聊天目标提醒已启用。持续目标保持完整范围；单个可运行聊天版本不代表产品完成。
