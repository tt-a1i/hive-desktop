# 完整目标的当前验收状态

2026-11-10。目标仍是可日用、融合T3 Code/Beluga/Cindy体验并参考Maka架构的完整macOS和Windows客户端，复用Hive Web协作；以下按原目标列出证据及缺口，不能用已实现项目反过来缩小范围，也不表示表中已经穷尽所有对标细节。

## 按原目标核对

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
| 现有Web协作与配置整合 | 团队职责/角色模板、任务与冲突、派单/消息/汇报、记忆、工作流、通知及程序/模型/认证配置已有接入 | 语言偏好与累计1046条主要页面/配置/聊天查找/协作/资源终端及静态和上下文错误文案已接入，模型编辑已补固定操作/提示、Tab与空格保存及冷读回，见[配置窄窗](validation/2026-11-09-model-editor-footer.md)，已纳入2b3c07e候选并补包内配置/退出恢复，见[配置候选](validation/2026-11-09-model-candidate.md)；模型/接口编辑另已补焦点与标签显露、正反Tab和实际滚动，见[滚动证据](validation/2026-11-09-model-editor-scroll.md)，新增/覆盖草稿及保存、英文17字号已补窄窗实测，完整字号/可访问性/Windows等边界仍在[#20](https://github.com/tt-a1i/hive-desktop/issues/20)；完整页面迁移、真实切换/草稿/窄窗仍未完成，见[语言记录](validation/2026-11-09-interface-language.md)、[#17](https://github.com/tt-a1i/hive-desktop/issues/17)；远程配置已接入并通过回环SAS配对和撤销断网/重开验证，原生设置/开关/配对码/拒绝已补包内实证，确认/撤销GUI、真实网关和Windows仍待，见[原生补验](validation/2026-11-09-remote-native-ui.md)及[最新候选](validation/2026-11-09-settled-candidate.md)、[#18](https://github.com/tt-a1i/hive-desktop/issues/18)；工作流删除最终按钮等仍待验收 |
| 停止、重连、恢复、窗口生命周期 | 所有权清理、队列/身份恢复、Mac实际后台死亡/重连；Mac EPERM回归先红后绿；最新源码Cmd+Q、冷启动草稿/语言恢复、窗口关闭均exit0并清理后台，见[原生补验](validation/2026-11-09-native-resume.md) | 旧21544/54534已不在进程表，其退出原因未证实；候选包和Windows矩阵未完成，不承诺恢复任意中断工具或exactly-once |
| macOS可实际使用 | current为1acd805；候选fb3dcd0整合模型/推理面板与设置导航，三组包gate、实际首次配置、已有真实Terra恢复、短窗键盘、原生发送真实Terra工具/回复和运行中草稿/冷恢复通过，见[最新候选](validation/2026-11-10-model-settings-candidate.md)；此前真实模型证据按原范围保留 | 完整页面、输入法、远程/工作流等矩阵未完成；候选尚未作为current推广 |
| Windows首版支持 | 已有平台接线与监督记录，本机默认功能MSVC类型检查通过；[跨平台](validation/2026-11-08-windows-cross-check.md) | 本批未取得Windows原生窗口、模型、外部CLI及包验收。用户明确暂时无Windows机器、先继续其他实施；不处理CI不等于取消Windows目标 |
| 完整、精美、Cindy/Maka级体验 | 已有固定源码参考与安装Cindy局部原生走查，并实施多轮聊天/配置/导航改进 | 整体对标仍未证实；还须按实际安装体验做全流程及边角检查，不能以局部截图或当前清单宣称一致 |
| 可安装与交付 | 有冻结Node/native依赖的Mac便携开发包及实际PTY/HTTP/SQLite检查 | 未签名/未公证；正式发行、完整安装体验与两个平台最终验收未完成 |

## 本轮候选包

canonical candidate已更新为fb3dcd03f909a1fbb7680dabc11cb759dd01b32b，158,245,333字节。ZIP SHA256为6e54e24d89918413ae87ae98e578d5e1a287e30493dff43df39de508bc893779，原生程序SHA256为c46baa54c093710c6d4e86695fc09580922fa6328f62a2ed24610ffdc888e692。

核对生产依赖未变后复用1534986冻结依赖，完整替换当前dist/原生程序；112个runtime文件和28,863个解压条目匹配，实际仅原生程序与desktop-settings.js字节变化。三组包检查（运行、模型、派单/工作流退出）通过；独立包真实窗口首次配置、真实Terra历史/草稿恢复、短窗模型与设置键盘操作、包内原生发送真实Terra工具/回复及运行中草稿、三次正常退出与独立冷读通过；见[最新候选](validation/2026-11-10-model-settings-candidate.md)。此前真实模型和其他后端/平台证据仍按原范围保留。

只替换candidate，current仍1acd805且哈希重新核对不变。确认无占用后清理本轮重复base与过时b539c26解压包，合计约1.16GiB分配字节，保留新候选应用、ZIP和原证据/状态。未签名/未公证，Windows暂无运行环境；完整产品仍未完成。前述及后续历史条目“候选未纳入/未更新”描述各次验证当时的状态，以本节具体纳入范围更新，不扩大其他验收。

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
