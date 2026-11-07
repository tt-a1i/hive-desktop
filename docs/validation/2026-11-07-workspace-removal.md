# 工作区移除验证

2026-11-07，基线 `39b1929` 后的工作树。本记录只覆盖工作区移除，不代表完整桌面产品验收。

## 真实问题

1. 用户在桌面添加项目后，无法移除已经结束或误加的工作区，只能额外打开 Hive Web；旧项目持续占据导航。
2. 桌面现在提供原生确认框，显示项目名、路径和删除范围。确认后停止该工作区的会话并删除 Hive 记录，刷新项目列表；取消保留原现场，失败说明当前状态并保留可恢复信息。
3. 不做这项，桌面的项目管理无法形成完整闭环，也不能独立完成 Web 已支持的日常操作。

## 实现和删除范围

桌面使用 `workspace.remove` JSONL 请求；成功返回 `ok:true` 和完整的已移除会话 IDs。可预期的失败返回 `ok:false`、错误码、阶段，以及工作区是否已经删除的三值状态。内部 API 仍抛出类型化异常，其他 RPC 错误契约保持原样。

操作依次持有真实 workflow 空闲租约、排空目标工作区的模型目录请求、等待已有会话打开、停止绑定及独立会话，再调用既有 Hive DELETE，最后原子更新桌面会话索引。该工作区暂时禁止新会话、模型目录、交互回应和相关写入；其他工作区继续可用。工作流仍在运行或清理时直接说明原因，不自动强制停止工作流。

固定 Hive 2.2.1 的原有 pnpm 补丁新增一个窄的空闲租约接口，跟踪脚本加载、排队执行、finally 清理，以及真实 Worker exit 和已接受的 host calls。逻辑失败及时 reject，避免等待 host calls 后才取消它们的死锁。租约期间所有 runner 启动入口拒绝新工作流。数据库 `stopped/completed/failed/interrupted` 仅是历史状态，不用作物理退出证明。原执行器装配补丁与 Windows PTY 补丁保留；通过 `pnpm patch/patch-commit` 更新并锁定散列，没有修改另一份 Hive checkout。

- 删除 Hive 工作区、团队、派单、消息、记忆、工作流记录和上传文件，以及桌面导航索引。
- 保留项目文件夹、`.hive/tasks.md`、外部 Agent 原生历史和物理 delivery spool。保留的 spool 不再出现在已删除的桌面会话导航中。
- 不修改 Agent 全局默认、登录凭据或原生配置；本切片没有模型请求，也没有使用 Magpie。
- Hive DELETE 的附带清理与桌面索引不构成跨系统事务。数据库删除后索引失败明确报告部分完成；DELETE 报错时重新读取权威工作区列表，不能确认时不伪称回滚。
- workflow 租约只证明脚本及其 host-call 生命周期；Agent 子进程另由桌面后续 stop/wait 处理。Shell 仍由既有 Hive DELETE 管理，本切片没有额外证明 Shell 的强退出保证。

## 实际验证

`pnpm --filter @hive-desktop/runtime check:workspace-removal` 包含两个不同层次的检查，均单独执行通过：

1. 真实 Hive HTTP/SQLite、真实工作流 Worker、原始 Pi 1.4.0 进程和实际 sidecar JSONL。运行中的工作流阻止移除；HTTP stop 后曾观察到一次数据库已 stopped 而租约仍拒绝，实际清理后允许。租约覆盖实际移除等待模型目录的阶段，team HTTP 新启动返回 409。历史终态行由夹具插入，用于证明历史状态不再误挡。
2. 真实 Worker dispatcher 配合合成 Worker 消息和可控 host-call gate。逻辑失败已返回、真实 Worker 已退出，但 host call 未结束时所有权回调仍为零；放开 gate 后恰好释放一次。这是确定性的模块级回归，不把短暂 stopped 窗口的观察次数当成稳定断言，也不声称此检查同时穿透 HTTP 和 runner 租约。

第一组另覆盖绑定与独立 Pi PID 均退出、无关工作区继续打开、项目与原生 transcript 字节保持、spool 保留、SQLite trigger 回滚、索引文件故障导致部分完成、目录进程排空及初始化失败、删除中的写入拒绝、实际 JSONL 业务失败结构。Pi 仅执行初始化和目录控制；测试选择 `openai/gpt-6.1-terra`，未发送推理。

收据复制到 `.artifacts/workspace-removal-evidence/`。原始通过目录为临时目录下的 `hive-workspace-removal-5y5EHD` 和 `hive-workflow-worker-gate-LrFvbR`。三个失败预检分别是夹具缺少 workflow meta description、工作流完成通知合理追加 spool 后旧断言失效、同步错误误用异步断言；清单保留这些失败，没有改生产语义来迎合夹具。

