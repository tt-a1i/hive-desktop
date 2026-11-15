# 新工作树的显式阻塞初始化

源码基线：`b8a1f936e8e8fa426ad08d14241f59601db67ab3`。本片仅实现 setup；不增加 settle 触发，不改厂商回合、Worker 汇报、任务验收或团队账本语义。

## 行为

项目设置可以将一个已保存脚本指定为新工作树初始化脚本，默认关闭。设置格式从 v1 兼容读取到 v2；旧脚本不会自动启用。新建对话框显示已保存名称、版本与阻塞说明，可显式跳过本次初始化；创建时校验所显示的设置 revision，设置变化须刷新。根目录会话和已有工作树不会追溯执行。

新 managed worktree 在 Git checkout 前保存 v3 intent，包含原 Worker/backend/model、启用事实、完整命令快照和唯一 requestId。脚本编辑、删除或角色切换不能更换这个快照。Git 工作树和会话索引发布后，创建进程可以启动这一已授权脚本，但不等待脚本结束来占用工作区锁。脚本仍由唯一 ProjectScriptRuns/Hive PTY 执行器管理；没有第二个进程管理器，也没有接收 command/cwd 的初始化 RPC。

独立 sidecar 保存初始化状态与版本。状态/重试/跳过 RPC 只接受工作区、原会话及操作的 expectedVersion；公开初始化状态隐藏命令和 owner token。初始化 pending/running/failed/cancelled/interrupted/unknown 时，open 只读磁盘历史，后端 require/load/start、prompt/resume、Hive 启动/ready 和实际队列执行均检查屏障。成功或明确跳过后仍须用户显式打开、发送或继续，不自动发送旧输入、恢复派单或启动 Agent。新 Worker A 的创建和 setup 不绑定它，不抢当前 B。

重试只使用原命令，保存新的 requestId 后才启动。跳过活动脚本先停止并等待真实 native exit；无法确认退出或目录身份无效时保持阻塞。关闭应用/删除工作区复用原有 lifecycle gate 和 owned native 停止流程，脚本元数据写盘失败不取消清理进程的责任。

## 已验证

以下均使用临时目录、真实 Git/Hive SQLite/本机 POSIX PTY，无真实模型请求、账号调用或登录：

- `pnpm --filter @hive-desktop/runtime check` 与 `build`：strict TypeScript 通过。
- `checks/project-script-settings.ts`：真实 SQLite CAS、v1 默认禁用迁移、未知 schema/损坏拒绝及冷读。
- `checks/project-script-runs.ts`：现有 root/A/B 原生 PTY、stdout/stderr、非零、signal/cancel、源目录丢失后的进程树停止、close/crash 回归通过。
- `checks/project-script-storage-failures.ts`、`project-script-stop-storage.ts` 与 `project-script-workspace-lifetime.ts`：保存失败/元数据丢失仍停止 owned 静默 native 进程树，SQLite 删除失败及并发 lifetime gate 回归通过。
- `checks/member-worktrees.ts` 与 `git-worktrees.ts`：原成员切换、busy/未结派单保护、工作树身份及恢复回归通过；模型请求数为 0。
- `checks/project-scripts-wire.ts`：现有 JSONL 设置/执行/日志/状态/幂等/停止/退出回归通过。
- `checks/project-worktree-initialization.ts`：A/B 正确 cwd/相对输出；四个后端同时 open/prompt/resume 均阻塞；根目录/disabled 不执行；保存后编辑和删除不替换原命令；原快照重试；非零/signal；skip/delete/close 清理实际静默进程树；owner marker、分支和 v3 intent 损坏不能重试或跳过；Worker A 不抢 B、不增加 Hive messages/dispatches/agent_runs；直接 Worker Chats.start 受屏障保护。
- `checks/project-worktree-initialization-wire.ts`：真实 JSONL 初始化摘要脱敏、只读 open、状态版本冲突、拒绝 caller command/cwd/requestId、已删除脚本的原快照重试、显式 skip、根会话无初始化。
- `checks/project-worktree-initialization-recovery.ts`：真实 checkout 后索引发布失败，保留原 Worker/backend/request/命令；冷恢复和 open 不运行脚本；显式重试可在设置删除后运行原命令；重复发布不另起一次；陈旧设置 revision 在 checkout 前拒绝；删除 live manifest 仍按可信内存 ownership 停止原生脚本后才能 skip。
- `checks/project-worktree-initialization-crash.ts`：独立后台真实 SIGKILL，保留输出及一次执行计数；冷读 interrupted，不重放；存在未知存活旧进程时 retry/skip 不可用并显示原因。测试清理仅使用该 fixture 自己捕获的 PID，产品不会猜 PID 或杀重用 PID。
- macOS Rust `cargo check --offline`、6 个 project_scripts model 测试、6 个 session_execution 测试及变更 Rust 文件的 rustfmt 检查通过；字面中文翻译键闭合检查无缺失。

可用 `pnpm --filter @hive-desktop/runtime check:worktree-setup` 重跑四组初始化检查。

## 限制与后续验收

硬崩溃后的脚本 native ownership 不会被重新认领。若原 runner manifest 是 interrupted/unknown，且没有可信终态，初始化保持阻塞，重试/跳过按钮不可用并说明退出未确认；保留工作树和日志，用户可新建独立会话。Refresh 会重新读取原 run；未来可信终态可以更新初始化，不仅凭曾出现 interrupted 就永久忽略后续证据。缺少 sidecar 但原 request 没有 runner claim 时不自动执行，显示待人工处理的中断；显式重试或跳过可继续。此片不宣称完整 hard-crash 恢复或跨进程全局 exactly-once。

真实 Windows 执行未验收。离线 `cargo check --target x86_64-pc-windows-msvc` 在 vendored gpui-pre build.rs 缺少 `llvm-rc` 处中止，没有安装工具或下载依赖。Mac 原生 GUI、键盘可达性、新包及安装入口仍待独立验收；该片没有进入并发 GUI 验收人员使用的冻结包，不竞争 GUI 会话。Claude/Antigravity 的真实账号与模型不在本轮范围，Pi invalid_grant 未重试。
