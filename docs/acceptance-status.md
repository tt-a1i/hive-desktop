# 完整目标的当前验收状态

## 当前开发包 992aae6 与清理（2026-11-13）

Sidecar 协议帧完整排空与异常 `shutdown` 在打开 `stdin` 上即时退出、会话索引原子刷盘、多路关闭错误 `AggregateError` 保留，以及全设置面板（外观/聊天/团队/内置 Pi）、设置搜索与会话搜索/归档弹窗的 `Viewport` + `Reveal` 窄窗键盘显露闭环（`992aae6`）已入 current/candidate、DMG 和 `/Applications/Hive Desktop.app`（ZIP SHA-256 `1c04393500e52e1b353866075fb20d6a9471457f9223091c8bda652072ae6d8f`）。精确 9 项变更、28,869 项清单/链接与 DMG/安装核对通过；包内四组正式包检查、协议排空与异常退出检查（`18 ms` / `13 ms`）、d43f55e 真实三会话 40 条历史冷读+角色搜索→Rust，以及原生 macOS 中英文启动恢复与英文深色 `20 px` 欢迎页/中文 Terra 冷读正常退出（`exit 0`，子进程清空）全部通过，零新厂商推理。`62e6a6e` 保留直接可运行回退，退役 `4b3135d` 解压树并清理 Rust 中间构建缓存与重复产物约 6.68 GB（6.23 GiB），见[包内证据与边界](validation/2026-11-13-sidecar-wire-and-settings-scroll-package.md)。下文待入包/当前为各阶段历史状态，完整目标继续。

## Sidecar 协议帧排空、异常关闭防卡死与全设置/会话搜索视口焦点闭环（2026-11-13，源码）

修复 `main.ts` 在并发流水线请求及异常 `shutdown` 时未关闭 `input` 或未等待 `protocolWrite` 回调导致挂死满 `3,000 ms` 或响应帧截断的问题，补齐 `ChatIndex` 原子落盘 `flush: true` 与 `StructuredLifecycle` / `DesktopSettings` 多路关闭错误 `AggregateError` 保留；补齐设置面板剩余纵向滚动分区、设置搜索与会话搜索/归档弹窗的 `Viewport` + `Reveal` + `.vertical_scrollbar` 键盘显露闭环及 `focus_scroll` 几何单测。五组协议与聚合实测、六组零推理运行时回归、`LOCALE_AUDIT_OK`（1,301 键）、Rust 36 项单元与 13 项进程监督测试、两项 Windows MSVC 交叉检查与 macOS release 构建通过，见[实证与边界](validation/2026-11-13-sidecar-wire-and-settings-scroll.md)。已纳入 `992aae6` 开发包，完整目标继续。

## 当前开发包 62e6a6e 与清理（2026-11-13）

运行时三阶段关闭隔离、三后端启动期握手即时取消与多句柄排空（`2abce78`），以及桌面跨边界自有诊断闭合本地化、会话资源浏览窄窗 `Viewport` + `Reveal` 键盘显露与交互/预览/导出/终端表面打磨（`62e6a6e`）已入 current/candidate、DMG 和 `/Applications/Hive Desktop.app`（ZIP SHA-256 `bfb4d71e551ec54c65cd83f0533af096793e62ff5433f7b53c3c8c6b5539c80d`）。精确 23 项变更、28,869 项清单/链接与 DMG/安装核对通过；包内四组正式包检查、关闭隔离与三后端启动取消检查、d43f55e 真实三会话 40 条历史冷读+角色搜索→Rust，以及原生 macOS 中英文启动恢复与欢迎页/Terra 冷读正常退出（`exit 0`，子进程清空）全部通过，零新厂商推理。`4b3135d` 保留直接可运行回退，退役 `9e6a44a` 解压树并清理重复/临时构建产物约 1.41 GB（1.31 GiB），见[包内证据与边界](validation/2026-11-13-shutdown-cancel-surfaces-package.md)。下文待入包/当前为各阶段历史状态，完整目标继续。

## 桌面端闭合错误本地化闭环、会话资源窄窗显露与交互/预览表面打磨（2026-11-13，源码）

补齐运行时跨边界剩余自有错误包装（`"登录失败"`、Codex 初始化失败首行引导、Claude 写入丢失提示、队列诊断落盘告警）与图片查看器自有错误的英文闭合本地化，且非自有 OS/SDK/CLI 原始多行诊断逐字节保持不变；为会话资源浏览弹窗（`#14` / `#20`）接入 `Viewport` + `Reveal` 窄窗键盘焦点显露、滚动条与等宽可选路径；统一 Codex/Claude/外部 Pi 交互卡片（等宽可选命令/Diff/JSON 输入）、文件预览、Markdown 导出（`#16`）与终端视口的视觉层次。`LOCALE_AUDIT_OK`（1,301 键）、Rust 35 项单元与 13 项进程监督测试、两项 Windows MSVC 交叉检查与 macOS release 构建通过，见[实证与边界](validation/2026-11-13-desktop-diagnostic-localization-and-surfaces.md)。即将连同 `2abce78` 纳入新开发包并执行包内与原生窗口核验，完整目标继续。

## 运行时关闭隔离、启动期握手即时取消与开发态远程命令路径（2026-11-13，源码）

复现并修复 `AgentModels.close()` 与 `Chats.close()` 无法取消在途 `Codex` / `External Pi` / `Claude` 启动期握手（从 >30 秒卡死收敛至 `204 ms` / `384 ms` 回收子进程，同时严格保持 `removeWorkspace` 原有排空语义）、`Chats.close()` 与 `TerminalManager` 因 `Promise.all` 快错提前返回导致兄弟句柄未排空、`createDesktopRuntime().close()` 缺少子系统隔离且覆盖前序异常，以及 `remote-commands.ts` 在 `src/` 开发态指向不存在的 `src/remote-cli.js`。四组前后对照、d43f55e 真实三会话副本冷读及九组零推理运行时回归通过，见[实证与边界](validation/2026-11-13-runtime-shutdown-and-startup-cancellation.md)。无厂商推理/面板操作/正式测试改动，待随桌面端改进一并纳入下一开发包，完整目标继续。

## 当前开发包 4b3135d 与清理（2026-11-13）

后台停止清理串行化、异常停止无泄漏回收与 Codex 账号探测即时取消（`cd32e9d`），以及桌面过期 `Runtime` 启动句柄回收、协作重连保留、记忆/角色面板 `Viewport` + `Reveal` 窄窗键盘显露、1,301 项双语闭合本地化与协作/成员/记忆/角色/工作流视觉层次打磨（`4b3135d`）已入 current/candidate、DMG 和 `/Applications/Hive Desktop.app`（ZIP SHA-256 `19e0ce4df703df3f840d44861d1409888c15ccf3b59a644f26148e9d77980d05`）。精确 13 项变更、28,869 项清单/链接与 DMG/安装核对通过；包内四组正式包检查、并发清理与探测取消检查、d43f55e 真实三会话 40 条历史冷读+角色搜索→Rust，以及原生 macOS 中英文启动恢复与欢迎页/Terra 冷读正常退出（`exit 0`，子进程清空）全部通过，零新厂商推理。`9e6a44a` 保留直接可运行回退，退役 `0a388b3` 解压树并清理重复/临时构建产物约 1.41 GB（1.31 GiB），见[包内证据与边界](validation/2026-11-13-teardown-panels-craft-package.md)。下文待入包/当前为各阶段历史状态，完整目标继续。

## 桌面后台生命周期竞态、次级面板焦点显露与界面层次打磨（2026-11-13，源码）

修复 `start_backend` 过期 `Runtime` 启动句柄未回收、后台重连重置 `collaboration.visible` 与断线静默清除在途任务保存状态的问题；修复 `members/view.rs` 模型读取错误的后端名称映射与双语闭合本地化，补齐远程设置、Pi 账号与对话导出的自有错误本地化（`locales/en.json` 达 1,301 条唯一键，覆盖率 100%）；为记忆与角色模板页接入 `Viewport` + `Reveal` 窄窗键盘显露，并融合 Beluga、T3 Code 与 Cindy 统一协作、成员、记忆、角色、工作流面板的 `960px` 居中容器、`Color::Card` 卡片与语义状态胶囊（推进 `#17`、`#20` 与整体体验）。Rust 35 项单元测试、13 项进程树监督检查、两项 Windows MSVC 交叉检查及 macOS release 构建通过，见[实证与边界](validation/2026-11-13-desktop-panels-hardening-and-craft.md)。随即连同 `cd32e9d` 纳入新开发包并执行包内与原生窗口核验，完整目标继续。

## 会话停止清理竞态、Codex 账号探测取消与显示日志原子写防护（2026-11-13，源码）

复现并修复 `Chats.stopLive()` 未挂载 `chat.cleanup` 导致释放期并发请求抛 `"Codex session is disposed"`、`stopAgentRun()` 在 `abort` 抛错且传输层存活时泄漏子进程/令牌/`running` 状态、`probeCodexAccount()` 在 `initialize` 挂起时需等待 `5,007 ms` 超时才响应取消，以及 `CodexProjection.save()` 重命名失败残留 `.tmp` 与 `team-tool.ts` 启动报错泄漏 `abort` 监听器。五组前后对照、d43f55e 真实三会话副本冷读与 `check:codex`、`check:codex-account`、`check:workflow-shutdown`、`check:workspace-removal` 等回归通过，见[实证与边界](validation/2026-11-13-runtime-stop-and-probe-hardening.md)。无厂商推理/面板操作/正式测试改动，待随桌面端改进一并纳入下一开发包，完整目标继续。

## 当前开发包 9e6a44a 与项目存储治理（2026-11-13）

暂停队列新输入保持与可执行程序/元数据防护（`271082f`）、跨层文案本地化与双语会话搜索（`bf44928`）及引导/输入框/协作卡片界面打磨（`9e6a44a`）已入 current/candidate、DMG 和 `/Applications/Hive Desktop.app`（ZIP SHA-256 `1924b6472632ae1657d719028fdb857fac15a1250158f620eebcafc2c040fb5b`）。精确 9 项变更、28,869 项清单/链接与 DMG/安装核对通过；包内四组正式包检查、队列保持与双语别名检查、d43f55e 真实三会话 40 条历史冷读+角色搜索→Rust，以及原生 macOS 中英文启动恢复与欢迎页/Terra 冷读正常退出（`exit 0`，子进程清空）通过，零新厂商推理。`0a388b3` 保留直接可运行回退，连同早期原型 `experiments/gpui-chat/target` 及过期临时副本共回收约 11.03 GB（10.27 GiB）无用存储（保留全部证据收据），见[包内证据与边界](validation/2026-11-13-hardening-localization-craft-package.md)。下文待入包/当前为各阶段历史状态，完整目标继续。

## 桌面引导、输入框与协作过程界面层次打磨（2026-11-13，源码）

融合 Beluga、T3 Code 与 Cindy 完善首屏 Agent 就绪卡片与三步引导、起始会话后端标识胶囊（`composer-backend-badge`）与建议胶囊对比度、工具过程矢量图标/计数胶囊及协作汇报卡片分层，同时保持全部既有元素 ID、无障碍标签与窄窗焦点滚动契约不变。Rust 测试、Windows 交叉检查与 Mac release 构建通过，见[说明与边界](validation/2026-11-13-desktop-surface-craft-polish.md)。即将随运行时与本地化修复一并组装新开发包并做原生窗口验证，完整目标继续。

## 跨层文案本地化、动态回退与双语会话搜索索引（2026-11-13，源码）

推进 `#17` 跨层文案与检索一致性：`chats.ts` 搜索索引补齐未绑定默认 `'New conversation'` 别名与绑定会话双语角色词（零 Agent 启动），`session_row.rs` / `search.rs` 对齐 `:__workflow__` 与角色副标题，缺来源投递提示、默认恢复摘要与空错误诊断改为渲染期动态本地化且不污染引用草稿，`language.rs` 闭合集 `localize_owned_error` 覆盖桌面与运行时自有错误并逐字节保留外部原文，`locales/en.json` 扩展至 1,298 条唯一键。`session-search`、双语别名与 d43f55e 真实三会话冷副本检索、Rust 测试及 Windows 交叉检查通过，见[实证与边界](validation/2026-11-13-cross-layer-localization-and-search-aliases.md)。未关闭 `#17`（仍待原生全表面/字号/VoiceOver/Windows 硬件核验），待入下一安装包，完整目标继续。

