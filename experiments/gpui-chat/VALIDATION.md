# GPUI 原型验证记录

日期：2026-11-06。macOS 原生聊天原型已实际构建、启动并通过部分交互走查；Windows、中文 IME 组合输入、退出交互和产品集成仍未验证。以下区分本地构建记录与根任务提供的 CUA 观察，不把源码接线或 CI 文件等同于验证通过。

## 环境与固定依赖

- 主机：macOS / aarch64，仅有 Xcode CommandLineTools。
- 官方 crates.io `gpui-kit =0.9.0`、`gpui-pre =0.3.11`，Cargo.lock 固定解析结果。
- 项目锁定 Rust 1.98.0；安装了 1.95.0 和 1.98.0 minimal 工具链，全局默认仍为 `stable-aarch64-apple-darwin`。rustup 在正常安装命令中自动由 1.29.2 更新到 1.29.3。
- `cargo +1.98.0 tree --locked -e features -i gpui-pre-macos` 确认 `gpui-kit → gpui-pre-platform → gpui-pre-macos` 启用 `runtime_shaders`。没有安装完整 Xcode / Metal 编译器，没有依赖补丁、`RUSTC_BOOTSTRAP` 或测试专用 fallback。

## 构建结果

| 命令 | 实际结果 |
| --- | --- |
| `cargo +1.93.1 build -j 3` | 失败：gpui-pre `src/profiler.rs:475,496` 使用 `std::hint::cold_path`，E0658。原日志 `build-rust-1.93.1.log`。 |
| `cargo +1.95.0 build --locked -j 3` | 失败：gpui-pre-macos `src/window.rs:896,957,1068` 使用 `Option::map_or_default`，E0658。原日志 `build-rust-1.95.0.log`。 |
| `cargo +1.98.0 build --locked -j 3` | 成功；切换编译器后重编译约 1m12s，已复用此前下载的依赖。不是包含下载的全新构建耗时。 |
| 聊天入口增量构建 | 成功，约 1.22s；格式化后重编译约 1.04s。 |
| 原生多行 composer 改动 | 成功，约 2.30s。 |
| 退出快捷键与最后窗口退出接线 | 成功，约 1.79s；交互复验尚未完成。 |

Rust 官方文档分别标记 [cold_path 从 1.95 稳定](https://doc.rust-lang.org/std/hint/fn.cold_path.html)、[map_or_default 从 1.98 稳定](https://doc.rust-lang.org/std/option/enum.Option.html)。这些依赖的实际使用决定了本次 macOS 构建至少需要 1.98。

最小窗口与聊天窗口均曾直接启动成功；早期 PID 45150、45381 是历史构建阶段进程，现已由根任务切换为 bundle 运行，不能当作当前进程信息。直接启动记录见 `run.log`，编译过程见 `build.log`。

## macOS bundle 与实际交互

根任务创建并通过 CUA 打开 `.artifacts/Hive GPUI Probe.app`，bundle ID 为 `ai.hive.desktop.gpui-probe`，窗口标题为 `Hive GPUI Probe`。这是本地 debug 演示包，未完成签名或分发验证。

根任务报告的实际窗口观察：

- 原生窗口截图与 AX 获取成功，成员按钮可切换选中状态。
- 点击真实 GET 后界面显示 `session 200 → HTTP/1.1 200 OK · []`：先建立 Hive UI session，再携带 cookie 请求空隔离工作区。没有创建工作区或运行 Agent。
- 演示流式 Markdown 与代码块可见，日志记录流开始及完成；内容明确标为 UI 演示。
- 原 composer 使用单行 Input，粘贴换行被压平；已改为库原生 `TextareaState` / `Textarea`，`.auto_grow(3, 6)`，后台地址仍使用单行 Input。
- 改动后粘贴 `你好，Hive。\n验证中文多行粘贴与编辑。`，再 Enter 并输入 `Mac + Windows`，AX 显示 3 行。
- 发送后输入清空，演示流期间发送按钮禁用。

以上中文验证使用剪贴板粘贴。CUA 的 Unicode `typeText` 注入与 `paste` 行为不同，不能据此判定 IME 失败。真实输入法候选窗、组合与提交仍未验证。

预置内容仅为数 KiB 的 8 次重复文本，提供滚动演示，不构成大规模聊天历史或虚拟化性能验证。

## 退出行为与当前验证限制

源码使用公开 API `QuitMode::LastWindowClosed`，并注册全局 Quit action：macOS `Cmd+Q`，其他平台 `Ctrl+Q`，调用 `cx.quit()`。这只定义独立选型原型自己的退出，不操作隔离 Hive 后台，也不定义 Hive Desktop 产品后台的退出语义。

退出接线构建成功，但后续 CUA 重抓屏在 ScreenCaptureKit `-3811` 处失败，发生在退出复验之前。因此 `Cmd+Q` 和最后窗口关闭后的进程退出**尚未交互验证**；不能把这次抓屏错误写成 GPUI 退出 bug 或退出已通过。此前截图与 AX 曾成功。

## 后台真实检查

`checks/backend.rs` 复用 UI 的 `src/backend.rs`，通过真实 TCP/HTTP 获取 `/api/ui/session` 的 `hive_ui_token` 后请求 `/api/workspaces`。本机使用隔离 URL `http://127.0.0.1:58591`，Rust 1.93.1 与 1.98.0 的独立自检都返回：

```text
session 200 → HTTP/1.1 200 OK · []
```

最终保留的断言覆盖认证后 HTTP 200 与隔离空列表；移除了两条“任何错误即可”的弱断言，避免将连接拒绝误当作输入 guard 验证。cookie 不进入日志，探针没有后台写操作。

## 资源快照

根任务最新 `ps` 观察如下，仅代表当时的 debug 演示进程：

| 进程 | PID（快照） | RSS KiB | 约 MiB |
| --- | --- | --- | --- |
| GPUI 原型 | 50754 | 135968 | 132.8 |
| 隔离 Node 后台 | 38903 | 57072 | 55.7 |
| 合计 | — | 193040 | 188.5 |

未包含 CLI、模型或其他 Agent 进程；没有与 Electron 等技术栈做同条件比较。PID 是历史快照，不保证文档阅读时仍存活。

debug 可执行文件约 87 MiB。`target/` 曾约 4.0 GiB，其中包含三个编译器版本的成功/失败产物；不能当作单次干净构建或分发包体积。

## Windows 与范围

根任务已编写本应用的 `.github/workflows/gpui-probe.yml`，但尚未运行，故 Windows 编译、原生窗口、输入与生命周期均未验证。应用采用跨平台 GPUI Kit 与 Rust 标准库，不足以代替 Windows 实际结果；安装包、签名和更新也未验证。

本原型没有执行模型或 Agent，没有修改现有 Hive 用户数据。尚未接入 SDK/RPC/ACP、团队操作、工具执行视图、会话持久化或产品后台生命周期；没有提交、推送或触发远端 CI。本次文档更新仅整理 README 与本记录，其他文件由根任务独立管理。
