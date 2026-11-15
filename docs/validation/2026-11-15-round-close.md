# 本轮收尾与跟进交付

按用户先收尾后继续推进的要求，本记录在原收尾基础上补记跟进轮交付，并严格区分源码、受控协议、单账号真实模型、原生窗口与安装包验证边界。PR [#46](https://github.com/tt-a1i/hive-desktop/pull/46) 仍保持 Draft、未合并到 main，总目标 [#39](https://github.com/tt-a1i/hive-desktop/issues/39)、配置及 ACP [#52](https://github.com/tt-a1i/hive-desktop/issues/52)、文本接续 [#53](https://github.com/tt-a1i/hive-desktop/issues/53) 保持开放。跟进轮同步以当前提交重新构建 Mac Release 包、通过全部 5 项离线包内门禁，按用户授权替换 `/Applications/Hive Desktop.app` 并更新 `hive-desktop-macos-arm64-candidate.zip` 与 `.artifacts/current-package/receipt.json`（具体 SHA-256 与体积以 `receipt.json` 为准）。

## 实现与修复

固定 profile revision、ACP 生产接入和有来源的跨后端文本接续已接入现有聊天及 Hive 协作。文本接续只接受可信、已停止且无未结责任的来源，预览后创建新目标及私有工作树；被动创建和打开不启动 Agent，首次普通输入才交付资料。Hive 派单、seen/report/outbox 继续作为权威，不复制原生 session ID 或消费凭据。

本轮也修复了实际集成发现的问题：独立索引快照导致正常归档被拒绝；首次预约输入撤回后上下文错位；关闭时已接纳操作与新启动的生命周期冲突；延迟元数据读取越到被动目标；接续及 ACP 状态的无障碍文本缺失。检查覆盖并发改名、等值保存、ABA、写盘失败、结果不明和冷恢复，未以重建队列或改写任务账本绕过错误。

Windows 原生窗口还发现会话标题菜单不响应语义 Invoke。`fdf42da` 将键盘/辅助功能激活交给现有 Popover Confirm 动作，保留鼠标点击和双击行为；改动仅在应用标题按钮，没有修改全局组件库。

## Mac 证据

- Rust：106 项通过，1 项明确忽略的子进程夹具。
- 冻结 `portable-2e339b5`：官方 Node 22.23.3 校验、内部链接、core/真实 PTY、Codex 生命周期及 **50 项包内功能检查通过**。
- 同一冻结运行时的 `portable-fdf42da`：202 个 dist 文件及 Node 二进制逐字节相同，依赖来自同一次冻结部署；更新后的原生程序通过上述 Rust 检查、core/真实 PTY 和实际 AX 菜单/重命名/保存/再次打开。没有把旧原生程序的界面证据冒充新程序验证。
- 实际隔离窗口完成 ACP 来源到受控 Codex 目标的预览、确认、私有工作树、被动零启动、首次输入和冷重开后的第二次输入。对端两次实际请求中，来源事实只出现一次；Root 独立只读核对通过。GUI 没有操作前 Git index 字节 hash，因此该细项只由独立后端夹具证明。

本地证据位于 `.artifacts/functional-parity/`：`package-handoff-2e339b5-functional.json`、`package-menu-fdf42da-runtime-equivalence.json`、`ui-qa/handoff-checks/root-independent-receipt.json` 及 `ui-qa/menu-checks/`。隔离窗口、后台和受控对端均已退出。

## Windows 证据

最终复验正在核对原始 `fdf42da` ZIP。原运行 [43685259345](https://github.com/tt-a1i/hive-desktop/actions/runs/43685259345) 已完成源码、进程监督、官方 CLI、原生编译、ZIP、解包 core/真实 PTY 和 Codex 生命周期；47 项功能检查在最后一个场景被整组 10 分钟限时中止，不能把原运行计为完整通过。

复验 [43688926660](https://github.com/tt-a1i/hive-desktop/actions/runs/43688926660) **通过**：检查脚本提交 `da71603` 下载同一原始 ZIP，47 项包内功能检查全部退出 0，实际模型请求为零，随后原生 UIA 窗口检查通过。整组预算调整为 15 分钟，三个多次冷启动场景仍各限 180 秒；断言未删减，没有重新编译或替换二进制。此复验跳过重复编译、进程监督和官方 CLI 工作，相关证据来自原始 `fdf42da` 运行，不能把原运行的失败状态改称通过。

原生窗口实际完成设置保存及固定配置版本保持、主题、成员会话及草稿/工具展开恢复、原主控查看真实已汇报派单、任务冲突检查和保存、标题菜单重命名、置顶/取消置顶，以及后台恢复后的会话选择。测试使用隔离数据和受控协议，没有进行账号登录或真实模型推理。

原包 SHA256 为 `40AEFB9C33211403F34ABD62950A4D7053F39AB3470ECE7F812AE5C52D9E36FB`，原提交 `fdf42daaf6033e170caba365caf1905ae3d72ae9`，复验夹具提交 `da71603ccbe6451f9318257df5e9abc7f88c523b`。Root 已核对来源、47 项结果、日志及窗口截图。证据在 `.artifacts/functional-parity/windows-final-replay-da71603/`，包括 `replay-provenance.json`、`replay-functional-receipt.json`、`windows-replay.log`、14 张截图及 `root-verification.json`。

POSIX mode/shebang 夹具不等于 Windows ACL 或同样的零进程证据。Windows 的受控原生窗口检查也不等于人工覆盖所有新入口。

## 跟进轮修复、ACP 闭环与当前边界

在前述收尾记录基础上，跟进轮新增完成以下验证与收口工作：

- **工作树与缓存清理**：移除 45 个遗留 Git worktree，清理 `.artifacts` 临时构建缓存（约 77 GB 降至约 1.7 GB），完整保留 `current-package/`、`hive-desktop-macos-arm64-candidate.zip`、`acp-failure-diagnosis/`、`native-acp-collaboration-prep/`、`acp-vendor-plan/` 与 `windows-final-replay-da71603/` 等历史诊断与收据。
- **界面文案、菜单与接续自动刷新修复**：补齐 `apps/desktop/src/locales/en.json`（0 缺失 key）；本地化默认 Pi 配置名称（`Built-in Pi` / `External Pi` 与中文对应文案）；收紧 `session-title-menu` 宽度并保留 `Confirm` 操作；在 `packages/runtime/src/chats.ts` 与桌面端会话状态链路中补齐文本接续首次发送结算/失败后的目标 `chat.open` 状态自动刷新；更新 `.github/workflows/windows-build.yml` 将功能检查超时放宽至 25 分钟并在 `replay` 阶段接入 `check-windows-terminal.mjs` 终端冒烟与日志上传（Windows workflow 当前仅完成 YAML 核对与本地 `cargo check --target x86_64-pc-windows-msvc` 交叉编译检查，尚待新一轮 GitHub Actions Windows 实跑）。
- **单账号真实 ACP 完整协作闭环（`run-heBSEm`）**：根因确认为 `packages/runtime/src/agents/acp-team-stdio.ts` 的 `tools/list` 缺少 MCP `annotations`，导致 Codex `0.169.0` 发出 `mcpServer/elicitation/request`（`turnId: null`）触发旧守卫拒绝；补充显式 `annotations` 并启用 `acp-collaboration-guard-v2.cjs`、`runtime-observer-v2.mjs` 与 `run-live-v2.mjs` 后，在单已有账号范围下完成三阶段闭环（总计 `starts: 4`、`steers: 0`、`accountReadFalseCalls: 4`、`allOwnedProcessesExited: true`、`elapsedMs: 64142`）：Native Codex Orchestrator（`gpt-6.1-terra` / `openai`）派单给官方 `@agentclientprotocol/codex-acp@2.6.0` Worker（`gpt-6.1-terra` / `openai`），Worker 调用 `mcp.hive-team.team` 的 `messages`（`seen: 0`）与 `report`（`scopedApprovals: []`）并写出 55 字节产物，Orchestrator 于 `reported_at` 后独立校验字节与 SHA-256，随后被动冷重开（`passive: true`、0 推理）及独立 `session/load` 零推理探针（`sessionNewCalls: 0`、`turnStartCalls: 0`、`turnSteerCalls: 0`）与两轮隐私扫描全部通过。历史失败/中断运行 `run-woSexD`、`run-2ZriPb`、`run-u6M7ni`、`run-UNZcHi` 及 3397 字节原始 spool 队列证据继续原样保留。

**保留的边界**：`run-heBSEm` 明确仅覆盖单已有账号（`noTwoAccountClaim: true`、`noGuiOrWindowsClaim: true`），不宣称双账号或 UI/Windows 原生协作；多配置混合协作 `run-VY56qr` 仍保持未通过并跟踪于 #52。受控协议接续不等于真实多后端模型接续；进行中派单迁移、原地回退、桌面更新、语音输入、隔离 HTML、外部密码库、Claude/Antigravity 真账号实测与 Pi 登录重试仍保持开放。当前线下验证（`PI_OFFLINE=1`）已复验 `pnpm check`、`pnpm build:runtime`、Rust 106 项核心测试 + 14 项 `process_supervisor_check` 测试、`check:session-archives` 及 Windows 交叉编译检查，完整打包交付与替换结果同步记录于 `.artifacts/current-package/receipt.json`。

完整细节见[配置与 ACP](2026-11-15-agent-profiles-acp.md)、[文本接续](2026-11-15-session-handoffs.md)及[原生历史](2026-11-15-native-history-live.md)。
