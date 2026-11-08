# macOS 已退出进程组清理（#15）

对应 [#15](https://github.com/tt-a1i/hive-desktop/issues/15)。会话资源断线走查时，原生监督日志出现 `sidecar cleanup not confirmed: Operation not permitted`；此时后台确已退出，但清理未确认会阻止可靠的重连/退出。

## 根因与最小修复

macOS 对只包含已退出但尚未回收主进程的进程组调用 killpg(SIGKILL)，返回 EPERM，而非 ESRCH。用真实独立 `/bin/sleep`、SIGTERM、waitid(WNOWAIT)、proc_listpgrppids 复现：目录只有原主进程，killpg=-1/errno1。清理的初次检查与发信号之间，最后后代可能退出，从而落入该状态。

Mac Scope::terminate 现在接收原 owned Child；只有信号返回 EPERM 且重新验证原主进程已退出、组内无后代时，才将其视为已终止。随后原有确认/回收流程仍要完成。主进程仍活着、有后代、枚举/等待失败或其他信号错误，继续报错。没有按错误文案吞异常，没有放宽所有权范围，没有提前回收主进程造成PID复用。

Windows Scope 接受同一个额外参数但不使用，Job终止、句柄等待与活动数判断未改。

## 验证

- 新增真实子进程回归 `terminating_exited_leader_confirms_empty_scope`，在未修代码上准确失败于 PermissionDenied；无mock。即使断言失败也先回收测试主进程。修复后通过。
- `cargo test --manifest-path apps/desktop/Cargo.toml process_supervisor -- --nocapture`：5通过、1子进程夹具入口忽略、24过滤，包含正常/异常/强制清理、独立进程不被误杀、真实Node启动前不派生Agent。日志 `.artifacts/session-resources/cleanup-race-red.log`、`cleanup-race-green.log`。
- Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml` 通过。
- Windows首次检查因为当前PATH没有llvm-rc而停在GPUI构建脚本，保留 `cleanup-windows-check.log`；加入现有LLVM目录后，`PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` 通过，见 `cleanup-windows-check-llvm.log`。不是Windows链接或真机证据。
- 原生源码窗口实际恢复原样外部Pi会话，SIGTERM已核对父子关系的owned Node后台。日志确认旧scope退出，系统按钮重启后台，草稿 `/skill:linked-resource` 保留，命令浏览重新读取实际67项；最后Cmd+Q、exit0/signal null。日志 `cleanup-fixed-ui.log`、`cleanup-fix-receipt.json`；没有chat.prompt或厂商推理。该轮正常清理forced=false，未声称它必然再次命中了精确竞态；精确OS分支由先红后绿的真实未回收子进程验证。
- `git diff --check` 通过。未改协议/schema/Node runtime，不跑无关全量模型/业务suite；没有删除或弱化既有测试。

源码小型验证程序退出且lsof无引用后已删除，原会话和证据保留。当前ZIP仍为1acd805资源浏览批次，不包含本修复，等待下一批包更新；完整产品与Windows原生目标继续开放。