## 暂停队列新输入保持、可执行程序解析与元数据原子写清理（2026-11-13，源码）

复现并修复 Codex 停止后或冷重开暂停/队首失败队列在发送补充消息时被 `activateOnce` 隐式恢复消费、`claude.ts` 命中同名目录/空 `PATH` 段/`win32` 无扩展名脚本，以及 `claude-metadata.ts` / `codex-metadata.ts` 原子替换失败残留 `.tmp` 文件。四组前后对照、d43f55e 真实三会话副本冷读与 `check:runtime`、`check:team` 等 12 组回归通过，见[实证与边界](validation/2026-11-13-runtime-queue-and-executable-hardening.md)。无厂商推理/面板操作/正式测试改动，待入下一安装包，完整目标继续。

## 当前开发包 0a388b3（2026-11-13）

外部 Pi 扩展诊断（`a26ab5a`）、外部 Pi 合规图片历史（`f8a9460`）与 Codex 合规图片历史及 `excludeTurns` 冷恢复（`0a388b3`）已入 current/candidate、DMG 和 Applications。八份 JS/map 精确更新、完整清单/链接与安装核对通过；包内外部 Pi 六组诊断、外部 Pi 与 Codex 跨轮超 32 MiB/单轮 6 图/dispatch 多图/33 MiB 超限拒绝、d43f55e 真实三会话 40 条历史副本冷读→Rust 及四组包检查通过，无新厂商推理/面板操作。保留 7b7940c 可运行回退并退役 11 个更早重复解压树（保留全部 ZIP 与收据），见[包内证据与边界](validation/2026-11-13-image-history-package.md)。下文待入包/当前为各阶段历史状态，完整目标继续。

## Codex 合规图片历史与增量分页（2026-11-12，源码）

原始 Codex 0.167.0 `app-server` 复现单轮 19.45 MB 合规多图、跨轮累计超 8 MiB 及 12 轮 38.90 MB 冷重开 `thread/resume` 单行超限锁死。改为 32 MiB 有界分片一次拼接、`thread/resume({ excludeTurns: true })`（保留落盘 `legacy` 线程回退）与 `thread/turns/list({ limit: 1 })` 增量前缀游标刷新。五组前后对照、d43f55e 真实三会话副本冷读及六组相关回归通过，见[实证与边界](validation/2026-11-12-codex-image-history.md)。无厂商推理/面板操作/正式测试改动，待入安装包，完整目标继续。

## 外部 Pi 合规图片历史（2026-11-12，源码）

真实 `SessionManager` 与原始 Pi 1.4.0 复现跨轮累计超 8 MiB 图片历史无法继续及重开、单轮 19.45 MB 合规多图事件帧超限。改为只读分块装载会话文件种子 + `get_entries({ since })` 增量刷新，并将有界单行上限对齐至 32 MiB（分片一次拼接）。12 轮 38.90 MB 冷读续聊、Hive 上传 4 轮多图+bash+冷重开、单轮 6 图、33 MiB 超限拒绝及七组相关回归通过，见[实证与边界](validation/2026-11-12-external-pi-image-history.md)。无厂商推理/面板操作/正式测试改动，未入 7b7940c，完整目标继续。

## 外部 Pi 扩展诊断（2026-11-12，源码）

原始 Pi 1.4.0/真实 bash 复现成功工具被钩子诊断误判失败、显式继续重复写入；最小条件修复后正常结算且不重复，诊断仍展示，命令/模型/写盘真失败保留。六组 HTTP/SQLite/原生 CLI 前后对照及相关回归、隔离配置原生对话检查通过；首次默认配置启动期超时尚未定因，见[实证与边界](validation/2026-11-12-external-pi-diagnostics.md)。无厂商/面板操作、正式测试变化，尚未入 7b7940c，完整目标继续。

## 当前开发包 7b7940c（2026-11-12）

Claude 完成回执批量核对已入 current/candidate、DMG 和 Applications。仅四份 JS/map 精确变化，完整清单/链接及安装核对通过；包内 Projection 十阶段、官方 SDK/受控 CLI 的缺失/空回执/冷恢复、最新真实 Terra 三会话 40 条完整历史与正式 JSONL→Rust、四组包检查通过。本批无新推理/面板操作，78fef23 保留回退，见[包内证据与边界](validation/2026-11-12-claude-completion-package.md)。下文未入包/当前为各阶段历史状态，完整目标继续。

## 最新源码真实 Terra 协作（2026-11-12）

d43f55e 上一次输入、真实 Codex/Terra medium 主控和两位 Worker 各一原生回合，独立写文件、显式汇报及主控只读字节断言通过。完整后台重开与正式 main.js 冷读，40 条完整历史/27 条工具结果、消息 IDs、来源和账本一致；真实 JSONL 经同版 Rust 解析器通过。见[真实协作与恢复边界](validation/2026-11-12-latest-terra-team.md)。零面板操作，Pi 登录搁置、无 Claude 推理；当前安装包仍 78fef23，完整目标继续。

## Claude 完成回执批量核对（2026-11-12，源码）

将每条回执重复扫描历史改为每批读取当前原生 UUID 集合，无跨事件缓存；保留缺失 UUID 过滤及旧历史空回执。合成 10000 行批量核对中位 757.0→3.0ms，官方 SDK/受控 CLI 的 500 回执 getter 10.6→0.22ms；10 阶段替换/合并/清空快照一致、冷读/新回执及四项现有回归通过。见[范围与证据](validation/2026-11-12-claude-completion-uuid.md)。非厂商推理/全应用性能，未入 78fef23，完整目标继续。

## 当前开发包 78fef23（2026-11-12）

Codex 快照索引与显示日志恢复/写盘一致性修复已入 current/candidate、DMG 及 Applications。依赖/原生源码未改，复用 3a18b94 冻结树，只更新四份 JS/map；完整文件/权限/链接与安装核对通过。包内 5000 回合实际 JSONL 夹具、六场景/32 组差分、真实文件错误、旧 Terra 三会话冷恢复及正式 JSONL→Rust、四组包检查通过，无新推理/面板。见[包内范围](validation/2026-11-12-codex-history-package.md)。3a18b94 保留回退；下文当前/待入包为各阶段历史，完整目标继续。

## Codex 显示日志恢复（2026-11-12，源码）

恢复完成标记改为一次局部合并、原子写盘成功后提交内存，修复写盘失败污染后续“历史细节缺失”判断。32 组成功差分和六场景完整落盘哈希一致，真实 EISDIR/后续部分 item 提示正确。5000 回合原生导入中位 386→4ms，受控 JSONL 会话装载单样本 456→78ms；范围见[实证](validation/2026-11-12-codex-recovery-completion.md)。五个现有回归/真实旧 Terra 三会话冷读通过，无新推理/面板，未入 3a18b94，完整目标继续。

## Codex 长历史快照（2026-11-12，源码）

真实 JSONL 的合成 5000 回合/10000 条历史复现每条消息全表查来源开销，改用现有会话内索引、成功写盘后作废。五次快照中位 118.6→2.0ms，前后完整 JSON 及新增回合 JSON 哈希相同、旧回退/重复语义和冷恢复保持；五个现有回归及真实旧 Terra 三会话只读恢复通过。见[证据与性能范围](validation/2026-11-12-codex-snapshot-index.md)。非全应用性能结论，无新推理/面板操作，未入 3a18b94，完整目标继续。

## 当前开发包 3a18b94（2026-11-12）

Pi 扩展诊断不再误判投递失败、工作流临时成员回收去除多余转发已入 current/candidate、DMG 和 Applications。包内六组 SDK/实际工具、两组真实 PTY、七条结构化运行、15 帧正式 JSONL→Rust、冷历史/失败/回执/队列和四组包检查通过；仅本地受控接口，无厂商调用/面板操作。完整八项精确差异、文件/链接与安装核对通过，原生程序未变，ff2aa70 保留回退。见[包内证据](validation/2026-11-12-pi-diagnostics-package.md)。下文当前/待入包为历史阶段，完整目标继续。

## Pi 扩展诊断与重复执行（2026-11-12，源码）

真实 SDK/工具复现成功回复后的扩展事件诊断被误记投递失败，显式继续令自有文件写入两次。仅把 command 事件保留为扩展输入失败；其他钩子诊断仍展示，成功投递正常结算。六例 SDK/HTTP/SQLite/真实 bash 与冷历史/回执/队列、三个现有回归通过；模型/命令真失败保留。见[根因与实证](validation/2026-11-12-pi-extension-diagnostics.md)。本地受控接口，无厂商调用/面板，未入 ff2aa70，完整目标继续。

## 工作流取消回收（2026-11-12，源码）

追踪到临时 Worker 正在由执行器回收时的多余取消转发，删除该异步步骤及两层 catch；账本/awaiter/回收机制保持。真实 PTY 手动/超时停止、常驻成员普通取消提示及冷持久化、结构化七运行/进程/pending/冷恢复与两个现有回归通过，零模型/面板请求。见[根因与验证](validation/2026-11-12-workflow-cancel-teardown.md)。尚未入 ff2aa70，下一包需重新生产部署，完整目标继续。

## 当前开发包 ff2aa70（2026-11-12）

工作流超时原因和完成通知 Unicode 修复已入 current/candidate、DMG 及 Applications。新生产部署、七项精确差异、完整清单/链接、包内七条运行与五条 Unicode 流程、主控通知/进程清理/冷恢复、12 条正式 JSONL→Rust 及四组包检查通过，零模型/面板请求。见[包内证据](validation/2026-11-12-workflow-reliability-package.md)。原生程序未变，未重复 UI 验收；a744821 保留回退。下文待入包/当前为各阶段历史状态，完整目标继续。

## 工作流完成通知 Unicode（2026-11-12，源码）

真实运行结果/JSON/Worker 汇报的五个案例确认完成通知切坏 emoji 或错误截短；生产通知生成按码点统一计算阈值/截断/剩余字符数，完整结果和报告保持。真实 CLI seen→HTTP report、持久化通知与完整冷恢复、三个现有回归通过，零模型/面板请求；见[证据](validation/2026-11-12-workflow-notice-unicode.md)。与超时原因修复一起待入下一包，完整目标继续。

## 工作流超时原因（2026-11-12，源码）

修复 maxDurationMs 超时被误记为用户停止：运行、嵌套子运行、派单取消和主控通知保留同一时限原因，手动停止与原终态语义保持。当前包旧 runtime 与新源码的真实 HTTP/SQLite/Pi/协议 Worker 对照、进程清理/七条运行冷恢复、通知原文及两个现有回归通过，零模型/面板请求。见[证据与边界](validation/2026-11-12-workflow-timeout-reason.md)。未入 a744821，下一包需重新部署生产依赖，完整目标继续。

## 当前开发包 a744821（2026-11-12）

恢复摘要与旧历史 JSONL 修复已入 current/candidate、DMG 及 Applications。新生产部署、九项精确文件差异、完整清单/链接、包内真实 Pi 恢复与旧会话冷读→Rust、四组包检查、原生启动/正常退出均通过，零模型请求。见[包内证据](validation/2026-11-12-recovery-unicode-package.md)。上一版 46659e2 保留供回退；本批没有将账号、Windows 或完整产品体验标为完成。下文“当前”及“未入包”为各阶段历史状态。

## 当前开发包46659e2（2026-11-12）

按新Hive补丁离线冻结重新部署381包，官方Node下载/校验、完整28,869清单/29链接、六项精确变化通过；原生程序及其他依赖文件保持。包内实际团队JSONL→Rust、原生成员读取/正常退出、四组包检查及旧Terra三会话只读冷开通过，无厂商请求。current/candidate、DMG、Applications更新；清理重复base/安装备份/deployed树，db12698回退及证据保留。见[新部署与包范围](validation/2026-11-12-team-preview-package.md)。冻结依赖基线现为46659e2；下方未入包/旧冻结基线为历史，完整目标继续。

