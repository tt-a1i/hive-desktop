# f9f3c2b 开发包：固定协作入口与聊天阅读设置

2026-11-14。源码 `f9f3c2bd8a80a82d0ceb3d3b74640bf1766e126d`，纳入 [#29 的实现与原生验证](2026-11-14-workspace-chat-settings.md)。current/candidate ZIP 已指向该版本，DMG 已生成并验证；用户正在运行的 `/Applications/Hive Desktop.app` 未替换。完整目标继续开放。

## 内容与完整性

- 默认功能的 macOS ARM64 优化 release，未启用 `ui-preview`。相对实查的 `26ebdea` 开发包仅更新 `Hive Desktop.app/Contents/MacOS/hive-desktop`。
- Node、116 份 runtime 输出、依赖/锁文件、图标、应用身份均保持；28,869 项文件/模式与 29 条内部链接在解压后完整匹配。
- ZIP：144,125,620 字节，SHA-256 `1f968d6a11384624bafe5eea095ad3cc001719d092d3278349d2bd1f912e9485`。
- 主程序 SHA-256：`4b88a721d9f7d78c5baa82d6f704fb2586c5fa4b0ed244cd03f7459d8420cdf0`。
- DMG：238,445,581 字节，SHA-256 `0078241e0901a8c530c34b5c7262458cda0841950804c15d5a990bf4eb730f70`。`hdiutil verify`、只读挂载后的完整应用清单与 Applications 链接核对通过，已卸载。

## 包内检查

以下均使用最终解压包，exit 0：

```sh
node scripts/check-package.mjs --bundle .artifacts/release-f9f3c2b/extracted/bundle
node scripts/check-packaged-codex.mjs --bundle .artifacts/release-f9f3c2b/extracted/bundle
node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-f9f3c2b/extracted/bundle
node scripts/check-packaged-queue.mjs --bundle .artifacts/release-f9f3c2b/extracted/bundle
node scripts/check-desktop-startup.mjs --bundle .artifacts/release-f9f3c2b/extracted/bundle --language en
```

覆盖真实 PTY/HTTP/SQLite、配置、派单/汇报、停止与恢复、原生启动导航、Codex 受控协议及内外 Pi 的本地响应队列。保留原检查的真实与合成边界，不当作新的厂商推理。

复制 d43f55e 真实 Terra 三会话状态至临时目录，以包内 Node/正式 `main.js` 读取；原完整 40 条历史、消息 IDs、模型、投递来源/完成 IDs 与 pending 字段一致，主控/成员双语角色搜索通过，进程 exit 0。没有对历史源目录发起恢复或新模型请求。

## 优化主程序实际窗口

为避免干扰用户同名安装实例，Computer Use 使用独立 QA 标识的外壳，主程序字节与本包相同，Resources 指向本包，移除 Node/runtime 覆盖；数据和 Pi 目录仍隔离。

实际冷开、原草稿、完整英文固定入口、English / 20 px、三项聊天设置、工具模式切换/固定保存反馈、成功工具分组和失败独立通过。Cmd+Q 后原生 exit 0，包内 Node 子进程清空。测试外壳标识与正式 Info.plist 不同，此处不作为正式安装或系统通知权限验证。

## 交付与存储

本地 ZIP 为 `.artifacts/hive-desktop-macos-arm64-current.zip` / `candidate.zip`，两者同字节硬链接。保留 `26ebdea` 原 ZIP、可运行解压包与全部证据；清理本批已核对且无占用的重复组装 base 459,055,472 逻辑字节。持续开发仍需要的 Rust 缓存予以保留。

包内收据与清单在 `.artifacts/release-f9f3c2b/`，原生界面、错误路径、草稿与进程证据在 `.artifacts/product-completeness-20261114/`。未签名/未公证，Windows 原生、账号/真实四后端组合、通知授权及完整功能体验仍按 [完整产品计划](../product-completeness.md) 继续验收；本批新增厂商推理为 0。
