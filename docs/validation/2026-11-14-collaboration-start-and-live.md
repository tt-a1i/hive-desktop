# 协作起步、窄窗任务编辑与真实 Codex 闭环

2026-11-14，工作项 [#31](https://github.com/tt-a1i/hive-desktop/issues/31)，完整目标继续。

## 改动

首次没有派单时，协作概览提供成员管理、已有主控会话及选择后端新建主控的实际入口，并说明派单/显式汇报流程；筛选或后续空页提供查看全部派单。只复用既有成员、会话和筛选操作，不自动添加成员、派单或请求模型。

真实窗口发现窄窗固定任务区把主要操作挤出首屏，现窄窗概览将任务文档放入派单列末尾，共用一个滚动区域；宽窗保留双栏，选中详情时保留详情视图。任务控件的焦点显露绑定实际滚动容器。另复现并修复点击编辑后输入框仍在屏幕外的问题，输入框也接入 `Reveal`。

## 原生操作与独立核对

使用独立 `Hive Product QA.app` 身份区分用户正在运行的安装实例，实际 GPUI 程序、生产编译的 Node runtime 与原始 Codex CLI；`ui-preview` 仅用于打开指定页面，没有注入聊天、派单或报告历史。

1. 从真实空工作区的协作页进入成员管理，创建恰好一位 `flow-worker`，选择 Codex 与 `openai/gpt-6.1-terra`，启动后待命。再从协作页选择 Codex 主控、Terra、中等推理；独立 SQLite/索引确认此时无投递或派单。
2. 保留预置两行草稿，在另一主控会话从原生输入框发送任务。主控使用注册的 Hive `team` 工具向现有成员派一单，成员实际创建并读回 `ui-team-proof.txt`，显式调用 `team report --dispatch … --seen 0 --stdin`。
3. 主控收到对应报告后执行独立只读 Python 断言，输出 `UI_BYTE_ASSERTIONS_OK`，唯一最终答复包含 `HIVE_UI_TEAM_COMPLETE`。原生窗口读到主控的实际命令/输出、已汇报派单和报告原文，详情 → 成员会话 → 返回详情 → 列表通过。
4. 独立读取原生工具记录与 Hive SQLite，确认正确成员/来源主控、显式原生汇报、`reported` 状态、空待投递 outbox，以及精确 24 字节 `HIVE_UI_TEAM_20261114_OK`。文件 SHA-256 为 `aee88d6a83779ad3d82abe42a88d29f0ee9b8a265b6dd44a3a5420e436c41f92`。
5. 完整关闭后，两次独立创建 runtime 并冷读三个会话，原线程、全部历史及消息 ID、完成投递 ID、模型与台账完全一致，`busy=false`、`pending=0`。第三个未发送会话仍无历史，原两行中文/emoji 草稿逐字保留。无拥有的进程继续占用测试项目。

此轮是一个 Codex 主控与一个已存在 Codex 成员的真实闭环，不推广为新的两成员并发或混合后端验收。报告正文列出了文件，但结构化 `artifacts` 数组为空；没有据此验收结构化产物附件。成员早期 report 工具未在当前虚拟列表视口中直接展开，显式调用由独立原生工具记录核对。

## 窗口、筛选和编辑

在隔离状态副本增加一个只启动、未推理的原生主控，实际验证“继续主控会话”路由。中文/英文、Dark、20px、约 822×503 窗口中三个主要操作均在首屏完整可见；键盘菜单、Queued 空结果 → View all dispatches → All/Page 1/唯一 Reported，以及真实详情往返通过。

任务草稿在约 822 → 1200 → 822 窗口切换中逐字保持；宽窗在右栏，窄窗在同一滚动列。Tab 能显露保存/取消，取消不保存团队文档。最终修复后点击编辑会自动显露整个输入框，直接 ASCII 键盘输入通过，中文/emoji 的系统粘贴精确保持。Computer Use 的 `typeText` 中文注入本次未成功，因此不宣称中文直接键盘、输入法或辅助技术完整验收。

四次原生运行正常退出 0，后台子进程清空；源记录在 `.artifacts/product-completeness-20261114/`：

- `collaboration-ui-location.json`、`collaboration-routing-location.json`：原状态及隔离副本位置。
- `collaboration-start-process.json`、`collaboration-routing-process.json`、`collaboration-layout-process.json`、`collaboration-focus-process.json`：实际退出收据。
- `collaboration-live/{receipt,ledger,snapshots,native-report,independent-lead-assertion}.json`：真实文件、工具、报告、完整冷恢复证据。

## 账号、平台与包边界

本轮 Pi 可用性检查一次即返回原生 `invalid_grant`，未有模型响应，工具数 0，失败输入保留为待处理。不重复重试或登录，不绕过认证，也不把其视为新 Pi 协作通过。Claude/Antigravity 按用户要求本轮不验收。

Mac 开发态构建和当前 MSVC 交叉检查通过。Windows 流水线及包状态分别记录，不把上述 Mac 操作当成 Windows 或正式安装验收。Windows 陈旧检查/可移植性修复见[专项记录](2026-11-14-windows-check-portability.md)。新开发包另列，当前正在运行的用户安装实例未替换。
