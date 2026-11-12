# 3a18b94 开发包：Pi 诊断归属与工作流回收

2026-11-12。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已更新到 `3a18b9421b41f22f479052e4a1cf31710cba6e5c`，包含[Pi 扩展诊断归属](2026-11-12-pi-extension-diagnostics.md)与[工作流取消回收](2026-11-12-workflow-cancel-teardown.md)。这是未签名、未公证的 Mac ARM64 开发包，完整产品目标仍未验收。

## 部署与完整性

重新执行 `pnpm --filter @hive-desktop/runtime deploy --prod --frozen-lockfile --offline --node-linker=hoisted .artifacts/release-3a18b94/deployed-runtime`，使用新 Hive 补丁锁文件，无依赖版本升级。复用已有官方校验的 Node 22.23.3、原生程序和资源，不重复下载/编译。

相对 ff2aa70，28,869 项完整清单中精确只有八项变化：Pi adapter 的 JS/map、Hive runtime-store-workflows.js 和五项部署元数据。原生、Node、其他依赖与资源字节保持。116 个 runtime 文件匹配当前源码 dist，ZIP 独立解压的全部文件/权限及 29 个内部链接匹配组装清单。

## 包内真实流程

三份夹具均实际加载包内 Node/runtime，不替换后台实现：

- Pi 六组：真实 SDK、工作区原生扩展、Hive HTTP/SQLite/spool 和真实 bash 工具。agent_settled / agent_end 诊断继续发出，但成功回复 pending 0、完成 ID 保留；本地文件只追加一次，显式 resume 不重跑工具。context 诊断仍保留且正常结算。扩展命令抛错与 HTTP 400 均 pending 1、原错误/输入保留。真实 bash exit 7 保留工具错误，后续成功回复正常结算。六会话的 JSON 历史、队列、失败和完成 ID 在完整冷开后保持。本组自有 loopback Responses 接口请求 7 次；没有厂商推理。
- 实际 PTY 两组：自有 Node 进程确实收到工作流任务后，经手动停止和 3000ms 超时停止，进程 ESRCH、临时成员删除、取消状态/原因与冷恢复记录正确。常驻内置 Pi 成员的普通 team cancel 提示仍进入队列，完整冷读后原 item 保持，常驻成员保留。这两组无模型请求。
- 结构化七运行：纯计算/手动停止、父子超时、正常结果、真实失败、受控 Codex 协议 Worker 超时；精确原因、派单取消、进程退出、冷 pending 0、七条完整记录保持。工作流取消转发到已退出成员的错误不再出现；没有过滤其他错误。受控协议进程不冒充厂商模型，无模型请求。

四项既有包检查实际 exit 0：`check-package.mjs`、`check-packaged-codex.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`，均使用 `--bundle .artifacts/release-3a18b94/extracted/bundle`。

另以包内正式 main.js JSONL 冷读六个 Pi 会话和九个工作流，保存未重新序列化的 stdout 帧。原生相同版本 serde_json 1.0.154 实际读取 15 帧，并断言四会话已结算/完成回执、两个会话仍失败 pending 1、九条工作流均为终态。没有 resume 这些冷读会话或新增接口请求。原生代码未变，本轮没有重复打开原生窗口，也没有据此宣称新增 UI/Windows 验收。

## 安装与交付

DMG 创建/校验及只读挂载通过，完整应用清单和 Applications 链接匹配，已卸载。更新 Applications 前验证旧清单与无打开文件；分阶段复制/替换后安装清单一致，安装副本本批未另启动。

ZIP 143,935,854 字节，SHA256 `dab28da0e09ccda0f09a593a25d84f298e7c4d813e4c7cd4f1ae77b71e177905`，current/candidate 同字节硬链接。

DMG 239,337,808 字节，SHA256 `e88d1c5b5f5a51bbccd4e85de444bb33c6805373a2165d33e8480aca84db0400`。

完整差异、部署/归档清单、原始夹具与实际 gate 退出码、JSONL/Rust、DMG/安装/发布收据位于 `.artifacts/release-3a18b94/`。

## 边界

源码类型检查/构建、真实输入/命令/失败重试/持久化/工作流关闭与团队回归见两份源修复记录。本批没有新增、修改或删除正式测试，没有跑全量测试：改动为一个 Pi 错误归属条件和删除一次工作流临时成员重复转发，采用相应真实工具/文件副作用/HTTP/SQLite/进程/冷恢复及包级检查验证。

使用的是本地受控 Responses 接口，不能称为真实 Terra 厂商推理；Pi OAuth 继续按用户要求搁置，真实推理仍仅 Terra。既有错误队列和旧历史不自动改写成功，用户可核对原输入。Claude 原生模型、通知系统授权、Windows 真机及完整产品 UIUX/正式发行验收仍开放。

## 存储整理

完整清单、回退 ZIP 哈希和无打开文件检查后，移除重复 base、旧安装临时备份和独立 deployed-runtime，合计 1,225,373,263 逻辑字节。当前解压包、ff2aa70 可运行回退及 ZIP、原始状态/日志/收据、源码补丁和锁文件保留；没有清理共享 Rust/pnpm 缓存或用户项目。部署树已经删除，需要时按上述冻结命令重新部署；具体清单见 cleanup.json / cleanup-deployed.json。
