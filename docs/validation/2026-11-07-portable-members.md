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

## 附：协作概览与分页

原生协作概览使用已安装 Hive 2.2.1 的任务、派单、消息接口，不新增协作账本。任务以 Markdown 展示且只读；派单支持状态筛选和每页 20 条；详情保留原始任务、汇报/取消原因和产物，消息按 `after_seq` 每次追加最多 100 条。派单“已汇报”与消息“已投递”分别显示，不推断主控已确认报告。

实际 CUA 打开 `.artifacts/external-pi-team-live-Zzqy1d/state` 的真实双 Agent 记录，验证成员名、已汇报派单及原文详情、切换 queued 后的空态。成员模型选择的独立验证见 [设置记录](2026-11-07-desktop-settings.md)。

另用 `.artifacts/collaboration-pagination-sJy07u/receipt.json` 的隔离合成数据验证边界：25 条派单先显示 01–20，下一页显示 21–25，上一页返回；首条派单消息先显示到 100，加载更多后显示到 105，按钮消失。任务 Markdown 的标题、勾选列表和代码块实际渲染。工作区/成员/任务通过真实 HTTP 建立，派单和消息通过 Hive 自身 SQLite store 填充并以 HTTP 回读；这是 UI 分页夹具，不代表额外真实 Agent 协作或模型调用。

滚动诊断记录 `.artifacts/collaboration-pagination-proof/scroll.log`：视口 736.5px，内容最大偏移 515.5px；CUA `down` 发出正 delta 并在顶部被 clamp，`up` 发出负 delta 后实际向下滚动。未改变生产滚轮方向，临时日志/监听已移除，未经验证的侧栏和成员布局调整已撤销。保留的协作视口结构经过实际滚动验证；没有证据称原结构必然有产品 bug。

独立源码审查未发现阻断项，范围评分 B；审核了安装版 HTTP 契约、状态区分、分页游标和 workspace/generation 防旧响应。Rust build/fmt 与 runtime typecheck/build 通过。此片不含任务编辑、报告确认操作、关联派单树汇总；Windows 原生窗口仍未验证。
