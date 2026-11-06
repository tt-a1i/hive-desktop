# 桌面基础阶段验证收据

日期：2026-11-07。范围：实际 GPUI 应用、Hive Node host、内置 Pi 结构化聊天。完整目标 #1 和基础阶段 #2 均保持开放。

## 版本与来源

- GPUI Kit 0.9.0 / gpui-pre 0.3.11，Rust 1.98.0；macOS arm64。
- 实际 Node 验证使用 22.23.3；Pi SDK 精确固定 1.4.0，与本机 Pi CLI 版本一致。
- Hive 使用 npm `@tt-a1i/hive@2.2.1`，由 pnpm lockfile 固定完整性。源代码调研快照是 `7066d1da8b794f611eb6ae6eeaf84df687079089`，发布包没有 gitHead，编译入口哈希也不同，因此不声称两者逐字相同。

## 已通过

- TypeScript 严格检查与 runtime 构建；Rust 原生应用构建和格式检查。
- `pnpm check:hive`：真实 HTTP + SQLite 创建、列出临时工作区，拒绝无效路径，关闭后端口关闭，数据目录环境恢复；不启动主控。
- `pnpm --filter @hive-desktop/runtime check:runtime`：真实 Node JSONL 子进程 → Hive 工作区 → Pi SDK 会话，列出模型与空历史；进程退出并重启后保留桌面会话身份。没有 mock 后台或模型请求。
- macOS 实际原生窗口：添加隔离项目、新建会话、搜索模型、选择模型、发送、工具过程和最终回复。普通 Cmd-Q 后应用及其 Node 子进程均退出；重开可恢复工作区和已有会话。
- Pi CLI 原生 `openai/gpt-6.1-terra` 短请求返回指定文本；随后桌面内同模型实际执行 write 和 read，并显示中文回复。未经过 Magpie。
- 文件 `/Users/tsk/code/hive-desktop/.artifacts/foundation-project/hive-proof.txt` 的内容断言为 `HIVE_DESKTOP_LIVE_OK`；Pi 持久会话记录包含成功 write/read 和 `gpt-6.1-terra` 的最终 assistant 消息。模型切换后 Pi 全局默认未变。

## 发现与修正

- 首次使用原有 Magpie 默认配置时连接失败，保留失败会话证据。用户随后指定只用 Codex GPT-6.1 Terra，并明确不使用 Magpie；之后只使用 Pi 原生提供方进行真实模型验证。
- 修复停止发生在 SDK 输入/认证预检期间时的取消遗漏，输入被扩展处理时缺少终态，以及创建会话与退出之间的生命周期缺口。SDK 预检本身不支持强行中断，取消需要等预检返回。
- 修复重试期间 Stop 消失、会话标题不刷新、流式输出强制回底、恢复会话缺失活动消息，以及模型列表不可实际使用的问题。
- 恢复消息采用稳定 ID、快照水位和排队事件；保留每段流式空白，避免破坏代码和 Markdown。空错误消息与内部 system 消息不生成空白卡片。
- 两份独立复核确认原有问题已修正；第二轮发现的空白增量过滤问题随后修复并通过 Rust 构建。

## 验证边界

- 使用的是本地开发 `.app`，不是已签名、可分发安装包。
- Windows 尚未实际构建或运行；既有选型工作流只是文件，未在远端执行。
- 工具真实执行和会话切换已观察，但没有覆盖长时间生成时反复切换的全部时序，也未完成真实模型取消与关闭后再次打开非空会话的完整验收。
- Hive 团队派单、显式汇报、seen/outbox 和结构化 Agent 执行尚未连接；外部 Claude Code/Codex/Pi 接入、配置全量整合、后台重连、打包与体验打磨仍未完成。
- 没有运行 Hive 全量测试：本次使用发布包既有接口，未修改其核心协议；验证集中于新增 host、IPC、Pi 生命周期及原生界面。
