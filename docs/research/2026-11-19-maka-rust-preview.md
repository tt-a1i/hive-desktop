# Apache Maka `rust-preview` 源码对比

源码基线为 `158d79f514dbd5bb6be89b668a5cfb81e5b3a2a2`。本次对比聚焦历史读取、桌面会话切换和进程资源回收。[分支源码](https://github.com/apache/maka/tree/158d79f514dbd5bb6be89b668a5cfb81e5b3a2a2)

| Maka 做法 | Hive 的取舍 |
| --- | --- |
| 历史页属于固定的序号和摘要，按游标读取，每次最多 256 条。单独的大字段可以按字节窗口请求，有界传输支持取消。 [历史游标](https://github.com/apache/maka/blob/158d79f514dbd5bb6be89b668a5cfb81e5b3a2a2/plugins/agent/state/src/session_history/sql.rs#L247) · [有界读取](https://github.com/apache/maka/blob/158d79f514dbd5bb6be89b668a5cfb81e5b3a2a2/plugins/agent/native/src/service/history/stream.rs#L239) | 先让运行中的跨 Agent 接续刷新跳过历史投影和传输，并在切换会话时释放旧界面容器。后续为长历史做分页时，复用 Hive 的原生消息 ID，保留跨会话搜索定位、已读位置及恢复正确性。 |
| 临时后台以所有者和客户端活动约束生命周期。系统测试覆盖所有者退出、主动释放后空闲、桌面退出和后台接替。 [后台生命周期](https://github.com/apache/maka/blob/158d79f514dbd5bb6be89b668a5cfb81e5b3a2a2/products/maka/cli/src/candidate.rs#L77) · [进程级验收](https://github.com/apache/maka/blob/158d79f514dbd5bb6be89b668a5cfb81e5b3a2a2/products/maka/cli/tests/integration/candidate.rs#L37) | 后续测试区分界面离开、仍有排队或协作工作以及应用退出。是否释放后台会话必须由 Hive 的运行、队列和交付状态决定。 |

`chat.open` 的桌面状态刷新可跳过活动会话的历史正文、消息 ID 和流式消息快照。正常打开仍接收完整历史。被动或只读会话保留原有磁盘读取及历史完整性检查。Rust 切换会话时释放聊天列表、待处理事件、工具输入和交付状态的旧容量，草稿及持久化会话照常恢复。

## 验证

- `session-handoff-runtime.ts` 用真实 Git、Hive SQLite、ACP 协议对端和生产 JSONL RPC 验证。刷新运行中的 Pi 和 ACP 接续会话只返回状态，普通打开保留完整历史和消息 ID，冷启动的被动 ACP 状态与完整打开一致。源码运行和本地 macOS 包内后台均已通过，未发起真实模型请求。
- macOS 本地开发包用原生窗口打开含 160 条历史的会话，切换到短会话后核对独立草稿，再切回核对原历史。关闭窗口后程序以 0 退出，后台确认了 `shutdown`。包内 Node、PTY、Hive HTTP、SQLite 和导航恢复检查已通过。受控数据、日志和文件摘要保存在 `.artifacts/maka-memory-mac-3H5hGV/`，主要收据为 `switch-receipt.json`、`native-switch-verified.log`、`packaged-handoff.log`、`native-startup.log` 和 `package-smoke.log`。
- Rust 测试 114 项通过，1 项辅助进程入口按设计忽略。Windows MSVC 交叉检查通过。尚无本次改动的 Windows 原生窗口和长会话常驻内存测量。应用保留打开正常会话时的全量历史读取。
