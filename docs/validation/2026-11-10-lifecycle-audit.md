# 后台生命周期定向复核

源码基线 `66eb15c`。本轮没有修改生产代码、测试或开发包；运行已有定向检查，核对工作区移除、工作流退出与终端资源清理。

## 实际运行

在 `packages/runtime` 使用当前开发包的 Node 22、已安装的 tsx，依次/独立运行以下现有检查，五个命令均 exit 0：

- `checks/workflow-removal-lifecycle.ts`：真实 Worker，受控任务源；逻辑失败或 Worker 退出时，已接受但尚未完成的 host call 仍持有所有权回调。
- `checks/workspace-removal.ts`：真实 Hive HTTP/SQLite、原始外部 Pi 1.4.0；活跃工作流阻止移除、目录读取互斥与等待、子进程退出、数据库删除失败回滚、局部索引失败及实际 JSONL 错误结构均通过。项目文件、原生会话与待投递 spool 保留；其他工作区仍可用。没有模型回合。
- `checks/workflow-shutdown.ts`：真实 Hive/SQLite、受控 Codex 协议子进程；正常完成/失败/显式停止历史保留，退出时的双 Worker、嵌套工作流和 CPU 循环冷启动后为 interrupted，相关两条派单为 cancelled，成员冷读 pending 0。五个受控协议进程均退出；这不是厂商推理验证。
- `checks/terminal-shell.ts`：真实 PTY、双 WebSocket；错误工作区拒绝、恢复握手、UTF-8/ANSI/分片输出、cwd/resize、detach/reattach、未确认数据的高水位与确认后继续、Ctrl+C 后 shell 可继续、stop/自然退出/工作区移除/后台关闭通过。没有直接观测底层 PTY pause 调用，不扩大流控结论。
- `checks/terminal-jsonl.ts`：独立后台进程的真实 JSONL、Hive 与 PTY；恢复、输入输出及 shutdown 成功。没有原生窗口渲染验证。

原始外部 Pi 使用 `HIVE_PI_COMMAND=/Users/tsk/Library/pnpm/pi`，Node bin 置于本次命令 PATH 首位。未更改全局默认、认证或模型配置；无新模型请求。

## 诊断与证据

`.artifacts/lifecycle-audit-66eb15c/` 保留五份日志、六份原始收据副本和汇总 `receipt.json`（含检查源码 SHA256、原证据目录及 exit code）。独立再次检查本次原始 Pi PID 和五个受控 Agent PID，均已不存在。

工作流退出日志包含四条 `workflow.cancelDispatch.forward` / `PtyInactiveError` 409：取消通知的目标成员已无活跃 run。没有过滤这些诊断，也未据此加入重试或 fallback；退出、冷读派单与队列断言实际通过。本轮只确认这些结果，不声称取消期间无任何错误或所有异常时序都已覆盖。

检查 `desktop-runtime.ts` 的关闭顺序及各 closer：模型配置写入链已保存 settled promise，账号登录失败/取消由已有流程收敛，模型查询以 allSettled 排空。本轮没有复现需要新增关闭框架的失败路径，因此没有为猜测增加实现。

本轮未跑全量 suite：范围是现有生命周期链路的定向复核，没有生产改动。未重打包或重跑原生 GUI；Windows 原生、真实 Claude、Pi 重新认证、生产远程、完整视觉/输入法/辅助技术和发行验收仍待，完整目标保持未完成。
