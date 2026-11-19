# Codex 会话浏览后的进程回收

桌面切换会话或返回工作区时，会通知后台当前显示的是哪个会话。上一会话的设置和历史读取结束后，后台会关闭仅用于显示且没有待处理工作的 Codex 进程。后台检查会话版本，晚到的旧请求不能重新打开会话。在途请求可以安全结束，只有当前会话的结果会发回窗口。

支持范围目前是已验证能够延迟原生续跑的 Codex 0.169.0。原生回合、交互、待发队列和已交给 Hive 的运行保持原有生命周期。显式发送、继续、模型修改等操作接管其取得的句柄。关闭结果无法确认时保留原所有者，重开会话会提示重启桌面后台。[浏览与关闭](../../packages/runtime/src/chats.ts) · [原生能力判断](../../packages/runtime/src/agents/codex.ts) · [桌面切换](../../apps/desktop/src/sessions.rs)

## macOS 原生对照

从前一版解包应用和本轮签名 ZIP 依次打开八个相同结构的 Codex 历史会话。每个会话有 100 条记录。实际通过侧栏切换，核对末条正文、原生设置请求和存活进程。使用真实 Hive 后台和受控 Codex JSONL 对端，未发起厂商推理。

| 浏览完八个会话后 | 上一版 | 本轮 |
| --- | ---: | ---: |
| 存活的测试进程 | 8 | 1 |
| 测试进程 RSS 合计 | 326.8 MiB | 40.7 MiB |

本轮再打开第一个会话，窗口能读到原历史，新开的进程与首次不同。沿历史导航返回工作区后，存活的测试进程为 0。关闭主窗口后，桌面收到后台关闭确认，以状态 0 退出，Node 进程结束。RSS 为各测试对端进程的合计，可能重复计入共享页。

## 协议与打包

- 新增回归使用生产 JSONL、真实 Hive HTTP/SQLite 以及受控 Codex 对端。覆盖快速 A→B→A、在途设置读取、过期请求、关闭后读取原图、恢复原生线程及消息 ID、运行中的回合、暂停队列、模型修改、空闲 Hive 运行和旧版本保留。源码与 ZIP 解包后台均通过。[回归](../../packages/runtime/checks/codex-desktop-view.ts)
- 运行时类型检查、跨后端接续、模式持久化、队列编辑和工作流关闭专项通过。Rust 单测 124 项通过，1 项辅助入口按设计忽略。Windows MSVC 交叉检查通过。
- macOS ZIP 使用 `41b31e9`，严格验签，解包前逐项核对 28,956 个文件、29 个链接和 2,710 个目录，解包后重新验签。包内 Node、PTY、Hive 工作区及退出检查通过。归档与验证记录保存在本地。

收据位于 `.artifacts/codex-desktop-view-XQF98I/`，包括 `baseline/receipt.json`、`final/receipt.json`、`final/desktop.log`、`packaged-view.log`、`archive-manifest.json` 和 ZIP。ZIP SHA-256 为 `0338bcf50e5867b4d1db53b8780b0367fe36cfba276d08a6a674e429815eb967`。验证后保留了旧 ZIP 和收据，并释放了旧解包副本所占的磁盘空间。

GPT-6.2 Sol 独立复核了共享句柄所有权、设置读取与导航竞态，以及 #39 当前的外部验收缺口。当前 Windows 原生更新、故障回退和 HTML 冷开仍缺实机收据。托管 GitHub 写入和真实 1Password 授权也仍需单独验证。[Windows 与 PR](2026-11-19-windows-native-update-and-pr.md) · [密码库](2026-11-19-pi-vault.md)
