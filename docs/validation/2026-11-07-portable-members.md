# 原生成员管理与便携包验证

2026-11-07，macOS arm64。产物是未签名、未公证的开发应用包，不是正式安装包。Windows 的构建、解压检查与真机交互分别记录。

## 打包及运行

`scripts/package-desktop.mjs` 使用 pnpm 10.30.3 的冻结部署锁文件及官方 Node 22.23.3。Node macOS arm64 archive SHA256 为 `23b25245dcfb9af7262f8ff142e9e2e0af025368117329e7a7458a51e5922f53`；验证来源是官方分发的 SHASUMS256。生产依赖及资源完整部署，Hive 补丁存在，没有指向包外的依赖链接。最早的 legacy deploy 尝试重新解析依赖，因此没有作为冻结产物；随后启用 injectWorkspacePackages，锁文件只增加对应设置，没有升级依赖。

`.artifacts/portable-integrated-darwin-arm64/Hive Desktop.app` 实际启动 GPUI 窗口，其直接子进程使用 `Contents/Resources/node/bin/node` 与 `Contents/Resources/runtime/dist/main.js`，没有 Node/runtime 环境覆盖。验证包只通过 LSEnvironment 指定独立数据目录与已安装的 Pi/Codex 可执行路径；因此不宣称已验证 GUI 启动环境的自动 CLI 发现。

`scripts/check-package.mjs` 在仓库外的临时 cwd 使用包内 Node，穿透真实 PTY shell、Hive HTTP/SQLite 工作区创建和关闭，不发起模型请求。runtime-location 的 4 个检查覆盖 macOS/Windows 目录布局、缺损包拒绝回退到源码，以及显式开发覆盖。路径检查不等同于另一个平台的启动验证。

## 原生界面操作

通过原生 CUA 操作应用，在独立工作区实际执行：添加项目 → 创建停止状态的外部 Pi 成员 → 明确指定 `openai/gpt-6.1-terra` → 启动 → 聊天得到精确 `UI_EXTERNAL_PI_OK` → 改名 → 停止。界面显示最终已停止状态及启动按钮。关闭并重开应用后，改名后的成员名称同时出现在侧栏和聊天标题，原有消息与模型保持。

证据 `.artifacts/desktop-member-ui/receipt.json` 和该目录下的原生 transcript，逐条核验所有 assistant 都是原生 OpenAI Terra，没有 Magpie。验证中保留成员与会话；没有通过界面执行最终删除，也未声称删除流程已真机验收。

走查修复了两处体验问题：长成员表单从被遮挡的侧栏移入可滚动主区域；成员改名后侧栏和聊天标题改用 Hive 当前成员名称。成员卡使用可读的角色、状态和后端名称。

## 独立复核与剩余范围

- 会话/RPC 审查：原跨 delivery/分支误确认 P1 保持修复；独立重跑真实 RPC 完成与会话检查，无新增阻断项。
- 共享运行时/成员审查：修复 HTTP 202 后过早刷新停止状态及旧响应覆盖新列表两项 P2；复核通过，成员范围评分 B。
- 证据/测试质量审查：明确构造历史、真实模型取消与进程后代清理的边界；新增逐条 Terra 断言并对保留收据核验；更新过时的桥接阶段状态。
- 打包审查：没有具体阻断缺陷；Windows zip 解压后检查已加入 CI。仍不能据此声明 Windows 原生窗口、外部 CLI 或正式发行完成。

实际通过 runtime typecheck/build、现有 check:team/check:codex/check:runtime、外部 Pi 定向 RPC/生命周期/真实团队检查、Rust 1.98 build、stable fmt、4 个路径检查和包内 smoke。未跑无差别全量测试；本批验证集中于新增后端、成员 UI 和打包边界。未删除已有测试。

完整产品目标仍开放：Claude Code、完整配置、协作工作台、后台重连、Windows 真机交互与签名安装包尚未完成。
