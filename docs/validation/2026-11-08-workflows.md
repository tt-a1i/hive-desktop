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

本机 `.artifacts/workflows/` 保存 seed/runner、backend-receipt、ui-persistence-receipt、native-stop-receipt、recovery-receipt、日志及截图；不是新的依赖副本。原生用当前 `apps/desktop/target/debug/hive-desktop` 和 `packages/runtime/dist/main.js`，只复用保留的包内 Node，不使用旧包 runtime 冒充新代码。当前 ZIP 尚未纳入这批 UI。

脚本与 Agent 配置均为 owned 验收数据。仅配置 `openai/gpt-6.1-terra`，提供商固定 `127.0.0.1:9`，没有访问厂商或模型推理；工作流结束通知曾尝试投递到这个不可达回环地址，产生的投递失败不作为模型协作验收。

首次计划中文名称被上游合法性规则拒绝，改用含字母的 owned 名称；未改上游产品语义。首次恢复夹具因移动目录留下旧会话绝对路径而失败，修正夹具索引。第二次停止过早导致尚未产生日志，改为等实际日志可读再关闭，随后完成/失败历史与日志、终态恢复均通过。该同进程双 runtime 恢复脚本完成断言后未及时自然退出，最终 SIGTERM，仅作为数据恢复证据，不能声称它证明了进程资源释放；独立原生正常退出另列。

CUA 曾遇锁屏、ScreenCaptureKit -3811、noWindowsAvailable 和滞后画面，重新绑定/键盘激活后继续。停止操作同时核对 AX 控件和后台持久状态，不能用旧截图单独证明停止结果。

## 尚未验收

#13 保持开放：实际非空工作流派单、产物预览与成员会话/派单详情返回的完整往返，操作/读取失败时的物理 UI、重连和在途切换精确时序，删除最终按钮、剪贴板内容回读、大历史性能及 Windows 原生仍需验证。父目标 #1、Cindy 完整体验对标和正式签名发行没有完成。允许列表来自 Hive，不代表八种 CLI 都已获得桌面结构化会话或工作流兼容性验收。
