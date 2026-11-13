# 9e6a44a 开发包：队列保持、跨层本地化、界面打磨与存储治理

2026-11-13，源码 `9e6a44aa9019695c56d4259984f4ea47ebeb7fb1`，合并纳入 [暂停队列新输入保持、可执行程序解析与元数据原子写清理](2026-11-13-runtime-queue-and-executable-hardening.md)（`271082f`）、[跨层文案本地化、动态回退与双语会话搜索索引](2026-11-13-cross-layer-localization-and-search-aliases.md)（`bf44928`）与 [桌面引导、输入框与协作过程界面层次打磨](2026-11-13-desktop-surface-craft-polish.md)（`9e6a44a`）。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已更新。

## 包与安装

相对 `0a388b3`，精确更新 9 个条目：

- 原生主程序：`Hive Desktop.app/Contents/MacOS/hive-desktop`（release 构建 SHA-256 `08cdfdf93a398fd634badb2f85e7c59319534d4b830b1a76d0e5b6316fe76177`）
- 后台编译产物（8 个文件）：`Hive Desktop.app/Contents/Resources/runtime/dist/` 下的 `chats.js`、`chats.js.map`、`agents/claude.js`、`agents/claude.js.map`、`agents/claude-metadata.js`、`agents/claude-metadata.js.map`、`agents/codex-metadata.js`、`agents/codex-metadata.js.map`

锁文件、依赖补丁、Cargo 依赖树、包内 Node 22.23.3 与应用身份保持不变，继续沿用 `3a18b94` 冻结生产依赖。116 个 `runtime/dist` 文件与当前源码构建一致；ZIP 解压后的 28,869 项文件/权限及 29 个内部符号链接与清单逐项一致。

新 ZIP 为 `143,939,015` bytes，SHA-256：

```text
1924b6472632ae1657d719028fdb857fac15a1250158f620eebcafc2c040fb5b
```

DMG（`234,362,215` bytes，SHA-256 `64b700a0203d1c5518a650c57a99906d979050a9cc26231f6a07dddd815b32fb`）通过 `hdiutil verify`，只读挂载核对全部 28,869 项应用清单及 `/Applications` 快捷链接后卸载。安装前核对旧 `/Applications/Hive Desktop.app`（`0a388b3`）清单与 `lsof +D` 无占用，经临时副本核验与备份原子替换，安装后 28,869 项清单一致。current/candidate 为同字节硬链接，仍为未签名、未公证开发包，不是正式发行。

## 包内与原生窗口实际验证

全程使用包内 Node 22.23.3、包内 `runtime/dist` 与包内原生 `hive-desktop`（清除 `HIVE_DESKTOP_NODE` / `HIVE_DESKTOP_RUNTIME` 覆盖，零新厂商模型推理）：

- **四组正式包检查全部 `exit 0`**（`actual-gates.json`）：
  ```sh
  node scripts/check-package.mjs --bundle .artifacts/release-9e6a44a/extracted/bundle
  node scripts/check-packaged-codex.mjs --bundle .artifacts/release-9e6a44a/extracted/bundle
  node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-9e6a44a/extracted/bundle
  node scripts/check-packaged-queue.mjs --bundle .artifacts/release-9e6a44a/extracted/bundle
  ```
- **包内队列保持、可执行程序/元数据防护与双语搜索验证**（`packaged-hardening-and-aliases-receipt.json`）：
  - Codex 运行中排队补充消息后停止（`abort`）再发补充消息：保持 `queriesBeforeResume: 1`、队列待处理 2 条且 `busy: false`，显式 `resume()` 后按序完整消费（`queriesAfterResume: 3`）。
  - 内置 Pi 队首投递失败后冷重开后台并追加补充消息：保持 `requestsBeforeResume: 1`、原队首 `deliveryFailure` 与 2 条待处理项不变，显式 `resume()` 后完整排空（`requestsAfterResume: 3`）。
  - `resolveClaudeExecutable` 空 `PATH` 段与同名目录过滤、`win32` 优先 `claude.exe` 拒绝无扩展名脚本/`.cmd`，以及 `claude-metadata` / `codex-metadata` 原子重命名失败零 `.tmp` 残留全部通过。
  - `chats.search` 对默认未绑定记录命中 `'New conversation'`、排除自定义同名 `'新会话'`，并支持冷态角色双语搜索（`Workflow` / `工作流`），零 Agent 启动。
