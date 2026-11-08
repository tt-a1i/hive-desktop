# 原生会话用量与上下文

## 使用场景与变化

用户在长对话中需要区分累计 Token 与当前上下文，并确认模型窗口；此前桌面没有读取入口，只能到各 Agent 原生界面查询。现在输入区“用量”可展开原生统计、刷新与收起，回合结束后自动更新。未知值保持未知；不提供订阅余额、账单或自造累计值。

## 数据来源

- 内置 Pi 1.4.0：`session.getSessionStats()`；外部 Pi：原始 RPC `get_session_stats`。会话累计包含已压缩历史，不能作为当前上下文；`contextUsage` 是原生估算，压缩后尚无新回复时可为 null。
- Codex 0.166：原生 `thread/tokenUsage/updated`，只接收当前 thread。展示原生 total、last 与最近报告的窗口容量；缓存输入/推理输出是原生计数的子集，不能再相加。没有报告则明确不可用。类型以原始 CLI `app-server generate-ts` 输出核对。
- Claude 官方 SDK 0.3.317：关联当前输入的 result。usage 是最近一轮主循环统计；modelUsage 为模型累计，保留最新报告，不把每轮累计再累加。主循环不含子 Agent/辅助调用。重新连接尚无报告时不可用，不从历史自行推导累计。

不发合成 `/usage` 聊天消息，不触发新模型回合，不修改全局模型默认。模型切换后面板关闭并重新读取；Codex 容量明确属于最近原生报告。

## 验证

`pnpm --filter @hive-desktop/runtime check:usage`：

- 真实 Pi SDK、原始外部 Pi CLI RPC、持久化会话与 compaction：累计 58 保留，压缩后的上下文为 null；无厂商推理。
- Codex JSONL 合成对端：其他线程通知不能写入统计，最新累计替换旧累计，非法计数只让统计读取失败，不破坏历史；后续有效通知可恢复。
- Claude 官方 SDK + 合成 CLI 对端：最近轮与累计作用域分离，101 → 141 不成为 242；重新连接历史保留而统计待原生报告。这不是原始 Claude 模型推理验证。
- 编译后生产 JSONL + 真实 Hive HTTP/SQLite/Pi：`chat.usage` 返回原生统计，不存在的会话报错，重复读取不改配置或发模型请求。

macOS GPUI 原生夹具分别通过浅色、深色、760px 窗口的 exercise/reopen 两进程流程：实际原生 SDK 读取预置会话累计 58，切换配置的窗口 20000 → 30000；本地 Responses 回环生成一轮实际 SDK 回复后累计 63；重叠读取最终刷新到最新值；注入读取错误保留 63 并能重试；延迟的 Alpha 回包不会污染 Beta 的 17；压缩会话重开保持累计 23、上下文未知。截图与 JSON 回执在 `.artifacts/agent-usage/`。预置历史和回环回复不代表厂商推理，GPUI 事件注入不代表物理输入验证。

定向回归：`pnpm check`、runtime `check:team`、`check:codex`、`check:external-pi`、`tsx checks/claude-runtime.ts`；`cargo test --locked --manifest-path apps/desktop/Cargo.toml`（20 通过，1 原生夹具忽略）；Windows MSVC 全桌面 `cargo check`。未跑无差别全量套件；本轮修改限于只读用量接入与原生显示，未更改 Hive 协作协议或数据库 schema。

## 未完成的验收

原始 Codex/Claude 厂商用量通知的真实模型回合、Windows GUI 与物理输入仍待验收。Pi OAuth 的真实模型验证仍受账号 grant 失效影响；不修改用户凭据，也不换模型绕过。整体 UI/UX 尚未达到 Cindy 完整体验，本功能通过不表示整项目完成。

## 本轮开发包

生产二进制已恢复（不含原生夹具入口），冻结依赖构建并生成 `.artifacts/hive-desktop-macos-arm64-usage.zip`，SHA256 `ebdb321ad6b27615b90671665b919166c7c748ad5e16f0aa91154a0010da3a35`，代码 `e4fef31`。解压后 Node/真实 PTY/Hive HTTP/SQLite/退出及原生窗口恢复检查通过；包内新增统计模块与源构建产物逐一校验一致。这是未签名、未公证的开发包。

Windows CI `42444376147` 三个 job 均为零步骤：GitHub annotation 明确指出付款失败或支出限额；不能作为构建代码失败，也不能代替 Windows 实际运行验收。