## 团队任务预览Unicode（2026-11-12，依赖补丁）

真实team.members JSONL复现Hive2.2.1任务预览截断emoji后被Rust解析器拒绝；既有依赖补丁只修正共享预览截取，完整任务不变。前后实际RPC/Rust对照、四项成员/团队回归、TS/build及离线冻结安装通过，无正式测试、面板或厂商请求。见[证据与依赖范围](validation/2026-11-12-unicode-team-preview.md)。锁文件patch hash已变化，下一包必须重新部署生产依赖，不能沿用旧冻结依赖组装；尚未入db12698，完整目标继续。

## 当前开发包db12698（2026-11-12）

Unicode标题修复已入current/candidate、DMG及Applications，只更新两个runtime模块/map，原生二进制保持。包内真实新输入边界/Rust解析、旧坏标题四记录的两次原生冷开及退出、emoji/自定义记录/原数据核对、四组包检查和旧Terra三会话只读恢复通过，无厂商请求。清理无占用重复base/安装备份，3d4422f完整回退与证据保留。见[本批包范围](validation/2026-11-12-unicode-title-package.md)。下方未入包为历史，完整目标继续。

## 自动标题Unicode与原生协议（2026-11-12，源码）

真实首条输入复现UTF-16截断留下半个emoji，旧自动标题及旧索引JSON均被原生同版serde_json拒绝。共享截取改为code point，旧自动标题读时只移除残片；正常emoji/自定义记录和原文件字节保持，Rust新响应解析通过。真实输入/队列/HTTP/SQLite及四个现有标题/搜索/归档/恢复检查通过。见[根因与证据](validation/2026-11-12-unicode-auto-title.md)。无正式测试、面板或厂商推理，未入3d4422f包，完整目标继续。

## 补充输入写盘拒绝恢复（2026-11-12）

内置/外部Pi在真实回复保持期间，以只读spool目录令补充原文+附件入队实际返回500；原pending完整对象/正在执行请求及上传原字节保持。恢复权限后显式重试、停止/冷开/显式继续及原生历史单次消费通过，两个检查exit0。见[精确范围](validation/2026-11-12-queued-input-write-rejection.md)。无生产/正式测试/包变化、面板或厂商调用，完整目标继续。

## 当前开发包3d4422f（2026-11-12）

汇报身份分层与图片解码并发限制已入current/candidate、DMG及Applications。四组包检查、原生16图历史/末尾缩略图/原图2400×2400与正常退出、完整原图字节/消息核对及旧Terra三会话只读冷开通过，无新模型请求。保留d644db9完整回退，将三个更早解压副本在manifest/ZIP哈希/无占用核对后退役，旧ZIP和证据仍保留。见[包内范围与存储](validation/2026-11-12-report-image-package.md)。下方各轮未入包及回退保留描述为历史状态，完整目标继续。

## 图片预览并发与内存（2026-11-12，源码）

草稿/历史预览统一用现有异步信号量限制两张同时解码，blocking工作实际持有许可，原图查看独立。同一16张2400×2400历史、前5秒原生父进程RSS峰值单次采样506.8→195.0MiB；末尾缩略图、16条原生ID/原图字节及零新增Hive消息/run核对通过，两窗口exit0。Mac构建/Windows类型检查通过，无正式测试或模型请求；见[指标范围与证据](validation/2026-11-12-image-preview-pressure.md)。不推广到全部图片/速度/平台，未入d644db9包，完整目标继续。

## 汇报身份与操作分层（2026-11-12，源码）

实际历史窗口复现成员/汇报标题被同行操作截断；改为身份/状态与自动换行操作分层。English20宽窗与约822×552窄窗完整标题、原文展开/成员往返/草稿保持通过，原/副本六张核心表和草稿逐字不变、两窗口exit0；Mac构建/Windows类型检查通过。见[截图及范围](validation/2026-11-12-report-heading-layout.md)。无正式测试或模型请求，未入d644db9包，完整目标继续。

## 当前开发包d644db9（2026-11-12）

协作原文复制与Codex长历史恢复优化已纳入current/candidate、DMG和Applications。四组包检查、包内长历史写盘/冷读一致性、旧Terra三会话只读恢复、欢迎启动/正常退出及安装完整清单通过。原生仅启动退出抽查，无新模型调用；清理无占用重复base/临时安装备份，保留3ed07c9可运行回退和全部验证状态。见[本批证据](validation/2026-11-12-copy-recovery-package.md)。下方未入包为各轮历史状态，完整目标继续。

## Codex长历史恢复（2026-11-12，源码）

将按回合反复全表filter改为一次局部索引，保留恢复与持久化语义。合成5,000回合/10,000记录包含真实写盘的四样本中位312.1→68.0ms；原/新完整JSON哈希一致，8组旧/新差分及五个现有Codex检查、真实Terra旧三会话只读冷开通过。见[证据与性能范围](validation/2026-11-12-codex-recovery-performance.md)。无正式测试、模型或面板操作，未入3ed07c9包，完整目标继续。

## 协作原文复制（2026-11-12，源码）

补齐派单汇报、取消原因及每条协作消息的复制入口，直接保留原字段。真实HTTP/SQLite夹具、一次Mac复制到未发送草稿及退出后的逐字检查通过；代码块/Unicode/尾部空格保持，无发送或模型请求。Mac构建、Windows类型检查通过，无正式测试变化。见[证据](validation/2026-11-12-collaboration-copy.md)。未入3ed07c9安装包，完整目标继续。

## 当前包后台边界补验（2026-11-12）

3ed07c9包内任务监听、模型配置、Pi历史/写盘、Claude写盘恢复及远程加密/撤销五组现有检查全部exit0；无需账号/面板/厂商推理。真实文件错误、HTTP/SQLite、SDK/RPC及加密PTY执行有断言；合成CLI/本地网关不冒称真实模型或互联网验收。ZIP哈希保持，无生产代码/正式测试变化。见[范围与证据](validation/2026-11-12-package-backend-boundaries.md)。完整目标继续。

## 当前开发包3ed07c9（2026-11-12）

协作整页分页与英文后端搜索修复已进入current/candidate、DMG及Applications。四组包检查、包内六组搜索/三会话只读冷开、解压包欢迎启动与退出0、DMG和安装完整清单通过；无新模型请求。清理无占用重复base及临时安装备份，3a3f873回退与所有状态/日志保留。见[本批证据](validation/2026-11-12-pagination-search-package.md)。下方未入包为各轮历史记录；账号、Windows原生及完整产品验收继续开放。

## 英文搜索与认证复查（2026-11-12）

External Pi/Built-in Pi的真实搜索缺口已在源码修正，六组名称及现有回归通过，未入3a3f873包。Pi原生Terra请求仍因OAuth invalid_grant失败，已abort清理；用户本轮明确暂不重登，并保持真实测试仅Terra，Claude推理边界不变。见[证据与排除项](validation/2026-11-12-search-labels-and-auth-boundaries.md)。完整目标保持开放，不因单个账号限制停止其他实施。

## 当前开发包3a3f873（2026-11-12）

当前/candidate、DMG及Applications更新3a3f873，纳入诊断复制、原生设置错误原因及会话打开错误隔离。四组包gate、包内Node设置对照/三会话冷读、解压包实际启动退出0及DMG/安装清单核对通过；新模型0，安装副本本轮未另启动。见[证据与边界](validation/2026-11-12-diagnostics-error-package.md)。后文未入包描述按当时阶段保留；所有账号、正式发行、Windows原生及完整体验缺口仍开放。

## Web诊断入口迁移（2026-11-12）

桌面远程设置补复制诊断与搜索关键词，复用已有Hive API。真实后台脱敏/认证检查、一次实际剪贴板往返与退出通过；Mac构建/Windows类型检查通过。见[范围及限制](validation/2026-11-12-remote-diagnostics.md)。未入645c9a5包，不扩大为所有平台或全流程验收。

## 混合路由与设置错误原因（2026-11-12）

四后端16个主控→Worker组合已通过原生控制、真实CLI/HTTP/SQLite和冷恢复检查；12个混合组合不冒称真实模型协作。另修共享设置错误丢失原生原因，生产JSONL旧/新对照和相关回归通过；见[证据](validation/2026-11-12-mixed-native-routing.md)。无面板操作，新诊断未入645c9a5包；账号/真实推理/Windows/整体体验边界不变。

## 当前包与活动成员（2026-11-12，645c9a5）

真实Terra一对二各1轮，显式汇报/独立字节断言及活动成员正确标签已验证；源码退出→包内三会话冷读/原生重开→退出，草稿和账本保持。四组新解压包gate、DMG完整内容核对和Applications清单替换通过。current/candidate/DMG/安装副本均为645c9a5；安装副本本轮未另做原生启动。见[本批范围](validation/2026-11-12-live-members-package.md)。后文“未入293afe3”“活动Worker待验”为当时历史状态；完整目标及Windows/账号/正式发行等缺口仍开放。

## 成员读取失败与原生标签复验（2026-11-12）

补原生失败/延迟/成功后历史标签矩阵；另修成员页首读失败被显示为空团队及错误缺失AX的问题。中英文原生错误/重试/缓存保留/真正空团队、草稿冷恢复、退出及HTTP/SQLite核对通过，新增模型0；Mac构建/Windows目标检查通过。见[证据](validation/2026-11-12-member-roster-native.md)。活动新Worker动态标签、Windows原生及完整目标仍待；未入293afe3包。下方无原生工具描述为前轮历史状态。

## 成员列表历史标签（2026-11-12，后续源码）

针对真实协作中工作成员短暂误标历史，打开/刷新协作页立即请求成员，并在刷新/读取失败/会话忙碌时使用中性名称；五行共享入口修正，Mac构建/Windows目标检查通过。当前无原生控制工具，标签矩阵实机复验待补；见[记录](validation/2026-11-12-member-roster-label.md)。未入293afe3包，完整目标继续。

## 安装包真实一对二与重开（2026-11-10，293afe3）

实际Applications内原始Codex0.167/Terra medium主控+双成员，三会话各1轮、两显式reported/精确文件/主控独立exit0字节断言、成员往返及两次退出/冷读/原生重开通过，未发送草稿精确保留；见[实际协作](validation/2026-11-10-installed-293afe3-team.md)。保留缺失项目文档工具失败及索引集合顺序变化；成员短暂历史标签仍待定位，完整目标继续。

## 当前开发包293afe3（2026-11-10）

本批通知/汇报语义、输入区/附件及退出错误修复已纳入Mac优化包；ZIP四组检查、旧负新正对照、真实三会话包内冷读、解压包及Finder安装副本启动/退出/附件恢复通过，零新推理。current/candidate/DMG/Applications同批，清理无占用重复base约514MiB，保留1e可运行回退；见[本批验证](validation/2026-11-10-composer-exit-package.md)。下文“未入1e”等为当时阶段记录，Windows原生与完整目标继续。

## Codex退出清理诊断（2026-11-10，后续源码）

原生故障走查复现原始EOF被通用cleanup错误覆盖；仅排除重复抛出的同一Error对象，定向先红后绿、完整Codex/两后端队列/TS/build及Mac错误+队列+双附件/冷读通过，零真实模型。见[诊断与恢复](validation/2026-11-10-codex-exit-cleanup.md)。随后复用Agent状态行加固定错误入口，中文20px窄窗点击显露/关闭/恢复状态及原数据冷读通过；英文入口/Windows原生继续。未入1e包，完整目标继续。

## 输入区空白修复（2026-11-10，后续源码）

三个不存在的错误/队列/附件区域不再参与 composer 间距；实际20px窄窗节省约30逻辑像素。12行草稿、撤销、真实附件添加/移除、合成排队/错误区域及冷读保留通过，Mac构建/Windows目标检查通过。长草稿附件再复现隐藏焦点，复用已有Reveal后键盘显露/预览/移除及正文保留通过；另补固定附件数量入口，鼠标定位/移除在英文窄窗及中文带队列组合通过；见[验证记录](validation/2026-11-10-composer-spacing.md)。无后台或测试改动，未入1e68306包，完整目标继续。

## 汇报状态的原生语义（2026-11-10，#28后续）

