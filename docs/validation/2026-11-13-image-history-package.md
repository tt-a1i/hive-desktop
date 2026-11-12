# 0a388b3 开发包：图片历史分页与外部 Pi 诊断

2026-11-13，源码 `0a388b32b83c7d2d568b7aec808c9a30fbe8050e`，合并纳入 [外部 Pi 扩展诊断](2026-11-12-external-pi-diagnostics.md)（`a26ab5a`）、[外部 Pi 合规图片历史](2026-11-12-external-pi-image-history.md)（`f8a9460`）与 [Codex 合规图片历史与增量分页](2026-11-12-codex-image-history.md)（`0a388b3`）。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已更新。本批仅刷新后台模块，无新厂商推理或面板操作。

## 包与安装

相对 `7b7940c`，仅精确更新 `Hive Desktop.app/Contents/Resources/runtime/dist/agents/` 下的 8 个文件：`codex.js`、`codex.js.map`、`codex-transport.js`、`codex-transport.js.map`、`external-pi.js`、`external-pi.js.map`、`external-pi-transport.js`、`external-pi-transport.js.map`。

原生源码、锁文件、依赖补丁和 package metadata 无变化，复用 `3a18b94` 冻结生产依赖、包内 Node 22.23.3 与应用身份。116 个 `runtime/dist` 文件全部与当前源码构建一致；ZIP 解压后的 28,869 项文件/权限、29 个内部符号链接与组装清单一致。原生程序 SHA-256 保持 `d9572c345976ebc0855f6ca5e7b24235b1c9da8f308f87790b61eae6dfcbcf6a`。

新 ZIP 为 143,939,462 bytes，SHA-256：

```text
9c3ee06be0d96b3698947007c3747ca7dad011a52b2587c1fb704d8a2e104d10
```

DMG（235,146,972 bytes，SHA-256 `0a4ac9b19d4f8937c437763669bb8fd310cdc133c25d5824e740d133996c4d02`）通过 `hdiutil verify`，随后只读挂载核对全部 28,869 项应用清单及 `/Applications` 快捷链接并卸载。安装前核对旧 `/Applications/Hive Desktop.app` 清单与 `lsof +D` 无占用，经临时副本核验与备份原子替换，安装后 28,869 项完整清单一致。current/candidate 为同字节硬链接，仍为未签名、未公证开发包，不是正式发行。

## 包内实际验证

使用解压包内 Node 22.23.3 与实际包内 `runtime/dist` / `node_modules` 运行本地受控协议验证（零厂商模型请求）：

- **外部 Pi 扩展诊断**：原始 Pi `1.4.0`、真实 Hive HTTP / SQLite 与真实 `bash` 工具覆盖 6 个场景（`settled`、`end`、`context`、`command`、`model`、`tool-failure`）及完整冷重开恢复；成功回合后的钩子诊断展示但不再误判投递失败或重复执行，`command` 与模型真失败保留。
- **外部 Pi 合规图片历史**：原始 Pi `1.4.0` 覆盖 6 个场景：12 轮累计 `38,901,360` 字符 base64（> 32 MiB）冷读恢复并继续第 13 轮、Hive 上传 4 轮多图 + `bash` 工具 + 冷重开、单轮 6 图 `19,450,680` 字符合规上限及冷重开、`dispatch` 3 图原字节保持、`ctx.newSession()` 新会话游标重置、33 MiB 单行超限有界拒绝。
- **Codex 合规图片历史与冷恢复**：原始 Codex `0.167.0` 覆盖 5 个场景：单轮 6 图 `19,450,680` 字符合规上限及冷重开、`DesktopRuntime` 4 轮多图 + 文本 + 冷重开、12 轮累计 `38,901,360` 字符（> 32 MiB）+ `hive_probe` 动态工具 + `excludeTurns: true` 冷重开（冷开新增请求 0）、`dispatch` 3 图原字节保持、33 MiB 单行超限有界拒绝。
- **最新真实 Terra 历史冷读与消息格式**：将 [d43f55e 真实主控与两成员](2026-11-12-latest-terra-team.md) 的状态目录复制到临时目录（原证据目录未改动），由包内正式 `main.js` 冷读三会话全部 40 条历史，消息、工具结果、IDs、模型与完成账本深比较一致，`pending: 0`、正式后台 `exit 0`、新增模型请求 `0`；输出的三条 JSONL 帧经同版 `serde_json` Rust 解析器校验通过。

四组包检查全部 `exit 0`：

```sh
node scripts/check-package.mjs --bundle .artifacts/release-0a388b3/extracted/bundle
node scripts/check-packaged-codex.mjs --bundle .artifacts/release-0a388b3/extracted/bundle
node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-0a388b3/extracted/bundle
node scripts/check-packaged-queue.mjs --bundle .artifacts/release-0a388b3/extracted/bundle
```

首次运行 `gates.py` 时，当前 shell 未设置 `OPENAI_API_KEY`，导致 `check-packaged-dispatches` 内 `user-input-recovery`（`PI_OFFLINE=1` 且队列暂停、无网络请求）在发送前因临时认证目录无 OpenAI 凭据被内置模型可用性检查拒绝；未改动任何正式检查脚本，仅在本次 `gates.py` 运行环境补充占位 `OPENAI_API_KEY` 后重跑，四组包检查均 `exit 0`。未增改删正式测试；原生二进制未变，本批未重复打开原生窗口，不扩大为桌面长图片历史渲染性能或 Windows 原生验收。通知送达/点击、Pi 登录重授与 Claude 真实推理边界保持不变，完整产品目标继续。

## 收据与存储清理

`.artifacts/release-0a388b3/` 保留 `assembly.json`、`manifest.json`、`dependency-diff.json`、`actual-gates.json`、`packaged-ext-pi-diagnostics-receipt.json`、`packaged-ext-pi-images-receipt.json`、`packaged-codex-images-receipt.json`、`wire-receipt.json` / `cold-frames.jsonl` / `wire-parser.log`、`dmg-receipt.json`、`installed-manifest-receipt.json`、`backend-receipt.json` 与 `receipt.json`。

存储清理分两步并在核对完整清单、归档 ZIP SHA-256 及 `lsof +D` 无占用后执行：

1. `cleanup.py` 删除本批重复 `base/bundle`、`7b7940c` 临时安装备份及诊断临时工作目录（`917,380,898` 逻辑字节），保留 `release-7b7940c/extracted/bundle` 与 `current.zip` 作为直接可运行回退（见 `cleanup.json`）。
2. `prune-superseded.py` 退役 11 个更早已被替代的 `extracted/bundle` 解压树（`0080304`、`1e68306`、`293afe3`、`d644db9`、`3d4422f`、`db12698`、`46659e2`、`a744821`、`ff2aa70`、`3a18b94`、`78fef23`，共 `5,040,676,904` 逻辑字节），所有对应归档 ZIP、DMG、脚本、夹具、日志与历史收据均完整保留，各旧目录写入 `storage-status.json` 记录退役与 `ditto -x -k` 恢复方式（见 `prune-superseded.json`）；历史文档中对上述更早解压目录的引用代表当时阶段状态。未触碰用户项目、凭据或共享缓存。
