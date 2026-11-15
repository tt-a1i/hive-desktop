# 本轮收尾与暂停

用户要求本轮已有工作收尾后暂停，不继续新增功能。本记录区分源码、受控协议、真实模型、原生窗口与安装包。PR [#46](https://github.com/tt-a1i/hive-desktop/pull/46) 保持 Draft，总目标 [#39](https://github.com/tt-a1i/hive-desktop/issues/39)、配置及 ACP [#52](https://github.com/tt-a1i/hive-desktop/issues/52)、文本接续 [#53](https://github.com/tt-a1i/hive-desktop/issues/53) 保持开放。没有合并到 main，也没有推广或替换用户安装实例；current/candidate 保持 `837abd1`，回退包保留。

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

## 保留的边界

真实原生 Codex Terra 的历史分叉已有独立通过记录；真实多配置及 Codex/ACP 混合协作整体仍未通过。最新混合用例在 2 次 start、0 steer 后被测试守卫的未知原生请求中断，没有完成 seen/report、产物核验或非空冷恢复。具体请求方法未记录，不能猜测根因。3397 字节原队列及两套备份 hash 一致，原回合已中止；没有重跑模型、补写 SQL 或把队列伪装成可原地恢复。

文本接续首次提交后需手动刷新状态，尚未完善自动状态刷新；受控协议接续不等于真实厂商模型接续。进行中派单迁移、原地回退、桌面更新、语音输入、隔离 HTML、密码库及其余完整目标保持开放。Claude/Antigravity 不做账号或真实模型验证，Pi 登录失败未重试，也未修改全局账号或模型配置。

完整细节见[配置与 ACP](2026-11-15-agent-profiles-acp.md)、[文本接续](2026-11-15-session-handoffs.md)及[原生历史](2026-11-15-native-history-live.md)。历史失败日志和原始收据保留；本轮已清理确认不再使用的项目构建缓存，实际释放约 1.06 GB，源码、回退包和证据未删除。
