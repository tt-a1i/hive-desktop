# 后端英文名称搜索与账号验收边界

2026-11-12，基于790c70c。生产改动仅`packages/runtime/src/chats.ts`的一行后端显示名称搜索词，补Built-in Pi与External Pi；保留中文名称、内部后端ID、AND匹配、分页及排序，不改变会话或协作行为。

## 可复现的搜索缺口

英文界面的外部Pi显示为External Pi，设置里的内置Pi为Built-in Pi，但后台搜索词只有“外部 Pi”“内置 Pi”及内部ID。实际旧包接口搜索这两个英文名称均为0结果，中文名称和其他两后端正常。

`.artifacts/backend-search-labels-20261112/check.mjs`分别加载3a3f873包和新源码的生产runtime。独立临时数据，真实Hive HTTP/SQLite和四种后端的持久空会话记录，项目/标题均不包含待测名称；未初始化Agent。

| 查询 | 旧包结果数 | 修复后结果数/后端 |
| --- | --- | --- |
| External Pi | 0 | 1 / pi-rpc |
| Built-in Pi | 0 | 1 / pi |
| 外部 Pi | 1 | 1 / pi-rpc |
| 内置 Pi | 1 | 1 / pi |
| Claude Code | 1 | 1 / claude |
| Codex | 1 | 1 / codex |

前后完整响应摘要分别存before-receipt/after-receipt，修复后检查要求每项只命中预期后端。独立只读SQLite确认messages、dispatches及agent_runs均为0。不是mock fetch、自测mock调用或源码字符串断言。

通过`pnpm check`、`pnpm build:runtime`、`pnpm --filter @hive-desktop/runtime exec tsx checks/session-search.ts`及`git diff --check`。现有回归覆盖跨项目/名称/后端、重开、分页限制与不初始化Agent。没有正式测试改动，没有UI变化或搜索面板巡检；未跑无关全量suite。新搜索词尚未入当前3a3f873安装包。

## Pi与Claude边界的当前事实

只读原生Pi认证元数据：OpenAI记录为OAuth，支持订阅登录，没有自定义OpenAI接口覆盖；auth.json修改时间2026-11-10，不能沿用此前文件时间推断可用性。因此仅做一次独立的openai/gpt-6.1-terra、medium请求，要求固定短答且禁止工具。SDK在原生OAuth刷新返回invalid_grant，未取得成功模型答复，toolCalls=0；失败时pending=1，显式abort后持久spool为空，拥有runtime已关闭。未换Magpie、其他模型或主动修改全局认证/模型配置。

元数据、原生失败、事件类型及清理收据保存在`.artifacts/pi-auth-recheck-20261112`。未复制或输出凭据正文。用户本轮明确“暂时先推进其他工作”，并再次确认“继续只用Terra”。Pi真实推理等重新登录后再验；Claude原生模型推理仍不在当前测试授权内，不重复尝试或改路由绕过，也不把已有原生控制/协议矩阵当成真实推理完成。

## 已排除的候选问题

检查项目移除调用时，起初只看到菜单接收项目ID、移除函数取当前项目，误判为可触发的错目标删除。追到实际render条件后确认菜单只显示在当前选中项目，该路径不可达。候选改动已全部撤回，没有删除工作区，没有制造或提交新的移除功能。debug二进制在源码恢复后重新构建；仅移除无占用的自有原型App壳，测试状态/日志保留在`.artifacts/project-removal-target-20261112`。该原型的构建/观察不当作生产修复证据。
