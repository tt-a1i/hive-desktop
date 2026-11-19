# 冷会话进程与历史原图读取

## 会话浏览的常驻开销

在上一版 macOS 解包应用中，使用包内 Node、真实 Hive HTTP/SQLite 和受控 Codex JSONL 进程。顺序打开八个已保存的普通 Codex 会话。每个会话有 100 条历史，读取正文和末条原生 ID 后再读取该会话的模式、权限与推理设置。八个独立进程在浏览结束后仍存活，RSS 合计为 303,472 KiB，约 296 MiB。这个数值按测试进程分别计量，可能重复计入共享页。两个普通冷 ACP 会话打开时均未启动适配器。退出后台后八个测试进程都已结束，没有 `turn/start` 请求。

GPT-6.2 Sol 独立检查了这条路径。普通 Codex 打开需要恢复原生历史，并取得模型和会话能力。桌面还会自动读模式和权限。被动 ACP、跨后端接续和历史分叉已有各自的冷读路径。已启动的 Hive 成员使用同一原生句柄处理派单、待投递及退出回调。下一步应测量仅用于查看会话的原生进程，并覆盖快速切换、与队列投递并发及原会话重新进入。归档也需要校验历史的完整性和原生身份。[会话打开实现](../../packages/runtime/src/chats.ts) · [Maka 生命周期参考](../research/2026-11-19-maka-rust-preview.md)

## 队列损坏后的历史原图

原实现用完整的只读会话快照取历史图片。注入一个损坏的待投递文件后，旧 macOS 包内 JSONL 的图片请求返回 `Invalid delivery spool`，尽管原图和消息记录仍可读。

冷读图片现在直接使用保存的消息投影，按工作区、消息 ID 和图片位置读取原字节。完整会话打开仍会报告队列损坏，测试核对坏文件内容保持原样。对于内置 Pi 显式用户输入，持久化的原生投递记录也能匹配窗口此前收到的活态消息 ID。普通历史的公开 ID 保持原来的来源。同一 ID 指向不同行时拒绝取图，跨工作区和越界请求也拒绝。[后台实现](../../packages/runtime/src/chats.ts) · [附件回归](../../packages/runtime/checks/image-attachments.ts)

## 本地验证

类型检查、真实 Pi 历史与附件回归、生产 JSONL、历史身份和 ACP 冷队列专项通过。测试使用本机回环及隔离数据，没有发起厂商推理。macOS 包复用 `9e9745a` 的原生可执行文件，运行时来自 `5d66a38`。签名、ZIP 清单、包内 PTY、Hive 工作区和关闭检查均通过。解包后重新验签，并通过历史原图 JSONL 回归。原生窗口复验打开交互预览、关闭主窗口和回收 Node，进程以 0 退出。

本地资料位于 `.artifacts/codex-browse-memory-RqeH92/`，包含 `baseline-with-acp/receipt.json`、新旧图片 JSONL 日志、`extracted-native-receipt.jsonl`、`archive-manifest.json` 和 ZIP。ZIP SHA-256 为 `0cedc46c96d96a59d8a15d22591f7817da0acb4fe4ad7f9e2971f16f34c4f244`。压缩前清理了一份已校验且未运行的旧包解压副本。旧 ZIP 和验证收据保留。

当前 Windows 仍缺原生窗口验证。共享构建 `aca54bb` 有原生窗口、便携包和功能收据，覆盖的是当时的版本。现有更新验收入口需要交互式 Windows、基本便携包与更高版本的候选程序。健康更新、故障回退和新包的 HTML 冷开仍待实测。实际托管 GitHub 写入和真实 1Password 授权也仍待独立证据。[既有 Windows 构建](https://github.com/tt-a1i/hive-desktop/actions/runs/44363997413) · [Windows 与 PR](2026-11-19-windows-native-update-and-pr.md) · [密码库](2026-11-19-pi-vault.md)
