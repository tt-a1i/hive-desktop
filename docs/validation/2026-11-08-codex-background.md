# 原始 Codex 的后台命令与分页恢复

## 从真实运行发现的问题

使用已安装的 `/Users/tsk/.npm-global/bin/codex` 0.166.0、`openai/gpt-6.1-terra`，在独立测试目录执行一个只打印两段标记并休眠的脚本。没有改全局模型或认证配置。初次检查发现：命令仍在后台运行，模型回合已结束；随后 hydrate 从仅保存完成条目的显示账本恢复，把尚未完成的工具丢掉了。

新会话原先显式要求 legacy 历史。官方协议说明新持久会话默认使用分页历史；本轮改为沿用原生默认，继续按实际 historyMode 兼容旧会话。真实新线程又暴露 materialize 边界：首条消息之前不能调用 turns/list。现在依据已有 materialized 状态跳过新线程的历史读取，未使用异常字符串匹配兜底。依据：[官方 app-server 文档](https://learn.chatgpt.com/docs/app-server)。

## 修复

- 工具开始时在已有投影文件中保留位置，维持工具与后续回答的事件顺序；不新增账本或 schema。
- 模型回合正常结束时保留未完成工具及已收到内容，不把模型回合完成等同于命令完成。
- hydrate 后重建活动工具索引，后续输出及工具完成事件仍能更新原记录。
- 原生完整分页历史返回同一工具的更新终态时，用它替换缓存的 inProgress；原有已确认记录及中断保留规则继续适用。
- 新会话不强制 legacy，已有 legacy 会话仍可恢复；尚未 materialize 的新分页会话不提前读取历史。

## 原始 CLI 与真实窗口

本轮全部厂商推理固定为 `gpt-6.1-terra`。保留了成功和失败尝试：

- `codex-tool-live-2q5iou`：等待 stdout 增量的最初尝试失败，模型回合先结束，最终快照缺少工具，成为修复依据。
- `codex-tool-live-OIcPXQ`：测试驱动在磁盘 PID 出现时过早断言工具开始；原生通知尚未发出。不是产品已显示工具的证明。
- `codex-tool-live-6iqntK`：收到原生工具开始通知后中断，取消投递未进入 completedDeliveryIds，重开后的记录一致。原生此时没有返回 stdout 增量，不宣称验证了实时日志。
- `codex-tool-live-SM0TO4`：新分页线程首次消息前读取历史被原始 CLI 拒绝，成为 materialize 修复依据。
- `codex-tool-live-bsDnN2`：修复后新会话使用 paginated；模型先完成、工具仍存在；关闭原生进程后重开，原生历史返回失败终态，工具身份保留。已检查测试脚本 PID 终止。

部分尝试用只录制并逐字转发 stdout 的本地包装器调用同一个原始 CLI，不改请求或响应。原始通知保存在本地 `.artifacts/codex-tool-live-*/native.jsonl`，未提交 Git。包装器的首次模块类型错误也保留，没有记为成功。

另以独立 Hive 桌面索引绑定上面真实分页线程，使用原始 Codex 后端恢复到 GPUI 窗口；不是 Pi 历史替身。核对后端、工具 ID、原生失败状态，展开查看两段实际标记。该只读窗口阶段未新发起模型回合。截图、脚本及收据在 `.artifacts/codex-background/`。

**实时日志边界**：官方列有 outputDelta，本轮原始 CLI 探针没有观察到它；合成回归验证“收到增量后如何累计”，原始 CLI 验证“真实开始/中断/后台状态与分页恢复”，两者不能互换。重开后的完整原生历史含两段输出，真实窗口已显示，但不能据此宣称运行中实时流出。

## 回归与平台

`pnpm --filter @hive-desktop/runtime build`、`check`、`check:codex` 通过。真实 JSONL 合成对端新增“回合结束但命令未结束 → hydrate → 后续输出与完成”，使用文件屏障加下一次读取释放后续事件，不依赖固定延时。另覆盖新分页线程不得提前读取历史、已缓存活动项被原生终态更新、事件顺序、中断和失败。未新增 UI 单测、未删测试、未改 Hive 协作协议/数据库。

本轮只改 Node Codex 接入和对应检查，未重复无关 Rust/Node 全量测试；原生夹具结束后重新构建生产 Rust 入口，避免留下帧驱动版本。未更新 ZIP，reading 包仍对应 `bc0839b`。

Windows CI 刷新至 `15473f7` 的 [run 42453451822](https://github.com/tt-a1i/hive-desktop/actions/runs/42453451822)：三个 job 均无执行步骤，annotation 为账号付款失败或支出限额。新增 Windows check:codex 步骤只完成接线，未执行；不算 Windows 原生运行证明。其他平台真实命令、系统物理操作、原生实时日志及正式发行继续待验收，完整目标保持开放。
