# 完整目标的当前验收状态

2026-11-09。目标仍是可日用、配置与细节体验对标Cindy/Maka的完整macOS和Windows客户端，复用Hive Web协作；以下按原目标列出证据及缺口，不能用已实现项目反过来缩小范围，也不表示表中已经穷尽所有对标细节。

## 按原目标核对

| 要求 | 已有证据及其范围 | 尚未满足或未获实证 |
| --- | --- | --- |
| GPUI + gpui-kit，复用Hive业务 | 原生Rust前端、固定Hive2.2.1、共享HTTP/SQLite、team/派单/显式汇报与执行器接缝；[方案](implementation-plan.md)、[接缝](adr/0001-reuse-hive-runtime.md) | 不能把架构接通或编译通过当完整产品 |
| 内置Pi，不新增审批 | 既有真实Terra主控/Worker记录；[协作](validation/2026-11-07-structured-team.md) | 当前OAuth刷新真实返回invalid_grant，须重登后复验；[最新认证证据](validation/2026-11-09-codex-image-live.md) |
| Codex作为主控/Worker | 原样CLI0.166.0真实派单/文件/汇报/恢复；新增真实Terra识图及整个runtime重开、原图字节保持；[Codex](validation/2026-11-07-codex.md)、[图片](validation/2026-11-09-codex-image-live.md) | 不等于所有厂商事件、复杂截图、Windows和所有混合后端组合已验收 |
| 外部Pi作为主控/Worker | 原样CLI1.4.0真实Terra团队与恢复；[外部Pi](validation/2026-11-07-external-pi-team.md) | 新的真实图片推理、所有扩展UI和最新平台组合未完整验收；认证与内置Pi分别记录 |
| Claude Code作为主控/Worker | 官方SDK、原样CLI无推理控制、持久化、工具回调、交互与合成对端；[Claude](validation/2026-11-07-claude-adapter.md) | 真实Claude模型团队协作未验收；保持只用Terra的用户约束，不换模型冒充成功 |
| 统一聊天、工具与成员过程 | 聊天投影、思考、工具Diff/文件预览、成员会话、归档/搜索/草稿、附件、排队输入已实施；README逐项指向实际记录 | 当前导出复制/窄窗/退出、部分在途时序、完整系统输入与可访问性矩阵未完成 |
| 现有Web协作与配置整合 | 团队职责/角色模板、任务与冲突、派单/消息/汇报、记忆、工作流、通知及程序/模型/认证配置已有接入 | 语言偏好与累计811条导航/配置/聊天/会话/协作/工作流/文件工具导出及外部交互文案已接入，完整页面迁移、真实切换/草稿/窄窗仍未完成，见[语言记录](validation/2026-11-09-interface-language.md)、[#17](https://github.com/tt-a1i/hive-desktop/issues/17)；远程配置已接入并通过回环SAS配对和撤销断网/重开验证，原生点击、真实网关和Windows仍待验收，见[记录](validation/2026-11-09-remote-settings.md)、[#18](https://github.com/tt-a1i/hive-desktop/issues/18)；工作流删除最终按钮等仍待验收 |
| 停止、重连、恢复、窗口生命周期 | 所有权清理、队列/身份恢复、Mac实际后台死亡/重连；新增Mac EPERM回归先红后绿；[恢复](validation/2026-11-08-backend-recovery.md)、[清理](validation/2026-11-09-macos-exited-group.md) | 当前两个owned验证窗口仍在，未取得其最终正常退出；不承诺恢复任意中断工具或exactly-once |
| macOS可实际使用 | 当前已验证ZIP为1acd805，含资源浏览；[当前包](validation/2026-11-08-current-package.md) | 新候选d9e061b已fresh deploy，包内远程协议、PTY/HTTP/SQLite及实际原生启动位置恢复通过；完整GUI未验收，不作为current推广 |
| Windows首版支持 | 已有平台接线与监督记录，本机默认功能MSVC类型检查通过；[跨平台](validation/2026-11-08-windows-cross-check.md) | 本批未取得Windows原生窗口、模型、外部CLI及包验收。用户允许不处理CI，不等于取消Windows目标 |
| 完整、精美、Cindy/Maka级体验 | 已有固定源码参考与安装Cindy局部原生走查，并实施多轮聊天/配置/导航改进 | 整体对标仍未证实；还须按实际安装体验做全流程及边角检查，不能以局部截图或当前清单宣称一致 |
| 可安装与交付 | 有冻结Node/native依赖的Mac便携开发包及实际PTY/HTTP/SQLite检查 | 未签名/未公证；正式发行、完整安装体验与两个平台最终验收未完成 |

## 本轮候选包

`.artifacts/hive-desktop-macos-arm64-candidate.zip`已更新为d9e061b234bcc7c36dc8a5f71c5cde8c4c015f50，包含清理修复、导出、远程配置和依赖修复。ZIP SHA256为74c1b29371010de375e058460ecdd751a98e0f1539ffd7f8eb2858f1ac841935，原生程序SHA256为97a8899edc0e7f281fd05832905a1943f0e4787ef895f5051e7fce4d667bd6cf。

本轮fresh frozen deploy并重新打包，独立解压核对112个runtime文件、28,863个包内条目及29个内部链接。包内Node的PTY/HTTP/SQLite和关闭、完整远程回环配对/SAS与加密配置边界、坏帧恢复和撤销503/重开补发通过；实际原生程序启动与已删除会话选择恢复也通过。脚本SIGTERM清理自有启动probe，不视作正常用户退出。详见[包内记录](validation/2026-11-09-remote-settings.md#mac候选包补验)。

current ZIP哈希f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6未变，候选没有推广。gate退出且lsof无引用后删除本轮部署/整包/解压副本；只保留一个新候选ZIP和证据，.artifacts约3.8GiB。旧66ef8f9候选由此替换，旧收据留在`.artifacts/remote-settings/superseded-candidate-receipt.json`；当前收据在`.artifacts/current-package/candidate-receipt.json`及`.artifacts/remote-settings/receipt.json`。同一原ZIP另通过真实加密PTY输入/输出、二进制UTF-8、坏WS流隔离及活跃设备撤销；[范围](validation/2026-11-09-remote-settings.md#加密终端与实时撤销补验)明确为macOS回环对端。CUA界面走查、真实网关和Windows仍未通过。

## 当前阻断与仍可推进的工作

- 本轮原生快捷键工具明确报告Mac锁屏且无法自动解锁，已请求手动解锁。先前窗口控件读取/画面滞后不够证明Hive死锁；这次锁屏报告也不能反推此前每一次超时的原因。保留进程54534、21544现场。
- Pi的原生OpenAI订阅刷新仍返回400 invalid_grant，已请求重新登录；不再用相同认证重复请求，不自动更换接口、认证或模型。
- 静态实现、Web遗漏项迁移、既有后台的定向验证和候选准备仍可独立推进，完整目标保持active。解锁后优先回到复制、窄窗、正常退出及候选原生验收，再推广包。

Web遗漏项来源：本地Hive的5d87d4ba10d9d66cfca32941a16c60e7e52476b0、版本2.2.1，LanguageSetting.tsx明确有zh/en按钮，SettingsMenu.tsx包含RemoteAccessSection；两文件无本地未提交改动。语言入口及首批文案已接入，完整迁移待续；远程已接入并补侧栏入口，证据与缺口如上。这是现有基线缺口，不是将新功能另塞进原目标。
