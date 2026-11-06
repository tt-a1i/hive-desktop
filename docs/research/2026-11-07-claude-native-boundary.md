# Claude Code 接入的下一步边界

2026-11-07，只读调研，未安装 SDK、读取凭据或发起 Claude 模型请求。用户要求模型测试统一使用原生 OpenAI Terra，因此不能将 Claude 的结构化协议验证冒充 Claude 模型执行验收。

当前本机原样安装的 Claude Code 为 `/Users/tsk/.npm-global/bin/claude` 2.1.317。候选为官方 TypeScript Agent SDK：持续输入 query、部分流式事件、interrupt、session resume、历史读取及进程内 MCP 工具回调，符合 Hive 现有会话适配接口；本项目当前没有该 SDK 依赖，也没有已完成的 Claude adapter。[SDK 参考](https://code.claude.com/docs/en/agent-sdk/typescript)、[流式输入](https://code.claude.com/docs/en/agent-sdk/streaming-vs-single-mode)、[自定义工具](https://code.claude.com/docs/en/agent-sdk/custom-tools)

实现时需保留 Claude 原生 session/message UUID，team 工具执行时读取当前 Hive run 的凭据并等待工具清理；interrupt 接受不等于回合完成。权限请求通过明确的 UI 响应处理，不将内置 Pi 的无审批策略扩大到外部 Agent；未支持的交互应显式失败。[权限与交互](https://code.claude.com/docs/en/agent-sdk/permissions)

认证方式需要分别界定：官方文档允许最终用户通过未修改的 Claude Code 自身流程登录，第三方应用不能收集、中转 Claude.ai 凭据或以自身产品提供订阅登录；SDK 嵌入文档要求 API key 或支持的云提供方。Hive 不能仅凭 SDK 技术上能启动 binary，就宣称任意订阅复用方式已获得确认。下一步应核对具体启动与认证方式，保留原样 CLI 和用户自身登录边界，不实现 token 提取或代理。[Claude Code 官方说明](https://code.claude.com/docs/en/legal-and-compliance)、[SDK 概述](https://code.claude.com/docs/en/agent-sdk/overview)

待实现及验证：精确 SDK/CLI 版本组合、无推理初始化/历史控制、Hive 工具回调、权限 UI、原生会话恢复和当前模型测试约束下可覆盖的范围。它不阻塞其他已授权的配置和平台工作。