补齐待处理成员Group/正文Label与两处状态Label，原先AX缺少成员归属和状态正文。生产后台/队列加隔离合成JSONL对端，在Mac中英文20px/窄窗实际跑通待处理→处理中→完成、两报告归属及只读冷恢复，零真实模型；实际steer分支仍按既有24模式与真实Terra证据分别记录。见[语义验证](validation/2026-11-10-report-state-accessibility.md)。Mac构建/Windows目标检查通过；未入1e包，VoiceOver播报/Windows原生及完整目标继续。

## 活动汇报状态（2026-11-10，#28后续源码）

原生steer追加的报告排在用户队头后，却因getter只看index0显示待处理。一处纳入既有steering状态，24组先红后绿/团队集成/TS/build通过；原生新一对二Terra三个会话各1轮、报告/文件/独立断言/唯一final和完整冷读保持，见[处理状态](validation/2026-11-10-report-processing.md)。短暂processing动态画面未捕获，检查证据与实机完成状态分开记录；当前1e68306包未含本项，完整目标继续。

## 团队通知成员身份（2026-11-10，后续源码）

实际双成员就绪通知标题均为“团队通知”，前端仅对report读取名字而忽略system/dispatch显式元数据。一处复用已有workerName读取修复；Mac真实历史宽/窄窗、原文展开和退出exit0通过，按chatId核对台账/原生轮数/投递与完成ID保持，零新推理。Mac构建/Windows目标检查通过，无UI测试或后台改动；见[来源阅读](validation/2026-11-10-notice-senders.md)。此项尚未纳入1e68306安装包，完整目标继续。

## 当前开发包1e68306（2026-11-10）

汇报同回合修复与欢迎页版本/连接摘要均已纳入优化Mac release。四组包内PTY/HTTP/SQLite/派单/Codex/内外Pi队列检查、真实三会话历史包内冷读、欢迎页实际版本/账号反馈、Finder替换及安装启动/退出通过；见[本批记录](validation/2026-11-10-report-package.md)。current/candidate/DMG/Applications同批，无新模型推理；真实Terra为同提交源码记录。清理无占用重复及第三旧解压约1GiB，0080304可运行回退保留，完整目标继续。下文按时间保留的“尚未入包”等是各阶段当时状态。

## Codex 双成员汇报同回合修复（2026-11-10，#27）

当前0080304实际安装包的一对二Terra协作暴露已读汇报额外触发两个空回合；共享队列复用原生steer处理report，24组先红后绿及真实HTTP/SQLite团队/Codex回归通过。相同任务的真实Mac源码复验从主控3轮+成员各1轮变为三会话各1轮，文件/显式汇报/主控独立字节断言与关联入口、完整冷读保持通过，见[本轮证据](validation/2026-11-10-codex-report-steering.md)。尚未更新当前安装包，其他后端和完整目标继续。

## 欢迎页连接反馈（2026-11-10，#26后续）

实际0080304中Codex检查后回欢迎只显示未检查版本；源码现把版本与连接/账号两项分开，复用设置页消息/过期判断，显示未保存路径并在保存后更新。真实Mac进行中/已有账号/过期/失败/未登录、中文英文20px窄窗键盘与完整重开恢复未检查均有证据，五张业务表仍0。见[欢迎页摘要](validation/2026-11-10-welcome-account-summary.md)。最终Mac构建/Windows目标检查通过；纯UI无测试改动或模型执行，当前0080304安装包未含此项，完整目标继续。

## 当前开发包0080304（2026-11-10）

Codex连接/未登录修复命令、命名空会话标题及默认关闭的本机提醒接线已纳入优化release。三组包内真实进程/HTTP/SQLite/远程/账号检查、ZIP解压原生窄窗复制/草稿/默认off、独立冷读、Finder实际更新及安装启动/退出通过。见[本批包记录](validation/2026-11-10-connection-package.md)。current/candidate ZIP、DMG、Applications同批；清理无占用重复及第三旧解压约1GiB，保留021c162可运行回退。零新模型/消息执行；通知送达授权、Windows原生及完整产品目标仍开放。下文“尚未入包”等是各原阶段记录。

## Codex 未登录修复入口（2026-11-10，#26后续）

原样0.167.0在隔离空配置中返回未登录；设置现提供按本次实际路径正确引用的登录命令与复制入口，路径变化隐藏旧命令，已有账号不显示登录入口。Mac中文/英文20px窄窗Tab显露、实际复制正文、原草稿及零消息/Agent run冷恢复通过。复用远程命令转义，参数实执行/账号/远程CLI回归、Mac构建/Windows目标检查通过。见[登录修复](validation/2026-11-10-codex-login-repair.md)。未执行登录或模型，Windows原生/新包等仍待，完整目标继续。

## Codex 连接与账号检查（2026-11-10，#26）

外部程序页新增独立只读连接/账号检查，复用原生进程与设置路径优先级；原样0.167.0实际连接/脱敏ChatGPT记录成功、PATH旧版0.46.0实际连接失败，不刷新凭据或启动模型。Mac英文20px窄窗路径变更过期提示、Tab显露/Return重查、返回精确草稿和独立冷读通过；协议错误/超时/取消/HTTP/SQLite及退出回收有定向检查。见[连接检查](validation/2026-11-10-codex-account-check.md)。不等于模型可用；其他原生账号类型/中文/Windows等矩阵仍待，#26与完整目标开放，安装包仍021c162。

## 命名空会话与真实回复复核（2026-11-10）

