# c0c1163 开发包：Beluga 引导页 + T3 Code 主窗口/侧栏/会话流/浮动输入框 UI/UX 重构

2026-11-13，源码 `c0c116382a8cc53433539c50cee37a7dad5bc9c5`，纳入 [Beluga + T3 Code UI/UX 重构验证](2026-11-13-beluga-t3code-ui-overhaul.md)（`c0c1163`）与 [设计复刻与融合记录](../research/2026-11-13-beluga-t3code-ui-overhaul.md)。current/candidate ZIP 与 DMG 已同步更新。

## 包与安装状态

相对 `5bb7b0b`，精确更新 1 个条目：

- 原生主程序：`Hive Desktop.app/Contents/MacOS/hive-desktop`（release 构建 SHA-256 `395b9f686b6219d05d35b30dcbe9e9009c099c7178fbb0163c5197af5f550595`）

锁文件、依赖补丁、Cargo 依赖树、包内 Node 22.23.3、116 个 `runtime/dist` 文件与应用身份保持不变，沿用 `3a18b94` 冻结生产依赖；ZIP 解压后的 28,869 项文件/权限及 29 个内部符号链接与清单逐项一致。

新 ZIP 为 `144,002,136` bytes，SHA-256：

```text
3ba3e9ccff339dac1a0498fc3f2d54aaf0feb6f83de5945b6611660f7c80241f
```

DMG（`228,092,164` bytes，SHA-256 `17ec0e33c19864263c2628a4a96fd49981f0c0aa5d0c4493b1c4b830a97d39e0`）通过 `hdiutil verify`，只读挂载核对全部 28,869 项应用清单及 `/Applications` 快捷链接后卸载。由于打包时检测到用户会话中正在运行旧 `/Applications/Hive Desktop.app`（`5bb7b0b`），安装脚本按安全保护未强制终止用户运行实例；已就绪的新版 `.app` 位于 `.artifacts/release-c0c1163/extracted/bundle/Hive Desktop.app`（以及 `.artifacts/release-c0c1163/Hive-Desktop-macos-arm64.dmg`），退出当前打开的旧应用后即可直接替换或打开体验。current/candidate 为同字节硬链接，仍为未签名、未公证开发包，不是正式发行。

## 包内与原生窗口实际验证

全程使用包内 Node 22.23.3、包内 `runtime/dist` 与包内原生 `hive-desktop`（清除 `HIVE_DESKTOP_NODE` / `HIVE_DESKTOP_RUNTIME` 覆盖，零新厂商模型推理）：

- **四组正式包检查全部 `exit 0`**（`actual-gates.json`）：
  ```sh
  node scripts/check-package.mjs --bundle .artifacts/release-c0c1163/extracted/bundle
  node scripts/check-packaged-codex.mjs --bundle .artifacts/release-c0c1163/extracted/bundle
  node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-c0c1163/extracted/bundle
  node scripts/check-packaged-queue.mjs --bundle .artifacts/release-c0c1163/extracted/bundle
  ```
- **最新真实 Terra 三会话冷读、角色双语搜索与协议校验**（`wire-receipt.json`、`wire-parser.log`）：
  - 复制 [d43f55e 真实主控与两成员](2026-11-12-latest-terra-team.md) 状态目录至临时目录（原目录未改动），包内正式 `main.js` 完成冷态 `chat.search('Orchestrator')` / `chat.search('Worker')` 与三会话全部 40 条历史冷读，正式后台 `exit 0`、新增模型请求 `0`，输出帧经 Rust 解析器校验通过（`PACKAGED_REAL_CODEX_COLD_FRAMES_VALID count=3`）。
- **包内原生 macOS 窗口中英文启动恢复与三场景原生运行正常退出**（`native-startup-{zh,en}.log`、`native-process-receipt.json`）：
  - `node scripts/check-desktop-startup.mjs --bundle ... --language zh` 与 `--language en` 均通过：新 T3 Code 主窗口骨架与侧栏自动恢复双工作区、失效会话回退、成员会话结构化思考/工具/汇报卡片并写回 SQLite 导航状态。
  - `native-smoke.py` 三次独立自有 PID 原生运行：(1) 中文浅色 `16 px` Beluga 欢迎引导页（`welcome-onboarding-zh-light-16px`）；(2) 英文深色 `20 px` Beluga 欢迎引导页（`welcome-onboarding-en-dark-20px`）；(3) 中文深色 T3 Code 主窗口冷读 `d43f55e` 真实 Terra 三会话副本（`real-terra-cold-restore-zh-dark`）完成 `chat.open` 渲染与正常退出；三次均 `exitCode: 0`，退出后原生进程与包内 Node sidecar 子进程全部确认清空（`allGoneAfterQuit: true`）。

## 收据与存储清理

`.artifacts/release-c0c1163/` 保留 `assembly.json`、`manifest.json`、`dependency-diff.json`、`actual-gates.json`、`wire-receipt.json` / `cold-frames.jsonl` / `wire-parser.log`、`native-startup-zh.log` / `native-startup-en.log`、`native-process-receipt.json`、`dmg-receipt.json`、`installed-manifest-receipt.json`、`backend-receipt.json`、`cleanup.json` 与 `receipt.json`。

在核对清单、ZIP 哈希、保存证据引用路径与 `lsof +D` 无占用后，`cleanup.py` 清理本批重复 `release-c0c1163/base/bundle`、已被替代的 `release-992aae6/extracted/bundle`（保留其归档 ZIP 与收据）、临时 UI 参考仓库克隆 `.artifacts/ui-refs-20261113` 以及 `apps/desktop/target` 下的中间调试/Windows 交叉检查/增量构建缓存（保留 `release/hive-desktop` 可执行文件），共回收 `6,836,317,734` 逻辑字节（约 **6.84 GB / 6.37 GiB**），同时保留 `release-5bb7b0b/extracted/bundle` 与 `current.zip` 作为直接可运行回退。多 Agent 互动编排专属 UI 按确认结论暂缓设计；系统通知送达/点击授权（`#25`）与真实 Windows 硬件执行仍保持开放。
