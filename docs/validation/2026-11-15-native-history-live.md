# 原生 Codex 历史分叉实测

2026-11-15。冻结源码 `9b1b5c29bc342d97489c70cb3feb2f3507bba1d8`，官方 Codex CLI 0.169.0，真实模型仅 GPT-6.1 Terra。使用新建隔离项目、状态目录和无派单的 Worker；没有修改全局配置或调用登录、账号接口。本文不代表 GUI、Windows 或安装包已验收。

## 通过的真实流程

经生产 JSONL 接口，普通分叉保留完整的一回合原生前缀，在新的专有工作树中创建子线程。首条消息“编辑并分叉”得到空原生历史及未发送草稿。创建、被动打开和冷恢复没有追加推理、重发 fork/revert、领取派单或改写协作账本。

随后显式向普通子会话发送一次请求，Terra 实际以相对路径读取并写入新工作树。源与子合计两次 `turn/start`、零 steer，分别耗时 8.949 秒和 9.765 秒。子会话保留原 Worker 身份；源 HEAD、索引、文件、原生 JSONL 和显示元数据全部不变。主代理另从磁盘独立核对这些哈希和子文件的 56 字节内容，子文件 SHA-256 为 `607d0507a1604ca6344b61b8c7a214c4e8b47df4c9f28c13e0f17eebb5d664db`，源目录没有该文件。

最终冷恢复保留普通子历史和编辑草稿，没有推理、原生修改、交互回调或队列重放。六个 Runtime、九个受管理 CLI 和一个只读握手探针均已退出并核对 ESRCH。

## 账本变化的阶段区别

创建与被动读取阶段账本完全不变。显式执行子会话时，既有 Hive 重启策略新增了一条 `system_recovery_summary`，时间位于用户 `chat.prompt` 之后、唯一子回合开始之前；它作为恢复上下文注入，没有额外模型回合。原有消息不变，dispatch、report、dispatch message 及相关 outbox 均为空。

因此，“跨显式激活后整个 messages 表完全相同”这个初始断言不成立。实际成立的是分叉不复制或回放协作事实，显式启动仍遵守当前 Worker 的生命周期规则。没有通过删掉新增行或改写数据库来制造通过。

## 发现并修复的问题及验证脚本边界

首次真实预览被版本门槛错误拒绝：实际服务端 UA 为 `hive_desktop/0.169.0 … (hive_desktop; 0.1.0)`，不是受控夹具中的 `codex_cli_rs/0.169.0`。`89ec09f` 修正为锚定应用名称后的原生版本，仍仅允许已验证的 0.169.0，不使用尾部客户端版本。另修复后续中断回合阻止分叉更早完整回合的问题；活动和未知状态仍拒绝。

继续验证复用了同一已完成的源回合，没有重跑模型。脚本还曾错误地从公开会话响应读取隐藏的原生 ID、使用跨显式激活的全表相等断言，以及比较 JSON 与 SQLite 对象的原型。这些失败收据均保留；修正后只做离线或零模型检查，没有增加模型预算或重复创建同一个原生分叉。

## 证据与剩余范围

目录：`.artifacts/functional-parity/native-history-proof-prep/run-IaM1cQ/`。

- `resumed-receipt-final.json`：最终通过范围、完整请求计数和退出状态。
- `phase-aware-assessment.json`、`explicit-child-lifecycle-ledger-delta.json`：账本阶段核对。
- `root-independent-file-check.json`：主代理独立磁盘核验。
- `receipt.json` 及 `resumed-receipt-attempt*.json`：保留的产品与脚本失败。

共享门禁另有正式 `execution-sqlite-outbox.ts` 回归：真实 team CLI、Hive HTTP/SQLite，在租约期间不领取 queued note；释放后生产监听器自动投递一次。旧 `--seen` 汇报被拒，真实新汇报正常到原主控；pending 跨冷启动不重复，不使用手动 drain 或 SQL 写入伪造结果。

当前仍只支持仓库根项目。原地回退、GUI、Windows、最新打包和新增 profile 组合仍须分别验收；此处不覆盖后续 ACP 或真实厂商账号能力。用户已安装应用和 current/candidate `837abd1` 未替换。
