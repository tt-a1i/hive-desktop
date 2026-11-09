# 完整目标的当前验收状态

2026-11-09。目标仍是可日用、配置与细节体验对标Cindy/Maka的完整macOS和Windows客户端，复用Hive Web协作；以下按原目标列出证据及缺口，不能用已实现项目反过来缩小范围，也不表示表中已经穷尽所有对标细节。

## 按原目标核对

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
| macOS可实际使用 | current为1acd805；新候选768d085纳入空恢复、Codex同回合通知/来源结算及近期导航/工作流详情修复，包内真实Terra主控/Worker47/3、唯一final/冷恢复、四组gate和实际工作流/产物/窄窗设置通过，见[最新候选](validation/2026-11-09-steered-candidate.md)；之前配置/成员阅读和模型证据按原范围保留 | 完整页面、输入法、远程/工作流等矩阵未完成；候选尚未作为current推广 |
| Windows首版支持 | 已有平台接线与监督记录，本机默认功能MSVC类型检查通过；[跨平台](validation/2026-11-08-windows-cross-check.md) | 本批未取得Windows原生窗口、模型、外部CLI及包验收。用户允许不处理CI，不等于取消Windows目标 |
| 完整、精美、Cindy/Maka级体验 | 已有固定源码参考与安装Cindy局部原生走查，并实施多轮聊天/配置/导航改进 | 整体对标仍未证实；还须按实际安装体验做全流程及边角检查，不能以局部截图或当前清单宣称一致 |
| 可安装与交付 | 有冻结Node/native依赖的Mac便携开发包及实际PTY/HTTP/SQLite检查 | 未签名/未公证；正式发行、完整安装体验与两个平台最终验收未完成 |

## 本轮候选包

canonical candidate已更新为768d085081db488cc307d565d59258d46075c31f，171,866,310字节，纳入空会话恢复、Codex同回合系统通知及近期原生焦点/导航/工作流详情修复。ZIP SHA256为b792ae16d978801a3d1c479f7538604ad44fcb70cb992e4abb69f0f6516b6881，原生程序SHA256为509e049e3fbe5113e0aefb1006a1d136b6eceeeb91bcc86453aee1ba1daf641e。

四组包检查、包内原始Codex/Terra主控亲自启动workflow/Worker显式report47/3、同回合唯一final与来源、冷恢复pending0通过；实际包窗口核对同一主控/工作流/产物及九入口窄窗菜单和设置返回。见[最新候选](validation/2026-11-09-steered-candidate.md)。由驱动发送主控请求，不冒称原生窗口发起或所有页面重新验收。

只替换candidate，current仍1acd805且哈希不变；未签名/未公证、Windows原生及完整产品仍未完成。无占用后清理解压副本，.artifacts约3.9GiB。此前“候选未纳入本轮修复”以本节列出的新包证据为准；其他功能/语言/平台验收仍按各自原范围。

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
