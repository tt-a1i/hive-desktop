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
| 现有Web协作与配置整合 | 团队职责/角色模板、任务与冲突、派单/消息/汇报、记忆、工作流、通知及程序/模型/认证配置已有接入 | 语言偏好与累计1046条主要页面/配置/聊天查找/协作/资源终端及静态和上下文错误文案已接入，模型编辑已补固定操作/提示、Tab与空格保存及冷读回，见[配置窄窗](validation/2026-11-09-model-editor-footer.md)，已纳入2b3c07e候选并补包内配置/退出恢复，见[配置候选](validation/2026-11-09-model-candidate.md)；模型/接口编辑另已补焦点与标签显露、正反Tab和实际滚动，见[滚动证据](validation/2026-11-09-model-editor-scroll.md)，新增/覆盖草稿及保存、英文17字号已补窄窗实测，完整字号/可访问性/Windows等边界仍在[#20](https://github.com/tt-a1i/hive-desktop/issues/20)；完整页面迁移、真实切换/草稿/窄窗仍未完成，见[语言记录](validation/2026-11-09-interface-language.md)、[#17](https://github.com/tt-a1i/hive-desktop/issues/17)；远程配置已接入并通过回环SAS配对和撤销断网/重开验证，原生点击、真实网关和Windows仍待验收，见[记录](validation/2026-11-09-remote-settings.md)、[#18](https://github.com/tt-a1i/hive-desktop/issues/18)；工作流删除最终按钮等仍待验收 |
| 停止、重连、恢复、窗口生命周期 | 所有权清理、队列/身份恢复、Mac实际后台死亡/重连；Mac EPERM回归先红后绿；最新源码Cmd+Q、冷启动草稿/语言恢复、窗口关闭均exit0并清理后台，见[原生补验](validation/2026-11-09-native-resume.md) | 旧21544/54534已不在进程表，其退出原因未证实；候选包和Windows矩阵未完成，不承诺恢复任意中断工具或exactly-once |
| macOS可实际使用 | current为1acd805；新候选2b3c07e另纳入配置固定操作栏/焦点滚动/新增ID，包内PTY/HTTP/SQLite、SDK配置及实际窄窗/草稿保存/冷读回/正常退出通过，见[最新候选](validation/2026-11-09-model-candidate.md)；之前真实Codex/Pi证据按原范围保留 | 完整页面、输入法、远程/工作流等矩阵未完成；候选尚未作为current推广 |
| Windows首版支持 | 已有平台接线与监督记录，本机默认功能MSVC类型检查通过；[跨平台](validation/2026-11-08-windows-cross-check.md) | 本批未取得Windows原生窗口、模型、外部CLI及包验收。用户允许不处理CI，不等于取消Windows目标 |
| 完整、精美、Cindy/Maka级体验 | 已有固定源码参考与安装Cindy局部原生走查，并实施多轮聊天/配置/导航改进 | 整体对标仍未证实；还须按实际安装体验做全流程及边角检查，不能以局部截图或当前清单宣称一致 |
| 可安装与交付 | 有冻结Node/native依赖的Mac便携开发包及实际PTY/HTTP/SQLite检查 | 未签名/未公证；正式发行、完整安装体验与两个平台最终验收未完成 |

## 本轮候选包

`.artifacts/hive-desktop-macos-arm64-candidate.zip`已更新为2b3c07e10144d6af567f63a254fd7d116bf9e057，171,839,922字节，纳入模型配置固定栏、字段/能力/接口显露及新增ID焦点。ZIP SHA256为a3e41d9e17baef853d10bd68f9fea6961fcd8100454a50d93724f3c91eff3e5c，程序SHA256为64a9fc79d4ab48b8e7ea5a98a272e596f4d05befe46b837ebd324c5555f65d73。

仅替换dd0230a候选的原生程序；112个runtime与当前dist匹配，28,833普通文件、29内部链接及31,570元数据保持。新包通过真实PTY/Hive/SQLite与四组SDK配置/覆盖/JSONL/常驻刷新；实际包内Node/runtime、窄窗输入/提示、草稿退出恢复/保存/再冷读回、新增ID/首模型/覆盖按钮和鼠标滚动通过。三次原生exit0、无进程/lsof引用后清理解压副本，见[最新候选证据](validation/2026-11-09-model-candidate.md)。本轮不重标之前真实模型和队列证据为新测试。

仅替换candidate槽位，.artifacts约3.9GiB。current仍为1acd805、哈希f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6，未推广。旧dd0230a收据在`.artifacts/model-candidate/superseded-candidate-receipt.json`，当前索引`.artifacts/current-package/candidate-receipt.json`。完整原生/平台及发行验收仍未完成。

## 当前阻断与仍可推进的工作

- [#21](https://github.com/tt-a1i/hive-desktop/issues/21)已在最新源码修复Codex工作流report提交后的过早回收，真实Terra/原生工具确认、独立冷恢复与实际UI通过；[证据](validation/2026-11-09-workflow-codex-report.md)。当前2b3c07e候选尚未包含，继续打包补验。

- [#19](https://github.com/tt-a1i/hive-desktop/issues/19)已在源码修复：Codex停止后关闭本聊天独占app-server，真实Mac后台命令退出、其他连接及原线程/队列保留；原生按钮和旧停止提示清理已补验，见[停止证据](validation/2026-11-09-codex-stop.md)。已纳入dd0230a候选并通过包内真实检查，Windows实际及逃逸进程不作保证。

- 原生访问现已恢复，最新源码已完成部分双语/导出/退出补验，见[证据及限制](validation/2026-11-09-native-resume.md)。旧54534、21544已不在进程表，不能补记正常退出；ScreenCaptureKit仍偶发捕捉失败，按后续真实状态核对。
- Pi的原生OpenAI订阅刷新仍返回400 invalid_grant，已请求重新登录；不再用相同认证重复请求，不自动更换接口、认证或模型。
- 用户恢复持续目标后，实际原生访问已可用，目标保持active。候选包已完成明确列出的原生操作，继续其余页面/输入、工作流、远程原生配置和完整包内验收；Pi重新登录、Claude真实模型和Windows环境仍各自列缺口。局部候选验收不等于完整体验，不推广current ZIP。

Web遗漏项来源：本地Hive的5d87d4ba10d9d66cfca32941a16c60e7e52476b0、版本2.2.1，LanguageSetting.tsx明确有zh/en按钮，SettingsMenu.tsx包含RemoteAccessSection；两文件无本地未提交改动。语言入口及首批文案已接入，完整迁移待续；远程已接入并补侧栏入口，证据与缺口如上。这是现有基线缺口，不是将新功能另塞进原目标。
