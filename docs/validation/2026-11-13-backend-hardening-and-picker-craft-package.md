# 5bb7b0b 开发包：后台凭据/交互/关闭边界加固与命令及搜索选择器焦点体验打磨

2026-11-13，源码 `5bb7b0b449e2467846521be3e791bd3c333aec1a`，纳入 [后台凭据/交互/关闭边界加固与命令及搜索选择器焦点体验打磨](2026-11-13-backend-hardening-and-picker-craft.md)（`5bb7b0b`）。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已同步更新。

## 包与安装

相对 `992aae6`，精确更新 13 个条目：

- 原生主程序：`Hive Desktop.app/Contents/MacOS/hive-desktop`（release 构建 SHA-256 `b840184ab2c051a5bd695148037c7b966143846544541e4cb60d538f1e733125`）
- 后台编译产物（12 个文件）：`Hive Desktop.app/Contents/Resources/runtime/dist/` 下的 `agents/external-pi-interactions.js(.map)`、`agents/pi-session-manager.js(.map)`、`chats.js(.map)`、`desktop-runtime.js(.map)`、`hive-host.js(.map)`、`pi-auth.js(.map)`

锁文件、依赖补丁、Cargo 依赖树、包内 Node 22.23.3 与应用身份保持不变，沿用 `3a18b94` 冻结生产依赖。116 个 `runtime/dist` 文件与当前源码构建一致；ZIP 解压后的 28,869 项文件/权限及 29 个内部符号链接与清单逐项一致。

新 ZIP 为 `143,976,854` bytes，SHA-256：

```text
d87a68ea6618a32b113cc952500eac54d0207d01f24e21e2deb63e4033be065c
```

DMG（`234,469,955` bytes，SHA-256 `fa9d67e459b8bb8f97ec414268ca4774055be8cd8f2b84e34b36a330d9b1e05b`）通过 `hdiutil verify`，只读挂载核对全部 28,869 项应用清单及 `/Applications` 快捷链接后卸载。安装前核对旧 `/Applications/Hive Desktop.app`（`992aae6`）清单与 `lsof +D` 无占用，经临时副本核验与备份原子替换，安装后 28,869 项清单一致。current/candidate 为同字节硬链接，仍为未签名、未公证开发包，不是正式发行。

## 包内与原生窗口实际验证

全程使用包内 Node 22.23.3、包内 `runtime/dist` 与包内原生 `hive-desktop`（清除 `HIVE_DESKTOP_NODE` / `HIVE_DESKTOP_RUNTIME` 覆盖，零新厂商模型推理）：

- **四组正式包检查全部 `exit 0`**（`actual-gates.json`）：
  ```sh
  node scripts/check-package.mjs --bundle .artifacts/release-5bb7b0b/extracted/bundle
  node scripts/check-packaged-codex.mjs --bundle .artifacts/release-5bb7b0b/extracted/bundle
  node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-5bb7b0b/extracted/bundle
  node scripts/check-packaged-queue.mjs --bundle .artifacts/release-5bb7b0b/extracted/bundle
  ```
- **包内凭据脱敏、交互取消排空、嵌套会话头与协议关闭防卡死检查**（`packaged-backend-hardening-receipt.json`）：
  - 包内 `PiAuth.logout` 传入 `AbortSignal` 并拦截损坏 `auth.json` 中的模拟凭据片段且保留本地 `cause`，在途 `logout` 目录读取收到 `close()` 后 `0 ms` 即时中止；
  - 包内 `ExternalPiInteractions.cancel()` 在首项取消写抛错时仍完整排空兄弟取消写后再拒绝，双路拒绝聚合为 `AggregateError(2)`；
  - 包内 `createDesktopSessionManager` 自动递归创建缺失深层会话父目录并生成 `0600` 会话头；
  - 包内 `DesktopRuntime` 在工作区移除期间精确拦截 `POST /api/ui/workspaces/<id>/...`（`WORKSPACE_REMOVAL_BLOCKED`），`runtime.close()` 后 `getNavigation` / `saveNavigation` 干净拒绝；
  - 包内 Node 22.23.3 启动 `main.js` 保持 `stdin` 打开：流水线并发请求与 `shutdown` 在 `10 ms` 内完整输出 `[0, 1, 2, 3]` 全部帧并退出 `0`；注入关闭异常时 `6 ms` 内输出 `DESKTOP_ERROR` 错误帧并主动退出 `1`。
- **最新真实 Terra 三会话冷读、角色双语搜索与协议校验**（`wire-receipt.json`、`wire-parser.log`）：
  - 复制 [d43f55e 真实主控与两成员](2026-11-12-latest-terra-team.md) 状态目录至临时目录（原目录未改动），包内正式 `main.js` 先执行冷态 `chat.search('Orchestrator')`（1 条）与 `chat.search('Worker')`（2 条：`proof-alpha`、`proof-beta`），再冷读三会话全部 40 条历史；消息、IDs、模型与完成账本一致，正式后台 `exit 0`、新增模型请求 `0`，输出帧经 Rust 解析器校验通过（`PACKAGED_REAL_CODEX_COLD_FRAMES_VALID count=3`）。
- **包内原生 macOS 窗口中英文启动恢复与深色 20px / 冷读正常退出**（`native-startup-{zh,en}.log`、`native-process-receipt.json`）：
  - `node scripts/check-desktop-startup.mjs --bundle ... --language zh` 与 `--language en` 均通过：原生窗口自动恢复双工作区、失效会话回退、成员会话结构化思考/工具/汇报卡片并写回 SQLite 导航状态。
  - `native-smoke.py` 两次独立自有 PID 原生运行：(1) 英文深色 `20 px` 字号全新欢迎引导页（`welcome-onboarding-en-dark-20px`）原生启动与正常退出 `exitCode: 0`；(2) 中文冷读 `d43f55e` 真实 Terra 三会话副本（`real-terra-cold-restore-zh`）完成 `chat.open` 渲染与正常退出 `exitCode: 0`；两次退出后原生进程与包内 Node sidecar 子进程均确认清空（`allGoneAfterQuit: true`）。

## 收据与存储清理

`.artifacts/release-5bb7b0b/` 保留 `assembly.json`、`manifest.json`、`dependency-diff.json`、`actual-gates.json`、`packaged-backend-hardening-receipt.json`、`wire-receipt.json` / `cold-frames.jsonl` / `wire-parser.log`、`native-startup-zh.log` / `native-startup-en.log`、`native-process-receipt.json`、`dmg-receipt.json`、`installed-manifest-receipt.json`、`backend-receipt.json`、`cleanup.json` 与 `receipt.json`。

在核对清单、ZIP 哈希、保存证据引用路径与 `lsof +D` 无占用后，`cleanup.py` 清理本批重复 `release-5bb7b0b/base/bundle`、`992aae6` 临时安装备份、已被替代的 `release-62e6a6e/extracted/bundle`（保留其归档 ZIP 与收据）以及 `apps/desktop/target` 下的中间调试/Windows 交叉检查/增量构建缓存（保留 `release/hive-desktop` 可执行文件），共回收 `5,753,203,758` 逻辑字节（约 **5.75 GB / 5.36 GiB**），同时保留 `release-992aae6/extracted/bundle` 与 `current.zip` 作为直接可运行回退。系统通知送达/点击授权（`#25`）与真实 Windows 硬件执行仍保持开放。