修复命名但尚未发送消息的会话被当成项目欢迎页：Mac 20px窄窗收起侧栏仍显示会话标题，重命名取消、新空会话欢迎页、草稿往返及独立冷读通过。见[标题验证](validation/2026-11-10-empty-session-title.md)。提醒关闭时另完成一次原样 Codex Terra Medium 真实回复、0工具及冷恢复，见[提醒后续](validation/2026-11-10-local-notifications.md#后续关闭提醒时的真实模型与冷恢复)。Mac构建/Windows目标检查通过；无全量测试，Windows原生仍未验收。OS通知授权待回复，送达/点击未验收；当前021c162安装包尚未含本轮源码，完整目标保持开放。

2026-11-10。目标仍是可日用、融合T3 Code/Beluga/Cindy体验并参考Maka架构的完整macOS和Windows客户端，复用Hive Web协作；以下按原目标列出证据及缺口，不能用已实现项目反过来缩小范围，也不表示表中已经穷尽所有对标细节。

## 按原目标核对

对照固定Web通知源码补回本机系统提醒接线：复用现有GPUI、默认关闭、本机偏好、通用提示及点击会话导航；三项逻辑/文件检查、Mac构建和Windows目标检查通过。Mac中文/英文20px窄窗、真实写盘失败保留原开关/移除故障重试、冷恢复、键盘显露通过，两个窗口正常退出，零消息/模型/Agent run。系统授权确认待用户回复，尚未发送通知，OS送达与点击返回不得当作完成。见[本机提醒进展](validation/2026-11-10-local-notifications.md)，#25开放，未进入当前021c162开发包。

021c162优化开发包已纳入远程正文/审计事件名称、动态显露及计划确认/删除后焦点。只更换原生程序，112个runtime与依赖不变；重跑包内启动/真实PTY/HTTP/SQLite、两组包内窄窗原生操作及独立冷读通过。Finder实际替换安装、欢迎/远程关闭/正常退出通过，新增模型回合0；旧后台套件明确沿用46373f6范围。见[本批开发包](validation/2026-11-10-remote-schedule-package.md)。ZIP/DMG/Applications同一批，保留46373f6可运行回退；清掉约1GiB重复与更早解压副本。完整目标、Windows原生及正式发行继续开放。

工作流计划删除补实机验收并修复确认区离屏和删除后焦点丢失：中文20px浅色窄窗，默认Cancel、再次回车取消、明确选择后仅目标删除、刷新焦点及继续阅读通过；独立HTTP冷读确认peer/原结果/日志逐字段保留。准备复验时显式resume消费既有通知，单列一次原样Codex Terra Medium只读回合，不冒称全轮零推理。见[计划删除](validation/2026-11-10-workflow-delete.md)。最终Mac构建/Windows目标检查通过；新源码尚未打包，其他时序/平台矩阵仍开放。

远程设置补设备/审计正文AX及精确撤销对象，并修复窄窗键盘展开确认区后按钮被推离视口。实际安装包安全码独立匹配并确认配对，源码冷恢复/取消/再次展开/手动滚动/确认撤销通过；独立SQLite和包内冷读确认目标墓碑、active设备0、零项目消息执行，隔离远程已关闭、自有网关正常退出。见[远程确认与撤销](validation/2026-11-10-remote-confirmation.md)。Mac构建/Windows目标检查通过，无新模型；新源码尚未纳入46373f6开发包，真实手机和Windows原生等边界仍开放。 后续访问历史补中英文事件名称，实际三类持久记录及中文20px窄窗长ID/原因换行通过，原始诊断保留；见同一验证记录。

工作流运行详情补日志/状态/阶段/时间的可读正文及结果局部选择；真实完成/失败/停止三态、12条完整日志复制、中文局部结果/两行错误复制和窄窗日志末尾通过。另复现并修复刷新时隐藏空状态导致的列表高度收缩偏移，实际鼠标往返与重新读取后位置保持。三个窗口正常退出、独立真实后台逐字段终态/14条日志/完整草稿/三条仍暂停系统通知冷读通过，见[运行阅读与位置](validation/2026-11-10-workflow-reading.md)。最终Mac构建/Windows目标检查通过，无模型回合；#13其余边界与完整目标仍开放，已纳入6148801开发包。

定时计划正文补齐可读语义与现有文本选择组件，长标题省略并保留完整名称/路径。真实计划鼠标恢复、键盘暂停、英文20px窄窗离屏按钮显露及Space恢复/暂停通过；独立SQLite/冷读确认两计划身份/参数/源码/时间保留、均暂停、零新执行/投递及精确复制草稿。原轮复制覆盖Cron片段；后续已补中文参数实际选区及真实定时执行的last_run_at非空分支实证，见[计划阅读与控制](validation/2026-11-10-schedule-reading.md)。Mac构建/Windows目标检查通过，#13其余边界与完整目标仍开放，已纳入6148801开发包。

源码b8feb47补部分真实原生组合输入：回车确认仍留在编辑器、零新投递，实际中文/emoji多行复制粘贴通过；“全选→组合输入→Escape”可留空，Cmd+Z精确恢复原文。独立核对原状态与测试副本两份草稿均保持，两个窗口正常退出。系统编辑器同条件对照尚未成立，首次解锁后的原始清空原因仍未定，不修改公共取消语义；见[组合输入补验](validation/2026-11-10-native-composition.md)。没有生产/测试/开发包变化或模型推理，不扩大为完整IME验收。

新项目草稿选择Codex创建会话后前插的问题已修复并补Mac原生交接/Unicode末尾追加/同owner手动位置/设置返回/窄窗验证；另修复自动恢复会话没有初始键盘焦点，完整重开后无需点击可继续追加。四次原生正常退出、独立真实后台精确两份草稿/零新投递/空历史/idle及Terra选择冷读通过，Mac构建/Windows目标检查通过。首次解锁后的异常清空尝试保留且根因未定，不扩大为全鼠标/剪贴板/输入法验收。见[草稿光标与聚焦](validation/2026-11-10-draft-caret.md)。已纳入6148801开发包，完整目标仍进行中。

源码66eb15c补后台生命周期定向复核：工作区移除/回滚/读取互斥、工作流双成员/嵌套/CPU退出与冷恢复、真实终端及独立JSONL共五个现有检查全部exit0；本次拥有的原始Pi及五个受控协议进程均已退出，原文件/会话保留。四条向已停止成员发送取消通知的409诊断原样保留，未扩大为所有异常时序无误。见[生命周期复核](validation/2026-11-10-lifecycle-audit.md)。本轮无生产/测试/开发包变化，无新模型推理；完整目标仍未完成。

模型与接口配置已增加固定搜索、匹配数量/空结果/清除；接口名称/ID/地址、模型名称/ID/组合身份、中英文、筛选后的精确保存与冷启动重搜通过。同时修复已有名称末尾空格导致未编辑就出现保存/假草稿的比较误报；窄窗收起重复分区标题，宽窗保留，见[模型配置搜索](validation/2026-11-10-model-config-search.md)。最终Mac构建/Windows目标检查通过，无新模型调用；源码改动尚未纳入fb3dcd0候选，Windows原生及完整目标仍待。

模型配置列表在深色/20px/约592×500窗口的离屏键盘操作已实际复现并修复；工具栏、接口/模型行及新增入口复用焦点显露，长列表正反74次Tab、手动滚轮后的同组显露、非法值不落盘、仅目标Token修改及完整冷读回通过，见[模型列表键盘](validation/2026-11-10-model-list-keyboard.md)。最终Mac构建/Windows目标检查通过；本轮源码修复尚未纳入fb3dcd0候选，VoiceOver和Windows原生仍待，#20保持开放。

缺失Codex程序→版本检查失败→原生编辑路径→检查不保存→键盘保存→返回重开同一历史/精确草稿已实际通过；冷读无额外用户投递或模型回合。校准过时的Codex验证版本0.167.0，真实probe/HTTP/SQLite/JSONL关闭定向检查通过，见[外部程序修复](validation/2026-11-10-agent-program-repair.md)。不等同账号/模型就绪或Windows原生验收，候选未纳入。

设置导航在浅色主题、20px 字号、约822×450窗口的底部裁切已修复：独立滚动、键盘自动显露、长标签省略及完整提示；实际正反Tab、滚轮、角色页面激活、632×450紧凑菜单和返回精确草稿通过，见[设置导航](validation/2026-11-10-settings-navigation.md)。Mac构建及Windows目标检查通过，Windows原生/VoiceOver仍未验收，候选未纳入。

源码59c1fe9补真实Terra运行中键盘输入与Queue点击：40秒工具期间输入完整后续请求、收到排队反馈，最终两条按序回复，冷读仅两次用户投递/一次真实工具、pending0。中文多行剪贴板粘贴及精确草稿恢复通过，见[运行中输入与排队](validation/2026-11-10-composer-queue-input.md)。不等同中文IME/空编辑器全交互验证；本轮无产品代码变化、候选未更新。

合并模型面板在后台回合结束后自动重开、抢焦点的问题已通过真实Terra回合复现并修复：选中会话忙碌时清除打开状态。修复后真实工具/最终回复、面板保持收起、原草稿保留及继续编辑/冷读通过，见[后台回合面板](validation/2026-11-10-model-picker-busy.md)。准备时的显式重启恢复回合单独记录；运行中追加键入未获实证，不扩大为完整输入法验收。候选未纳入。

对照安装T3 0.1.7后，源码输入区合并模型/推理入口并增加两列独立滚动；真实Codex在约822×450中完成Medium→High→Medium、Terra同模型选择/筛选、正反Tab、空结果、焦点恢复及冷读。输入区单行、草稿保留，见[模型与推理面板](validation/2026-11-09-model-effort-panel.md)。无新增推理，其他后端/忙碌/更小窗口等矩阵待验，候选未纳入。

账号工具栏焦点框左缘裁切已加最小内边距并实际复验；隔离无效合成凭据下的API/订阅/未保存三态及更新/重新登录标签、英文单项计数、最终构建冷启动/退出均通过，见[账号显示变体](validation/2026-11-09-pi-account-variants.md)。凭据字节不变，无认证/模型调用；不代表真实认证可用，候选尚未纳入。

Pi账号页补提供商/本机保存状态分组与AX正文，并复用焦点自动滚动。源码Mac约822×450的42提供商长列表正反Tab、搜索空结果、OpenAI实际3项筛选、中英文状态及工具栏显露已验证，见[账号阅读](validation/2026-11-09-pi-account-reading.md)。无认证/模型请求；工具栏焦点框左缘仍局部裁切、其余认证/平台矩阵待验，候选尚未纳入本次源码。

首次使用的“先配置 Agent”已改为三个明确设置去向，外部程序页补登录/版本检查边界。全新隔离数据中逐项点击、约822×450菜单和键盘选择外部程序已验证；退出后零项目/消息/会话且无凭据，见[首次配置分流](validation/2026-11-09-welcome-setup.md)。不代表完整就绪检测；候选未更新，完整目标继续。

推理设置补齐英文原生说明及AX正文；源码Mac约822×450下以Tab/Enter到达并执行重新读取、Escape/Enter关闭和重新打开、保留Medium与草稿已补验，用量面板的键盘首进/End/重开也通过。见[推理阅读与键盘](validation/2026-11-09-reasoning-reading.md)。无新推理请求；候选未更新，完整矩阵仍待。

最新源码补验：英文约822×450窗口的聊天操作已分组换行，用量改为可滚动浮层，发送/停止操作保持在输入卡片内；真实原始Codex 6.1 Terra完成工具读取、唯一回复及独立冷恢复，未发送草稿保留。见[输入区与用量](validation/2026-11-09-composer-usage.md)。本轮尚未纳入候选包，Windows仅交叉检查，完整目标继续。

| 要求 | 已有证据及其范围 | 尚未满足或未获实证 |
| --- | --- | --- |
| GPUI + gpui-kit，复用Hive业务 | 原生Rust前端、固定Hive2.2.1、共享HTTP/SQLite、team/派单/显式汇报与执行器接缝；[方案](implementation-plan.md)、[接缝](adr/0001-reuse-hive-runtime.md) | 不能把架构接通或编译通过当完整产品 |
| 内置Pi，不新增审批 | 既有真实Terra主控/Worker记录；[协作](validation/2026-11-07-structured-team.md) | 当前OAuth刷新真实返回invalid_grant，须重登后复验；[最新认证证据](validation/2026-11-09-codex-image-live.md) |
| Codex作为主控/Worker | 原样CLI0.166.0真实派单/文件/汇报/恢复；此前候选包已从原生聊天框跑通真实Terra单Worker及完整退出/冷启动，见[包内协作](validation/2026-11-09-packaged-codex-team.md)；新增真实Terra识图及整个runtime重开、原图字节保持；[Codex](validation/2026-11-07-codex.md)、[图片](validation/2026-11-09-codex-image-live.md) | 不等于所有厂商事件、复杂截图、Windows和所有混合后端组合已验收 |
| 外部Pi作为主控/Worker | 原样CLI1.4.0真实Terra团队与恢复；[外部Pi](validation/2026-11-07-external-pi-team.md) | 新的真实图片推理、所有扩展UI和最新平台组合未完整验收；认证与内置Pi分别记录 |
| Claude Code作为主控/Worker | 官方SDK、原样CLI无推理控制、持久化、工具回调、交互与合成对端；[Claude](validation/2026-11-07-claude-adapter.md) | 真实Claude模型团队协作未验收；保持只用Terra的用户约束，不换模型冒充成功 |
| 统一聊天、工具与成员过程 | 聊天投影、思考、工具Diff/文件预览、成员会话、归档/搜索/草稿、附件、排队输入已实施；源码Mac导出复制/失败重试/窄窗/AX提示，以及导出和文件预览Home/End/双向翻页、中文文件选区复制已取得实证，见[原生补验](validation/2026-11-09-native-resume.md) | 成员职责另已修复AX正文、首尾/翻页和选区复制，102行/3,531字符在Mac实证，见[成员阅读](validation/2026-11-09-member-responsibilities.md)；已纳入dd0230a候选并补包内首尾/翻页/窄窗验证，见[包内证据](validation/2026-11-09-stop-candidate.md)。真实Codex原生两条排队/停止/冷恢复/显式继续已补验，并修复额外恢复唤醒，见[排队恢复](validation/2026-11-09-queued-codex-ui.md)。VoiceOver播报、滚轮/完整键盘/字号、部分在途时序、系统输入与可访问性矩阵未完成；局部窄窗不推广到所有页面 |
| 现有Web协作与配置整合 | 团队职责/角色模板、任务与冲突、派单/消息/汇报、记忆、工作流、通知及程序/模型/认证配置已有接入 | 语言偏好与累计1046条主要页面/配置/聊天查找/协作/资源终端及静态和上下文错误文案已接入，模型编辑已补固定操作/提示、Tab与空格保存及冷读回，见[配置窄窗](validation/2026-11-09-model-editor-footer.md)，已纳入2b3c07e候选并补包内配置/退出恢复，见[配置候选](validation/2026-11-09-model-candidate.md)；模型/接口编辑另已补焦点与标签显露、正反Tab和实际滚动，见[滚动证据](validation/2026-11-09-model-editor-scroll.md)，新增/覆盖草稿及保存、英文17字号已补窄窗实测，完整字号/可访问性/Windows等边界仍在[#20](https://github.com/tt-a1i/hive-desktop/issues/20)；完整页面迁移、真实切换/草稿/窄窗仍未完成，见[语言记录](validation/2026-11-09-interface-language.md)、[#17](https://github.com/tt-a1i/hive-desktop/issues/17)；远程配置已接入并通过回环SAS配对和撤销断网/重开验证，原生设置/开关/配对码/拒绝已补包内实证，确认/撤销GUI已补实际隔离配对与冷读（见[最新补验](validation/2026-11-10-remote-confirmation.md)），真实网关和Windows仍待，见[原生补验](validation/2026-11-09-remote-native-ui.md)及[最新候选](validation/2026-11-09-settled-candidate.md)、[#18](https://github.com/tt-a1i/hive-desktop/issues/18)；工作流计划删除已补上述目标/peer/历史实证，精确在途时序与平台矩阵仍待 |
| Web本机通知迁移 | 默认关闭的GPUI系统提醒接线及本机偏好/异常/窄窗键盘/冷恢复，见[本机提醒](validation/2026-11-10-local-notifications.md) | 系统授权待确认，关闭状态真实Terra回复已验证；开启后的送达/点击/平台及音效矩阵未验收；[#25](https://github.com/tt-a1i/hive-desktop/issues/25)开放，不能把此前Webhook接入当本地提醒已完成 |
| 停止、重连、恢复、窗口生命周期 | 所有权清理、队列/身份恢复、Mac实际后台死亡/重连；Mac EPERM回归先红后绿；最新源码Cmd+Q、冷启动草稿/语言恢复、窗口关闭均exit0并清理后台，见[原生补验](validation/2026-11-09-native-resume.md) | 旧21544/54534已不在进程表，其退出原因未证实；候选包和Windows矩阵未完成，不承诺恢复任意中断工具或exactly-once |
| macOS可实际使用 | current/candidate现为46659e2优化release，按新Hive补丁重新部署生产依赖；四组包检查、实际团队JSONL/Rust、原生成员读取/退出、DMG及安装完整清单通过，见[当前开发包](validation/2026-11-12-team-preview-package.md)。真实Terra来自先前645c9a5，当前包只有三会话只读冷开；安装副本本轮未另启动 | 完整页面、输入法、远程/协作/辅助技术与性能矩阵仍未完成；开发包不是完整交付 |
| Windows首版支持 | 已有平台接线与监督记录，本机默认功能MSVC类型检查通过；[跨平台](validation/2026-11-08-windows-cross-check.md) | 本批未取得Windows原生窗口、模型、外部CLI及包验收。用户明确暂时无Windows机器、先继续其他实施；不处理CI不等于取消Windows目标 |
| 完整、精美、Cindy/Maka级体验 | 已有固定源码参考与安装Cindy局部原生走查，并实施多轮聊天/配置/导航改进 | 整体对标仍未证实；还须按实际安装体验做全流程及边角检查，不能以局部截图或当前清单宣称一致 |
| 可安装与交付 | 有冻结Node/native依赖的Mac便携开发包及实际PTY/HTTP/SQLite检查 | 未签名/未公证；正式发行、完整安装体验与两个平台最终验收未完成 |

## 当前开发包与候选

最新指针为46659e2，ZIP/DMG、Applications及验证边界见[当前开发包](validation/2026-11-12-team-preview-package.md)。下面6148801及其他历史条目按当时阶段保留，不作为最新current或现存目录保证。

### 历史：6148801

current/candidate共同指向`61488017561e0e5e821c07b0dfec9ed6fad6719e`的Mac ARM64优化release，143,419,605字节，SHA256 `ea78c52c37a8273e541c79229e560b2a2915c0f9ad417caec8623419a24e0321`，两路径为同字节硬链接。原6edac4c ZIP/收据和可运行解压应用保留。

纳入协作任务正文默认收起、草稿光标/自动恢复焦点、工作流日志/状态/计划阅读、列表刷新位置、键盘逐页与计划确认说明。112个runtime输出、28,863个条目/模式、29个内部链接匹配；六组包gate全部exit0。新包实际English20冷启动直接追加中文多行草稿、工作流末条日志/来源及窄窗翻页、计划英文说明和取消通过，两套包内独立冷读逐字段核对通过；本轮新模型请求0，真实厂商证据沿用相同runtime的前包记录。见[本轮开发包](validation/2026-11-10-workflow-package.md)。

lsof确认无引用后清理重复组装base约513.5MiB，保留状态/证据/ZIP和旧应用。Windows原生、正式发行、完整UI/输入法/辅助技术/性能及协作远程矩阵继续开放。以下历史条目描述当时状态，以本节最新纳入范围为准。

## 当前阻断与仍可推进的工作

- [#21](https://github.com/tt-a1i/hive-desktop/issues/21)已在最新源码修复Codex工作流report提交后的过早回收，真实Terra/原生工具确认、独立冷恢复与实际UI通过；[证据](validation/2026-11-09-workflow-codex-report.md)。已进入7c28580候选并补[包内真实工作流/恢复/原生验收](validation/2026-11-09-report-candidate.md)。

- [#19](https://github.com/tt-a1i/hive-desktop/issues/19)已在源码修复：Codex停止后关闭本聊天独占app-server，真实Mac后台命令退出、其他连接及原线程/队列保留；原生按钮和旧停止提示清理已补验，见[停止证据](validation/2026-11-09-codex-stop.md)。已纳入dd0230a候选并通过包内真实检查，Windows实际及逃逸进程不作保证。

- 原生访问现已恢复，最新源码已完成部分双语/导出/退出补验，见[证据及限制](validation/2026-11-09-native-resume.md)。旧54534、21544已不在进程表，不能补记正常退出；ScreenCaptureKit仍偶发捕捉失败，按后续真实状态核对。
- Pi的原生OpenAI订阅刷新仍返回400 invalid_grant，已请求重新登录；不再用相同认证重复请求，不自动更换接口、认证或模型。
- 用户恢复持续目标后，实际原生访问已可用，目标保持active。候选包已完成明确列出的原生操作，继续其余页面/输入、工作流、远程原生配置和完整包内验收；Pi重新登录、Claude真实模型和Windows环境仍各自列缺口。局部候选验收不等于完整体验，不推广current ZIP。

Web遗漏项来源：本地Hive的5d87d4ba10d9d66cfca32941a16c60e7e52476b0、版本2.2.1，LanguageSetting.tsx明确有zh/en按钮，SettingsMenu.tsx包含RemoteAccessSection；两文件无本地未提交改动。语言入口及首批文案已接入，完整迁移待续；远程已接入并补侧栏入口，证据与缺口如上。这是现有基线缺口，不是将新功能另塞进原目标。

## 桌面主控工作流新增实证

主控已从原生聊天框接收请求并亲自编写/启动workflow，唯一真实Codex/Terra Worker显式汇报，主控独立文件核验83/4及冷恢复通过，见[记录](validation/2026-11-09-native-workflow-orchestrator.md)。这是源码runtime实证，未扩大候选包结论。内部系统通知正文改为默认折叠，Mac实际展开/收起通过；结束通知重复答复及已汇报Worker暂停原派单仍未解决。完整目标保持未完成。

#22已补修复与实际证据：真实Codex/Terra汇报回收后pending=0、独立冷恢复及旧故障spool副本原生Mac核对通过，未汇报责任/用户输入/其他投递保留，见[队列结算](validation/2026-11-09-reported-dispatch-queue.md)。上节“已汇报Worker暂停原派单”缺口已解决；结束通知重复答复仍未解决，候选包未纳入本轮修复。

远程接入已补Mac实际设置入口、中文窄窗配对码复制、回环待确认/拒绝、全局提醒、连接开关与目录绑定命令，修复并实证中英文配对关键AX信息和读取完成提示。见[原生补验](validation/2026-11-09-remote-native-ui.md)。先前“原生点击待验收”已覆盖这些具体操作；确认/撤销GUI、生产网关/手机、完整键盘/VoiceOver及Windows仍待，#18开放，候选ZIP未更新。

设置键盘新增源码实证：远程窄窗正反Tab、手动滚动后的同组切换、缩窗及键盘生成/取消配对通过，较高英文卡片的尾部操作裁切已修；模型配置英文17字号字段/能力焦点回归通过。见[记录](validation/2026-11-09-settings-keyboard-reveal.md)。只覆盖远程与模型的列出路径，其他页面、动态设备、完整字号/VoiceOver/Windows仍待；候选0e72137未包含本批。

窄窗设置源码现使用单行当前分区菜单，宽窗保留侧栏。中英文17字号、键盘选择、当前勾号、搜索与模型/角色草稿往返/冷恢复、窄窗返回会话及原输入焦点已补实证，见[紧凑导航](validation/2026-11-09-compact-settings-navigation.md)。此项未扩大为各设置页内部完整验收，候选仍0e72137未纳入本轮。

空会话恢复已补源码修复：只有启动上下文时保存恢复事实、保持空闲；有实际历史和排队输入仍沿用恢复处理。真实Hive/Pi JSONL及原始Codex无模型控制链路通过，见[空会话恢复](validation/2026-11-09-empty-session-recovery.md)。此项不代表工作流结束重复回复已解决，候选0e72137未纳入本轮。

Codex活跃主控的系统通知已接入同回合投递，针对上文工作流结束通知排到用户回合之后的路径，真实Terra主控/Worker完成47/3后只一条final，冷恢复pending0；适配器接受后Stop另验证通知ID保留、未误记完成。见[同回合投递](validation/2026-11-09-steered-system-delivery.md)。此结论限于列出路径：晚到通知、其他后端、原生窗口/Windows及候选包仍各自待验收，未将所有重复答复情形标为解决。

同回合通知冷历史已补Mac原生展示：独立fork真实47/3历史，团队通知折叠/原文、唯一final、成员report完成和工作流往返核对通过；详情移除重复全局配置卡，列表保留配置，窄窗与产物预览通过。见[原生恢复](validation/2026-11-09-steered-native-workflow.md)。这不等于流式追加/Windows/全部UI矩阵验收，候选未更新。

显式输入驱动的冷启动另补源码修复与原生实证：已有历史且无其他投递时，恢复事实进入上下文，不抢先再开回复；实际Mac窗口发送给原样Codex/Terra只新增一个回合并回复COLD_INPUT_OK，退出及冷读pending0/来源保持。手动恢复和启动失败后的清理经真HTTP/SQLite/Pi检查。见[输入恢复](validation/2026-11-09-user-input-recovery.md)。候选仍768d085，未包含这次源码修复。

已接收输入反馈已修复并补原生证据：后台接受后至厂商消息出现前仍显示正文、处理/等待状态，按 delivery ID 去重；中英文 AX 名称、冷启动和正常退出通过。使用可控本地协议对端复现，未增加真实模型结论。见[接收反馈](validation/2026-11-09-received-input-feedback.md)。Mac 构建与 Windows MSVC 检查通过，候选 ZIP 未更新，#6 与完整目标继续开放。

聊天辅助功能新增源码修复：用户/助手正文、已展开工具参数/输出及团队原文按可见状态提供文本语义。真实 Terra 主控与 Worker 历史的原生 AX 读取、通知展开/收起边界、正常退出通过，本次未运行新模型。见[聊天可读性](validation/2026-11-09-conversation-accessibility.md)。Mac 构建与 Windows 编译检查通过，完整 VoiceOver/流式/Windows 原生及候选包仍待。

上述输入恢复、已接收反馈及聊天AX语义现已纳入06ee28b候选ZIP，四组包检查通过；实际包窗口向原始Terra发送冷输入，只有一个新回合/唯一回复，退出和包内独立冷读pending0。见[最新候选](validation/2026-11-09-reading-candidate.md)。前文相关“候选未纳入”由本次具体证据更新；其他配置/协作/平台仍保留原验证范围，current未推广，完整目标开放。

一对多新增真实证据：原始Codex/Terra主控亲自启动Promise.all双Worker，独立输入/产物49/3及61/4，两派单显式reported、工具活动区间重叠、主控唯一final，三会话冷恢复pending0。Mac原生核对三会话/双分支结果/产物并正常退出。见[两位Worker](validation/2026-11-09-two-worker-live.md)。源码运行，不扩大为包内/Windows/混合厂商或完整产品验收。

双成员工作流停止新增修复：真实停止后原代码仍保留已取消任务pending1；现沿Hive账本终态/所有权清理cancelled与reported派单，保留用户/通知/汇报/未结/其他成员输入。真HTTP/SQLite/SDK回归先红后绿，可控协议双子进程停止/冷恢复pending0及旧故障数据Mac原生补验通过。见[取消队列](validation/2026-11-09-cancelled-dispatch-queue.md)。本轮无模型推理，候选ZIP尚未纳入，完整目标开放。

工作流停止新增原生实证：实际聊天/工具创建两位可控协议成员，从详情点击停止，真实run/派单/进程/队列与冷恢复核对通过。修正停止误显示为红色错误、取消原因误称汇报，中英文原生对照及真实工作流失败保留红色通过。见[原生停止](validation/2026-11-09-native-workflow-stop.md)。本轮无厂商推理，候选包尚未更新，完整目标开放。

取消队列与停止展示现已纳入78721c7候选，四组包检查通过，实际包窗口停止双成员、真实子进程退出、包内冷读pending0。见[最新候选](validation/2026-11-09-terminal-candidate.md)。前文两项“未入ZIP”由本次证据更新。此轮为可控厂商peer而非真实模型；Windows环境待提供，其余完整验收继续，current未推广。

工作流详情新增信息顺序调整：结果/原因、成员派单与日志优先，完整来源/校验/参数移至页尾并可复制。相同真实双Worker历史的Mac宽/窄窗前后对照通过，英文非空参数显示夹具/复制及原历史不变已核对，见[结果优先](validation/2026-11-09-workflow-result-order.md)。无模型推理，Windows仅编译检查，候选78721c7尚未包含本轮。

侧栏参照本机Cindy少量常驻入口，现将工作流/记忆/终端/归档收进更多，核心成员与协作常驻。同一真实三会话窄窗从会话不可见到三条可见，四路由、当前勾号、键盘/双语/空项目限制及正常退出通过，见[更多导航](validation/2026-11-09-navigation-more.md)。无模型或Shell执行，Windows仅编译与既有脚本选择器维护，候选尚未更新，完整目标开放。

成员卡片现使用Hive真实pending_task_count显示未结派单数，并补AX分组及窄窗整组操作。实际双Worker和追加派单验证1→2/另一人1，中英文单复数、会话往返/筛选、停止与进程退出通过，见[成员工作量](validation/2026-11-09-member-workload.md)。本轮无模型推理，Windows仅编译；运行中退出记录failed/runtime closing的新生命周期问题仍待核对。候选未更新，完整目标开放。

运行中正常退出被误记失败已补根因修复：Hive runner关闭时中断执行、取消等待/脚本Worker并排空清理后再关存储。真实HTTP/SQLite/协议子进程回归覆盖双Worker、嵌套、纯计算及既有三种终态，Mac实际Cmd+Q/冷启动显示已中断通过，见[关闭工作流](validation/2026-11-09-workflow-shutdown.md)。未做新模型或Windows原生验证，旧错误记录不重写；候选尚未纳入，完整目标继续。

派单详情新增原生语义修复：任务/汇报/取消原因、关联责任、问答与回复引用现可从AX读取，中英文真实Hive消息/显式report记录已核对；主控与成员草稿在派单往返、跨项目和退出重开后分别保留。见[派单可读性](validation/2026-11-09-dispatch-accessibility.md)。本轮无模型推理，Windows仅编译，完整VoiceOver/输入法仍待，候选1534986未更新。

晚到通知另取得真实Codex/Terra证据：主控先以WORK_STARTED结束后驱动才放行Worker，Worker显式report37/3，真实通知触发主控独立核验且只新增LATE_REPORT_OK 37/3一次；两会话完整冷恢复pending0、Mac实际历史/工具/工作流/产物及退出复核通过。见[晚到汇报](validation/2026-11-09-late-worker-report.md)。这是一位Worker/单通知案例，不扩大为并发通知、其他后端、Windows或所有重复情形解决；文件预览AX正文仍待补验，候选未更新。

可选中文本新增共享AX包装：文件预览短正文/路径/只读说明及长文Home/End/翻页已取得源码Mac实证，成员职责/展开思考/原始补丁复用同一包装。见[正文语义与明确待验项](validation/2026-11-09-selectable-reading.md)。选区复制被窗口状态变化和截图故障打断，其他入口/错误态尚待原生补验；未将该项标记完成，当时native PID56760仍在隔离副本运行；后续已补正常退出与下述验证，候选未更新。

同版本可选中文本补验已继续完成具体路径：文件空/二进制失败保留旧正文/恢复、局部sum与全文复制、思考和成员职责展开/收起语义、真实Pi edit返回原始补丁及完整复制均通过；窗口PID56760/63757/65464均Cmd+Q exit0且无新模型请求。见[后续记录](validation/2026-11-09-selectable-reading.md)。补丁另已补全量AX文件并独立对照字符串；前一条复制中断与窗口运行状态已更新。英文/完整VoiceOver/Windows/候选包仍各自待验，完整目标未完成。

内置Pi默认选择已补真实SDK/Hive定向检查：全局/项目/聊天优先级、默认文件不被聊天选模改写、冷恢复及失效选择保留均通过；独立解压1534986候选的包内模型gate包含同一检查也通过，请求数0。见[默认模型](validation/2026-11-09-pi-default-model.md)。此前原始补丁夹具显示其他模型系未提供目标定义/认证，未为此修改生产选择逻辑。候选/current不变，真实Pi登录/模型、Windows和完整配置体验仍待。

首次进入页已参考实际打开的 T3 Code / Beluga 落地：项目主操作、现有账号/程序入口、三步协作说明及英文/窄窗滚动经过 Mac 窗口检查；构建及 Windows 类型检查通过。仅为入口切片，完整视觉/功能对标仍待，包未更新。另清理约1.15 GiB可重建Rust增量缓存，源码/依赖输出/证据/ZIP保留。见[入口与参考记录](validation/2026-11-09-desktop-entry.md)。

新会话入口补项目名称/路径及主控选择说明。原生文件夹选择创建项目、两行草稿转移、旧Codex失败保留、界面内检查/保存兼容路径并重试同一会话、Terra冷恢复均已验证，无推理请求；修复打开失败仍显示打开中及错误AX不可读。见[项目入口](validation/2026-11-09-project-entry.md)与[源码参考](research/2026-11-09-desktop-source-references.md)。Windows实机、英文第二轮走查及完整对标仍待；包未更新。

外部程序页的版本/来源/兼容性说明、错误与保存反馈已接入可读文本；小窗口Tab到下方程序和保存按钮时自动滚动，键盘保存/重新读取及英文Browse修正均经Mac实际检查。真实版本/不存在路径检查、配置恢复与原草稿保留通过，无模型请求；macOS构建及Windows类型检查通过，Windows原生/完整VoiceOver仍待。见[程序配置阅读](validation/2026-11-09-program-reading.md)。包未更新。

协作页长文档下的任务操作、过滤器、派单卡和产物入口已接入既有焦点滚动；详情与成员会话往返有明确焦点，成员会话切换可直接输入。实际小窗验证保存/复制/取消、真实产物预览及未发送草稿，冷启动仍是原有线程与 reported 派单，见[协作键盘操作](validation/2026-11-10-collaboration-keyboard.md)。仅本轮开发二进制；候选 ZIP 未更新，完整键盘/辅助技术矩阵和 Windows 原生仍开放。

首次进入页已补默认主操作焦点和欢迎动作自动显露；侧栏新增项目表单的路径/选择/添加按钮也使用现有焦点滚动，成功新增后窄窗侧栏自动收起。最终源码真实验证了路径输入→可见添加按钮→正确项目页→直接输入未发送草稿，SQLite无消息/派单/Agent执行；见[首次项目键盘流程](validation/2026-11-10-welcome-keyboard.md)。首次系统目录选择后立即输入仍未通过，已独立记录[#24](https://github.com/tt-a1i/hive-desktop/issues/24)；本轮源码未进入34aebbe候选，完整目标继续开放。

[#24](https://github.com/tt-a1i/hive-desktop/issues/24)已定位为系统目录选择器完成后原窗口未重新获得操作系统键盘激活；仅输入框聚焦不足。共享选择器结束时交回原Window后，真实鼠标取消→键盘重开、首次确认→不点输入框首段文本、退出重开精确草稿均通过，见[目录选择窗口交接](validation/2026-11-10-picker-window-activation.md)。源码Mac/Windows编译通过；Windows原生及新候选纳入仍待，完整目标开放。

已补真实原生附件全链路：系统选中文件→项目草稿→原生菜单选Codex→显式Terra/Medium→一次原生Send→实际工具读取上传副本→精确三行回复→重开原线程/附件/未发送草稿。上传源文件临时变化后历史附件Reload仍读原副本，源文件已还原，见[附件与主会话交接](validation/2026-11-10-native-attachment-handoff.md)。附件/程序路径/导出保存三个取消路径实测正常，未机械扩展#24补丁；本轮无产品代码变化、新包或Windows原生结论，完整目标开放。

英文新建空会话的默认标签已统一为New conversation，普通/置顶/搜索显示共用判据；只翻译未绑定Agent且未自定义的默认标签，存储值、首条消息自动命名和真实标题不改。原生验证默认/显式同名中文标题、双语切换、置顶/搜索与冷恢复，旧原生对话缓存逐字节保留且无新推理，见[默认会话标签](validation/2026-11-10-default-session-labels.md)。#17仍开放，本轮源码未进入34aebbe候选。

主体布局已参照本机T3/Beluga继续整理：新建/搜索/更多共用一行，团队/协作仍常驻；项目页标题降层级、团队入口保留图标和可访问名称，起始操作横向换行。修复上轮滚动包装漏横向约束导致长会话/菜单越过侧栏的回归，先用仅父容器约束的独立构建复核，再验紧凑布局。相同默认字号窗口前后对照、Dark/20短窗、鼠标/键盘路由、置顶长标题、草稿冷恢复通过，见[主体布局](validation/2026-11-10-main-layout.md)。旧原生历史逐字节保留，无新推理；源码未纳入34aebbe候选，完整目标仍开放。

ff86889已另组装暂存开发包：独立解压/全量条目核验、三组包回归、全新状态配置原始Codex0.167.0、原生选择Terra/Medium与一次Send、工具读取41/3、原线程/中英文草稿冷恢复均有证据。首次目录确认出现一次409尚未解释，独立新状态单次选择与直接输入通过，故保留观察，不推广canonical候选/current；见[全新包流程](validation/2026-11-10-fresh-package-flow.md)。仅清理该包无进程引用的重复base约592MiB，保留ZIP/解压应用/全部状态证据。完整目标仍开放。

共享Hive HTTP错误现会带上后台实际返回的文字原因：真实重复项目409和不存在路径400均经原生窗口验证，路径输入与未发送草稿保留；HTTP/SQLite定向检查先红后绿，runtime构建/类型检查通过，见[错误原因展示](validation/2026-11-10-hive-error-reasons.md)。不改变创建/冲突语义、不吞错或重试；首次picker409触发仍未解释。本轮使用源码runtime override，所有ZIP均未更新，无推理请求。

欢迎页新增四个明确Agent配置入口，外部程序直达对应输入框；复用真实版本检查并区分未检查/未保存/输入已改/程序不存在，不把版本视为登录或模型就绪。原生四入口、真实Codex0.167.0、失败/保存/冷启动、Light16英文与Dark20中文窄窗键盘通过；期间发现并修复共享程序配置读取覆盖读取期间新输入，首次输入与Reload即刻输入均完整保留。见[Agent配置引导](validation/2026-11-10-welcome-agent-setup.md)。源码Mac构建和Windows目标检查通过；candidate仍252c19c，未更新ZIP，无认证/模型请求，完整就绪检测与完整目标仍开放。

已有项目的外部会话打开失败，现在按当前Agent定向进入对应程序路径。原生复验又发现已保存路径首次回填重置早期选区、导致新旧路径拼接；入口改在配置读取完成后聚焦并全选，关闭/读取失败/切换设置页时丢弃待交接意图。真实Codex缺失→整段替换→版本检查→键盘保存→原会话重开→冷恢复通过，原线程/一次原输入工具回复/精确草稿保留，无新推理；见[项目内程序修复](validation/2026-11-10-project-agent-repair.md)。Mac构建/Windows目标检查通过，candidate仍252c19c；其他后端同类恢复和Windows原生未据此验收。

成员页已修复长列表中键盘焦点/重命名框不可见，以及有效保存后旧校验仍残留的问题；整页采用受约束的焦点滚动，避免小窗口被筛选/表单占满。实际 Dark20 窄窗完成第六位成员键盘重命名、职责展开/精确复制、新建停止成员、删除确认取消与冷启动保留；真实 SQLite 核对原12位成员及新增1位，消息/派单/Agent执行均0。Mac构建及Windows目标检查通过，见[成员键盘流程](validation/2026-11-10-member-keyboard.md)。本轮仅源码，current/candidate仍为edbbae1；模型列表键盘、完整辅助技术及Windows原生验收仍开放。

成员启动模型列表已接入内外层焦点滚动，打开直接搜索、选择或收起后回到成员搜索，并让加载/空/错误提示可访问。原生Codex实际10项目录的末项与反向遍历、Terra保存及冷恢复、真实ENOENT后收起均通过，未启动成员/未推理；SQLite断言配置及零消息/派单/执行通过。Mac构建与Windows目标检查通过，见[成员选模键盘流程](validation/2026-11-10-member-model-keyboard.md)。仅源码，安装包仍edbbae1；Windows原生和完整辅助技术矩阵仍开放。

工作流页已补列表/详情的焦点显露，覆盖执行器长设置、运行卡、计划确认、复制与产物/派单动作；取消计划确认后回到工具栏。9条真实历史运行/1条暂停计划的隔离副本中，Mac Dark20窄窗实际完成末条运行、精确来源参数复制、执行器保存/恢复、计划取消、实际产物预览与派单往返；SQLite核对运行/日志/派单/消息/Agent执行保持不变。见[工作流键盘](validation/2026-11-10-workflow-keyboard.md)。最终Mac构建/Windows目标检查通过，仍仅源码；删除最终按钮、实时Stop及完整平台/辅助技术矩阵不据此验收，#13与完整目标保持开放。

错误恢复源码已统一后端修复入口：内置Pi账号/模型配置、外部程序指定字段，错误详情可访问；窄窗进一步修正整个输入卡片的高度边界，内容滚动且底部操作固定，多行标题显示压成单行但原值/AX保留。真实SDK/回环401/Hive/SQLite保留原请求和附件，Mac Dark20窄窗配置往返/复制/编辑/命令浮层通过；实际Codex缺失程序修复、原线程历史和七行草稿/发送焦点回归通过，未新增推理。独立队列/附件/配置/草稿/历史断言与Mac构建/Windows目标检查通过，见[错误修复与输入卡片](validation/2026-11-10-error-repair-composer.md)。仅源码，包仍7ae27ca；完整设置返回焦点、运行中交互、首项目布局与全部平台/输入法矩阵继续开放。

配置关闭后的焦点现由稳定按钮句柄与下一帧恢复：Mac实际账号↔模型键盘往返、窄窗及输入框快捷键返回可继续输入，原失败请求/附件/配置/草稿冷读不变。另以真实Codex0.167.0/Terra Medium验证45秒工具期间七行排队、按序双回复、120秒工具实际Stop及子进程提前退出、未发送草稿与中断状态冷恢复。见[返回焦点与运行中输入](validation/2026-11-10-settings-return-running-composer.md)。Mac构建/Windows目标检查通过；仅源码，包仍7ae27ca。外部程序返回独立复演、其他后端/Windows原生/完整输入法及辅助技术矩阵仍开放。

新建会话后直接输入已修复：成功创建后复用现有输入焦点交接，切换会话清除旧意图。Mac Dark20窄窗项目＋与全局New两路均无需再点编辑框即可输入，未发送草稿冷恢复精确；SQLite核心表与原聊天索引不变，无新增模型请求。见[新会话输入](validation/2026-11-10-new-chat-input-focus.md)。Mac构建/Windows编译通过，仅源码，包仍7ae27ca；其他后端原生复演及完整产品体验仍待。

暂停队列的继续操作已移至固定输入底栏，保持后台条件与handler，解决Dark20窄窗被消息卡片挤到下方的问题。真实Codex/Terra Medium实际Send→双Queue→Stop→退出→冷启动仍暂停→窄窗点击继续→A/B各一次→冷读原ID完成且无重放，草稿精确保留；内置Pi/原始外部Pi回环及Codex/Claude合成对端既有定向回归均通过。见[冷队列继续](validation/2026-11-10-cold-queue-resume.md)。Mac构建/Windows编译通过，仅源码、包仍7ae27ca；嵌套滚动/队列收起后的阅读位置已记录待修，完整目标继续。

输入区已去除队列卡片内的嵌套滚动，逐卡键盘显露，整体高度约束包含Agent状态与底部提示。短消息最小一行、助手复制/引用移到标题行，解决已复现的窄窗仅见消息操作而正文被挤出的情况；实际宽→窄、七行→一行、旧消息阅读位置、精确复制/引用及草稿恢复通过，SQLite/索引/真实Codex冷历史不变且无新请求。见[滚动与阅读](validation/2026-11-10-composer-scroll-reading.md)。Mac构建/Windows目标检查通过，仅源码、包仍7ae27ca；实时队列收起全过程未重跑，其他主题/语言与平台矩阵继续开放。

协作概览的只读任务正文默认收起，顶部展开/编辑仍可直接使用，已有草稿自动显示；真实长任务文档窄窗由首屏只有正文变为直接可见负责人/Reported/汇报摘要/产物数。展开正文文本语义、编辑焦点、草稿退出重开与键盘取消、原派单详情及37/3产物预览通过；SQLite/聊天索引/原任务与项目哈希不变，无新增模型请求。见[协作概览](validation/2026-11-10-collaboration-overview.md)。Mac构建/Windows编译通过，仅源码；包仍6edac4c，完整协作/语言/平台矩阵继续开放。

工作流列表与详情现复用既有可聚焦阅读区域，支持方向键、Home/End、逐页键；Mac实际宽窗及英文20窄窗阅读到LOG_12/来源参数、结果复制焦点往返通过。三条真实运行、14条日志、精确草稿与3条暂停系统通知冷读不变，无模型请求；Mac构建/Windows目标检查通过，见[工作流正文键盘阅读](validation/2026-11-10-workflow-page-reading.md)。仅源码、包仍6edac4c；完整辅助技术/平台与全部动态刷新路径继续开放。

定时计划新增真实调度→Worker结果→暂停→Mac历史/上次运行→冷恢复证据，只有A触发一次、B未触发，中文参数选区复制与原未发送草稿逐字核对通过。原生确认缺少说明与计划身份已修；Tab到取消、Space退出且两计划保留通过，最终删除未执行。见[定时执行与确认](validation/2026-11-10-schedule-execution.md)。Mac构建/Windows编译通过；无模型请求、仅源码、包仍6edac4c，#13与完整目标开放。

外部Pi取消请求拒绝的真实JSONL复现已补根因修复：原投递结束并回收所属进程、首个错误只通知一次；两拒绝路径的真实Hive/SQLite运行失败/令牌撤销/原投递与正文完整冷恢复、正常取消及对端退出对照通过。Pi首轮七项及Codex取消回归、TS检查/构建通过；最终复跑保留一次首次get_state超时，单项复核通过但原因未证实，无模型请求。见[外部Pi取消失败](validation/2026-11-10-external-pi-abort-rejection.md)。仅源码，包仍6148801；Windows原生/新包/完整目标开放。

Claude中断被拒绝后调用已返回但CLI仍活着已用官方SDK/真实子进程复现并修复：沿既有关闭函数等待进程退出。真实Hive/SQLite失败运行/令牌撤销/两输入原ID冷恢复及显式继续各完成一次通过；正常停止排队、四类写盘丢失、真实Claude无推理进程死亡及TS构建通过，见[Claude取消失败回收](validation/2026-11-10-claude-abort-rejection.md)。仅源码，包仍6148801；合成取消对端不等于Claude厂商模型验收，Windows原生与完整目标继续开放。

导出预览缺少可读正文/会话/截断说明已在源码补齐，并复用现有文本选择。Mac英文16宽窄窗实际滚轮/逐页/局部选择、全文复制与33,333字节系统保存逐字一致、取消保留、键盘过程开关与完整版嵌套围栏/长正文、原草稿冷恢复通过；原始状态及副本原生历史不变，SQLite零新消息/执行。见[导出阅读与保存](validation/2026-11-10-export-reading.md)。Mac构建/Windows编译通过，仅源码、包仍6148801；完整VoiceOver/语言字号/系统纯键盘及Windows原生继续开放。

成员模型目录失败现可就地进入对应后端配置并显式重读，修复入口不依赖当前聊天。Mac英文20宽窄窗实际错误→Codex字段完整替换/版本检查→键盘保存→返回同成员筛选→键盘重读/搜索真实Terra目录通过；13成员/原模型launch冷读不变、零消息/执行，源夹具哈希不变。内置预设updated_at启动刷新单列排除，未冒称数据库字节不变。见[成员模型修复](validation/2026-11-10-member-model-repair.md)。Mac构建/Windows编译通过，仅源码、包仍6148801；其他后端原生、完整引导与平台矩阵继续开放。

当前Mac开发包已刷新为ecad294，包含本轮成员/导出界面与Pi/Claude取消回收修复。六组包检查＋两组包内取消失败回归、两个实际包窗口正常退出及成员/完整导出复制冷读通过；仅删无引用重复组装base回收约513.5MiB，旧包/应用/状态保留。见[开发包记录](validation/2026-11-10-member-export-package.md)。无新模型请求，完整目标/Windows原生继续开放。

连续新建成员成功后仅清空已提交名称/职责，角色/后端沿用；真实Mac英文20窄窗Tab/Space/Return创建、重开空表单、重复名称409及收起/重开保留失败输入通过。15成员冷读仅比基线新增1位停止Codex reviewer，旧14位不变、零执行；Mac构建/Windows编译通过，见[成员表单](validation/2026-11-10-member-form-reset.md)。仅源码，包仍ecad294；在途编辑时序未单独复演，完整目标继续。

成员创建的两步HTTP已真实复现配置失败后残留无后端成员/同名409，现通过单个本地请求复用Hive addWorkerWithLaunch事务。production JSONL＋真实HTTP/SQLite验证四后端、非法输入、写入失败回滚/原名重试、默认语言及冷身份，Mac真实失败→保留输入→直接重试→冷读16成员/零执行通过。见[成员事务](validation/2026-11-10-member-atomic-create.md)。runtime构建/类型、check:hive、Mac build和Windows目标check通过，仅源码，包仍ecad294；完整目标继续。

当前Mac开发包刷新3339ce1：包内事务专项＋六组gate通过，原始Codex0.167/Terra Medium实际空团队创建→选模型→Start→成员直聊→真实文件工具80/4→Stop→两次正常退出/冷恢复通过。独立冷读保留唯一线程/1输入完成ID/原文件/两行草稿、run exited且无重放，见[成员生命周期包](validation/2026-11-10-member-lifecycle-package.md)。本轮1个真实用户模型回合，不将成员直聊充作新一对多验收；Windows原生与完整产品仍开放。

欢迎页内置Pi现显示实际已读取的本机保存数量/空配置/读取失败及下一步，冷启动不保留Ready推断。真实损坏凭据复现SDK无signal返回旧缓存，现严格读取、固定脱敏修复提示，并在首次失败保留Pi目录定位入口；生产JSONL先红后绿，真实SDK登录/移除/关闭排空回归通过。Mac英文状态0/1/2/失败恢复、中文20窄窗键盘往返、Finder定位与零执行冷读通过，见[欢迎账号反馈](validation/2026-11-10-welcome-account-status.md)。Mac/Windows目标构建及runtime检查通过，仅源码，包仍3339ce1；保存不等于真实登录有效，完整目标继续。

Mac分发新增真实图标/版本与DMG，包内身份解码、六组后台gate及Pi账号安全读取专项通过；Finder安装至Applications、全部28,865条清单、安装资源启动/欢迎/设置返回及退出回收通过，见[安装证据](validation/2026-11-10-macos-installation.md)。未签名/公证，未测下载隔离或Windows原生；完整目标继续。

图片系统Open/Save已取得真实Mac操作证据：中文/空格导入、emoji文件名保存、取消、系统覆盖提示取消保留sentinel及确认后原字节一致均通过。原图尺寸/成功提示缺失于AX树的问题已修复，源码实际AX与593×507逻辑窄窗/原尺寸/退出/冷读原上传与草稿保持通过，见[系统保存](validation/2026-11-10-image-system-save.md)。零模型/执行，Mac构建和Windows类型检查通过；VoiceOver/Windows原生等完整矩阵仍开放，本次源码修复未入db6fb20包。

成员最终删除补真实Mac实证：本轮自有stopped/Codex测试成员经筛选→取消→593×507逻辑窄窗确认删除，目标及启动配置消失，筛选/焦点与另两人原ID/职责/后端冷读保持。确认姓名、筛选计数/空结果AX缺失已修复，最终原生AX、取消及独立冷读通过，见[成员删除](validation/2026-11-10-member-deletion.md)。零推理/执行、Mac构建和Windows类型检查通过；运行中/带历史删除、VoiceOver/Windows原生仍开放，未入db6fb20。

已删除成员的旧会话prompt/resume/模型操作会重新写回孤立launch配置的问题已复现并修复：复用Hive store现有getAgent校验，两处写入统一；真HTTP/SQLite/Pi回归原包先红/源码绿，历史/其他成员配置与完整冷恢复保持且零模型请求。四组现有团队/恢复检查、TS/build通过，见[删除后会话](validation/2026-11-10-deleted-member-session.md)。未入db6fb20包，不扩大为所有删除时序或跨存储模型变更原子性。

对照实际Beluga欢迎/T3布局后，Hive欢迎加入原标识/工作台副标题、窄标题与紧凑可换行步骤；English默认16、中文16/冷启动20及列出宽窄窗、主按钮系统选择取消/外部配置往返通过，零新项目/会话/执行，Mac构建和Windows类型检查通过。原logo/三份通知字节保留，见[欢迎视觉](validation/2026-11-10-welcome-brand.md)。仅源码未入db6fb20，不扩大为竞品完整体验或所有UI矩阵已达标。

同名项目/会话的搜索结果歧义已实际复现并修复：复用Hive规范项目路径筛选，结果目录摘要/完整路径tooltip与AX可区分。真实Mac宽窄窗⌘K、beta/Shared唯一命中/Return和Down返回Alpha、两份草稿/原ID/模型/导航冷读精确保持、零模型/执行通过，见[路径查找](validation/2026-11-10-session-project-paths.md)。搜索扩展先红后绿，归档/TS/build/Mac和Windows类型检查通过；完整主题字号/VoiceOver/Windows原生待验，未入db6fb20。

近期欢迎/路径/图片/成员语义及删除后配置修复已进入46373f6 current/candidate：六组包gate＋包内删除后历史/路径专项通过，真实解压app项目草稿往返/冷读、Finder更新Applications/品牌欢迎及退出回收通过，见[本批开发包](validation/2026-11-10-navigation-package.md)。ZIP143,856,220字节，SHA4b0fe6ee…；前文“未入db6fb20”为历史阶段现已纳入。无新增厂商模型请求，完整UIUX/厂商/Windows原生与正式发行仍待。
