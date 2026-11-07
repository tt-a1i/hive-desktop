# 外部程序设置验证

2026-11-07，macOS。应用现在可以保存外部 Pi/Codex 的可执行文件路径，不必依赖手工环境变量。设置写入现有 Hive SQLite app-state 的 `desktop_executables` 字符串值，不修改任何 Agent 的全局配置或默认模型。

优先级为环境变量、应用设置、PATH。页面同时显示保存值与实际来源；空值恢复 PATH。保存只影响之后创建的外部进程，正在运行的会话不被替换。

## 验证

- `checks/desktop-settings.ts` 运行真实 Hive HTTP/SQLite 与外部 Pi RPC，验证保存、完整运行时重启、空值清除、环境覆盖、空白/控制字符/过长输入拒绝，持久化失败后保留旧值。真实已存在的 Pi handle 在更新路径后仍保持；新 handle 会使用更新后的路径，失效路径报错，修正后可以打开。无模型请求。收据 `.artifacts/desktop-settings-H7yZ9m/receipt.json`。
- 编译后的 `main.js` 实际 JSON-RPC 子进程验证了 `desktop.settings.get/update`、无效请求错误、关闭和重启后读取保存值。无模型请求。收据 `.artifacts/desktop-settings-rpc-YSc7vA/receipt.json`。
- 原生 CUA 在应用包中输入并保存实际 Pi/Codex 路径，退出后重开仍保留。由于 macOS Launch Services 缓存旧测试包的环境变量，另用新 bundle identity 及相同独立数据目录验证：不设置任何 Agent/Node/runtime 路径环境变量，页面明确显示两个实际路径均来自“应用设置”，外部 Pi 原有会话可打开。测试应用位于 `.artifacts/portable-settings-darwin-arm64/Hive Settings Proof.app`。
- 独立审查确认数据来源优先级、新 handle 才读取配置、失败传播、页面 generation 防过期响应；无新增阻断项，范围评分 B。runtime typecheck/build、Rust build/fmt 与 diff 检查通过。

本页仅覆盖可执行程序设置，不代表完整模型提供商、角色模板、命令预设、工作流等配置已经迁移。Windows 原生页面和可执行发现仍需实际平台验证。没有新增模型回合，也未删除或跳过既有失败测试。

## 附：成员原生模型目录

`agent.models` 根据指定工作区和实际后端读取 Pi SDK、Codex app-server 或外部 Pi RPC 的目录。临时会话只用于读取目录，结束时等待释放原生进程并删除本次创建的临时目录；不建立 Hive 聊天、run、token 或派单，不发送 prompt，也不改变全局默认模型。配置来源沿用上面的可执行程序设置。

真实三后端检查与独立复查均通过，收据分别为 `.artifacts/agent-models-pjRWdn/receipt.json` 和 `.artifacts/agent-models-8uVrzg/receipt.json`。编译后 JSON-RPC 收据为 `.artifacts/agent-models-rpc-D7kfeY/receipt.json`。检查包含原生 Terra 可用、临时进程退出、失效路径、无效请求及 Pi settings 哈希不变。并发同键请求合并与 close 期间拒绝新请求已审查源码，未单独做并发运行验证。

原生成员页按成员 ID 与后端绑定选择，实际外部 Pi 目录搜索 `gpt-6.1-terra` 后选择 `openai` 项，页面显示该精确模型并启用启动按钮，成员仍停止；本次只验证选择，没有再次启动或调用模型。目录也可能列出用户原有的 Magpie 配置，列出不代表已连接或发送请求。

## 附：会话草稿切换

后续原生走查发现所有会话共用输入框，切换会话会把未发送内容带到另一个成员。现按会话/无会话工作区分别保存当前进程内的草稿，发送只清对应草稿，发送失败保留原会话的内容。草稿尚未跨应用退出持久化。

第一版使用嵌套窗口 handle 更新，在真实按钮事件中未生效；没有将构建通过当验收。修复后记录实际输入归属与目标，在 render 使用当前 Window 更新。CUA 实测两个既有外部 Pi 会话分别写入 A/B，往返恢复各自完整草稿；新增无会话工作区写入 C，往返项目也分别恢复 B/C。只切换和输入，没有点击发送或调用模型。两个验证会话均明确选为原生 Terra。

