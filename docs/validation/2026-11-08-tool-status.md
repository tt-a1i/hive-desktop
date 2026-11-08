# 工具中断、拒绝与部分日志

## 实际问题

Codex 的 outputDelta 原先逐段转发，而桌面工具更新替换正文，执行中的日志只剩最后一段；item/started 没进入快照，切换/读取状态时也会遗漏正在执行的工具。回合中断且缺少 item/completed 时，仅保留了助手/思考片段，没有保留未完成工具。桌面实时 tool_end 又只看 is_error，会把原生 declined 暂时显示为“完成”，随后历史 message_end 才改成拒绝。

## 修复

- CodexEvents 保留当前工具及所属回合，把正在执行的工具纳入原有显示快照，并累计日志增量。原生完成记录到达后取代临时记录；完成后的迟到输出不会重新开启该工具。
- interrupted/failed 回合结束时，尚无完成记录的工具保留现有输出，使用已有 hiveInterrupted 显示标记；不将原生 status 改成完成，也不把显示标记送给模型。已完成工具不再被标中断，其他回合的工具不参与本回合收尾。
- 原生历史导入、已有显示账本恢复也在终止回合里标注仍为 inProgress 的工具，避免恢复后永久显示执行中。沿用原来的投影文件，没有新 store/schema。
- 桌面历史与实时完成共用状态解释：拒绝、中断、取消、失败、执行中、完成分别显示，未知显式状态显示“状态未知”。内置 Pi 等没有显式 toolStatus 的完成事件仍尊重原有错误标记。不改变成员 idle/working/stopped 或派单显式汇报规则。

## 验证

`pnpm --filter @hive-desktop/runtime check`、`build` 和 `check:codex` 通过。原有规范化检查扩展到累计输出、正在执行快照、回合隔离、已完成工具不被中断、迟到输出、真实投影文件重开和原生历史导入。

新增 `checks/codex-interrupted-tools.ts` 启动真实 JSONL 子进程，协议对端为明确的合成实现：两段输出 → 等待实际 abort → 原生回合 interrupted（没有工具 completed）→ 重开进程与账本 → 拒绝工具 → failed 回合中仍有未完成工具。核对全部已收到日志、状态恢复、原生记录未被改写，以及取消投递没有进入 completedDeliveryIds。它不是本轮原始 Codex CLI/厂商推理验证，没有模型请求。

Rust：`cargo test --locked --manifest-path apps/desktop/Cargo.toml` 为 24 通过、1 忽略，新增一个状态契约检查；完整 Windows MSVC 默认功能类型检查通过。

Mac 浅色常规、浅色 760px 和深色 760px：将上述真实 JSONL 检查产生的规范化记录保存到 Pi 原生历史夹具，经真实 Hive/桌面加载，检查累计日志和执行中/中断/拒绝/未知状态；额外用生产事件入口检查 tool_end 拒绝到 message_end 的一致性。该窗口本身是 Pi 历史夹具，不冒充连接了原始 Codex。未操作物理键盘/系统剪贴板。脚本、日志及收据在 `.artifacts/tool-status/`。

本次改动集中在 Codex 事件/显示投影及桌面工具状态，运行对应完整 Codex 定向回归；未跑无关 Node 全量 suite，未改 Hive 队列或 SQLite。没有删除测试。夹具结束后重新构建生产入口。

## 未完成边界

原始 Codex 模型执行与中断、本机实际 UI 操作、其他后端的中断语义以及 Windows GUI 仍待各自验证；程序异常退出而没有终止通知时，继续沿用既有显示缺口提示，不宣称已能恢复任意中断工具。本批未更新 ZIP：reading 开发包仍对应 `bc0839b`，不包含本批后端修复，不能直接复用旧冻结 runtime 冒充新包。完整产品目标继续开放。
