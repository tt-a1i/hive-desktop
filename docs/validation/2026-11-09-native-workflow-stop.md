# 原生停止工作流与终态展示

2026-11-09，源码开发应用，非候选包完整验收。

## 实际按钮链路

隔离目录 `.artifacts/native-workflow-stop`，真实Hive/SQLite/结构化执行层/原生Node子进程。厂商端使用明确标识的可控JSONL peer，无网络模型推理。测试peer通过已注册team原生工具启动实际工作流，不模拟工作流结果或子进程退出。

1. 原生聊天发送自有控制标记，peer调用实际team workflow run，创建两位独立临时Worker，各保持原生回合执行中。
2. 从实际“工作流”页面进入详情，窗口看到运行中、2位执行成员及两条已提交派单；独立读盘确认run running及两个进程存在。
3. 点击实际“停止运行”。run `db7bfeae-24c1-4614-b215-73b001b6e0e8` 变stopped，两条派单cancelled；独立进程检查确认两Worker退出。
4. 两位历史成员会话仍保留派单正文，没有继续处理的旧任务；投递spool均为空。Cmd+Q正常exit0，PID86068。
5. 独立重建runtime冷读，两成员thread不变、pending0/busy=false；保留停止及后续真实失败记录。

这是原生按钮到真实后台的停止链路，不等于真实Codex模型取消或Windows验收。

## 走查发现及修复

原工作流详情把主动停止展示为红色“运行错误”，并把已取消派单的report_text标成“汇报”。取消并没有产生成功成员汇报，这个标签不准确。

现按已有status显示：stopped为“停止原因”、interrupted为“中断原因”，列表与详情使用正常/次要颜色；真正失败仍为红色“运行错误”。取消派单显示“取消原因”，与现有协作详情一致；保留后端原始原因，不靠字符串猜状态。原因文本补充AX语义。未改变后台状态或投递协议。

Mac最新构建实际核对：

- 停止记录中英文“停止原因 / Stop reason”、两条“取消原因 / Cancellation reason”，正文为原始Stopped by user，正常颜色。
- 用真实工作流执行器运行自有 `throw new Error('OWNED_FAILURE_REASON')`，实际status failed；中文“运行错误”、英文“Run error”及红色错误仍明确。不是手改数据库伪造错误状态。
- 中文PID90051及英文PID91890均Cmd+Q正常exit0，英文冷启动保留历史与状态。
- 本次没有实际制造interrupted分支；其文案映射为源码实现，不扩大成原生实证。

证据：before/after-stop.png、stop-fixed-zh/en.png、failure-fixed-zh/en.png、before-stop.json、stop-receipt/cold-receipt/receipt.json、三次进程/退出/RPC日志及可控peer源码，均在上述目录。

## 检查和范围

`cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、`cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop` 和 `git diff --check` 通过。

修改仅涉及工作流UI条件、颜色和双语文案，没有新增/修改自动测试，未重复后台全量suite。本轮沿用上一轮取消队列修复的定向回归并新增实际原生操作。候选ZIP仍06ee28b，尚未纳入9e352ee后台修复及本轮UI；Windows原生、真实厂商中断边界和完整产品仍待。
