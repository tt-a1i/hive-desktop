# 62e6a6e 开发包：运行时关闭隔离、启动期握手即时取消与全表面双语/窄窗打磨

2026-11-13，源码 `62e6a6e0677eb68a4c21e11a5eef346091b52954`，合并纳入 [运行时关闭隔离、启动期握手即时取消与开发态远程命令路径](2026-11-13-runtime-shutdown-and-startup-cancellation.md)（`2abce78`）与 [桌面端闭合错误本地化闭环、会话资源窄窗显露与交互/预览表面打磨](2026-11-13-desktop-diagnostic-localization-and-surfaces.md)（`62e6a6e`）。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已同步更新。

## 包与安装

相对 `4b3135d`，精确更新 23 个条目：

- 原生主程序：`Hive Desktop.app/Contents/MacOS/hive-desktop`（release 构建 SHA-256 `41fa3d769af4e6d4eed365cf4d3e1514fde9c104108a1c0ce665ae3799c57fe7`）
- 后台编译产物（22 个文件）：`Hive Desktop.app/Contents/Resources/runtime/dist/` 下的 `agent-models.js(.map)`、`chats.js(.map)`、`desktop-runtime.js(.map)`、`remote-commands.js(.map)`、`terminal.js(.map)`、`agents/claude-input.js(.map)`、`agents/claude.js(.map)`、`agents/codex-transport.js(.map)`、`agents/codex.js(.map)`、`agents/external-pi-transport.js(.map)`、`agents/external-pi.js(.map)`

锁文件、依赖补丁、Cargo 依赖树、包内 Node 22.23.3 与应用身份保持不变，沿用 `3a18b94` 冻结生产依赖。116 个 `runtime/dist` 文件与当前源码构建一致；ZIP 解压后的 28,869 项文件/权限及 29 个内部符号链接与清单逐项一致。

新 ZIP 为 `143,971,563` bytes，SHA-256：

```text
bfb4d71e551ec54c65cd83f0533af096793e62ff5433f7b53c3c8c6b5539c80d
```

DMG（`229,413,285` bytes，SHA-256 `ec354ad35399fb97b474104ddac2b1ccf61bb99543063e746ee026aeade9bd8f`）通过 `hdiutil verify`，只读挂载核对全部 28,869 项应用清单及 `/Applications` 快捷链接后卸载。安装前核对旧 `/Applications/Hive Desktop.app`（`4b3135d`）清单与 `lsof +D` 无占用，经临时副本核验与备份原子替换，安装后 28,869 项清单一致。current/candidate 为同字节硬链接，仍为未签名、未公证开发包，不是正式发行。

## 包内与原生窗口实际验证

全程使用包内 Node 22.23.3、包内 `runtime/dist` 与包内原生 `hive-desktop`（清除 `HIVE_DESKTOP_NODE` / `HIVE_DESKTOP_RUNTIME` 覆盖，零新厂商模型推理）：

- **四组正式包检查全部 `exit 0`**（`actual-gates.json`）：
  ```sh
  node scripts/check-package.mjs --bundle .artifacts/release-62e6a6e/extracted/bundle
  node scripts/check-packaged-codex.mjs --bundle .artifacts/release-62e6a6e/extracted/bundle
  node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-62e6a6e/extracted/bundle
  node scripts/check-packaged-queue.mjs --bundle .artifacts/release-62e6a6e/extracted/bundle
  ```
- **包内关闭隔离与三后端启动期握手即时取消检查**（`packaged-shutdown-and-startup-cancel-receipt.json`）：
  - 包内 `remote-commands.js` 正确定位 `runtime/dist/remote-cli.js`（`pass: true`）。
  - `codex` + `pi-rpc` + `claude` 三路目录查询挂起时调用 `agentModels.close()`：`207 ms` 完成关闭并回收全部三个挂起子进程（`childrenReaped: true`）。
  - 三路会话打开挂起 + 一路快错与一路慢速释放并存时调用 `chats.close()`：`357 ms` 排空全部 `opening` 与 `live` 句柄（`siblingFinishedAtReturn: true`）、回收三路挂起子进程并保留原错误。
  - `settings.close()` 与 `piAuth.close()` 同时抛错时调用 `runtime.close()`：`terminalClosed: true`、`agentModelsClosed: true`，聚合返回 `["settings-close-fault", "piauth-close-fault"]`。
- **最新真实 Terra 三会话冷读、角色双语搜索与协议校验**（`wire-receipt.json`、`wire-parser.log`）：
  - 复制 [d43f55e 真实主控与两成员](2026-11-12-latest-terra-team.md) 状态目录至临时目录（原目录未改动），包内正式 `main.js` 先执行冷态 `chat.search('Orchestrator')`（1 条）与 `chat.search('Worker')`（2 条：`proof-alpha`、`proof-beta`），再冷读三会话全部 40 条历史；消息、IDs、模型与完成账本一致，正式后台 `exit 0`、新增模型请求 `0`，输出帧经 Rust 解析器校验通过（`PACKAGED_REAL_CODEX_COLD_FRAMES_VALID count=3`）。
- **包内原生 macOS 窗口中英文启动恢复与冷读正常退出**（`native-startup-{zh,en}.log`、`native-process-receipt.json`）：
  - `node scripts/check-desktop-startup.mjs --bundle ... --language zh` 与 `--language en` 均通过：原生窗口自动恢复双工作区、失效会话回退、成员会话结构化思考/工具/汇报卡片并写回 SQLite 导航状态。
  - `native-smoke.py` 两次独立自有 PID 原生运行：(1) 英文全新欢迎引导页（`welcome-onboarding-en`）原生启动与正常退出 `exitCode: 0`；(2) 中文冷读 `d43f55e` 真实 Terra 三会话副本（`real-terra-cold-restore-zh`）完成 `chat.open` 渲染与正常退出 `exitCode: 0`；两次退出后原生进程与包内 Node sidecar 子进程均确认清空（`allGoneAfterQuit: true`）。

## 收据与存储清理

`.artifacts/release-62e6a6e/` 保留 `assembly.json`、`manifest.json`、`dependency-diff.json`、`actual-gates.json`、`packaged-shutdown-and-startup-cancel-receipt.json`、`wire-receipt.json` / `cold-frames.jsonl` / `wire-parser.log`、`native-startup-zh.log` / `native-startup-en.log`、`native-process-receipt.json`、`dmg-receipt.json`、`installed-manifest-receipt.json`、`backend-receipt.json`、`cleanup.json` 与 `receipt.json`。

在核对清单、ZIP 哈希、保存证据引用路径与 `lsof +D` 无占用后，`cleanup.py` 清理本批重复 `release-62e6a6e/base/bundle`、`4b3135d` 临时安装备份、已被替代的 `release-9e6a44a/extracted/bundle`（保留其归档 ZIP 与收据）、临时协议校验构建目录及本轮回归临时收据目录，共回收 `1,405,328,908` 逻辑字节（约 **1.41 GB / 1.31 GiB**），同时保留 `release-4b3135d/extracted/bundle` 与 `current.zip` 作为直接可运行回退。系统通知送达/点击授权（`#25`）与真实 Windows 硬件执行仍保持开放。
