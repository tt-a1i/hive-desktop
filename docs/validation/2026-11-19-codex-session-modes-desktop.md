# Codex 会话模式与权限的桌面验收

桌面会区分当前原生回合已生效的设置与下回合将使用的设置。回合结束时，只对仍待生效的设置重新读取后台结果。界面从原生确认结果更新状态，设置菜单和输入框都提供辅助功能标签。会话或后台连接发生切换时，上一会话的迟到结果仍会被丢弃。

## macOS 实际操作

使用本地临时签名的 arm64 开发包、包内 Node、Codex CLI 0.169.0 和隔离工作区。在当时的对话中固定模型为 `openai/gpt-6.1-terra`。通过窗口按钮选择模式与权限，进行两次真实回合。

| 会话设置 | 原生协议和结果 |
| --- | --- |
| 计划、只读 | 原生请求为 `plan`、`readOnly`、`on-request`，网络访问关闭。模型执行只读命令并在计划中准确引用测试文件的标记。回合结束后，输入框状态自动更新为计划和只读已生效。 |
| 默认协作、工作区写入 | 窗口先显示下一回合选项，保留现行计划和只读状态。第二个回合使用 `default`、`workspaceWrite`、`on-request`，网络访问关闭。模型写入单独的测试文件，逐字节核对通过。回合结束后，窗口自动显示新的两项已生效状态。 |
| 切换至内置 Pi 再返回 | 内置 Pi 显示不支持会话模式，无 Codex 权限选项。返回 Codex 仍显示原来的两项已生效设置。 |
| 正常退出和冷启动 | 历史保留两个实际回合。原生 `thread/resume` 使用同一线程 ID、工作区写入和 `on-request`。输入框恢复两个下回合选项，冷启动没有推理请求。两次原生退出均有后台确认，测试拥有的 Codex 进程正常退出。 |

原生观察器仅允许上述两个真实回合，核对模型、会话、协作模式、权限、审批策略与回合状态。完全访问使用既有受控协议检查。此次没有调用 Claude 或 Antigravity 模型、尝试 Pi 登录或修改全局模型和账号配置。

## 检查和证据

- `pnpm --filter @hive-desktop/runtime check:session-modes` 四组通过，包含两轴持久化、忙碌保护和失败恢复，Claude 仅使用受控对端。
- Rust 116 项通过，1 项子进程入口按设计忽略。Windows x86_64 MSVC 交叉检查通过，尚缺对应的原生窗口走查。
- 本地包通过严格签名验证和包内 Node、PTY、SQLite、Hive HTTP 与退出冒烟检查。没有发布。
- 证据位于 `.artifacts/native-session-mode-aVDXJw/`，含 `receipt.json`、实际原生回合、冷启动及桌面退出日志和 `package-smoke.log`。

## 后台回执与 ACP 原生选项

GPT-6.2 Sol 复现了 Codex 冷恢复会忽略原生沙箱和审批回执。现在显式保存的权限先核对原生沙箱类型和审批策略，再允许发送。只有回执与权限预设完全一致时才显示已生效。若线程沿用了额外的可写目录或网络设置，界面继续显示下回合待生效，下回合发送完整权限参数。回合请求须返回有效的原生回合 ID 才更新生效状态。若已保存的模式被后端取消支持，发送仍被阻止，原会话的历史查看及模式修复入口还需补齐。

ACP v1 按当前原生会话公开的选项显示模式。通用对端可以公开一个模式选择器，也支持旧的 `session/set_mode`。官方 Codex ACP 公开协作模式和权限预设时，桌面分别显示两组。设置更改核对原生回执，随后的原生通知可以更新生效状态。冷连接读取 `session/load` 的当前状态，不重放设置。只读浏览不建立 ACP 连接，显式连接也不会接管同一成员已有的活跃会话。[ACP v1 会话模式](https://agentclientprotocol.com/protocol/v1/session-modes) · [ACP v1 配置选项](https://agentclientprotocol.com/protocol/v1/session-config-options)

使用实际 Codex 0.169.0 核对了三种权限的线程启动回执，以及同一测试线程的冷恢复回执。实际官方 `@agentclientprotocol/codex-acp` 2.6.0 公开了两组选项，并确认计划和只读。原生方法记录无推理请求。另用真实 Hive、SQLite 和受控 ACP 对端验证了连接后新投递能正常执行、另一活跃会话不被替换、两种模式协议、冷读不重放、拒绝错误回执，以及同一批原生帧里的后续更新优先于先前回执。

从新版签名 ZIP 解包后，在 macOS 原生窗口通过“连接 ACP 以设置模式”打开官方 Codex ACP，分别选择 Plan 和 Read-only，两个控件都报告当前生效。用受控 ACP 对端再完成选择、关闭、冷重开并重新连接，确认 `session/load` 使用同一原生会话，未重发设置。三次正常退出均收到后台确认，测试进程全部退出。包内 Codex 适配器还用隔离目录验证三种权限的原生启动回执，并用上一批测试线程的副本确认冷恢复时的默认协作和工作区写入已生效。以上检查均未发起推理。

源码为 `c855e8f` 和 `41a11be`。127 项 Rust 测试通过，1 项子进程入口按设计忽略。模式、Codex、ACP 注册及协作检查和包内 Node、PTY、Hive HTTP/SQLite 冒烟通过。Windows x86_64 MSVC 交叉检查通过，Windows 原生操作仍待验收。未发布。

macOS ZIP SHA-256 为 `78d1c7c074d54e2dd333fdabca0715792e29030a5cfe5f5a3700f4c5fcbbbc7a`。签名及归档清单已核对。证据位于 `.artifacts/native-mode-verification-PdWMNg/`，原生窗口见 `window-official-2agDUr/receipt.json` 和 `window-controlled-DJ3fbP/receipt.json`，包内 Codex 回执见 `packaged-codex-o0GpXz/receipt.json`。
