# 桌面后台进程监督

日期：2026-11-07。Mac 和 Windows CI 定向检查通过；不是完整桌面生命周期验收。

## 实际问题与改动

Node 后台崩溃后，后代进程可能继续持有 stdout，导致仅监听管道 EOF 的桌面无法识别故障。原来的退出实现还会让并发调用者因 Child 已被取走而提前认为清理完成。

现在独立观察 Node leader；macOS 使用应用独占的进程组，Windows 使用 kill-on-close Job。清理序列化并共享已完成报告，Mac 在所有组信号发送完毕前不 reap leader，避免 PGID 复用。正常窗口退出等待清理结果；失败时保留窗口和原 runtime，显示错误并允许重试。

Windows Node 在收到 initialize 前会打开 Hive HTTP/SQLite，但不会自动启动 Agent。Job 绑定发生在桌面发送任何请求之前。通用 fixture 的子进程启动也由 stdin gate 控制；本实现不是任意进程的 suspended launcher。

## 验证

在 macOS arm64 运行：

```sh
cargo test --locked --manifest-path apps/desktop/Cargo.toml --features process-supervisor-check --bin process-supervisor-check -- --nocapture
cargo check --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop
cargo fmt --manifest-path apps/desktop/Cargo.toml -- --check
```

10 通过、0 失败，1 个子进程 fixture 入口忽略（由父测试显式执行）。覆盖正常退出、异常 leader、强制清理、独立进程不受影响、重复清理、两个并发调用者等待真实退出，以及后代持管道时 pending RPC 仍失败。真实 Node 启动检查确认 initialize 前没有 Agent 子进程。收据：`.artifacts/sidecar-supervisor/receipt.json`，日志：同目录 `macos-check.log`。

Windows 同一检查命令已加入 CI，包含外层 sidecar Job 和内层 Job 的后代清理 fixture。`bf2204a` 的首次运行在并行 fixture 临时目录创建遇到 AlreadyExists；`4090285` 的运行进入异常退出用例后报 Owned child survived cleanup。后者只有 PID 重查，没有原始进程对象身份，不能判断是否确有残留。`0089580` 改为原子分配临时目录，并在启动后持有原进程句柄、清理后检查同一对象。后续 [e3809c9 Windows run 42309381626](https://github.com/tt-a1i/hive-desktop/actions/runs/42309381626) 的 `native-supervision` 为 9 通过、0 失败、1 fixture 入口忽略：正常、强制和 leader 异常退出均确认原始子孙句柄已终止、独立进程仍存活；另覆盖并发 shutdown 与后代持有 stdout。该 run 的 build 及解压后 runtime/PTY smoke 通过，首次整体因独立 `native-clis` 检查失败；只重跑该失败 job 后 attempt 2 成功，整次运行现为成功，见 [CLI 验证](2026-11-07-windows-cli-ownership.md)。监督和 build 结果来自首次执行，未重跑。Mac 仍为 10 通过、1 fixture 忽略。运行日志和 artifact 清单位于 `.artifacts/windows-e3809c9/`。

Mac 原生窗口另以 `bf2204a` 冻结包和隔离状态运行：空外部 Pi 会话连接原始 CLI，选择 openai/gpt-6.1-terra，没有发送模型请求。Cmd-Q 后窗口、Node、Pi 三个原始 PID 全部消失；再次启动后只 SIGKILL Node，所属 Pi 也被清理，窗口显示后台断开，仍能 Cmd-Q 退出。收据在 `.artifacts/sidecar-ui-proof/{normal,crash}-result.json`。这不是活跃 detached 工具测试，也没有验证 Dock 的最终退出回调。

## Self-Review

- A 架构：新增 binary 令默认启动二义，已通过 default-run 修复；最终 B+。
- B 真实 bug：系统最终退出回调只有 200ms，原先 3 秒 grace 不合适；已改零 grace，独立静态复核通过。
- C 检查质量：补充外部 gate，使两个 shutdown caller 在真实 Node 收到请求后均保持 pending；最终 A-。
- D 范围对齐：正常退出曾忽略清理错误，已保留窗口和原 runtime，并显示可重试错误；最终 B。

所有已报告阻塞项已修；评审不替代 Windows 或原生 Dock 退出实测。没有删除既有测试，没有加入按命令名选择的测试 fallback。

## 尚未覆盖

- Mac 仅确认所属进程组；Pi SDK bash 实际使用 detached:true，主动离组的工具不在此报告内。正常 Node 存活时 Pi abort 负责其工具取消，不能用组清理成功推断后台崩溃后的活跃工具都已结束。
- 系统不可取消的最终退出回调为 best-effort；零 grace 不证明 200ms 内已完成等待。
- Windows 嵌套 Job 已由上述 CI fixture 验证；Windows 原生窗口退出以及真实活跃模型工具尚未在本片中验证。
- 未加入自动重连、会话 resume 或输入重发；任何后续重连都必须保留未知执行状态，不自动重复请求。
