# 当前 Mac 开发包

当前包纳入工作流页面、关联派单与成员会话入口、阅读位置及后台重连恢复，同时保留先前成员职责、字号/历史消息重测和 Claude 退出结算修复。源提交 `c18617d63907eca42443fd57ace135766983a978`。

- ZIP：`.artifacts/hive-desktop-macos-arm64-current.zip`
- ZIP SHA256：`b633f561bd04686d6b08922bb737bd0b552475f98a37aef9be458cca69679a82`
- 原生程序 SHA256：`ce8d7c2945bc95ef189ef800919ef7343a643adab3dcab47f647a2f9ea81b2b4`
- 未签名/未公证的 Mac arm64 开发包；Windows 原生和完整产品目标尚未完成。

current.zip 是固定位置，按一批变更更新，不为每次小改留下完整应用/依赖树。上述哈希只对应本批，历史源码和验收记录独立保留。

## 本次包内验证

以 ca9eeb6 的上一 current 包为基线，仅替换正式原生可执行程序。packages/runtime、依赖元数据、锁文件、pnpm patch 和 Cargo 依赖配置未变。独立解压后验证原生程序与当前构建一致，108 个 runtime 文件与生产 dist 全部匹配，29970 个未变归档条目逐一匹配（含 Node、依赖及链接目标）。未再复制一套生产依赖。

- `check-package.mjs --bundle ...`：包内 Node、真实 PTY、Hive agent-manager 自然退出/停止、HTTP/SQLite 工作区与后台关闭通过，无模型请求。
- 使用包内 Node 执行 `check-desktop-startup.mjs --bundle ...`：实际原生窗口读取保存的项目并恢复失效会话选择通过。脚本用信号清理窗口，不计作物理鼠标验收或正常 UI 退出。
- 另从新 ZIP 解压应用，用系统鼠标/键盘打开工作流、实际关联派单与驱动汇报记录，进入已有成员会话，核对重开草稿，再返回原工作流且保持原阅读位置。owned 包进程 29297 正常 Cmd+Q、exit 0。没有 HIVE_DESKTOP_RUNTIME / HIVE_DESKTOP_NODE 源码覆盖，实际用包内 Node/runtime。
- 为隔离用户正在运行的其他 Hive 窗口，只修改验证解压副本的目录名和 Info.plist bundle 标识；原 ZIP 未改。故障注入、停止和重连的较完整物理走查在相同哈希原生程序/生产 runtime 下另测，详见[工作流记录](2026-11-08-workflows.md)，不声称包内逐项重跑了全部场景。

本轮模型配置仅为 `openai/gpt-6.1-terra`，验证提供商是不可达回环地址，没有访问厂商或模型推理。显式 report 是验证程序提交，不能冒认为模型完成任务。

## 沿用的后台验证边界

108 个 runtime 文件与 ca9eeb6 包完全一致。上批包内 Claude 四范围退出结算 gate（官方 SDK + 合成 CLI）、队列与持久化检查按其原证据沿用，本次不重复无变化后台的全部回归。见[Claude 退出结算](2026-11-08-claude-write-loss.md)。不把哈希沿用写成重新实测 Claude 厂商推理。

## 产物保留

检查完成并确认验证进程退出、lsof 无打开引用后，删除本次约 591MiB 临时解压副本和约 110.6MiB 可重建的原生程序副本，保留小型脚本、会话与验证证据。ZIP 仅保留 current、claude-write-loss、settings-search；先核对 tasks-watcher ZIP 与历史收据哈希后删除这份已被替代的生成包。不删除旧聊天工作目录、会话或仍被使用的依赖。

收据 `.artifacts/current-package/receipt.json` 与 `.artifacts/workflows/package-receipt.json`；清理记录 `package-cleanup-receipt.json`。全量模型/协议 suite、Windows 真机和正式签名发行不在本批已验证范围内。
