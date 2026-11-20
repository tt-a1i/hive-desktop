# 完整产品实施与验收

2026-11-19 当前工作以 [功能总任务 #39](https://github.com/tt-a1i/hive-desktop/issues/39) 的本地可用性和后端正确性为先。T3 Code、Cindy 和 Beluga 提供功能参考。界面改动限于实际操作所需的入口、状态和错误反馈。Apache Maka 的 Rust 分支提供内存及运行时回收参考。[Maka 源码对比](research/2026-11-19-maka-rust-preview.md)

用户已取消语音输入，交付清单以本次更新为准。普通聊天输入与草稿照常使用。

## 15 类功能的当前验收范围

各次原生操作、故障复现和对应包版本见[验收记录](acceptance-status.md)。

截至 2026-11-20。Windows 包 2299875 通过全部 **63 项包内功能检查**，未请求模型推理。检查点锁与范围保护用了 37.696 秒，部分恢复与冲突重试用了 38.884 秒，每组限时 60 秒。各行分别列出窗口、受控对端及真实模型证据。[Windows 63 项收据](https://github.com/tt-a1i/hive-desktop/actions/runs/44615629118) 原先超时的旧 ZIP 也已用匹配版本跑完全套 63 项。检查点保护与恢复分别为 25.016 秒和 45.054 秒。[旧包对照](https://github.com/tt-a1i/hive-desktop/actions/runs/44628020581)

| 功能 | 已验证的平台与对端 | 当前剩余范围 |
| --- | --- | --- |
| 1. 待发编辑、撤回与停止继续（[#40](https://github.com/tt-a1i/hive-desktop/issues/40)） | macOS 窗口及真实 Codex/Terra 验过编辑、撤回和冷恢复；真实 Hive/SQLite 与受控 Codex 覆盖拒绝追加后的停止、编辑和单次继续。[队列记录](validation/2026-11-14-functional-input-search-references.md)  Windows 受控 Codex 已验证冷态编辑、撤回、附件保留及显式单次继续。[原生窗口](https://github.com/tt-a1i/hive-desktop/actions/runs/44614866570)  受控对端已区分明确拒绝与投递不确定，后者支持先核对原文再显式放弃本地等待。 macOS 原生窗口已验证该处置，Hive 原文及草稿保留，受控对端只记录一次回合。 二次确认的独立取消按钮经 macOS 实窗验证，取消后队列和草稿逐字节不变。 | Windows 同一固定包已验[明确拒绝后编辑及单次继续](https://github.com/tt-a1i/hive-desktop/actions/runs/44637214943)和[结果不明时核对及放弃本地等待](https://github.com/tt-a1i/hive-desktop/actions/runs/44637219730)，其他真实后端仍待验；旧格式无法区分未发与收到 ACK 后中止。 |
| 2. 全文搜索、归档与阅读位置（[#38](https://github.com/tt-a1i/hive-desktop/issues/38)、[#41](https://github.com/tt-a1i/hive-desktop/issues/41)） | macOS 窗口覆盖归档搜索、元数据缺失提示及位置恢复；Windows 固定包通过 Unicode 等价查询、原消息定位和草稿返回。[Windows 作业](https://github.com/tt-a1i/hive-desktop/actions/runs/44597607498)  Windows 缺失工作树的归档搜索、打开原消息及只读冷启动已通过。[窗口收据](https://github.com/tt-a1i/hive-desktop/actions/runs/44613571276)  Windows 已验证中段滚动锚点，切回与冷开后四条原消息的纵坐标逐项相同。[冷恢复](https://github.com/tt-a1i/hive-desktop/actions/runs/44625237770) | macOS 离线后台 45 分钟、三次强杀冷开后历史和草稿摘要均保持。其他历史组合与长期跨版本阅读状态仍待验证；旧 Codex 记录未存标记时，无法可靠判定元数据是否丢失。 |
| 3. 项目文件引用与快照（[#42](https://github.com/tt-a1i/hive-desktop/issues/42)） | macOS 和 Windows 原生窗口均已验证移走工作树源文件后，重启仍能读取私有快照。Windows 同名仓库文件保留不同内容，两次应用退出码均为 0，后台均确认关闭。另有真实 Codex/Terra 读取副本。[文件引用记录](validation/2026-11-19-project-reference-worktrees.md) · [Windows 冷开收据](https://github.com/tt-a1i/hive-desktop/actions/runs/44604549909) | 其他真实后端消费引用仍待验。 |
| 4. Git、Diff 与 PR（[#43](https://github.com/tt-a1i/hive-desktop/issues/43)） | macOS 窗口验证大补丁分页、跨页暂存预览、本地推送及受控 PR；macOS 提交/推送钩子与 Windows 提交钩子在后台强杀后均完成进程回收核对。[Windows 原生作业](https://github.com/tt-a1i/hive-desktop/actions/runs/44597605905) · [Git 远端](validation/2026-11-19-native-git-remotes.md) | Windows 当前 Git/PR 窗口、实际托管 GitHub 写入及其他中断时点仍待验；远端是否已接受操作需重新核对。  102 文件回放已核对离屏暂存预览与第一页取消；展开补丁时的直接取消、滚动可达性与冷开仍待验。 |
| 5. 独立工作树生命周期（[#45](https://github.com/tt-a1i/hive-desktop/issues/45)） | macOS 窗口验证清理、保留分支和只读恢复；真实 Git 多进程检查覆盖并发恢复与删除收据，Windows 固定包对应并发检查通过。[工作树清理](validation/2026-11-19-worktree-removal.md) · [Windows 包内结果](https://github.com/tt-a1i/hive-desktop/actions/runs/44595481053)  Windows 冷开前后，两棵不同分支的脏工作树均归属正确，窗口明确拒绝清理并保留修改。[窗口收据](https://github.com/tt-a1i/hive-desktop/actions/runs/44629097764)  Windows 干净树经确认后移除，分支和处置收据保留，原会话只读冷开。[成功移除](https://github.com/tt-a1i/hive-desktop/actions/runs/44632602383) | Windows 的损坏或遗留收据自愈窗口路径仍待验；外部 Git 操作不受 Hive 内部锁保护。 |
| 6. 检查点、历史编辑、回退与分叉（[#50](https://github.com/tt-a1i/hive-desktop/issues/50)） | macOS 真实 Codex 回退与故障注入核对原身份、备份、文件、索引和冷恢复；Windows 固定包通过受控协议检查。[原生历史验证](validation/2026-11-15-native-history-live.md) · [回退恢复](acceptance-status.md#历史回退写盘后强杀后台2026-11-19) | Windows 原生选择、备份、回退和故障恢复仍待验；其他后端不能推断具有相同原生回退能力。  外来索引锁的原生回退拒绝已准备 Windows 专项。 |
| 7. 跨后端接续与冷队列（[#53](https://github.com/tt-a1i/hive-desktop/issues/53)） | macOS 验证真实 Codex→官方 ACP 接续及冷续聊；受控 ACP 验证选区、遗漏、撤回与冷恢复，Windows 固定包现行和旧快照两组均通过。[接续记录](validation/2026-11-15-session-handoffs.md) · [Windows 包内结果](https://github.com/tt-a1i/hive-desktop/actions/runs/44595481053)  Windows 原生预览已验证选区、来源、遗漏和取消后草稿恢复。[实窗回放](https://github.com/tt-a1i/hive-desktop/actions/runs/44616388325) | Windows 原生实际投递链路、其他真实厂商组合与长期运行仍待验；旧快照无法补回当时未记录的选区说明。 |
| 8. 项目脚本与工作树初始化（[#47](https://github.com/tt-a1i/hive-desktop/issues/47)） | macOS 窗口、真实 PTY 和 JSONL 验证运行中收据丢失后的读取、停止与冷开；Windows 真实 cmd/PTY 验证同类进程和清理。[脚本验收](acceptance-status.md#脚本运行期间丢失收据2026-11-19) · [初始化验收](validation/2026-11-19-worktree-setup-native.md) | 当前 Windows 从配置到初始化、重试及冷开的完整窗口链路仍待验；后台进程与收据同时丢失时不能凭空恢复运行身份。 |
| 9. 结构化提问与附件（[#51](https://github.com/tt-a1i/hive-desktop/issues/51)） | macOS 窗口及真实 Codex/Terra 验证提问附件私有副本读取；Windows 包内受控 Codex 检查覆盖多问题、副本丢失和恢复后单次交付。[桌面验收](validation/2026-11-19-codex-question-attachments-desktop.md) · [Windows 包内结果](https://github.com/tt-a1i/hive-desktop/actions/runs/44595481053) | Windows 原生选择器与真实模型回合仍待验；提交前核验不能阻止之后文件被删除或同长度替换。 |
| 10. 快捷键与输入法（[#44](https://github.com/tt-a1i/hive-desktop/issues/44)） | Windows 原生窗口通过美式→德式→美式布局切换及两项绑定；macOS Apple 拼音候选的 Return 与布局切换后的 Cmd+, 已验证。[Windows 作业](https://github.com/tt-a1i/hive-desktop/actions/runs/44596595507) · [输入法范围](acceptance-status.md#windows-布局改变后的已保存快捷键2026-11-20) | Windows 输入法组合态、其他输入源及完整快捷键动作矩阵仍待验；现有结果只覆盖所列布局和按键。  TSF 配置与中文组件的只读清单待托管 runner 实测。 |
| 11. 原生会话模式与权限（[#49](https://github.com/tt-a1i/hive-desktop/issues/49)） | macOS 真实 Codex/Terra 验证 Plan/只读与 Default/工作区写入；受控对端和窗口覆盖失效模式、草稿与历史恢复。[模式验收](validation/2026-11-19-codex-session-modes-desktop.md) | Windows 当前模式窗口及其他真实厂商能力仍待验；内置 Pi 不提供等同的原生 plan/execute 与审批轴。 |
| 12. 固定配置、多实例与 ACP 团队（[#52](https://github.com/tt-a1i/hive-desktop/issues/52)） | macOS 用同一已有账号的两套固定配置，完成真实 Codex 主控→官方 ACP 成员派单、显式汇报、文件核验和冷恢复。[真实协作](validation/2026-11-19-native-acp-collaboration.md) | Windows 真实模型团队流程、双账号与其他厂商组合和长期稳定性仍待验；这不是双账号隔离证据。 |
| 13. 只读账号配额（[#48](https://github.com/tt-a1i/hive-desktop/issues/48)） | macOS 受控 CLI 和窗口核对额度、读取时间及辅助进程回收；另有早期真实账号只读查询。[配额验收](validation/2026-11-19-codex-profile-quota.md)  Windows 原生页面从隔离 CLI 读取 78% / 300 分钟，核对进程树回收及无关进程存活。[窗口及进程回放](https://github.com/tt-a1i/hive-desktop/actions/runs/44622613933) | 真实账号切换、额度随时间变化及厂商未提供的限制仍待验证。 |
| 14. 桌面更新与回退（[#39](https://github.com/tt-a1i/hive-desktop/issues/39)） | macOS 已验证隔离更新、回退与多种中断恢复。Windows 私有渠道安装 0.1.1 和失败回退至原 0.1.0 均通过，标题可访问，同一会话草稿恢复并确认所属进程退出。[macOS 更新](validation/2026-11-19-macos-signed-update.md) · [Windows 两场景收据](https://github.com/tt-a1i/hive-desktop/actions/runs/44610051238) | Windows 已有小型签名夹具的解包后中断收据。完整包在各中断点的窗口体验、长期跨版本升级和其他文件系统仍待验。 |
| 15. 隔离 HTML 与 1Password（[#39](https://github.com/tt-a1i/hive-desktop/issues/39)） | macOS 窗口验证 HTML 预览与冷恢复；私有 `op` 夹具及 Pi SDK 回环验证引用和进程回收。[HTML](validation/2026-11-19-html-artifact-preview.md) · [1Password](validation/2026-11-19-pi-vault.md)  Windows 假 op 与管道后代已核对实际 Job 归属及后台强杀后的回收。[Windows 进程收据](https://github.com/tt-a1i/hive-desktop/actions/runs/44621023302) | Windows **WebView2 子框架导航仍出现白页，隔离验收未通过**；真实 1Password 授权及解锁仍待验。 |

另有 Windows 固定 ZIP 的 **48 图**原生回放：首尾图、滚动、切换空会话、原图字节与退出后的进程检查通过；内存数值仅代表该夹具和托管环境，不代表完整长时稳定性。[48 图作业](https://github.com/tt-a1i/hive-desktop/actions/runs/44587198224)

## 近期实施记录

Windows 归档搜索结果与按钮现在提供会话和摘要的可访问名称。静态更新标题改为使用 Windows UIA 实际读取的文本值；新包还需分别验证读屏、安装和回退。[确认依据和验证范围](acceptance-status.md#windows-归档结果与更新标题的读屏内容2026-11-20)

Windows 完整构建给原生更新及收据上传留出单独时间；每个功能的超时仍独立检查。[整作业超时的实证](acceptance-status.md#windows-完整构建的更新验收时间2026-11-20)

Windows 原生已确认文件选择器使用会话工作树，并能选中私有快照；后续冷开结果及归档只读搜索继续分别核验。[证据与验证范围](acceptance-status.md#windows-工作树文件引用冷恢复入口2026-11-20)

Windows Git 提交钩子的进程归属通过真实桌面 transport、JSONL 请求和系统句柄核对；Windows 原生作业已确认强杀后台后的同组回收和外部锁保留。[测试范围](acceptance-status.md#windows-桌面后台与-git-提交钩子归属2026-11-20)

Windows 固定包已实测预组合查询命中分解形式的历史正文、摘要、原消息定位及返回主会话；原消息字节和两处草稿均经核对。[验证范围](acceptance-status.md#windows-历史正文的-unicode-搜索与原消息定位2026-11-20)

固定 Windows ZIP 的全部 62 项包内功能检查通过。工作树并发用时 60.919 秒，现行和旧版交接冷恢复分别在各自的 60 秒内完成。[实测耗时与回放范围](acceptance-status.md#windows-工作树并发检查的时间预算2026-11-20)

Windows 更新确认标题增加了读屏名称，并可对同一便携 ZIP 独立回放安装和回退。旧包首轮在安装器交接前失败。[来源核对和原生验证边界](acceptance-status.md#windows-原生更新确认窗口2026-11-20)

Windows 包内交接检查分别验证现行与旧版快照的冷恢复和单次投递，各自保留 60 秒预算；原生窗口可在独立的功能检查失败后继续验收。[原始超时与检查范围](acceptance-status.md#windows-包内交接的冷恢复验收2026-11-20)

Windows 旧包在布局从美式切到德式后未能以原快捷键打开搜索。修复后的 Windows 包在美式、德式及切回美式后三次都能打开搜索，配置字节和测试前的系统布局都已核对。[原生基线与候选验收范围](acceptance-status.md#windows-布局改变后的已保存快捷键2026-11-20)

Windows 固定 ZIP 已完成 48 图原生查看、滚动、切换空会话及退出后的字节和进程检查。[桌面内存采样与完整回放](acceptance-status.md#windows-多图窗口完成切换与关闭2026-11-20)

包内后台完成了 30 分钟历史打开、切换、分页搜索和清空视图的连续操作，退出后保存历史完整。另一次短测记录了停止请求后 120 秒内的自然内存回落。[后台压力与 RSS 实测](acceptance-status.md#30-分钟离线历史读取2026-11-20)

Windows 快捷键专项可分别验证切换布局前后及切回后的行为，结果需结合实际系统按键与窗口变化判断。[布局切换验收范围](acceptance-status.md#windows-键盘布局切换专项验收入口2026-11-20)

Git 提交失败时，读屏可读出具体的索引锁原因。macOS 也会在后台退出时回收 PR 客户端及同组子进程。[原生窗口与受控 PR 客户端验收](acceptance-status.md#提交失败的读屏提示与-pr-客户端回收2026-11-20)

macOS 上的 Git 提交或推送遇到后台强杀时，原生宿主会回收 Git 钩子及其同组辅助进程。残留索引锁保留供检查。[中断时的进程与仓库实测](acceptance-status.md#git-提交与推送中断后的进程回收2026-11-20)

Windows 原生回放定位到了 HTML 子文档导航后显示白页的问题。新可执行文件的两个取消回调均已执行，复测仍然白屏。[子窗口实证与修复范围](acceptance-status.md#windows-html-预览的子框架导航2026-11-20)

Windows 图片夹具修正了预加载器路径和 UI Automation 加载顺序；同一 ZIP 已核对 48 张原始图片的字节，WebView2 失败时会保留实际子窗口证据，附件回归按 JSON 协议核对 Windows 路径。[Windows 首次失败与复验范围](acceptance-status.md#windows-原生验收夹具的失败定位2026-11-20)

安装器的正常、收据故障和中断案例分组运行。退出验证会核对实际系统进程，失败时可看到安装器停在了哪一步。[包内更新器故障验收](acceptance-status.md#更新器故障注入的进程退出证据2026-11-20)

Windows 多图历史可针对指定便携 ZIP 单独验收，记录窗口状态、工作集与 private bytes。[验收入口与验证范围](acceptance-status.md#windows-多图历史实测入口2026-11-20)

新交接快照会告诉接收方首尾消息，以及范围前后省略了多少条，归档来源与目标冷队列沿用同一份文本。旧快照仍按原文投递一次。[选区与冷交付验收](acceptance-status.md#交接历史的选区与遗漏说明2026-11-20)

即使唯一暂存文件在第 101 项，第一页也能打开完整提交预览。实际提交仍核对后端预检结果。[原生预览与取消对照](acceptance-status.md#第一页之外的暂存文件2026-11-20)

并发恢复和移除同一工作树时，陈旧进程不能覆盖已移除收据。持锁进程被强杀后可重新对账。[跨进程 Git 实测](acceptance-status.md#跨进程工作树移除收据2026-11-20)

macOS 上的 1Password 引用读取由桌面宿主管理。在私有 op 夹具中，读取结束或后台被强杀后，持有输出管道的辅助进程均已退出。[进程与真实授权边界](acceptance-status.md#1password-读取期间的进程回收2026-11-20)

桌面后台启动与读取内嵌 Hive 版本不再查询 npm。内嵌组件随签名桌面包升级。[离线启动对照](acceptance-status.md#内嵌-hive-启动时的更新渠道2026-11-20)

阅读位置文件中的单项损坏现在只影响该项。macOS 冷启动仍可定位其他会话的原消息，并保留草稿。[原生窗口前后对照](acceptance-status.md#阅读位置文件中的单条损坏记录2026-11-20)

多问题回复在提交前核对每份私有附件。途中缺失时保留草稿，恢复后可以重试一次。[附件丢失与恢复验收](acceptance-status.md#多问题回复提交时丢失附件2026-11-20)

macOS 的 Codex 账号与额度探测会回收先退出的 CLI 遗留的辅助进程，设置页仍可读取完整额度窗口。[配额探测与进程回收验收](acceptance-status.md#codex-配额探测的辅助进程回收2026-11-20)

只读成员会话可以在恢复历史文件后从同一窗口重新核验原身份并打开。macOS 实测保留原草稿、历史和成员导航。[同页恢复验收](acceptance-status.md#成员历史文件恢复后在原窗口重开2026-11-19)

在聊天和侧栏搜索中，macOS 系统拼音显示候选时，两条改绑的全局快捷键均保留了当前输入焦点。聊天中首次 Return 确认候选，第二次普通 Return 才发送一次。[输入法候选实测](acceptance-status.md#改绑快捷键与系统拼音候选2026-11-19)

Codex 归档搜索可识别带标记的 Hive 投递元数据丢失，提示恢复原文件。macOS 窗口实测覆盖带项目文件引用的消息、缺失工作树与原投递 ID。[归档搜索验收与旧记录边界](acceptance-status.md#codex-投递元数据丢失后的归档搜索2026-11-19)

历史回退在原生对端写盘、桌面后台尚未收到回执时中断，已通过 macOS 故障注入验证冷启动核对与显式继续。[回退恢复验收](acceptance-status.md#历史回退写盘后强杀后台2026-11-19)

聊天工具卡的原始补丁按页排版，页内文字可选择，工具栏仍可复制完整补丁。回到合并或并排视图会释放分页阅读器。[macOS 内存和冷恢复验收](acceptance-status.md#聊天工具卡的大补丁分页2026-11-19)

长会话导出已补测取消系统保存面板、反复打开、关闭及切换会话后的活动内存，保留快照时仍可完整复制。[长会话复验](acceptance-status.md#长会话导出关闭后的内存2026-11-19)

手动项目脚本执行期间，即使运行收据消失，后台仍按项目和请求编号识别原进程，设置页可以读取日志并停止该次执行。macOS 真实 PTY 与 JSONL 通信已核对重复请求、版本冲突和命令实际执行次数。Windows 的同场景已取得真实 cmd、PTY 与进程清理收据。[脚本收据验收](acceptance-status.md#脚本运行期间丢失收据2026-11-19)

macOS 本地优化包已验证原生 Codex 和官方 ACP 的真实协作及被动冷恢复，并实测长会话 Markdown 导出的内存。当前版本的 Windows 原生包与实际窗口仍待复验。更新使用隔离渠道，本轮交付本地证据。[协作验收](validation/2026-11-19-native-acp-collaboration.md) · [导出内存实测](validation/2026-11-19-streaming-markdown-export.md)

桌面更新在安装或回退完成后回收解包文件和安装事务。回退后保留经过签名校验的归档供离线重试，收据写入失败时保留恢复入口。[更新清理验收](validation/2026-11-19-update-installer-ownership.md#安装和回退完成后的清理)

桌面更新下载被强制中断时，下次检查和下载会回收已退出进程遗留的半包。macOS 原生设置页通过中断、重开、重试及签名归档校验。[下载重试验收](validation/2026-11-19-update-installer-ownership.md#更新下载被强制中断后的重试)

macOS 更新准备使用文件克隆减少临时磁盘占用，实际限额卷验证低空间准备及错误后的重试。设置页会提示空间不足，并可用同一签名归档离线继续。安装器在首次记录写入前被终止时，手动恢复可核对并保留旧版，清理候选事务。[低空间和启动中断验收](validation/2026-11-19-update-installer-ownership.md#低磁盘空间准备和首次记录前的恢复)

新格式的更新准备在创建目录时记录进程归属。检查更新或重试准备可回收进程已退出且尚未交接的事务。已交接的事务继续由安装器或手动恢复入口处理。[解包中断验收](validation/2026-11-19-update-installer-ownership.md#更新准备进程中断后的清理)

更新回退后，结果收据写入失败也会重开通过校验的旧版。真实签名应用验证了原数据库和草稿保留、恢复入口补写原始诊断及避免重复启动。[回退启动验收](validation/2026-11-19-update-installer-ownership.md#写入结果失败后的回退启动)

#45 的独立会话工作树清理已补齐。macOS 原生窗口验证本地文件保护、确认清理、保留分支及冷启动只读搜索。真实 bare 仓库和包内 JSONL 验证已推送证明与在用保护。[工作树清理验收](validation/2026-11-19-worktree-removal.md)

工作树移除后若收据尚未写完，冷打开会确认原仓库和分支，再补写收据。原派单继续打开同一份只读历史。真实 Git 和 macOS 解包窗口已验证收据写入失败及冷恢复。[工作树收据验收](validation/2026-11-19-worktree-removal.md#清理完成后收据中断)

独立工作树在 Git 面板推到不同名的远端分支后，冷重开可按已验证的推送目标确认清理。macOS 解包窗口核对了真实本地推送、两端分支保留和原会话只读恢复。[不同名分支验收](validation/2026-11-19-worktree-removal.md#推送到不同名分支后的清理)

#43 的 macOS 原生窗口验证了过期推送拒绝、工作树提交推送到本地 bare 仓库，以及受控 GitHub 对端的 PR 创建、更新和草稿状态读回。辅助功能树可读取推送和 PR 结果，以及需核对的原因。实际托管账号写入和当前 Windows 原生操作仍待验收。[Git 远端验收](validation/2026-11-19-native-git-remotes.md)

macOS 本地包确认同一 HTML 产物跨文件详情复用预览窗口，保留交互状态。主窗口正常退出时会结束仍打开的预览和 Node 后台。只读 Codex 冷历史同次读取校验完整性，隔离大日志探测的 Node 峰值 RSS 中位数下降 124.0 MiB。[窗口与冷历史验收](validation/2026-11-19-preview-windows-cold-codex.md)

同一会话的多张工具卡写入同名 HTML 时，首次打开各张工具卡的交互预览会分别保存当时的内容。macOS 解包实测删除源文件并重启后，每张工具卡仍可恢复各自的快照，旧版按路径保存的快照可单独打开。[同名工具产物验收](validation/2026-11-19-html-artifact-preview.md#同名工具产物的独立预览)

文件详情按页排版大文件，保留原行号和全文复制。macOS 解包窗口通过翻页、快照存储错误后的重试，以及删除源文件后冷启动恢复交互。关闭文件详情会停止等待读取、定位与快照结果，迟到的快照不会打开预览。预览聚焦时 ⌘Q 走正常退出流程，可以确认或取消，重开会话保留草稿。[分页和存储验收](validation/2026-11-19-html-artifact-preview.md#大文件源码分页与快照存储恢复) · [预览窗口退出验收](validation/2026-11-19-html-artifact-preview.md#从-html-预览退出及关闭后收到快照)

Pi 文件的原生会话 ID 与记录不符时，正文搜索和冷打开都会报告来源异常。恢复原文件后，同一会话可重新打开。已有 ACP 会话缺少本地路径时会提示并拒绝继续。macOS 解包窗口验证了全局搜索、侧栏、原消息定位和只读恢复。[历史来源验收](validation/2026-11-19-unicode-history-search.md#历史文件与原生会话身份)

Codex 搜索与冷打开使用同一组可见记录和投递 ID。可选索引损坏时，可读正文仍能查到，并提示历史可能不完整。macOS 原生解包窗口通过两项目定位、断线清理和重连。辅助功能可读取恢复提示。[Codex 冷历史验收](validation/2026-11-19-unicode-history-search.md#搜索断线与重连的原生窗口对照)

Codex 结束当前回合并拒绝追加输入时，正在停止的会话保留原待发消息。消息可编辑，显式继续后只投递一次。两个 macOS 解包版本在实际 Hive 队列和受控协议对端完成前后对照。[停止与继续验收](validation/2026-11-14-functional-input-search-references.md#追加被拒绝时停止并继续)

归档切换查询、离开页面或后端断开时会释放旧搜索结果。macOS 解包实测无效查询不能恢复之前的会话，新查询可恢复准确目标。长分叉夹具中，打开归档第一页时 Rust 进程的峰值物理内存增量中位数由 21.3 MiB 降至 9.3 MiB。[归档与搜索内存验收](validation/2026-11-19-unicode-history-search.md#归档结果与桌面搜索内存)

会话内查找支持 Unicode 的预组合与分解形式。macOS 解包实测 Markdown、思考、工具补丁和助手失败的定位，以及只有已保存调用的工具恢复到原卡片。[会话内查找验收](validation/2026-11-19-unicode-history-search.md#会话内-unicode-查找与工具消息定位)

团队等页面或窄窗侧栏隐藏对话列表时，Rust 桌面释放 Markdown 渲染对象和按需历史图片。Mac 760 像素宽窗口打开侧栏时，夹具中的桌面 RSS 中位数下降 6.6 MiB，返回后原会话和草稿保留。[隐藏会话内存验收](validation/2026-11-19-long-history-markdown-memory.md#隐藏当前会话时释放渲染资源)

冷态历史原图读取已与待投递文件解耦，内置 Pi 可以在后台重启后按先前接收的投递 ID 找回原图。桌面已回收仅用于浏览的空闲 Codex 0.169.0 进程。macOS 解包窗口实测浏览八个会话后，常驻测试进程从八个降为一个，返回工作区后降为零。已由 Hive 接管的会话和待发任务通过独立检查。当前 Windows 原生验收继续跟踪。[历史原图](validation/2026-11-19-cold-browse-and-history-images.md) · [浏览进程回收](validation/2026-11-19-codex-desktop-view-release.md)

跨 Agent 接续的自动刷新只读取状态，窗口按事件序号保留较新的队列和交互。隔离冷 ACP 对照的刷新帧从 6.36 MB 降到 8,767 字节，完整打开仍能读取全部 1,505 条记录。[接续状态验收](validation/2026-11-19-handoff-target-state.md)

ACP 接续目标在身份及首条投递回执均可核对时，可冷态编辑和撤回后续尚未投递的输入。放弃不确定投递后，当前窗口立即清除旧的等待状态，并保留原生记录和 Hive 审计。[冷队列验收](validation/2026-11-19-codex-search-acp-cold-queue.md#acp-接续完成后的冷队列)

选择代码检查点回退前会核对 Git 索引锁和目标对象。macOS 原生窗口验证锁存在时保留原历史、文件和暂存区，解除锁后能回退并单独重开完整备份。[锁冲突和冷恢复验收](validation/2026-11-19-native-rewind.md#git-索引锁与回退前检查)

Codex 额度设置已通过 macOS 原生窗口和真实只读查询核对，辅助功能可读取窗口、普通使用资格与后台返回的读取时间。[额度验收](validation/2026-11-15-project-scripts-worker-worktrees-account-limits.md#2026-11-19-原生窗口与读取时间)

项目文件选择器可用不同 Unicode 写法找到同一个文件。macOS 原生窗验证了工作树隔离、文件准确引用和源文件移走后的冷态快照。ACP 冷队列版本也重跑了真实 Terra 主控和官方 ACP 成员的派单、文件汇报和冷恢复。[文件名验收](validation/2026-11-19-project-reference-worktrees.md#unicode-文件名) · [真实协作复验](validation/2026-11-19-native-acp-collaboration.md#acp-冷队列修复后的真实复验)

自定义快捷键保存和分发使用同一平台键位映射，避免不同写法的等效键位绕过冲突检查。macOS 解包实测默认组合键和旧自定义组合键均触发对应动作，Shift‑Tab 和窗口失焦可退出录制。[原生快捷键验收](validation/2026-11-19-keybindings-native.md)

macOS 的 Node 后台被强制终止后，正在运行的项目脚本和同组子进程会被回收。中断执行记录保留，单次运行的日志缺失也不会阻止后台冷启动。[脚本进程与恢复验收](validation/2026-11-19-worktree-setup-native.md)

损坏的项目脚本运行记录不会阻止后台冷启动，其他工作树仍可按原请求读取初始化、明确重试或跳过。macOS 原生窗已验证原命令、执行次数及草稿冷恢复。[损坏记录验收](validation/2026-11-19-worktree-setup-native.md#损坏的脚本运行记录与冷恢复)

手动项目脚本遇到损坏记录时会提示，且仍可读取其他运行的历史和日志。macOS 解包窗口验证了另一项目的坏记录保留、当前工作树脚本启动和停止、子进程回收及冷重开。[手动脚本历史验收](validation/2026-11-19-worktree-setup-native.md#手动脚本历史的损坏记录隔离)

手动脚本在启动前被拒绝时，可以按原请求编号核对后重新选择脚本。macOS 解包验证了另一项目的坏记录不再阻塞恢复，当前请求的损坏记录仍会保护原请求。[启动拒绝恢复验收](validation/2026-11-19-worktree-setup-native.md#核对启动前被拒绝的请求)

1Password 读取命令已退出而辅助进程仍持有管道时，移除引用或退出可以回收本次读取的进程。macOS 解包窗口验证了两种取消、原消息冷恢复和使用新引用重试一次。[密码库退出验收](validation/2026-11-19-pi-vault.md#读取命令已退出时的取消与退出)

Codex 问题附件按整次回复限制数量。草稿保存跟随原问题，切换会话仍可读回。一个项目的损坏收据不会阻断其他项目的普通问答或审批。[问题附件验收](validation/2026-11-19-codex-question-attachments-desktop.md)

Codex 问题附件的重试只核对当前提问。其他损坏收据不影响提交前失败后的重试，可读收据仍能冷启动查看。macOS 原生窗口验证恢复副本后只交付一次。[重试和冷恢复验收](validation/2026-11-19-codex-question-attachments-desktop.md#收据损坏时保留已验证历史和可重试回复)

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
