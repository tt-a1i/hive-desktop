# Claude Code 接入的下一步边界

2026-11-07 初次只读调研时，未安装 SDK、读取凭据或发起 Claude 模型请求。后续受限实现及验证见下方补充；初次调研的认证边界仍保留。用户要求模型测试统一使用原生 OpenAI Terra，因此不能将 Claude 的结构化协议验证冒充 Claude 模型执行验收。

当前本机原样安装的 Claude Code 为 `/Users/tsk/.npm-global/bin/claude` 2.1.317。候选为官方 TypeScript Agent SDK：持续输入 query、部分流式事件、interrupt、session resume、历史读取及进程内 MCP 工具回调，符合 Hive 现有会话适配接口；初次调研时，本项目没有该 SDK 依赖或 Claude adapter；该描述不代表下方补充后的源码状态。[SDK 参考](https://code.claude.com/docs/en/agent-sdk/typescript)、[流式输入](https://code.claude.com/docs/en/agent-sdk/streaming-vs-single-mode)、[自定义工具](https://code.claude.com/docs/en/agent-sdk/custom-tools)

实现时需保留 Claude 原生 session/message UUID，team 工具执行时读取当前 Hive run 的凭据并等待工具清理；interrupt 接受不等于回合完成。权限请求通过明确的 UI 响应处理，不将内置 Pi 的无审批策略扩大到外部 Agent；未支持的交互应显式失败。[权限与交互](https://code.claude.com/docs/en/agent-sdk/permissions)

认证方式需要分别界定：官方文档允许最终用户通过未修改的 Claude Code 自身流程登录，第三方应用不能收集、中转 Claude.ai 凭据或以自身产品提供订阅登录；SDK 嵌入文档要求 API key 或支持的云提供方。Hive 不能仅凭 SDK 技术上能启动 binary，就宣称任意订阅复用方式已获得确认。下一步应核对具体启动与认证方式，保留原样 CLI 和用户自身登录边界，不实现 token 提取或代理。[Claude Code 官方说明](https://code.claude.com/docs/en/legal-and-compliance)、[SDK 概述](https://code.claude.com/docs/en/agent-sdk/overview)

初次调研留下的实施清单包括：精确 SDK/CLI 版本组合、无推理初始化/历史控制、Hive 工具回调、权限 UI、原生会话恢复和当前模型测试约束下可覆盖的范围。下方补充逐项界定当前结果；它不阻塞其他已授权的配置和平台工作。


## 当日补充：独立适配器已实现，无推理验证通过

已新增 `packages/runtime/src/agents/claude*.ts`，以官方 `@anthropic-ai/claude-agent-sdk` **0.3.317** 对接本机原样 CLI **2.1.317**。`pathToClaudeCodeExecutable` 明确指向用户安装，保留原生配置与认证流程；适配器不读取或转发凭据，不写全局 settings，也未将 Claude 路由到 Terra/Magpie。`HIVE_CLAUDE_COMMAND` 可显式指定原生 CLI。

`createClaudeSession` 实现既有 `PiSessionHandle`，并提供 native `sessionId` 恢复、原生 catalog、结构化增量、原生 UUID 溯源、按需 Hive team MCP 回调、停止与进程清理。SDK 不公开物理 transcript 路径，因此 `sessionFile` 为 undefined，不能将 Hive 投影元数据文件冒充原生 transcript。Hive 上下文通过文档化的 `shouldQuery:false` 原生记录写入；原始类型保持 user，桌面投影按 UUID 将其识别为隐藏上下文。实际派单与汇报仍由既有 Hive CLI/账本处理。

已通过的证据分开记录：

- **真实原样 CLI/官方 SDK，无模型输入**：初始化、52 个 commands、5 项模型目录、SDK MCP server 连接握手、session-local set-model 控制、idle interrupt/close。
- **真实无推理 transcript/control**：只发送 `shouldQuery:false` 上下文；确认没有 assistant 消息，公开历史 API 可读取，native UUID 在重启/resume 后保持，lazy team binding 未被提前调用。
- **真实 owned 进程生命周期**：记录并 SIGKILL 适配器自身 child PID，收到一次 unexpected-exit，后续 prompt 在提交前拒绝，dispose 等待清理；正常关闭无 unexpected-exit。
- **明确标记为合成的 projection/completion 检查**：原生 block UUID 与 API message 分组、同内容不同 UUID 块不被吞掉、stream/final display ID、来源原始记录、精确 input/result/assistant 关联，以及失败、排队和分支不匹配的拒绝路径。
- **直接官方工具 callback + 真实 Hive CLI + 本地 HTTP fixture**：active 外拒绝、执行时获取当前 binding、binding 切换、实际 CLI abort/清理。它没有 Claude 模型参与，不等于模型选择了 MCP 工具，也不等于真实 Hive team dispatch/report 闭环；MCP 注册握手由上一项真实 SDK 检查单独验证。

验证命令：`pnpm --filter @hive-desktop/runtime check`、`pnpm --filter @hive-desktop/runtime build`、`pnpm --filter @hive-desktop/runtime check:claude`。收据位于 `.artifacts/claude/{native-init,session,lifecycle}.json`；完整边界见 [适配器验证记录](../validation/2026-11-07-claude-adapter.md)。收据不保存 account/auth response。

**仍未完成或未获实证的部分**：shared ChatManager/Hive executor/UI 接线、真实 Claude 推理及其流式输出、真实权限/用户输入 UI、模型选择 MCP 工具、Orchestrator/Worker team-live、推理中止和 active tool 期间的实际 Claude 进程死亡、Windows 进程与打包。这些不能写成已集成、可日用或真实团队验收通过。所有模型实测仅能使用 Terra 的原约束未改变，本轮未调用任何 Claude 模型；无推理协议成功也不证明订阅认证/第三方分发方式已获得确认。
