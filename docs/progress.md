# Hive Desktop 进度

更新：2026-11-07。

- [完整产品目标 #1](https://github.com/tt-a1i/hive-desktop/issues/1)：开放，涵盖 macOS / Windows 桌面、统一 Agent 聊天及现有 Hive 团队协作接入。完整应用尚未完成。
- [基础阶段 #2](https://github.com/tt-a1i/hive-desktop/issues/2)：开放，目标是实际 Hive Node sidecar、持久内置 Pi 会话和 GPUI 真实聊天链路。它是 #1 的子任务，完成后仍需集成结构化 Hive 派单与汇报及外部 Agent，不能据此声称团队协作已完成。

正式应用代码位于 `apps/desktop` 与 `packages/runtime`。macOS 已跑通实际工作区、Pi 会话、模型选择、真实 write/read 工具及最终回复；真实模型固定使用 Pi 原生 `openai/gpt-6.1-terra`，不经过 Magpie。详细范围见 [基础阶段收据](validation/2026-11-07-foundation.md)。

结构化内置 Pi 已接入 Hive 2.2.1 的执行器装配接缝，同一套 team CLI、HTTP、SQLite 派单和显式汇报已由两个真实 Terra 会话完成。桌面保存待投递消息，Pi 保存完成标记，Hive 保持唯一协作账本。原生界面可切换主控与成员，工具默认折叠并保留详情、错误和复制。真实恢复、取消和取消后继续已验证，见 [协作验收记录](validation/2026-11-07-structured-team.md)。

聊天来源已通过独立元数据区分，实际窗口验证了派单、恢复通知与用户粘贴的 Hive 格式文本；完整模型上下文保留。追加真实验证确认来源随会话恢复、取消后的 shell PID 退出及继续对话。外部 Codex app-server 已接入同一执行器：两个真实 Terra 会话完成主控/Worker 协作与整体恢复，桌面新建、模型选择、输入和来源展示已走查，支持范围与一次失败检查的模型选择违规审计见 [Codex 验证](validation/2026-11-07-codex.md)。

Windows CI 已在包含外部 Pi 桥接的提交 `9033d61` 通过正式应用与 Node runtime 构建，并上传 exe（[运行记录](https://github.com/tt-a1i/hive-desktop/actions/runs/42279508208)）。后续便携打包配置会捆绑官方 Node 与冻结锁文件的生产依赖，并检查 zip 解压后的真实 PTY/runtime；其结果单独记录，构建通过不代表 Windows 真机验收。

外部 Pi 已通过 RPC 与显式 Hive 扩展进入统一会话和团队运行时，保留自己的安装、认证、配置与扩展。两个原生 Terra 会话完成实际派单、文件执行、显式报告和核验，整体重开后保留原生身份，并用新的工具名与绑定继续调用 team。真实子进程退出检查覆盖 token/run 终止和待投递保留。见 [外部 Pi 团队证据](validation/2026-11-07-external-pi-team.md)。原始桥接阶段记录保留，但不再代表当前接入状态。

原生界面加入成员创建、改名、启动、停止和会话入口。成员默认停止，启动前显式指定模型；停止会等待实际 run 退出，旧请求结果不会覆盖新列表。原生操作验证了外部 Pi 成员聊天、改名、停止和重开，走查发现的表单遮挡与改名标题滞后均已修复。便携 macOS 开发包已实际验证包内 Node、真实 PTY、HTTP/SQLite 及应用窗口启动；外部 CLI 仍为用户安装，测试路径显式配置。包未签名或公证，不视为正式安装包。见 [原生界面与打包证据](validation/2026-11-07-portable-members.md)。

外部程序设置页已能保存 Pi/Codex 路径并显示实际来源；复用 Hive SQLite，优先级为环境变量 → 应用设置 → PATH，只对新启动进程生效，不改 Agent 全局默认。真实 RPC/SQLite 与原生设置页面均已验证，见 [设置证据](validation/2026-11-07-desktop-settings.md)。

下一步仍需完善外部 Agent 的结构化接入、成员管理和配置体验、任务与协作主页、应用后台重连及 macOS / Windows 安装包。Windows 尚未真机验收。基础阶段 #2 保持开放，完整目标 #1 不缩减为当前这条闭环。

每天 09:00 的当前聊天目标提醒已启用。持续目标保持完整范围；单个可运行聊天版本不代表产品完成。
