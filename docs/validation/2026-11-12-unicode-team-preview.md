# 团队任务预览的Unicode截断

2026-11-12，基于db13785。沿原生实际读取字段追踪同类问题，固定Hive2.2.1的routes-workspaces.js在UI/CLI共享团队序列化中用dispatch.text.slice(0,60)生成open_dispatches.task_preview。59个x之后接🐝会留下半个代理项；即使Rust成员结构忽略该字段，transport首先解析整个serde_json::Value，因此仍会拒绝整个响应。

## 修复与依赖边界

通过既有pnpm patch/patch-commit机制，在@tt-a1i/hive@2.2.1补丁新增一个单行修正：Array.from(dispatch.text).slice(0,60).join('')。沿用原字段/60字符预览/顺序/状态/权限/完整任务存储，不改协作协议或另建账本。未修改本地Hive权威仓库。

版本继续2.2.1，patch hash由7c02ce83275d11a54b1fba8504e8006000ef989c8a05b07edd75c116a2bc7bd5更新为28564fe99a7532693ce5bfd313172e50fab2ab9978da6de17203f4952c813c2c。pnpm-lock.yaml仅三处相应哈希变化，没有依赖版本更新；pnpm-workspace.yaml保持。pnpm install --frozen-lockfile --offline通过。

比较新源依赖与前开发包中的Hive自身1,177个发布文件，唯一变化为该路由。初次把完整部署树与源码安装树比较时，也包含嵌套marked/.bin的部署差异；修正为明确排除嵌套node_modules，不能拿这份自身文件对照冒称传递依赖已全部核对。下一开发包必须按新补丁重新部署生产依赖，不得继续套用要求锁文件不变的旧code-only组装路径。

## 实际证据

`.artifacts/unicode-team-preview-20261112/check.mjs`通过真实runtime创建隔离工作区和停止成员，关闭后仅在自有SQLite准备一条queued派单；这是序列化夹具，不冒称模型派单。之后启动正式dist/main.js子进程，通过真实JSONL请求initialize和team.members，穿透authenticated Hive HTTP/SQLite，保存未经重新封装的原始stdout响应行。

- 修复前真实task_preview为59个x加\ud83d，isWellFormed=false；桌面同版serde_json1.0.154的独立Rust程序读取该实际响应，报unexpected end of hex escape。
- 修复后相同工作区/成员/派单响应精确为59个x加完整🐝，60 code points；Rust实际解析并逐字/字符数断言通过。
- 两次真实后台正常shutdown exit0；DB完整任务文本精确保持，agent_runs=0，无模型/面板或账号操作。

pnpm check、pnpm build:runtime、git diff --check通过；四个现有定向检查均exit0：member-creation.ts、structured-runtime.ts、team-tool.ts、team-runtime.ts（命令前缀pnpm --filter @hive-desktop/runtime exec tsx checks/）。包含真实创建回滚、HTTP/SQLite/CLI、派单/seen/report与状态恢复；无厂商模型请求。未增改删正式测试，独立断言式程序、Rust源码、前后真实JSONL、补丁编辑副本/提交/冻结安装日志和receipt保留在上述目录。

当前db12698开发包尚未含这份Hive补丁。未跑无关全量suite；其他CLI错误/恢复摘要/webhook/文件名中的截断命中没有据此扩大为已验证，后续需按拥有字段和实际消费者分别追踪。Windows原生、当前包更新、真实账号及完整产品目标继续开放。
