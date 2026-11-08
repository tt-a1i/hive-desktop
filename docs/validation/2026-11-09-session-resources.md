# 会话资源浏览（#14）

对应 [#14](https://github.com/tt-a1i/hive-desktop/issues/14)。在现有 Pi 命令选择器上增加“浏览资源”，读取同一条 `chat.commands`，不增加扫描器、安装器或资源 store。

## 行为与参考

用户可以按技能、提示模板、扩展命令筛选，搜索完整说明、名称、来源及路径；查看原生返回的用户/项目/临时范围，复制命令或来源路径、只读查看来源文件、在文件管理器定位。补入草稿复用现有选择逻辑，保留参数和多行正文，不发送或执行。

2026-11-08 直接查看本机 Cindy CN 0.1.128 的设置、工具、插件和本地技能详情，参考其完整说明、范围及来源文件展示。没有修改其设置、安装资源或发送消息；这轮只实现会话实际加载资源的浏览，不等于复制 Cindy 全部插件/技能管理功能。

内置 Pi 使用 SDK 实际目录，外部 Pi 使用原始 CLI get_commands。后者原生返回 builtin:llama.cpp、builtin:mcp，内置 SDK 此夹具没有返回；如实展示差异，内置/非本地路径没有文件预览按钮。Codex 与 Claude 不显示 Pi 专属入口。重新读取仅取已加载目录，不承诺 reload 磁盘新安装资源。

面板绑定会话、打开请求及连接代次。加载返回、补入草稿、来源预览和定位前核对当前绑定；后台断开时保留旧目录并提示重新打开，拒绝旧面板动作。复制只使用已显示文本。普通汇报产物仍受原有工作区路径边界约束；原生资源预览仅对用户点击的目录来源文件按父目录调用既有规范路径/大小/类型受限只读器。

## 实际验证

所有数据位于 `.artifacts/session-resources/`，使用真实 Hive HTTP/SQLite、Pi SDK 及原样外部 Pi 1.4.0；模型固定为 openai/gpt-6.1-terra，离线且无厂商请求。夹具自有用户技能、项目技能、项目外目录软链接、提示模板及 handled 扩展命令。Pi 原生加载器也读取本机已有 `~/.agents/skills`，只读；本轮并非完全隔离全局资源的测试。

- 实际目录断言：内置65项、外部67项，均包含四种自有资源；另一个项目没有继承本项目技能，但保留用户技能。来源路径由原生接口返回，见 `catalog-receipt.json`。
- Mac 原生源码窗口：中文完整说明搜索“资源穿透”只返回自有用户技能；只读预览看到实际 `GLOBAL_RESOURCE_FILE` 文件内容，位于工作区之外。截图 `sdk-browser.png`、`sdk-preview.png`。
- 原草稿 `/old 保留参数\n第二行正文` 选择模板后成为 `/resource-template 保留参数\n第二行正文`。外部 Pi 的项目软链接来源预览看到实际 `LINKED_RESOURCE_FILE`；缩窄到约642逻辑像素宽后正文、路径和预览操作仍可达。
- 后台直接子进程退出后，旧面板重新读取出现明确过期提示，目录保留；点击补入不关闭面板，外部草稿保持 `/skill:linked-resource`。重开应用后可以重新打开实际67项目录、读取并预览软链接来源。
- 退出后从实际草稿文件核对上述两份内容，SQLite 的 messages/dispatches/agent_runs/report_outbox 均为0；两轮 RPC 日志没有 chat.prompt。`ui-receipt.json` 保存断言。
- 最后一轮源码应用正常退出：code0、signal null，见 `narrow-exit.json`。前一轮人为杀后台后的父进程已退出，但该终端退出收据丢失，日志保留 cleanup-not-confirmed 告警，不能宣称该轮退出清理成功。

首次窗口打开曾因创建子 entity 时重入 HiveApp 更新而 SIGABRT。现改为 GPUI defer_in 后读取父状态；后续两轮实际打开均正常。失败日志 `first-ui.log` 保留。系统截图接口数次报 ScreenCaptureKit 错误或返回旧动画帧，未将这些帧当作产品布局证据；没有保存成功的 stale-connection.png，过期保护依据实际窗口观察与草稿落盘断言。

## 检查与边界

`cargo build --manifest-path apps/desktop/Cargo.toml`、`cargo check --manifest-path apps/desktop/Cargo.toml` 和 `git diff --check` 通过。现有 `cargo test --manifest-path apps/desktop/Cargo.toml file_viewer -- --nocapture`：1通过、28过滤；没有新增 UI 镜像测试，没有修改 runtime、Hive 协议/schema 或删除既有测试。纯原生 UI 和已有只读器复用未跑无关全量运行时测试。

Windows 原生、正常连接下 RPC 读失败保留目录、键盘/输入法完整矩阵及文件管理器定位的逐项操作仍未验收；连接失效已实际验证，不用它替代所有读取错误。较完整源码验证使用有界复用的小应用槽；随后 current ZIP 已纳入1acd805，包内实际目录、来源预览、原草稿及正常退出通过，见[当前开发包](2026-11-08-current-package.md)。#14 保持开放，完整 Hive Desktop 目标继续推进。
