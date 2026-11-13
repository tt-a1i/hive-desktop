# Sidecar 协议帧排空、异常关闭防卡死与全设置/会话搜索视口焦点闭环

2026-11-13，基线 `ca15013`（当前开发包 `62e6a6e`）。

## 改进范围与问题定位

全量复核 `packages/runtime/src/` 的协议流/退出/持久化路径与 `apps/desktop/src/` 全部 `overflow_y_scroll` 滚动面板（推进 `#1`、`#17`、`#20`），修复四项后台协议与错误聚合边界，并补齐剩余设置分区及会话搜索弹窗的 `Viewport` + `Reveal` 键盘焦点自动显露闭环：

1. **Node Sidecar（`main.ts`）协议帧完整排空与异常 `shutdown` 防卡死**：
   - Rust `Runtime::shutdown_checked`（`apps/desktop/src/transport.rs`）通过 `self.tx` 发送 `{"id":0,"method":"shutdown","params":{}}` 时，写入线程仍持有打开的 `ChildStdin` 直至子进程退出。
   - 原 `main.ts` 在 `dispatch('shutdown')` 成功分支后才调用 `input.close()`，且 `input.on('close')` 未追踪在途 RPC 任务与 `protocolWrite` 回调就立即 `process.exit(...)`：一旦 `close()` 在异常关闭路径抛错，`catch` 分支发送错误帧后不会调用 `input.close()`，导致 Node sidecar 在打开的 `stdin` 上挂死满 `3,000 ms` 直至被 `SIGTERM` 强制终止（`forced: true`）；同时并发流水线请求或背压下的 JSONL 响应帧存在退出截断风险。
   - 现改为追踪全部在途请求（`inFlight`）与 `protocolWrite` 写回调（`pendingWrites` / `writeFrame`），将 `if (method === 'shutdown') input.close()` 移入 `finally`，并在 `input.on('close')` 中依次等待 `Promise.allSettled([...inFlight])`、`close()` 与 `Promise.allSettled([...pendingWrites])` 排空后再调用 `process.exit(ok ? 0 : 1)`。
2. **会话索引原子落盘刷盘与多路关闭错误聚合保留**：
   - `chat-index.ts` 的 `ChatIndex.commit` 在 `renameSync` 前补齐 `writeFileSync(..., { encoding: 'utf8', flush: true })`，与 `delivery-queue.ts`、`pi-model-file.ts`、`codex-metadata.ts`、`claude-metadata.ts` 的原子持久化契约一致（保持目录 `0o700`、文件 `0o600`）。
   - `structured-runtime-lifecycle.ts` 的 `close()` 与 `desktop-settings.ts` 的 `close()` 原通过 `results.find(...)` 仅抛出首个拒绝原因而静默丢弃第 2 个及之后的清理异常；现统一收集全部拒绝原因为单错误或 `AggregateError`，与 `desktop-runtime.ts`、`chats.ts`、`terminal.ts` 对齐。
3. **全设置面板、设置搜索与会话搜索/归档弹窗的 `Viewport` + `Reveal` 键盘显露闭环（`#20`）**：
   - 核查 `apps/desktop/src/` 全部纵向滚动容器，补齐剩余缺少 `Viewport` + `Reveal` 或纵向滚动条的表面：
     - `settings.rs` / `app.rs` / `settings/language.rs` / `settings/appearance.rs` / `composer.rs`：为“外观”（语言、主题、字号控制）、“聊天”（发送快捷键）与“团队”（自动补充成员开关及重新读取）接入独立 `ScrollHandle`、`Viewport`、`.vertical_scrollbar` 与 `Reveal`，并为“远程接入”、“内置 Pi”、“通知”滚动容器补齐 `.vertical_scrollbar`；
     - `settings/search.rs`：为 `#settings-search-results` 接入 `Viewport` + `.track_scroll` + `.vertical_scrollbar` 与每条命中卡片的 `Reveal`，并在搜索词变更时重置滚动位置；
     - `settings/pi_accounts/view.rs`：补齐提供商计数清除筛选行、活动登录卡片及提示输入区的 `Reveal`；
     - `sessions/search.rs`：为“搜索会话 / 已归档会话”（`#session-search-results`）接入定高弹性容器 + `Viewport` + `.vertical_scrollbar` 以及重试按钮和每条会话结果行的 `Reveal`，同时保留原有的 `↑/↓` `scroll_to_item` 与 `Enter` 打开契约。
   - `focus_scroll.rs` 抽取 `viewport_scroll_delta` 并补充越界上滚、越界下滚、视口内保持及超高分组底部操作对齐的纯几何单元测试。

## 验证证据与边界

- **Sidecar 协议排空、异常关闭与聚合错误实测**（`.artifacts/sidecar-wire-drain-20261113/receipt.json`）：
  - `chatIndexFlush`：原子写入 `flush: true` 与 `0600` 权限重读通过；
  - `lifecycleMultiErrorClose` / `desktopSettingsMultiErrorClose`：双路并发拒绝均完整执行停止并返回 `AggregateError`；
  - `sidecarPipelinedShutdownWithOpenStdin`：真实子进程保持 `stdin` 打开并流水线发送 `desktop.settings.update` + `chat.search` + `shutdown`，`21 ms` 内完整刷出 `[0, 1, 2, 3]` 全部 JSONL 帧并 `exit 0`；
  - `sidecarFaultedShutdownWithOpenStdin`：注入关闭异常并保持 `stdin` 打开发送 `shutdown`，`12 ms` 内完整刷出 `{ "id": 0, "error": { "code": "DESKTOP_ERROR", ... } }` 并主动 `exit 1`（消除原 `3,000 ms` 挂死）。
- **运行时与桌面端回归检查**：
  - `pnpm check && pnpm build:runtime` 及 `check:runtime`、`check:workspace-removal`、`check:remote-settings`、`check:terminal`、`check:workflow-shutdown` 全部 `exit 0`（零厂商模型推理）；
  - `LOCALE_AUDIT_OK`（1,301 键 / 1,194 处引用）、Rust 36 项单元测试、13 项进程树监督测试、两项 Windows MSVC 交叉编译检查与 macOS release 构建均 0 警告通过。

接下来将把本批更新组装进最新 macOS ARM64 开发包，执行包内四组正式门禁、包内协议排空/关闭隔离与启动取消检查、真实 Terra 三会话冷读与中英文原生窗口核验，并同步更新 `/Applications/Hive Desktop.app` 及清理冗余构建产物。