- **最新真实 Terra 三会话冷读、角色搜索与协议校验**（`wire-receipt.json`、`wire-parser.log`）：
  - 复制 [d43f55e 真实主控与两成员](2026-11-12-latest-terra-team.md) 状态目录至临时目录（原目录未改动），包内正式 `main.js` 先执行冷态 `chat.search('Orchestrator')`（1 条）与 `chat.search('Worker')`（2 条：`proof-alpha`、`proof-beta`），再冷读三会话全部 40 条历史；消息、IDs、模型与完成账本一致，正式后台 `exit 0`、新增模型请求 `0`，输出帧经 Rust 解析器校验通过（`PACKAGED_REAL_CODEX_COLD_FRAMES_VALID count=3`）。
- **包内原生 macOS 窗口中英文启动恢复与冷读正常退出**（`native-startup-{zh,en}.log`、`native-process-receipt.json`）：
  - `node scripts/check-desktop-startup.mjs --bundle ... --language zh` 与 `--language en` 均通过：原生窗口自动恢复双工作区、失效会话回退、成员会话结构化思考/工具/汇报卡片并写回 SQLite 导航状态。
  - `native-smoke.py` 两次独立自有 PID 原生运行：(1) 英文全新欢迎引导页（`welcome-onboarding-en`）原生启动与正常退出 `exitCode: 0`；(2) 中文冷读 `d43f55e` 真实 Terra 三会话副本（`real-terra-cold-restore-zh`）完成 `chat.open` 渲染与正常退出 `exitCode: 0`；两次退出后原生进程与包内 Node sidecar 子进程均确认清空（`allGoneAfterQuit: true`）。

## 收据与项目存储治理

`.artifacts/release-9e6a44a/` 保留 `assembly.json`、`manifest.json`、`dependency-diff.json`、`actual-gates.json`、`packaged-hardening-and-aliases-receipt.json`、`wire-receipt.json` / `cold-frames.jsonl` / `wire-parser.log`、`native-startup-zh.log` / `native-startup-en.log`、`native-process-receipt.json`、`dmg-receipt.json`、`installed-manifest-receipt.json`、`backend-receipt.json`、`cleanup.json`、`prune-project-bloat.json` 与 `receipt.json`。

在核对清单、ZIP 哈希、文档引用路径与 `lsof +D` 无占用后执行两阶段存储治理，共回收约 **11.03 GB（10.27 GiB）** 项目无用构建与临时产物：

1. `cleanup.py` 清理本批重复 `release-9e6a44a/base/bundle`、`0a388b3` 临时安装备份，并在校验归档 ZIP 与清单后退役已被替代的 `release-7b7940c/extracted/bundle`（共 `1,374,841,327` 逻辑字节），保留 `release-0a388b3/extracted/bundle` 与 `current.zip` 作为直接可运行回退。
2. `prune-project-bloat.py` 清理早期已定案原型 `experiments/gpui-chat/target`（约 5.4 GB，完整保留源码与验证文档）、`.artifacts/sidecar-build-target`、3 份临时 `node_modules` 副本、16 份早期单次调试 `.app` 副本（完整保留对应目录内的截图、AX 树、JSON 收据与脚本）、`packages/runtime/.artifacts/` 中未被文档引用的百余个 `mkdtemp` 临时目录（保留文档引用的 `codex-session-1793990313196` 与 `codex-interactions-BNGKwF`），以及 `apps/desktop/target/debug/` 中已过期的旧二进制副本（共 132 项、`9,651,394,721` 逻辑字节）。未触碰用户凭据、项目外目录或全局缓存。
