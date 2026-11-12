# a744821 开发包：恢复摘要与旧历史可读性

2026-11-12。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已更新到 `a74482178718e841b787f9fd38b07e710304e0a5`。包含[恢复 Unicode 修复](2026-11-12-recovery-unicode.md)。这是未签名、未公证的 Mac ARM64 开发包，完整产品验收仍开放。

## 包内证据

- 生产依赖重新执行 `pnpm --filter @hive-desktop/runtime deploy --prod --frozen-lockfile --offline --node-linker=hoisted .artifacts/release-a744821/deployed-runtime`。无依赖版本升级，补丁基线改为 a744821。
- 复用上一包经官方校验的 Node 22.23.3、未变的原生二进制、图标及身份信息，不重复下载 Node 或构建 Rust。完整文件清单证明与 46659e2 精确只有九项变化：main.js/map、两个 Hive 生产模块及五个部署元数据文件。
- 116 个 runtime 文件与源码构建输出一致；ZIP 独立解压后 28,869 项文件、权限及 29 个内部链接与组装清单一致。
- 解压包内 Node/runtime 运行实际 Hive HTTP/SQLite/Pi SDK 重启恢复，新摘要保留完整 emoji，任务原文保持，受控模型端点请求数 0。直接调用包内格式化函数另验证三种摘要边界及错误预览，不冒称该格式化检查为 HTTP report 集成。
- 包内生产 JSONL 冷读修复前的同一损坏 SDK 会话，原生同版本 serde_json 接受实际 stdout 帧；新恢复 snapshot 同样通过 Rust 解析。原 SDK 文件逐字节及 SHA256 保持。没有 resume 旧会话或发起厂商请求。
- 四项现有包检查 `check-package.mjs`、`check-packaged-codex.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs` 均以 `--bundle .artifacts/release-a744821/extracted/bundle` 实际运行，exit 0。涉及真实 PTY/HTTP/SQLite，合成对端不等于厂商推理。
- 解压包原生窗口在独立数据目录启动，未设置 Node/runtime 覆盖；AX 确认后台就绪、配置入口可用，正常 Quit 后 exit 0，原生父进程和其 Node 子进程均已退出。本轮原生仅检查启动退出；历史可读性由生产 JSONL 和同版 Rust 断言证明，未声称在窗口逐条读过旧历史。
- DMG 创建和校验通过，只读挂载后的应用完整清单及 Applications 链接匹配，已卸载。更新 Applications 前核对旧清单并确认无打开文件，分阶段复制与替换，安装后的完整清单一致；安装副本本轮未另启动。

## 交付物

ZIP：143,935,720 字节，SHA256 `1e066630357dec9a786d62833e4b680ff2ee000f749adae6fd50b740400dc993`。current/candidate 为同字节硬链接。

DMG：237,150,973 字节，SHA256 `65199756659ce1e5dd1b3b0791d66fe02c58d10bea59bf3c049b1f8591a4f1f7`。

原生二进制 SHA256 `d9572c345976ebc0855f6ca5e7b24235b1c9da8f308f87790b61eae6dfcbcf6a`，与前包相同。源码、锁文件、部署日志、完整差异、JSONL/Rust 证据、实际 gate 退出码、AX、原生退出、DMG/安装收据位于 `.artifacts/release-a744821/`。

本批使用定向回归、源码类型检查/构建和包级验证，未跑全量测试：改动集中于文本生成和 JSONL 序列化。没有新增或修改正式测试。Pi OAuth 按用户要求继续搁置，真实推理限制仍为 Terra；本轮没有新推理。Claude 原生模型、系统通知授权、Windows 真机及完整产品体验尚未验收。

## 存储整理

完整清单相等、回退 ZIP 哈希和无打开文件检查后，删除本次重复 base、旧安装临时备份及独立 deployed-runtime，合计 1,225,372,116 逻辑字节。当前解压包、46659e2 可运行回退及 ZIP、任务/会话数据、源补丁、锁文件、日志和收据保留；没有清理共享 Rust/pnpm 缓存或用户项目。部署树已经删除，需要时按上述命令重新部署。清单见 cleanup.json 与 cleanup-deployed.json。