## 附：角色模板与成员职责复用

原生角色模板页复用 Hive 2.2.1 的 `role-templates` CRUD。内置模板只读，自定义模板可创建、编辑和删除；完整 PATCH 保留原 command、args、env，当前页面仅编辑名称、角色和职责。成员表单可选择 Worker 模板，填入角色及多行职责，然后再显式创建停止状态的成员；主控模板不会出现在 Worker 选择列表。

真实 HTTP/SQLite 收据 `.artifacts/role-template-api-proof/receipt.json` 覆盖创建、完整更新保留额外字段、按模板建停止成员、后台重启持久化和删除。没有启动 Agent 或发出模型请求。独立源码复核检查了安装版契约、失败保留表单、旧响应隔离和具体删除对象，无阻断项，范围评分 B。

原生 CUA 在 `.artifacts/role-template-ui/Hive Role Templates.app` 验证了内置只读、中文多行模板创建和编辑回填、Worker 模板选择和职责填充，最终创建 `template-ui-reviewer` 并显示“审查 · 已停止 · Pi”，尚未选模型，启动按钮禁用。清空模板名称后保存会显示“请输入模板名称”，职责不丢失。删除确认显示具体模板名称；此次窗口走查保留自定义模板，没有执行最终删除。中文通过粘贴输入，尚未验收真实输入法组合输入。

补充走查：应用退出重开后模板仍在；删除确认旁的“取消”实际恢复普通删除按钮并保留模板。
# Navigation and setting drafts (2026-11-07)

The macOS native navigation now has one four-backend creation menu and one settings entry, with external-program and role-template tabs. Workspace and session entries share a scrollable area; team, collaboration and settings remain outside that list. At widths below 820 logical pixels, navigation occupies the content area and closes when a destination is selected. Chat text and the composer use a centered maximum width.

Actual CUA in `.artifacts/navigation-ui-proof/Hive Desktop.app`, using the isolated pagination fixture state, verified keyboard creation of a builtin Pi chat and pointer creation of Codex, external Pi and Claude chats. No prompt was sent. A Chinese multiline Pi draft survived switching through all three other backends and returning; its session model was explicitly changed to native `openai/gpt-6.1-terra`, without changing global defaults.

Independent review found that switching settings tabs discarded unsaved executable edits. Reopening now retains existing inputs; only the initial load, successful save or explicit reload fills persisted values. CUA entered three different unsaved paths, switched to role templates and back, verified all three survived, then used explicit reload to restore saved values. The actual native CLI paths were subsequently saved in this test data directory. Initial-load inputs are disabled until saved settings arrive.

The window was actually resized from 1180×853 to 760×583 logical pixels. The session list scrolled to the last chat, opening it closed navigation, and its draft remained visible. Settings scrolled to the save/reload buttons; member role-template selection filled the existing Chinese multiline responsibility; collaboration Markdown and dispatch filters remained reachable. Existing pagination rows are synthetic, not additional model work. Native Windows interaction, IME composition and the final visual quality target remain unverified.

## 全局自动组队引导

设置页复用 Hive 2.2.1 的 GET/PUT `/api/settings/team-autostaff`，显示全局状态、保存和错误反馈。开关改变主控指导语，不直接创建或删除成员。独立真实 HTTP/SQLite 检查覆盖默认开启、关闭、错误类型请求 400 且状态不变、开启及两次重启持久化。

macOS 窗口实际关闭、重新读取、返回后再打开设置并重新开启；未保存的 Codex 路径草稿保持不变，最终 SQLite `team.autostaff` 为 true。使用独立桌面数据目录，未改用户项目或认证。证据见 [Pi 与设置窗口收据](../../.artifacts/pi-interactions-ui-proof/receipt.json)。本轮没有验证 Windows 设置窗口；可运行 `pnpm --filter @hive-desktop/runtime check:team-autostaff` 重验后端契约。
