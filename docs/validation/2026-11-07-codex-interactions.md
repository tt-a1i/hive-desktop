# Codex 审批与补充输入验证

本记录只覆盖列出的交互及证据，不代表完整产品、所有厂商事件或平台已完成。Codex 目前处理三类原生请求：`item/commandExecution/requestApproval`、`item/fileChange/requestApproval` 和 `item/tool/requestUserInput`。审批只针对当前请求，不提供整会话策略授权；本次没有给 Pi 内建新增审批能力。

## 真实原生文件审批

[真实运行收据](../../.artifacts/codex-native-interactions-cJe2z8/receipt.json) 使用 Codex CLI 0.166.0 和 `openai/gpt-6.1-terra`，配置 `on-request`、只读沙箱和用户审批。模型请求新增一个测试专有标记文件，审批前文件不存在；单次接受 `item/fileChange/requestApproval` 后文件写入，原生命令 `cat` 读回精确内容，回合完成。该次只验证了文件审批；`cat` 的执行不证明命令审批流程，也没有验证真实模型的补充输入请求。

[先前预检失败收据](../../.artifacts/codex-native-interactions-Ecwar7/receipt.json) 保留了 0.166.0 拒绝 `untrusted` 策略的事实：未发送模型提示，不能算审批测试通过。

## 合成协议与桌面走查

[合成协议收据](../../packages/runtime/.artifacts/codex-interactions-BNGKwF/receipt.json) 使用真实 JSONL 传输、Hive HTTP 和 SQLite，覆盖 13 个会话中的请求隔离、状态快照、取消、终止、后端死亡及秘密输入不由 Hive 持久化等。合成对端不调用模型或执行命令。秘密不由 Hive 持久化不意味着 Codex 或外部系统绝不会回显或保存它。

macOS 原生窗口使用合成对端走查，相关协议事件见 [74743 收据](../../.artifacts/codex-interactions-ui-y1ytxqbh/receipts/74743.jsonl) 和 [82633 收据](../../.artifacts/codex-interactions-ui-y1ytxqbh/receipts/82633.jsonl)：

- 命令单次允许，等待解除期间锁定；文件变更单次拒绝，文件不变。
- 补充输入检查空值、选 Alpha、掩码秘密、切换会话保留输入、中文 Other 提交及解除锁定后清空草稿。取消问题发送 `turn/interrupt`，不提交答案。
- 760×583 原生窗口可滚动到操作按钮。断开检查中只终止本次拥有的 Node 进程，未发送草稿保留，可复制当前草稿；写入、模型切换和新建会话禁用，未自动重发。

这些窗口操作使用的对端是合成的，并未执行模型或命令。窗口走查基于当时的旧构建；后续状态文案和空提示修正正在重新构建，尚未以新构建完成视觉验证。工作区选择、取消和 Finder 当前文件夹另有实际走查，不等同于原生审批验证。

## Self-Review

此前四角度独立复核：架构 B+、真实边界 B、验证质量 A-、协议对齐 B。合成对端的错误响应曾因记录 undefined 而崩溃、造成错误路径假阳性；已分开记录错误与结果，并验证原生 -32603、回合中断和随后 setModel 成功。审批取消仍保留原生 cancel 语义，按钮文案改为“取消请求”；问题取消走 turn/interrupt。

最终增量独立审查发现 P2：切换到空交互会话仍显示前一会话待回应状态。已在打开会话及快照同步时更新当前状态，复核通过；后台、断线、退出期间的状态守卫保留。文件选择器取消不添加工作区，打开项目复用 Hive 白名单；真实模型检查仅接受测试所有的精确单文件操作。

本轮执行 TypeScript check/build、13 会话交互检查（`codex-interactions-z7WFnP`）、Rust fmt/check/build；未为纯 UI 添加测试，未重复全量无关检查。没有删除原有检查；`check:codex` 已包含交互检查。完整产品和 Windows GUI 仍未验收。