已执行且通过：`pnpm check`、`pnpm build:runtime`、Rust fmt/check/build，以及既有 `check:hive`、`check:team`、`check:runtime`。后面三项日志在 `.artifacts/workspace-removal-regression/`，使用 Node 22.23.3，不进行模型推理。删除 guard 的早期实现影响过期交互错误类型，已经恢复原 guard 顺序并重跑 Codex 和 Claude 交互检查通过。

没有删除旧测试，没有引入仅供检查的生产 fallback。按风险运行现有 Hive/队列/团队回归及新增真实生命周期检查，未无差别重复整个项目的模型或平台测试集。

## macOS 窗口与打包

本次重新执行 frozen production deploy，产物位于 `.artifacts/workspace-removal-ui-proof/Hive Desktop.app`，没有复用旧 Hive 依赖。`check-package` 通过包内 Node、真实 PTY、Hive agent-manager 自然退出与主动停止、HTTP/SQLite 和后台关闭。随后仅为窗口问题更新原生二进制，依赖保持同一次冻结部署；各轮二进制与源码散列在 `fixture/` 下记录。

隔离 A/B/C 项目和 Pi agent 目录，没有凭据、推理或全局配置改动。通过实际原生窗口验证：

- 确认框完整显示项目名、路径及删除/保留范围。Return 没有执行删除，Escape 取消后 A 的中文草稿仍在。
- SQLite BEFORE DELETE 故障导致真实删除失败；A 和草稿保留，提示明确说明部分会话可能已停止。
- 清除故障后移除 A，自动转到 B，B 的未发送中文草稿完整保留。另从数据库与索引确认 A 已移除；原项目 marker 与原生历史可读。窗口夹具仅记录这些文件的当前散列，逐字节保留证明来自前面的集成检查。
- 约 760×583 逻辑尺寸下，发现长错误提示溢出。共享提示组件已修复换行、限高滚动和关闭按钮布局；再次注入 C 的删除故障，正文完整可读且关闭按钮可点击。
- 移除最后一个工作区后，项目/会话列表清空，新会话、成员与发送入口禁用，输入无旧草稿。重新添加 A 的目录后，旧会话未复活。
- 同一路径重新添加曾残留“工作区已移除”状态。共享选择/列表读取入口已更新就绪状态，并在最终二进制中实际重做移除与添加确认修复。三轮应用退出均返回零，三个包内 Node PID 已消失。

窗口收据为 `.artifacts/workspace-removal-ui-proof/receipt.json`。C 在窗口中没有活跃工作流；真实工作流阻断由独立 HTTP/Worker 检查证明。草稿仍仅保存在应用进程内，不声明跨应用重启保留。此次也未通过 GUI 人为延迟触发所有异步交错。

## 边界与任务 verdict

| 项目 | 结果 |
| --- | --- |
| 生命周期、删除、部分失败协议 | 已实现，真实 HTTP/SQLite/进程与 JSONL 检查通过 |
| workflow 物理清理与移除租约 | 已实现；真实租约/HTTP 与确定性 Worker gate 分别验证 |
| 原生确认、草稿清理、失败后保留 | 已实现，Mac 原生窗口及源码独立复核通过 |
| 项目、原生历史与 spool 保留 | 真实文件断言通过；不推断所有外部 Agent 都不写自己的历史 |
| Worker 原生 error 事件、dispose 自身失败、DB 提交后的 Hive 清理失败 | 未分别注入验证；不把逻辑 done-failure、初始化失败或 DB trigger 当作等价证据 |
| Windows | 本次变更尚未实跑；先前 `39b1929` CI 成功不覆盖此次补丁，原生窗口仍未验收 |

## Self-Review

四个独立角度并行审查，并对修复后二次复核：

- A 架构：C+ → A-。严重项是把 stopped 当作清理完成；已修为真实 activity 与 Worker ownership，并在最外层持租约。
- B 真实边界：B → B+。确认原清理间隙已修，数据库后失败不伪造回滚；窗口发现的长错误提示和重新添加后的旧状态问题均在共享入口修复，并独立复核与原生复测。
- C 验证质量：B → B+ → A-。补齐独立 Pi、实际 JSONL、写入 guard 和 team HTTP 409；再补确定性 host-call gate，防止仅记录 stopped 时间窗口而挡不住原回归。
- D 需求与契约：B- → B+。历史 interrupted 永久阻断已修；删除与保留范围、部分失败文案对齐。

严重项均已修复，无 reviewer 结论冲突。综合 B+；以上评分不表示完整产品或 Windows 原生窗口完成。
