# 最新源码的真实 Terra 双成员协作

2026-11-12，源码 `d43f55edc8eee3d4f4d59ba3eebf443eee61d04c`。实际原始 `codex-cli 0.167.0` / app-server，独立项目、桌面数据目录及 Hive HTTP / SQLite；不是受控协议对端或 mock。没有操作桌面面板。

## 真实执行与独立核验

仅一次用户输入，由 Codex 主控通过 Hive 注册的 `team` 工具建立 `proof-alpha`、`proof-beta` 两位成员，分别派单。每位 Worker 只写自己的文件，实际调用 `team report --dispatch … --seen 0 --stdin` 显式汇报。没有使用 CLI 内置子代理代替 Hive 成员。

| 产物 | 精确内容，无结尾换行 | 字节数 |
| --- | --- | ---: |
| alpha-proof.txt | HIVE_LATEST_TERRA_ALPHA_OK | 26 |
| beta-proof.txt | HIVE_LATEST_TERRA_BETA_OK | 25 |

主控在两条派单实际 reported 后执行独立的只读 Python 字节断言，输出 `BYTE_ASSERTIONS_OK`，exit 0；最终完成标记只出现一次。驱动另外直接读取文件、核对字节与 SHA-256，读取真实 SQLite 确认两条派单均由本项目 orchestrator 发出、汇报对应正确成员、report outbox 无未送达记录。三会话结束时均 busy false / pending 0，没有待处理汇报或原生审批请求。

三个 Codex 原生 session JSONL 的 `turn_context` 均明确记录 `gpt-6.1-terra` / medium，各一回合及一个 task_complete；与桌面模型快照一致。总计三个原生 app-server 回合，不把 Hive 的系统投递数当作模型回合数，也不据此声明厂商 HTTP 请求数。

## 完整后台重开与桌面协议

初次运行后关闭整个 runtime，再创建 runtime 读取全部三会话：完成 delivery IDs、原始投递来源、模型和账本保持，文件字节仍正确。

随后另起正式 `packages/runtime/dist/main.js` 子进程，经实际 JSONL `initialize` / `chat.open` / `shutdown` 读取同三会话，逐字段深比较运行结束时的完整 `history`、`historyMessageIds`、model、pendingReports、pendingUserInputs；全部相同。共 40 条历史，其中 27 条 toolResult，包含原始 commandExecution / dynamicToolCall、输出、汇报工具和来源，未仅检查完成标记。进程正常 exit 0，无 prompt 或 Hive resume。

保留这三条正式 stdout 响应，由与原生应用同版的 `serde_json 1.0.154` 解析并核对模型、历史、完成 IDs、busy false / pending 0。此检查证明当前真实历史帧可被 Rust JSON 解析器读取，不等于 GPUI 已实际渲染或完整 UI 验收。复用解析器日志中的 `PACKAGED_…` 是旧程序标签；本轮输入明确来自上述源码 main.js，不能当作新包验证。

## 本地证据与执行边界

`.artifacts/live-team-d43f55e-20261112/` 保留：

- `run.mjs` / `prompt.txt` / `run.log`：一次真实模型执行的驱动、请求与终态；**不要为了冷读重跑 run.mjs**。
- `receipt.json` / `baseline.json` / `snapshots.json`：实际工具、完成回执、派单与 40 条历史。
- `independent-assertion.json`：主控原生只读字节断言及 exit 0。
- `native-model-receipt.json`：三条原生 JSONL 路径及 turn_context 模型/强度，核对时未复制或输出凭据。
- `cold-wire.mjs` / `wire-receipt.json` / `cold-frames.jsonl`：正式主进程冷读、完整历史对照及原始 stdout。
- `state/` / `location.json`：隔离 Hive SQLite、聊天与项目位置，保留用于继续冷读。

实际补验命令：

```sh
'.artifacts/release-78fef23/extracted/bundle/Hive Desktop.app/Contents/Resources/node/bin/node' .artifacts/live-team-d43f55e-20261112/cold-wire.mjs
cargo run --offline --quiet --manifest-path .artifacts/release-78fef23/parser/Cargo.toml -- .artifacts/live-team-d43f55e-20261112/cold-frames.jsonl
```

本轮不改生产代码或正式测试，不跑全量 suite：验证重点是最新源码真实厂商协作、完整恢复和实际消息边界；此前 d43f55e 的静态/相关回归见 [Claude UUID 核对](2026-11-12-claude-completion-uuid.md)。当前安装包仍为 78fef23，本次不更新 ZIP、DMG 或 Applications。真实推理只用 Terra，Pi 登录保持搁置，没有 Claude 厂商推理。Windows 真机、系统通知授权、其他后端账号边界与完整产品体验继续开放。
