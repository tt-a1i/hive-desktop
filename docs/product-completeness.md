# 完整产品实施与验收

2026-11-19 当前工作以 [功能总任务 #39](https://github.com/tt-a1i/hive-desktop/issues/39) 的本地可用性和后端正确性为先。T3 Code、Cindy 和 Beluga 提供功能参考。界面改动限于实际操作所需的入口、状态和错误反馈。Apache Maka 的 Rust 分支提供内存及运行时回收参考。[Maka 源码对比](research/2026-11-19-maka-rust-preview.md)

用户已取消语音输入，交付清单以本次更新为准。普通聊天输入与草稿照常使用。

macOS 本地优化包已验证原生 Codex 和官方 ACP 的真实协作及被动冷恢复，并实测长会话 Markdown 导出的内存。当前版本的 Windows 原生包与实际窗口仍待复验。更新使用隔离渠道，本轮交付本地证据。[协作验收](validation/2026-11-19-native-acp-collaboration.md) · [导出内存实测](validation/2026-11-19-streaming-markdown-export.md)

桌面更新在安装或回退完成后回收解包文件和安装事务。回退后保留经过签名校验的归档供离线重试，收据写入失败时保留恢复入口。[更新清理验收](validation/2026-11-19-update-installer-ownership.md#安装和回退完成后的清理)

桌面更新下载被强制中断时，下次检查和下载会回收已退出进程遗留的半包。macOS 原生设置页通过中断、重开、重试及签名归档校验。[下载重试验收](validation/2026-11-19-update-installer-ownership.md#更新下载被强制中断后的重试)

#45 的独立会话工作树清理已补齐。macOS 原生窗口验证本地文件保护、确认清理、保留分支及冷启动只读搜索。真实 bare 仓库和包内 JSONL 验证已推送证明与在用保护。[工作树清理验收](validation/2026-11-19-worktree-removal.md)

工作树移除后若收据尚未写完，冷打开会确认原仓库和分支，再补写收据。原派单继续打开同一份只读历史。真实 Git 和 macOS 解包窗口已验证收据写入失败及冷恢复。[工作树收据验收](validation/2026-11-19-worktree-removal.md#清理完成后收据中断)

#43 的 macOS 原生窗口验证了过期推送拒绝、工作树提交推送到本地 bare 仓库，以及受控 GitHub 对端的 PR 创建、更新和草稿状态读回。辅助功能树可读取推送和 PR 结果，以及需核对的原因。实际托管账号写入和当前 Windows 原生操作仍待验收。[Git 远端验收](validation/2026-11-19-native-git-remotes.md)

macOS 本地包确认同一 HTML 产物跨文件详情复用预览窗口，保留交互状态。主窗口正常退出时会结束仍打开的预览和 Node 后台。只读 Codex 冷历史同次读取校验完整性，隔离大日志探测的 Node 峰值 RSS 中位数下降 124.0 MiB。[窗口与冷历史验收](validation/2026-11-19-preview-windows-cold-codex.md)

同一会话的多张工具卡写入同名 HTML 时，首次打开各张工具卡的交互预览会分别保存当时的内容。macOS 解包实测删除源文件并重启后，每张工具卡仍可恢复各自的快照，旧版按路径保存的快照可单独打开。[同名工具产物验收](validation/2026-11-19-html-artifact-preview.md#同名工具产物的独立预览)

Pi 文件的原生会话 ID 与记录不符时，正文搜索和冷打开都会报告来源异常。恢复原文件后，同一会话可重新打开。已有 ACP 会话缺少本地路径时会提示并拒绝继续。macOS 解包窗口验证了全局搜索、侧栏、原消息定位和只读恢复。[历史来源验收](validation/2026-11-19-unicode-history-search.md#历史文件与原生会话身份)

归档切换查询、离开页面或后端断开时会释放旧搜索结果。macOS 解包实测无效查询不能恢复之前的会话，新查询可恢复准确目标。长分叉夹具中，打开归档第一页时 Rust 进程的峰值物理内存增量中位数由 21.3 MiB 降至 9.3 MiB。[归档与搜索内存验收](validation/2026-11-19-unicode-history-search.md#归档结果与桌面搜索内存)

会话内查找支持 Unicode 的预组合与分解形式。macOS 解包实测 Markdown、思考、工具补丁和助手失败的定位，以及只有已保存调用的工具恢复到原卡片。[会话内查找验收](validation/2026-11-19-unicode-history-search.md#会话内-unicode-查找与工具消息定位)

团队等页面或窄窗侧栏隐藏对话列表时，Rust 桌面释放 Markdown 渲染对象和按需历史图片。Mac 760 像素宽窗口打开侧栏时，夹具中的桌面 RSS 中位数下降 6.6 MiB，返回后原会话和草稿保留。[隐藏会话内存验收](validation/2026-11-19-long-history-markdown-memory.md#隐藏当前会话时释放渲染资源)

冷态历史原图读取已与待投递文件解耦，内置 Pi 可以在后台重启后按先前接收的投递 ID 找回原图。桌面已回收仅用于浏览的空闲 Codex 0.169.0 进程。macOS 解包窗口实测浏览八个会话后，常驻测试进程从八个降为一个，返回工作区后降为零。已由 Hive 接管的会话和待发任务通过独立检查。当前 Windows 原生验收继续跟踪。[历史原图](validation/2026-11-19-cold-browse-and-history-images.md) · [浏览进程回收](validation/2026-11-19-codex-desktop-view-release.md)

跨 Agent 接续的自动刷新只读取状态，窗口按事件序号保留较新的队列和交互。隔离冷 ACP 对照的刷新帧从 6.36 MB 降到 8,767 字节，完整打开仍能读取全部 1,505 条记录。[接续状态验收](validation/2026-11-19-handoff-target-state.md)

ACP 接续目标在身份及首条投递回执均可核对时，可冷态编辑和撤回后续尚未投递的输入。放弃不确定投递后，当前窗口立即清除旧的等待状态，并保留原生记录和 Hive 审计。[冷队列验收](validation/2026-11-19-codex-search-acp-cold-queue.md#acp-接续完成后的冷队列)

项目文件选择器可用不同 Unicode 写法找到同一个文件。macOS 原生窗验证了工作树隔离、文件准确引用和源文件移走后的冷态快照。ACP 冷队列版本也重跑了真实 Terra 主控和官方 ACP 成员的派单、文件汇报和冷恢复。[文件名验收](validation/2026-11-19-project-reference-worktrees.md#unicode-文件名) · [真实协作复验](validation/2026-11-19-native-acp-collaboration.md#acp-冷队列修复后的真实复验)

自定义快捷键保存和分发使用同一平台键位映射，避免不同写法的等效键位绕过冲突检查。macOS 解包实测默认组合键和旧自定义组合键均触发对应动作，Shift‑Tab 和窗口失焦可退出录制。[原生快捷键验收](validation/2026-11-19-keybindings-native.md)

macOS 的 Node 后台被强制终止后，正在运行的项目脚本和同组子进程会被回收。中断执行记录保留，单次运行的日志缺失也不会阻止后台冷启动。[脚本进程与恢复验收](validation/2026-11-19-worktree-setup-native.md)

Codex 问题附件按整次回复限制数量。草稿保存跟随原问题，切换会话仍可读回。一个项目的损坏收据不会阻断其他项目的普通问答或审批。[问题附件验收](validation/2026-11-19-codex-question-attachments-desktop.md)

Codex 核对线程及回合的原生回执后更新模式。ACP v1 可读取并选择后端提供的会话模式，官方 Codex ACP 可分别选择协作与访问权限。[原生模式验收](validation/2026-11-19-codex-session-modes-desktop.md)

Codex 保存的模式失效时，会话仍可只读打开。用户可以检查并明确保存可用的替代项。修复不会触发推理或清空历史、草稿与待发消息。历史分叉也可在独立工作树中检查模式，无需先激活 Agent。macOS 解包窗口已通过冷重开和进程退出验证。[保存模式恢复验收](validation/2026-11-19-codex-session-modes-desktop.md#保存模式失效后的恢复)

## 2026-11-14 历史计划

2026-11-14。用户已在当前会话确认并授权持续推进。本文整理执行顺序；需求和工作项继续使用 GitHub Issues，父任务为 #1，首批工作为 [#29](https://github.com/tt-a1i/hive-desktop/issues/29)。完整目标仍未完成。

## 产品约定

- 用户最新明确：UI/UX 先复刻 T3 Code，作为主窗口、侧栏、聊天/输入区、菜单、全部设置和引导的唯一基准。此前结合 Cindy、Beluga 的安排由此取代。逐页核对真实操作与对应源码，不添加没有实际功能的占位选项；Hive 专属成员/协作功能采用相同组件和交互。
- 默认进入聊天工作台；团队成员、协作与工作流可从空项目发现和进入。团队与协作保留固定入口，替代 11 月 13 日普通工作区隐藏入口的安排。
- 后端正确性优先。复用 Hive 的身份、派单、显式汇报、消息水位、投递和恢复语义，内置 Pi 不新增审批。四后端均需覆盖主控和成员，分别记录混合后端组合。
- 账号验收范围：用户于本轮明确 Claude 尚未登录/配置，无需验收 Claude；Antigravity 同样无需验收。不尝试登录、配置或调用其真实模型，保留未验证边界并继续其他工作。当前真实模型验证只用已授权的 Terra。
- 设置覆盖账号、模型/接口、外部 Agent、聊天、外观、团队、工作流、通知和远程接入；控件有实际作用、范围说明、写入反馈和错误恢复。
- 验收以真实安装包中的连续操作为准：引导 → 配置 → 打开项目 → 聊天/工具 → 派单/显式汇报/主控继续 → 结果检查 → 停止/退出/恢复。局部构建、mock、截图不代表完整交付。

## 本轮基线和参考证据

- Hive Desktop：从 main `26ebdea09ef407c4bf06d3d42743c25fe1ec768f` 开始，工作区干净。本轮实查 current/candidate ZIP 为该提交的 release，SHA-256 `22b6262e0fb33e351a0fce4a2b0d0c18cb4187fbb9a796dd042a9ce6773996ce`；README 的 `c0c1163` 记录属于历史状态。
- Hive 权威仓库：已刷新 origin/main，为 `6b4e83feffd87e7c5e389c9cba879948998150c0`。只读核对，不修改另一个正在实施的 Hive 工作区。当前 Desktop 仍固定 `@tt-a1i/hive@2.2.1` 与既有补丁；升级依赖须另验兼容。
- T3 Code 源码：`e5835c993a6af969088d1c8fe62278f711852b0a`。已读 [SettingsPanels.tsx](https://github.com/pingdotgg/t3code/blob/e5835c993a6af969088d1c8fe62278f711852b0a/apps/web/src/components/settings/SettingsPanels.tsx) 和 [settingsLayout.tsx](https://github.com/pingdotgg/t3code/blob/e5835c993a6af969088d1c8fe62278f711852b0a/apps/web/src/components/settings/settingsLayout.tsx)：聊天显示选项、设置范围及搜索目标的焦点/滚动处理作为实现参考。
- T3 Code 0.1.9 原生窗口：实际观察新会话、已有会话和 20 个设置分类首页，包括 Providers、Threads、Chat、Projects、Keybindings 等。未发送模型消息、未改设置，因此不声称验证其保存行为。
- 最新唯一参考实查：T3 Code 0.1.9 / `24e77473ecf4`，约256px侧栏、居中限宽聊天/输入区；设置是背景模糊上的居中圆角浮层，带左侧分组搜索导航、右侧分类面包屑与关闭按钮。下一步先复刻设置浮层与导航，再逐页收口主窗口、聊天和输入区；现有 Hive 后台能力与草稿/保存语义保留。Cindy/Beluga 以下记录仅作历史证据。
- Cindy：`3a474759524e8bf804bda4ddf6d2f4c3188918e2` 的 [SettingsSidebarNav.tsx](https://github.com/makecindy/cindy/blob/3a474759524e8bf804bda4ddf6d2f4c3188918e2/apps/desktop/src/renderer/components/settings/SettingsSidebarNav.tsx)，核对搜索导航、选中状态与可访问语义。
- Beluga 原仓库现重定向至 `beluga-labs/beluga`，本轮 main `c5c991aa99fd25ff47e3d8a80a5e9698bc30a036`；原生窗口仍在欢迎引导，仅观察该页面，未跳过引导或推断后续流程。
- T3布局和组件在Rust/GPUI中实现，保存其MIT许可并随包分发；固定源码及哈希记录于本地参考证据。历史Cindy/Beluga资料不再作为新UI实现依据。

## 按完整流程推进

界面实施以 T3 Code 为唯一基准贯穿下表。先将当前整页设置调整为 T3 的设置浮层及分组导航，再按主窗口/侧栏、聊天与输入区、具体设置与引导逐页复刻；同时继续修复实际后端和 Windows 验证问题。

| 顺序 | 缺口与结果要求 | 核验方式 |
| --- | --- | --- |
| 1 | 空项目即可找到团队/协作/工作流；聊天设置支持工具活动阅读和思考默认展开，旧偏好兼容、重启保留、保存失败不改变生效值；开发预览不能改写用户文件 | Mac 原生空项目导航、宽/窄窗口、中英文设置、真实历史和冷启动；持久化错误路径 |
| 2 | 梳理并补齐成员配置、主控选择与协作主页的创建、开始、停止、查看结果、返回会话流程；区分创建成员、启动成员与派单 | 真实 Hive HTTP/SQLite、原生窗口、实际 Agent 派单/汇报/主控核验、异常与冷恢复 |
| 3 | 账号/模型/外部程序和首次引导互通；状态与修复入口准确，默认值和会话实际值区分；设置保存/取消/校验/草稿覆盖完整 | 原生真实表单与独立配置冷读，保留原用户配置与未保存草稿；账号授权单列 |
| 4 | 聊天操作、附件、工具过程、Diff、查找/归档/导出、快捷键、长历史、输入法与可访问性逐流程核验并修复 | 真实输出与文件、长历史和跨页面返回；记录输入法/辅助技术/性能的具体范围 |
| 5 | 工作流/远程/通知与 Web 既有能力逐项对照，补齐实际操作和状态反馈 | 真运行/计划/配对及冷恢复，系统授权和真实网关分别记录 |
| 6 | 已授权后端角色与混合组合、退出/异常清理、Mac/Windows 打包和安装验收 | 按实际平台分别记录；真实模型继续仅用用户指定 Terra，Claude/Antigravity 本轮不验收；签名/公证及系统权限需实际条件 |

## 每批交付

2026-11-14 第二批补齐协作起步/空筛选恢复，修复窄窗首屏操作及任务输入聚焦，实际窗口完成 Codex 主控 → 既有成员 → 显式汇报 → 独立字节核验及两次完整冷读，见[精确范围](validation/2026-11-14-collaboration-start-and-live.md)。Pi 当前 `invalid_grant`，不重复登录；Claude/Antigravity 本轮不验。Windows 检查中的实际阻断继续修复并分别记录。

记录改动前触发方式、修复后的可观察行为、检查命令/退出码、原生操作和包内版本。源码修复、历史证据重读与新真实 Agent 调用分开记录。维护可复查的未完成项，禁止用大量外观微调代替功能缺口的收口。

T3设置浮层与导航的实际配置、焦点和恢复流程跟踪于[#36](https://github.com/tt-a1i/hive-desktop/issues/36)，已进入0e478c2优化Mac开发包并完成有限实机验收；背景模糊、Windows及后续主窗口、聊天、具体设置与引导继续开放。

标题菜单/双击重命名、项目面包屑和单行模型/推理工具栏跟踪于[#37](https://github.com/tt-a1i/hive-desktop/issues/37)。实际调试窗口与一次真实Terra工具停止/队列继续、两次完整冷读已通过，见[本批证据](validation/2026-11-14-t3code-chat-controls.md)；优化包和Windows另验，不覆盖尚未实际操作的Pi命令及其他差距。

仅清理本项目可再生缓存或已被替代且不占用的重复产物；保留用户数据、原始参考、必要验证状态及可运行回退。存储不足不以删除证据或测试规避。
