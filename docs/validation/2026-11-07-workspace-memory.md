# 工作区记忆原生界面验证

日期：2026-11-07。复用 Hive 2.2.1 的现有记忆端点、SQLite 和注入开关；未增加记忆账本或模型任务。

用户需要查看工作区已有的长期上下文，并控制哪些条目继续参与上下文。桌面现在提供正文/标签搜索、状态与范围筛选、来源详情、注入开关，以及现有固定、禁用、批准、拒绝和归档操作。列表明确提示最多 50 条。安装版 PATCH 仅支持 pinned/disabled；正文编辑没有真实接口，因此未提供编辑按钮。

## 真实接口验证

`pnpm --filter @hive-desktop/runtime exec tsx checks/workspace-memory.ts` 使用临时数据目录、真实 Hive HTTP/SQLite 和合成记忆，完成 47 条请求。覆盖正文/标签/中文搜索、工作区隔离、状态与 scope、设置重开持久化，以及 400/403/404/409 错误路径。没有 mock fetch，没有模型调用，dream_runs 为 0。

实际契约：active 可固定/禁用/归档；candidate 可批准/拒绝；其他状态操作按现有 Hive 错误返回。批准不是 Agent 工具权限审批。注入开关影响后续构建的上下文，不会撤回已进入既有对话的内容。

## 原生窗口验证

冻结 binary SHA256：`818778fd62a10c5022c56356fadc5375da504e6ca8aba658585caad6be8e60be`。候选包 `.artifacts/memory-ui-proof/Hive Desktop.app`，独立 bundle ID `com.hive.desktop.memoryproof`，数据目录 `.artifacts/workspace-memory-gOyFIb/state`。

实际完成中文搜索、展开来源、取消固定、启用条目、批准候选后归档、拒绝另一候选、工作区注入开关及工作区/用户范围筛选。操作后直接读取隔离 SQLite，确认对应状态与标志已持久化；两个工作区的开关分别为 false/true，dream_runs 仍为 0。

第二工作区建立空 Pi 会话后，在记忆页切换进入该工作区仍保留记忆页，没有被会话列表异步响应带回聊天。没有发送聊天消息。760×583 逻辑像素窗口下筛选和来源详情可滚动到达。中文输入通过粘贴完成，不视为 IME 验证。

收据：`.artifacts/workspace-memory-gOyFIb/receipt.json`（接口），`.artifacts/memory-ui-proof/receipt.json`（窗口和最终 SQLite 状态）。

## 独立复核与边界

独立审查发现两个 P2，均已修复并复核：自动打开会话覆盖记忆页；隐藏页面后写入结果被丢弃。现在仅聊天页自动打开首个会话；写入结果按发起工作区保存，返回时显示结果并重新读取数据。没有改变 Hive 的状态权限。

fmt/check/build 和 diff 检查通过。纯 UI 未增加自动化测试；实际接口有上述穿透检查。未人为延迟 HTTP 或注入失败来验证精确切页时序；未完成 Windows 原生界面验证。仅使用本次隔离合成记忆，未修改用户真实记忆。
