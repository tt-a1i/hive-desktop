# Pi 历史压缩与写盘失败

对应 #3 的来源恢复和持久化验收。没有修改用户 Pi 安装、全局设置、权限模式、Hive 协议或 SQLite schema。

## 复现与修复

1. **压缩上下文导致聊天记录消失。** 9de9648 的内置 Pi snapshot 使用原生 buildSessionProjection，压缩后只剩保留段；真实 SDK JSONL 检查中 first/second 两条原始输入变成仅 second。现从当前持久分支展示聊天，模型仍沿用 Pi 原生压缩投影。保留原始 user 内容、来源 entry 与应用输入 ID，复用外部 Pi 的消息投影 helper；运行中尚未持久化的 SDK 消息尾段单独衔接。context_edit 只改变模型上下文，不改写聊天原文；其他分支不进入当前聊天。
2. **完成标记写入失败却在内存里确认。** Pi 1.4.0 SessionManager 先改内存再 appendFileSync。本机真实 EACCES 复现：JSONL 没有完成标记，旧内置适配器 completedDeliveryIds 却已包含该 ID。现只在状态写入成功后更新完成集合；写盘失败的句柄停止接收新输入，由已有失败清理链路释放，重试从磁盘重新加载。
3. **外部 Pi 重试接到不存在的父节点。** 原样外部 CLI 中，自建扩展把自身测试 JSONL 改为只读，后续 Hive 状态写入失败。旧客户端继续复用其 SDK 内存，修复权限后重试虽在内存里成功，实际 JSONL 中却有缺失的 parentId。现私有 Hive bridge 变更失败即停止复用该会话，回合结算后交给现有 owner 清理。普通模型失败、用户扩展命令失败仍沿用原有语义；不根据错误字符串猜测磁盘错误。

## 检查方式与证据边界

- `check:pi-history`：真实 SDK SessionManager 文件 + 内置 SDK / 原样外部 Pi RPC，验证压缩后原始聊天、具体 entry ID 与角色保留；模型上下文确实省略旧输入且使用 summary；context_edit 不改原文；切分支和冷重开不串来源。用公开持久化 API 准备检查会话，不宣称 summary 是厂商模型生成。
- `check:pi-persistence`：真实 Hive HTTP/SQLite/队列/SDK。内置检查只在目标完成标记的写入边界设置自建文件为只读，原始 OS append 实际返回 EACCES；没有替换 SDK、队列或 SQLite。外部检查由真实用户扩展改变自己的测试文件权限，无父进程文件系统 instrumentation。两条均验证未误确认、原投递保留、旧句柄释放、同 ID 重试成功、所有实际 parentId 存在以及冷重启的完成记录。
- Hive 失败后重新启动会新增自动恢复投递。检查只把原投递的完成、移除与持久状态作为验收，不把新的恢复消息当成原投递失败；该恢复可能因测试回环端口 9 拒绝模型请求而继续保留。没有厂商请求，测试配置唯一模型为 gpt-6.1-terra。
- 两个修改前的失败断言分别保留在 `.artifacts/pi-user-input/history-before.log`、`persistence-before.log`、`external-persistence-before.log`，修改后及定向回归日志也在此目录。所有临时数据路径以当次日志为准。

包内 Pi gate 使用同一套行为检查，绑定包内 Node、SDK 和 runtime，支持检查解压开发包；不通过源码字符串包含关系推断行为。脚本为 scripts/check-packaged-pi.mjs。Windows 本轮未执行这些原生 CLI/权限故障检查；macOS 的 chmod 故障注入不冒充 Windows ACL 验收。新包及原生窗口状态见后续包内记录。

## 本轮验证结果

`pnpm check`、`check:pi-history`、`check:pi-persistence`、`check:pi-user-input`、`check:team`、`check:external-pi` 通过。新增检查先分别复现缺失历史、假完成确认及缺失 parent，再验证对应修复；没有删测试，没有修改测试所需的生产 fallback。只跑本次历史/持久化/生命周期相关回归，没有重复无关全量检查。

把真实 Hive 输入检查的两条会话追加原生 compaction checkpoint，仅保留最后一条 user 在模型上下文，随后用实际 GPUI 窗口重新打开。内置 Pi 浅色常规窗口、外部 Pi 深色 760×583 均恢复全部原始输入和图片，各 delivery 只出现一次；第一条 Skill 在已压缩范围内仍可阅读。证据 `.artifacts/pi-user-input/compacted-ui-receipt.json` 及当轮 `*-native-original-input.png` / `*-receipt.json`。这是源码原生窗口检查，不冒充系统输入法或 Windows GUI 验收。

恢复正式原生入口后 Mac 生产构建通过。Rust 源码与 9de9648 的 Windows 类型检查版本相同，本轮不重复 Windows 交叉构建；新增 Node 逻辑的 Windows 原生验证仍未完成。清理了本项目 target/debug/incremental 中约 13 GiB 可重建编译缓存，没有删除源码、验证产物、会话或业务数据。
