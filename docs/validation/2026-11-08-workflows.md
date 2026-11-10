# 工作流桌面入口（2026-11-08，#13）

本批接入真实 Hive 工作流运行、结果、错误、日志、关联派单、停止、已有定时计划控制，以及实验开关和默认/允许执行器配置。仅增加 Rust UI，沿用 `hive.request` 和固定 `@tt-a1i/hive@2.2.1` 的 HTTP、SQLite、Worker 与配置，没有新建脚本库、调度器、业务表或审批。工作流仍由主控编写/启动。

## 已验证

- `cargo check --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、`cargo build --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、`git diff --check` 通过。纯 UI 本批不新增实现镜像测试，也不运行全量模型/协议套件。
- 使用生产编译 runtime、包内 Node 22.23.3、真实 Hive HTTP/SQLite/Worker，运行确定性完成、抛错、等待停止三个脚本。结果对象、阶段、日志、错误及 stopped 均由后台产生，没有 mock HTTP 或合成模型回复。
- 新数据默认实验开关为关闭；显式开启后运行。计划通过真实主控认证 API 创建，暂停/恢复后读回；非法空允许列表返回 400，原配置保留；终态再次 stop 返回 409；关闭功能后历史仍能读取。
- Mac 原生窗口验证全部/运行中筛选、完成详情、参数/校验值/日志、计划暂停/恢复、删除确认出现后取消；通过原生按钮把实际由计划触发的等待 Worker 停为 stopped，并另从真实 SQLite/HTTP 核对 `Stopped by user`。每分钟验收计划已经 HTTP 删除，其余 owned 计划暂停。
- 原生保存默认 codex、允许 codex/pi，刷新不覆盖尚未保存的选择，保存后按钮恢复无改动状态；SQLite 读回及应用重开均保留 codex 默认项。
- 两个真实工作区分别显示对应运行，空项目无记录；走查发现返回有会话项目时自动恢复会话会挤掉工作流页，已在共享会话恢复入口排除工作流视图，重编译后双向切换验证通过。
- 实际 1284 物理像素 / 约 642 逻辑像素窄窗，折叠导航后工作流列表和配置可访问。沿用滚动容器与稳定 ID，轮询沿用现有三秒工作区更新任务。
- 源码读取/操作结果检查 workspace、connection generation 和页面 generation；轮询错误保留旧数据，独立操作错误不会被下一次成功读取清除。丢弃执行器草稿时先清除旧显示，避免重新读取失败后把未保存值显示成已保存。
- 第一、第三、第四轮 owned 原生应用 Cmd+Q 均正常 exit 0；第二轮因更换验证 bundle 标识而用 SIGTERM 结束，仅前三个正常退出记录作为原生退出证据。

[完成详情与日志](assets/workflows/detail.png) · [窄窗口](assets/workflows/narrow.png)

## 证据和校准

本机 `.artifacts/workflows/` 保存 seed/runner、backend-receipt、ui-persistence-receipt、native-stop-receipt、recovery-receipt、日志及截图；不是新的依赖副本。原生用当前 `apps/desktop/target/debug/hive-desktop` 和 `packages/runtime/dist/main.js`，只复用保留的包内 Node，不使用旧包 runtime 冒充新代码。该初轮记录之后，current ZIP 已更新到 c18617d；包内范围见[当前开发包](2026-11-08-current-package.md)。

脚本与 Agent 配置均为 owned 验收数据。仅配置 `openai/gpt-6.1-terra`，提供商固定 `127.0.0.1:9`，没有访问厂商或模型推理；工作流结束通知曾尝试投递到这个不可达回环地址，产生的投递失败不作为模型协作验收。

首次计划中文名称被上游合法性规则拒绝，改用含字母的 owned 名称；未改上游产品语义。首次恢复夹具因移动目录留下旧会话绝对路径而失败，修正夹具索引。第二次停止过早导致尚未产生日志，改为等实际日志可读再关闭，随后完成/失败历史与日志、终态恢复均通过。该同进程双 runtime 恢复脚本完成断言后未及时自然退出，最终 SIGTERM，仅作为数据恢复证据，不能声称它证明了进程资源释放；独立原生正常退出另列。

CUA 曾遇锁屏、ScreenCaptureKit -3811、noWindowsAvailable 和滞后画面，重新绑定/键盘激活后继续。停止操作同时核对 AX 控件和后台持久状态，不能用旧截图单独证明停止结果。

