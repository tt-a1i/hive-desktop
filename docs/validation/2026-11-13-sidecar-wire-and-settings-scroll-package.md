# 992aae6 开发包：Sidecar 协议帧排空、异常关闭即时退出与全设置/搜索视口焦点闭环

2026-11-13，源码 `992aae68de976b019d98b5a8c281ce80cb90200a`，纳入 [Sidecar 协议帧排空、异常关闭防卡死与全设置/会话搜索视口焦点闭环](2026-11-13-sidecar-wire-and-settings-scroll.md)（`992aae6`）。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已同步更新。

## 包与安装

相对 `62e6a6e`，精确更新 9 个条目：

- 原生主程序：`Hive Desktop.app/Contents/MacOS/hive-desktop`（release 构建 SHA-256 `26e348744cea2748ebc8e3fe2bf32519e83e8c48f1a1bbaac75a00d6d279e344`）
- 后台编译产物（8 个文件）：`Hive Desktop.app/Contents/Resources/runtime/dist/` 下的 `chat-index.js(.map)`、`desktop-settings.js(.map)`、`main.js(.map)`、`structured-runtime-lifecycle.js(.map)`

锁文件、依赖补丁、Cargo 依赖树、包内 Node 22.23.3 与应用身份保持不变，沿用 `3a18b94` 冻结生产依赖。116 个 `runtime/dist` 文件与当前源码构建一致；ZIP 解压后的 28,869 项文件/权限及 29 个内部符号链接与清单逐项一致。

新 ZIP 为 `143,977,499` bytes，SHA-256：

```text
1c04393500e52e1b353866075fb20d6a9471457f9223091c8bda652072ae6d8f
```

DMG（`220,847,465` bytes，SHA-256 `7800d8c98dea259b5f8207bfd401b097278d905610d70609606e878f28f90d3c`）通过 `hdiutil verify`，只读挂载核对全部 28,869 项应用清单及 `/Applications` 快捷链接后卸载。安装前核对旧 `/Applications/Hive Desktop.app`（`62e6a6e`）清单与 `lsof +D` 无占用，经临时副本核验与备份原子替换，安装后 28,869 项清单一致。current/candidate 为同字节硬链接，仍为未签名、未公证开发包，不是正式发行。

## 包内与原生窗口实际验证

全程使用包内 Node 22.23.3、包内 `runtime/dist` 与包内原生 `hive-desktop`（清除 `HIVE_DESKTOP_NODE` / `HIVE_DESKTOP_RUNTIME` 覆盖，零新厂商模型推理）：

- **四组正式包检查全部 `exit 0`**（`actual-gates.json`）：
  ```sh
  node scripts/check-package.mjs --bundle .artifacts/release-992aae6/extracted/bundle
  node scripts/check-packaged-codex.mjs --bundle .artifacts/release-992aae6/extracted/bundle
  node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-992aae6/extracted/bundle
  node scripts/check-packaged-queue.mjs --bundle .artifacts/release-992aae6/extracted/bundle
  ```
- **包内协议帧排空、异常关闭防卡死与多路错误聚合检查**（`packaged-wire-and-shutdown-receipt.json`）：
  - 包内 `ChatIndex` 原子刷盘与 `0600` 权限重读通过；`createStructuredLifecycle.close()` 与 `DesktopSettings.close()` 在多路拒绝时均完整聚合返回 `AggregateError`。
  - 包内 Node 22.23.3 启动 `main.js` 并保持 `stdin` 打开，流水线发送并发请求与 `shutdown`：`18 ms` 内完整输出 `[0, 1, 2, 3]` 全部 JSONL 帧并正常退出 `0`。
  - 注入关闭异常并保持 `stdin` 打开发送 `shutdown`：`13 ms` 内完整输出 `{ "id": 0, "error": { "code": "DESKTOP_ERROR", ... } }` 错误帧并主动退出 `1`（消除原 `3,000 ms` 挂死）。
- **最新真实 Terra 三会话冷读、角色双语搜索与协议校验**（`wire-receipt.json`、`wire-parser.log`）：
  - 复制 [d43f55e 真实主控与两成员](2026-11-12-latest-terra-team.md) 状态目录至临时目录（原目录未改动），包内正式 `main.js` 先执行冷态 `chat.search('Orchestrator')`（1 条）与 `chat.search('Worker')`（2 条：`proof-alpha`、`proof-beta`），再冷读三会话全部 40 条历史；消息、IDs、模型与完成账本一致，正式后台 `exit 0`、新增模型请求 `0`，输出帧经 Rust 解析器校验通过（`PACKAGED_REAL_CODEX_COLD_FRAMES_VALID count=3`）。
- **包内原生 macOS 窗口中英文启动恢复与深色 20px / 冷读正常退出**（`native-startup-{zh,en}.log`、`native-process-receipt.json`）：
  - `node scripts/check-desktop-startup.mjs --bundle ... --language zh` 与 `--language en` 均通过：原生窗口自动恢复双工作区、失效会话回退、成员会话结构化思考/工具/汇报卡片并写回 SQLite 导航状态。
  - `native-smoke.py` 两次独立自有 PID 原生运行：(1) 英文深色 `20 px` 字号全新欢迎引导页（`welcome-onboarding-en-dark-20px`）原生启动与正常退出 `exitCode: 0`；(2) 中文冷读 `d43f55e` 真实 Terra 三会话副本（`real-terra-cold-restore-zh`）完成 `chat.open` 渲染与正常退出 `exitCode: 0`；两次退出后原生进程与包内 Node sidecar 子进程均确认清空（`allGoneAfterQuit: true`）。

## 收据与存储清理

`.artifacts/release-992aae6/` 保留 `assembly.json`、`manifest.json`、`dependency-diff.json`、`actual-gates.json`、`packaged-wire-and-shutdown-receipt.json`、`wire-receipt.json` / `cold-frames.jsonl` / `wire-parser.log`、`native-startup-zh.log` / `native-startup-en.log`、`native-process-receipt.json`、`dmg-receipt.json`、`installed-manifest-receipt.json`、`backend-receipt.json`、`cleanup.json` 与 `receipt.json`。

在核对清单、ZIP 哈希、保存证据引用路径与 `lsof +D` 无占用后，`cleanup.py` 清理本批重复 `release-992aae6/base/bundle`、`62e6a6e` 临时安装备份、已被替代的 `release-4b3135d/extracted/bundle`（保留其归档 ZIP 与收据）、临时协议校验构建目录以及 `apps/desktop/target` 下的中间调试/交叉编译/增量构建缓存（保留 `release/hive-desktop` 可执行文件），共回收 `6,684,641,371` 逻辑字节（约 **6.68 GB / 6.23 GiB**），同时保留 `release-62e6a6e/extracted/bundle` 与 `current.zip` 作为直接可运行回退。系统通知送达/点击授权（`#25`）与真实 Windows 硬件执行仍保持开放。
