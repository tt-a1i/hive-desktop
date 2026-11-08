# Codex 中断被拒绝后的清理与恢复

## 复现与修复

当原生 `turn/interrupt` 返回 RPC 错误、同时没有发出回合结束通知，原来的 `abort()` 直接抛错，未等待正在处理的 prompt 清理。真实 JSONL 子进程检查在修复前失败于“中断返回后 prompt 必须已结算”，原回合仍未结算；不是用异常文本猜测状态。

现在所有中断调用走同一入口，同一回合的并发取消只发一次中断。若已收到原生结束通知，晚到的中断 RPC 错误不再拆掉可用连接；否则保留原始错误，关闭所属 app-server 连接并等待 prompt 清理，交给现有运行失败/退出路径撤销 Hive run 和 token。`abort()` 的错误路径也等待 prompt，包含开始响应尚未返回时发起取消的情况。

连接退出错误与原生回合结束明确区分，不把断连当作正常结束。未确认投递保留原 ID，不标成功；此路径返回停止失败并允许会话重新打开，不假装收到成功中断。关闭所属 CLI 连接不能证明它先前启动的任意外部后台程序已结束，不作此承诺。

## 验证

新增 `checks/codex-interrupt-rejection.ts`，运行真实 JSONL 子进程，覆盖：

- 中断被拒绝且没有回合终态：prompt 先清理、所属子进程退出、只通知一次退出，投递不确认。
- 先发回合终态再返回中断错误：不拆连接，下一回合仍可用，已取消投递不误确认。
- `turn/start` 响应前取消：通过原生读取释放实际启动响应，再检查拒绝后的清理。
- 中断时对端进程退出：不能把断连误当正常终态，保留错误并完成清理。
- 每个回合实际 JSONL 对端只收到一次中断请求。

同一检查启动真实 Hive HTTP/SQLite 和完整 Chats/队列，通过真实用户输入启动合成后端回合；中断失败后检查旧运行为 error、token 撤销、旧 PID 退出。重开原会话保留完全相同的待投递 ID，再完整关闭并重启 runtime，仍是同一条未执行/未确认消息。不是整条 mock 链，也不代表此轮原始厂商模型产生了拒绝。

`pnpm check`、`pnpm --filter @hive-desktop/runtime check:codex`、`check:team` 与新增完整重启检查通过。原始本机 Codex 0.166.0 的无模型生命周期检查另通过：原生创建/模型选择、终止、Hive run/token 撤销、未完成投递恢复。它验证原始 CLI 控制兼容性，没有尝试强造厂商中断错误或发起新推理。

将新检查加入包内 Codex gate，绑定包内适配器及 desktop runtime；上一份实际附件 ZIP 的解压 runtime 被新行为断言拒绝，证据 `.artifacts/codex-interrupt-rejection/old-package-negative.log`。其余日志在同目录。

没有修改 SQLite schema、Hive 协议、模型或权限规则；不跑无关全量检查，不重复 Rust 构建。此页记录后端源码验证，原始模型中的拒绝频率未知，Windows 原生验收仍未完成。
