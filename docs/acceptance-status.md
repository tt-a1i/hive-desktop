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
| 现有Web协作与配置整合 | 团队职责/角色模板、任务与冲突、派单/消息/汇报、记忆、工作流、通知及程序/模型/认证配置已有接入 | Web已有语言设置和远程配置没有原生对应页，分别见[#17](https://github.com/tt-a1i/hive-desktop/issues/17)、[#18](https://github.com/tt-a1i/hive-desktop/issues/18)；工作流删除最终按钮等仍待验收 |
| 停止、重连、恢复、窗口生命周期 | 所有权清理、队列/身份恢复、Mac实际后台死亡/重连；新增Mac EPERM回归先红后绿；[恢复](validation/2026-11-08-backend-recovery.md)、[清理](validation/2026-11-09-macos-exited-group.md) | 当前两个owned验证窗口仍在，未取得其最终正常退出；不承诺恢复任意中断工具或exactly-once |
| macOS可实际使用 | 当前已验证ZIP为1acd805，含资源浏览；[当前包](validation/2026-11-08-current-package.md) | 新候选66ef8f9仅包内静态/运行时gate通过，未做新包启动恢复/完整GUI，不作为current推广 |
| Windows首版支持 | 已有平台接线与监督记录，本机默认功能MSVC类型检查通过；[跨平台](validation/2026-11-08-windows-cross-check.md) | 本批未取得Windows原生窗口、模型、外部CLI及包验收。用户允许不处理CI，不等于取消Windows目标 |
| 完整、精美、Cindy/Maka级体验 | 已有固定源码参考与安装Cindy局部原生走查，并实施多轮聊天/配置/导航改进 | 整体对标仍未证实；还须按实际安装体验做全流程及边角检查，不能以局部截图或当前清单宣称一致 |
| 可安装与交付 | 有冻结Node/native依赖的Mac便携开发包及实际PTY/HTTP/SQLite检查 | 未签名/未公证；正式发行、完整安装体验与两个平台最终验收未完成 |

## 本轮候选包

`.artifacts/hive-desktop-macos-arm64-candidate.zip`，源提交66ef8f9dfb58b515a2850a60b5c89dc253fcfbe5，纳入7e68869清理修复及85a0f73导出。ZIP SHA256为26d7ed356acdabf52aadc96909cfae025c2baea456c11f4bf33662c69456b1b9，原生程序SHA256为e3fc37b364789d615e55a92ec0f8ac0fcee71f99ae63a281f3f93c8e59e4bca2。

独立解压核对原生程序、108个runtime文件及29,970个未改条目。用包内Node运行check-package，真实PTY/Hive/SQLite和关闭通过，无模型请求。runtime/依赖与current相同，仅替换原生程序；本批检查脚本变化不混入生产dist。

current ZIP哈希f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6未变。候选未推广为current。gate进程退出且lsof无引用后已删除临时解压副本，保留一个固定候选ZIP，.artifacts约3.8GiB；解锁后按同一哈希重新解压做原生验收。收据在 `.artifacts/current-package/candidate-receipt.json` 和 `.artifacts/conversation-export/candidate-cleanup.json`。

## 当前阻断与仍可推进的工作

- 本轮原生快捷键工具明确报告Mac锁屏且无法自动解锁，已请求手动解锁。先前窗口控件读取/画面滞后不够证明Hive死锁；这次锁屏报告也不能反推此前每一次超时的原因。保留进程54534、21544现场。
- Pi的原生OpenAI订阅刷新仍返回400 invalid_grant，已请求重新登录；不再用相同认证重复请求，不自动更换接口、认证或模型。
- 静态实现、Web遗漏项迁移、既有后台的定向验证和候选准备仍可独立推进，完整目标保持active。解锁后优先回到复制、窄窗、正常退出及候选原生验收，再推广包。

Web遗漏项来源：本地Hive的5d87d4ba10d9d66cfca32941a16c60e7e52476b0、版本2.2.1，LanguageSetting.tsx明确有zh/en按钮，SettingsMenu.tsx包含RemoteAccessSection；两文件无本地未提交改动。桌面SettingsSection尚无这些入口。这是现有基线缺口，不是将新功能另塞进原目标。
