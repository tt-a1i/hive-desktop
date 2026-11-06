# Hive Desktop 进度

更新：2026-11-07。

- [完整产品目标 #1](https://github.com/tt-a1i/hive-desktop/issues/1)：开放，涵盖 macOS / Windows 桌面、统一 Agent 聊天及现有 Hive 团队协作接入。完整应用尚未完成。
- [基础阶段 #2](https://github.com/tt-a1i/hive-desktop/issues/2)：开放，目标是实际 Hive Node sidecar、持久内置 Pi 会话和 GPUI 真实聊天链路。它是 #1 的子任务，完成后仍需集成结构化 Hive 派单与汇报及外部 Agent，不能据此声称团队协作已完成。

正式应用代码位于 `apps/desktop` 与 `packages/runtime`。macOS 已跑通实际工作区、Pi 会话、模型选择、真实 write/read 工具及最终回复；真实模型固定使用 Pi 原生 `openai/gpt-6.1-terra`，不经过 Magpie。详细范围见 [基础阶段收据](validation/2026-11-07-foundation.md)。

结构化内置 Pi 已接入 Hive 2.2.1 的执行器装配接缝，同一套 team CLI、HTTP、SQLite 派单和显式汇报已由两个真实 Terra 会话完成。桌面保存待投递消息，Pi 保存完成标记，Hive 保持唯一协作账本。原生界面可切换主控与成员，工具默认折叠并保留详情、错误和复制。真实恢复、取消和取消后继续已验证，见 [协作验收记录](validation/2026-11-07-structured-team.md)。

聊天来源已通过独立元数据区分，实际窗口验证了派单、恢复通知与用户粘贴的 Hive 格式文本；完整模型上下文保留。追加真实验证确认来源随会话恢复、取消后的 shell PID 退出及继续对话。Codex app-server 的通信与会话适配正在接入，尚未完成桌面主控/Worker 验收。

下一步仍需完善外部 Agent 的结构化接入、成员管理和配置体验、任务与协作主页、应用后台重连及 macOS / Windows 安装包。Windows 尚未实际构建或真机验收。基础阶段 #2 保持开放，完整目标 #1 不缩减为当前这条闭环。

每天 09:00 的当前聊天目标提醒已启用。持续目标保持完整范围；单个可运行聊天版本不代表产品完成。