## 追加：非空派单、阅读位置与故障恢复

真实 `agent()` 工作流通过自定义角色模板的 `default_command: hive:pi` 与明确的 `openai/gpt-6.1-terra` 启动成员，产生实际 workflow-run/dispatch 关联。驱动以真实成员 token 读取 messages/required_seen_seq，再通过正式 report API 提交明确标注为驱动生成的结果和真实文件路径。awaiter 收到汇报，工作流状态 completed、派单 reported，临时成员清理后保留会话。`dispatch-receipt.json` 保存实际回读。没有模型推理或伪造模型回复；不可达回环地址的通知尝试仍不代表模型验证。

首次尝试修改内置 pi CLI 预设返回 409（只读），没有绕过约束或改数据库预设；改用 Hive 已有自定义角色模板，验证后通过 API 删除该 owned 模板。普通 canonical CLI 的 PTY 路径不因此被重定向或宣称兼容。

原生验证了工作流 → 派单详情 → 成员会话 → 派单详情 → 原工作流，以及工作流直接进入已经选中的同一成员会话再返回。产物预览读取真实文件，结果复制到未发送成员草稿后与原文一致，应用正常退出重开后草稿仍一致。

发现返回详情会跳到顶部，给列表与当前详情分别接上现有 ScrollHandle，切换工作区/选择其他运行才重置；重编译后返回保持日志/派单阅读位置。已退出的成员从真实历史会话显示名称，workflow pseudo-agent 显示“工作流”，不再把这两类身份直接显示成 UUID。

通过仅作用于 owned SQLite 的 BEFORE UPDATE 触发器拒绝 `workflow.cli-policy` 写入，原生保存返回 500、原配置保持 codex、pi 草稿保留；成功读取/轮询后失败提示仍在。移除触发器后同一按钮重试成功，清除错误并结束未保存状态；再恢复 codex。故障触发器已不存在，见 `policy-write-fault-receipt.json`。首个草稿回读脚本误用 text 字段，改读实际 input 字段后核对成功，未改产品格式。

终止 owned 原生应用的唯一 Node sidecar（核对父 PID、可执行文件），页面明确断开并禁用写操作，保留旧数据。首次重启暴露共享恢复入口漏恢复 workflows.visible，导致被聊天页替换；沿现有视图恢复 tuple 补上后，第二轮同样断开/原生“重新启动后台”保留原工作流详情并重新读取。两轮原生正常退出。不是模拟 RPC 成功。

另暂时移除已完整备份、无关联派单的 owned completed 行，使真实详情接口返回 404；原生显示“保留上次结果”并继续展示原结果。随后逐字段精确恢复同一行、SQLite quick_check=ok，自动刷新清除读取错误。此故障仅作用于验收数据，不修改生产协议或加入测试 fallback。

[派单与成员往返后位置](assets/workflows/dispatch-roundtrip.png) · [写盘失败保留草稿](assets/workflows/policy-write-failure.png) · [读取失败保留结果](assets/workflows/read-failure.png)

## 尚未验收

#13 保持开放：删除最终按钮、跨连接在途操作的精确时序、大历史性能及 Windows 原生仍需验证；Mac 开发包已更新且独立物理往返验证通过，见[当前开发包](2026-11-08-current-package.md)。父目标 #1、Cindy 完整体验对标和正式签名发行没有完成。允许列表来自 Hive，不代表八种 CLI 都已获得桌面结构化会话或工作流兼容性验收。

2026-11-10 补验：修复计划正文的无障碍缺失，增加现有可选择文本，实际鼠标/键盘暂停恢复、英文20px窄窗显露、真实SQLite与冷读通过；局部复制和未覆盖项分别记录，见[计划阅读与控制](2026-11-10-schedule-reading.md)。此轮源码未进入当前开发包，也未把上述删除/平台边界改为完成。

## 真实Codex/Terra补验与汇报回收

生产workflow agent()的真实Codex/Terra Worker已读取/计算/写文件并显式汇报，发现并修复回收过早造成native report误报失败；确定性回归先红后绿、最终全新实际模型检查、持久恢复和原生结果/产物/成员往返通过，见[真实工作流与#21](2026-11-09-workflow-codex-report.md)。由驱动启动工作流，模型执行Worker职责，不冒称模型主控编写。此修复尚未进入2b3c07e候选。
