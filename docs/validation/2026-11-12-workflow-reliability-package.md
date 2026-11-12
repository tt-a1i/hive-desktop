# ff2aa70 开发包：工作流原因与完成通知

2026-11-12。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已更新到 `ff2aa7084b59a25d537131c9ca1137e54c37fa89`，纳入[超时原因](2026-11-12-workflow-timeout-reason.md)与[完成通知 Unicode](2026-11-12-workflow-notice-unicode.md)修复。未签名、未公证的 Mac ARM64 开发包，完整产品验收仍开放。

## 部署与交付物

- 重新执行 `pnpm --filter @hive-desktop/runtime deploy --prod --frozen-lockfile --offline --node-linker=hoisted .artifacts/release-ff2aa70/deployed-runtime`，不沿用旧 patch hash 的依赖树。无依赖版本升级。
- 复用已校验的 Node 22.23.3 和原生程序，不重复下载或编译。相对 a744821 完整清单精确只有七项变化：workflow-runner.js、runtime-store-workflows.js 和五项部署元数据。原生程序、Node、其他依赖及资源逐字节保持。
- 116 个 runtime 文件匹配当前源码 dist；独立 ZIP 解压后 28,869 条文件/权限清单与 29 个内部链接匹配。
- ZIP 143,935,787 字节，SHA256 `4eeb2f76ace36f931a75ab305d6e1086f76212bbb046c70491ae121f0f083900`；current/candidate 为同字节硬链接。
- DMG 233,378,035 字节，SHA256 `de98fa371e28b48b47035ba819750cda93bebda7879c635f6f3b907b1549596b`。创建/校验通过，只读挂载后完整应用清单和 Applications 链接匹配，已卸载。
- Applications 替换前核对旧清单和无打开文件，分阶段复制/替换后核对完整清单。本批未另打开原生窗口；没有把旧版启动证据记成本批的 UI 验收。

## 包内实证

包内 Node/runtime 实际运行两份与源码同断言的自有夹具，未替换后台实现。使用真实 Hive HTTP/SQLite、脚本 Worker、Pi SDK 会话；主控队列暂停。Codex Worker 为受控原生协议进程，报告由驱动提交，未冒称模型推理。自有 loopback 模型端点实际请求数 0。

- 七条运行覆盖纯计算超时、手动停止、父超时/子运行、返回结果 7、真实失败及等待 Worker 超时。250/500/2000ms 原因正确，父子/派单取消原因一致，手动停止保持用户停止；真实 Worker 任务已进入对端后触发时限。Worker 子进程退出，冷开 pending 0，七条完整记录保持。
- 四条实际持久化主控通知的 error 行分别与运行原因相等；每个样例对应一条通知，不扩大为所有并发通知路径恰好一次保证。
- 五条 Unicode 流程覆盖长字符串、长 JSON、未超限 emoji 结果、汇报边界及短 emoji 汇报。生产队列中的通知全部 well formed，摘要、剩余码点数、转义及未超限原文精确匹配。完整运行结果/派单正文、重启后的记录和持久化通知原文保持，受控 Worker 进程退出。
- 完整重开后用包内正式 main.js JSONL 读取上述 12 条运行，保存未重新打包的 stdout 帧；原生相同版本 serde_json 1.0.154 实际解析全部帧并断言时限原因与 Unicode 结果存在。不是只用 Node JSON.parse 验证自己。
- 四项既有包检查实际 exit 0：`check-package.mjs`、`check-packaged-codex.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`，均 `--bundle .artifacts/release-ff2aa70/extracted/bundle`。真实 PTY/HTTP/SQLite 和合成对端的范围沿原检查，不等同厂商推理。

原始日志、实际退出码、source/deploy/ZIP 清单与差异、两组包内夹具、生产 JSONL/Rust、DMG/安装/发布收据保存于 `.artifacts/release-ff2aa70/`。

## 边界

源码已完成类型检查/构建、离线冻结安装和相关现有回归（见两份修复记录）。本批未跑全量测试：仅两个已有工作流分支的原因传递与文本生成改动，以真实工作流、派单、队列、进程清理、完整冷恢复和包级回归验证。未新增、修改或删除正式测试。

本批没有面板操作或新模型调用；原生二进制保持且后台实际响应通过同版 Rust 解析，不据此新增原生交互/Windows 声明。既有取消转发到已退出成员的 PtyInactiveError 日志在包内仍可见，未隐藏或扩大修改；账本取消、进程退出和 pending 0 均有断言。旧错误原因和通知原文不重写。Pi OAuth 继续搁置，真实推理仍仅 Terra；Claude 模型、系统提醒授权、Windows 真机与完整体验验收仍开放。

## 存储整理

核对完整清单、回退 ZIP 哈希并确认无打开文件后，删除重复 base、临时旧安装备份和独立 deployed-runtime，合计 1,225,373,268 逻辑字节。当前解压包、a744821 可运行回退/ZIP、全部验证状态/日志/收据、源补丁和锁文件保留；没有删除共享 Rust/pnpm 缓存或用户项目。部署树已清理，需要时按上面的冻结命令重建；清单见 cleanup.json 和 cleanup-deployed.json。
