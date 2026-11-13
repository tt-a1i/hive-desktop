# 4b3135d 开发包：停止清理竞态加固、次级面板显露与视觉层次打磨

2026-11-13，源码 `4b3135d0189c46421be3610095f2307d96d66f61`，合并纳入 [会话停止清理竞态、Codex 账号探测取消与显示日志原子写防护](2026-11-13-runtime-stop-and-probe-hardening.md)（`cd32e9d`）与 [桌面后台生命周期竞态、次级面板焦点显露与界面层次打磨](2026-11-13-desktop-panels-hardening-and-craft.md)（`4b3135d`）。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已同步更新。

## 包与安装

相对 `9e6a44a`，精确更新 13 个条目：

- 原生主程序：`Hive Desktop.app/Contents/MacOS/hive-desktop`（release 构建 SHA-256 `dd8137e5c978eccdd4094e52611398082fbffca4e23ddf554eaaef85e7e02d96`）
- 后台编译产物（12 个文件）：`Hive Desktop.app/Contents/Resources/runtime/dist/` 下的 `chats.js`、`chats.js.map`、`codex-account.js`、`codex-account.js.map`、`structured-runtime-lifecycle.js`、`structured-runtime-lifecycle.js.map`、`team-tool.js`、`team-tool.js.map`、`agents/codex-projection.js`、`agents/codex-projection.js.map`、`agents/codex-transport.js`、`agents/codex-transport.js.map`

锁文件、依赖补丁、Cargo 依赖树、包内 Node 22.23.3 与应用身份保持不变，沿用 `3a18b94` 冻结生产依赖。116 个 `runtime/dist` 文件与当前源码构建一致；ZIP 解压后的 28,869 项文件/权限及 29 个内部符号链接与清单逐项一致。

新 ZIP 为 `143,980,499` bytes，SHA-256：

```text
19e0ce4df703df3f840d44861d1409888c15ccf3b59a644f26148e9d77980d05
```

DMG（`219,125,962` bytes，SHA-256 `23328d6a600452691860cd1c2a20c180820c9270228329bf0d5d0a65129fe0bf`）通过 `hdiutil verify`，只读挂载核对全部 28,869 项应用清单及 `/Applications` 快捷链接后卸载。安装前核对旧 `/Applications/Hive Desktop.app`（`9e6a44a`）清单与 `lsof +D` 无占用，经临时副本核验与备份原子替换，安装后 28,869 项清单一致。current/candidate 为同字节硬链接，仍为未签名、未公证开发包，不是正式发行。

## 包内与原生窗口实际验证

全程使用包内 Node 22.23.3、包内 `runtime/dist` 与包内原生 `hive-desktop`（清除 `HIVE_DESKTOP_NODE` / `HIVE_DESKTOP_RUNTIME` 覆盖，零新厂商模型推理）：

- **四组正式包检查全部 `exit 0`**（`actual-gates.json`）：
  ```sh
  node scripts/check-package.mjs --bundle .artifacts/release-4b3135d/extracted/bundle
  node scripts/check-packaged-codex.mjs --bundle .artifacts/release-4b3135d/extracted/bundle
  node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-4b3135d/extracted/bundle
  node scripts/check-packaged-queue.mjs --bundle .artifacts/release-4b3135d/extracted/bundle
  ```
- **包内停止清理串行化、异常停止无泄漏回收、探测取消与原子写防护检查**（`packaged-teardown-and-probe-receipt.json`）：
  - `stopLive` 释放期间并发 `chats.open` 与 `chats.reasoning`：`cleanupSetDuringStop: true`、等待旧句柄释放后返回 `openDuringDisposeRunId: null` 与 `reasoningLevel: "medium"`。
  - `stopAgentRun` 遇到 `abort` 抛错且传输层未主动断开：`exitedInTime: true`、SQLite `runStatus: "error"`、鉴权令牌吊销、子进程回收（`childDead: true`）且 `liveCleared: true`。
  - `probeCodexAccount` 在 `initialize` 挂起时触发 `AbortController.abort()`：`462 ms` 内返回 `status: "cancelled"` 并回收挂起子进程。
  - `CodexProjection.save()` 原子重命名失败零 `.tmp` 残留（`leftoverTmp: 0`）。
- **最新真实 Terra 三会话冷读、角色双语搜索与协议校验**（`wire-receipt.json`、`wire-parser.log`）：
  - 复制 [d43f55e 真实主控与两成员](2026-11-12-latest-terra-team.md) 状态目录至临时目录（原目录未改动），包内正式 `main.js` 先执行冷态 `chat.search('Orchestrator')`（1 条）与 `chat.search('Worker')`（2 条：`proof-alpha`、`proof-beta`），再冷读三会话全部 40 条历史；消息、IDs、模型与完成账本一致，正式后台 `exit 0`、新增模型请求 `0`，输出帧经 Rust 解析器校验通过（`PACKAGED_REAL_CODEX_COLD_FRAMES_VALID count=3`）。
- **包内原生 macOS 窗口中英文启动恢复与冷读正常退出**（`native-startup-{zh,en}.log`、`native-process-receipt.json`）：
  - `node scripts/check-desktop-startup.mjs --bundle ... --language zh` 与 `--language en` 均通过：原生窗口自动恢复双工作区、失效会话回退、成员会话结构化思考/工具/汇报卡片并写回 SQLite 导航状态。
  - `native-smoke.py` 两次独立自有 PID 原生运行：(1) 英文全新欢迎引导页（`welcome-onboarding-en`）原生启动与正常退出 `exitCode: 0`；(2) 中文冷读 `d43f55e` 真实 Terra 三会话副本（`real-terra-cold-restore-zh`）完成 `chat.open` 渲染与正常退出 `exitCode: 0`；两次退出后原生进程与包内 Node sidecar 子进程均确认清空（`allGoneAfterQuit: true`）。

## 收据与存储清理

`.artifacts/release-4b3135d/` 保留 `assembly.json`、`manifest.json`、`dependency-diff.json`、`actual-gates.json`、`packaged-teardown-and-probe-receipt.json`、`wire-receipt.json` / `cold-frames.jsonl` / `wire-parser.log`、`native-startup-zh.log` / `native-startup-en.log`、`native-process-receipt.json`、`dmg-receipt.json`、`installed-manifest-receipt.json`、`backend-receipt.json`、`cleanup.json` 与 `receipt.json`。

在核对清单、ZIP 哈希、保存证据引用路径与 `lsof +D` 无占用后，`cleanup.py` 清理本批重复 `release-4b3135d/base/bundle`、`9e6a44a` 临时安装备份、已被替代的 `release-0a388b3/extracted/bundle`（保留其归档 ZIP 与收据）、临时协议校验构建目录及过期调试二进制，共回收 `1,407,063,237` 逻辑字节（约 **1.41 GB / 1.31 GiB**），同时保留 `release-9e6a44a/extracted/bundle` 与 `current.zip` 作为直接可运行回退。系统通知送达/点击授权（`#25`）与真实 Windows 硬件执行仍保持开放。
