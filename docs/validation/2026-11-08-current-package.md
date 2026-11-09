# 当前 Mac 开发包

当前包纳入会话资源浏览（完整说明搜索、范围、来源文件预览及草稿补入），并保留工作流、成员职责、字号/历史消息和 Claude 退出结算修复。源提交 `1acd8050fb756c5748e0a51ecf20bb4d7544fb30`。

- ZIP：`.artifacts/hive-desktop-macos-arm64-current.zip`
- ZIP SHA256：`f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`
- 原生程序 SHA256：`65e2d944b10f9bab4425147d161e4edeaa6e927e59a6f333b5bfd3cb9ac1f46e`
- 未签名/未公证的 Mac arm64 开发包；Windows 原生和完整产品目标尚未完成。

current.zip 是固定位置，按一批变更更新，不为每次小改留下完整应用/依赖树。上述哈希只对应本批，历史源码和验收记录独立保留。

## 2026-11-09 包内验证

以 c18617d 的上一 current 包为基线，仅替换正式原生可执行程序。packages/runtime、依赖元数据、锁文件、pnpm patch 和 Cargo 依赖配置未变。独立解压后验证原生程序与当前构建一致，108个runtime文件与生产dist全部匹配，29970个未变归档条目逐一匹配（含Node、依赖及链接目标）。未再复制一套生产依赖。

- `check-package.mjs --bundle ...`：包内Node、真实PTY、Hive agent-manager自然退出/停止、HTTP/SQLite工作区与后台关闭通过，无模型请求。
- 包内Node执行 `check-desktop-startup.mjs --bundle ...`：实际原生窗口读取保存项目并恢复失效会话选择通过。脚本用信号清理窗口，不计作物理鼠标验收或正常UI退出。
- 从新ZIP解压应用，核对外部Pi原草稿，使用命令按钮打开实际67项资源目录、搜索项目技能并只读打开软链接来源。实际窗口看到 `LINKED_RESOURCE_FILE` 内容；进程76721正常Cmd+Q、exit0。没有HIVE_DESKTOP_RUNTIME / HIVE_DESKTOP_NODE源码覆盖，实际用包内Node/runtime。再次检查草稿未变、messages仍为0且没有chat.prompt。
- 为隔离其他Hive窗口，只修改验证解压副本的目录名和Info.plist bundle标识；原ZIP未改。系统捕捉偶有旧帧/动画，包内来源预览最终正常截图实际可见；不以延迟帧替代结果。更完整的中文、多行草稿、断线及窄窗范围见[会话资源](2026-11-09-session-resources.md)，不声称每项都在包内重复。

本轮模型配置仅为openai/gpt-6.1-terra，验证提供商是不可达回环地址，无厂商推理。

## 沿用的后台与工作流证据

108个runtime文件与ca9eeb6完全一致。先前包内Claude四范围退出结算gate（官方SDK + 合成CLI）、队列与持久化检查按其原证据沿用，本次不重复无变化后台的全部回归。见[Claude退出结算](2026-11-08-claude-write-loss.md)。不把哈希沿用写成重新实测Claude厂商推理。

上一c18617d包的工作流/成员往返、恢复草稿、阅读位置和正常退出记录仍见[工作流](2026-11-08-workflows.md)，本轮保留其原生实现，不冒充已重新逐项验收。

## 产物保留

检查完成并确认验证进程退出、lsof无打开引用后，删除临时解压副本及可重建的小型原生程序副本。核对settings-search ZIP与历史收据哈希后删除这份已被替代的生成包；ZIP只保留current、workflows、claude-write-loss三份，.artifacts回到约3.4GiB。没有删除旧聊天工作目录、会话或仍使用的依赖。

当前收据 `.artifacts/current-package/receipt.json`，本批证据及清理收据 `.artifacts/session-resources/`。全量模型/协议suite、Windows真机和正式签名发行不在本批已验证范围内。

## 后续候选更新

current仍为本页记录的1acd805。candidate现为768d085，纳入空会话恢复、Codex同回合系统通知/来源结算及近期焦点/紧凑导航/工作流详情修复。包内真实Terra主控与Worker完成47/3、同回合唯一final/冷恢复、四组包检查及实际原生工作流/产物/窄窗设置通过。候选SHA256为b792ae16d978801a3d1c479f7538604ad44fcb70cb992e4abb69f0f6516b6881，见[最新候选](2026-11-09-steered-candidate.md)。前轮[队列与远程候选](2026-11-09-settled-candidate.md)、[汇报候选](2026-11-09-report-candidate.md)、[配置候选](2026-11-09-model-candidate.md)按各自范围保留，未推广为current。
