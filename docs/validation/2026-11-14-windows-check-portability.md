# Windows 检查程序可移植性

2026-11-14，工作项 [#30](https://github.com/tt-a1i/hive-desktop/issues/30)。前期修复检查程序；后续原生调查另发现并局部修复外部程序页裁切。流水线配置未修改。

## 观察与改动

Windows run [43556960351](https://github.com/tt-a1i/hive-desktop/actions/runs/43556960351) 再次在既有检查处失败：Codex 失败夹具的 `.artifacts` 父目录不存在；附件源文件名包含 Windows 禁止的双引号；附件及失败重试检查在序列化 JSON 中搜索未经转义的路径。

- Codex 夹具先创建证据目录，仍执行真实 JSONL 子进程退出、工具取消与身份写盘失败重试。
- Windows 使用包含中文、空格及 emoji 的合法附件名；POSIX 另保留双引号覆盖。
- 正向附件检查从实际 HTTP 请求/历史文本中解析独立 JSON 元数据行，精确比较名称、路径与字节数。固定 Windows wire 字节覆盖请求和文本内两层 JSON 转义。用户正文完整比较固定原始消息，禁止文件上下文混入。

## 本机验证

以下命令在 `/Users/tsk/code/hive-desktop` 运行，均退出 0，无厂商模型请求：

- `pnpm --filter @hive-desktop/runtime check:files`：真实 Hive HTTP/SQLite、内置 Pi SDK、原始外部 Pi、附件副本、删除源文件、冷恢复及编译后 JSONL。
- `pnpm --filter @hive-desktop/runtime check:delivery-failures`：原投递/图片/文件重试、真实 SDK 受控响应、写盘故障告警及冷恢复。
- `pnpm --filter @hive-desktop/runtime exec tsx checks/file-attachments.ts`：收紧原始用户消息断言后的定向重跑。
- Codex 失败检查另在新建的空临时 cwd 运行，首次不存在 `.artifacts`；退出 0 并生成原收据。日志和 cwd 记录在 `.artifacts/product-completeness-20261114/codex-clean-cwd.{log,json}`。正常包目录下的定向执行也退出 0。

四个独立 reviewer 检查覆盖、恢复语义、Windows 编码及范围；共同发现仅检查元数据缺失会放弱用户正文隔离断言，已改为固定完整消息比较并重跑。四者最终复核均无遗留发现，未把静态审查称为 Windows 实机验收。

## Windows 后续运行

run [43558093991](https://github.com/tt-a1i/hive-desktop/actions/runs/43558093991) 的 `native-clis` 和 `native-supervision` 已成功。Windows 实际原始 Pi/Codex CLI、附件复制/队列失败重试等相应步骤通过。构建 job 越过原临时目录错误后，在 `codex-steering.ts` 的写盘故障夹具失败：Windows 的 `chmod(0500)` 没有禁止目录写入，未成功制造故障，导致预期保留的两条投递已正常消费。

该故障注入改为先保留原 metadata 目录，再用普通文件阻塞同一路径，结束后恢复；所有平台都必须实际触发写入失败，仍要求两条投递和零完成回执，不修改产品代码或放宽断言。`pnpm --filter @hive-desktop/runtime exec tsx checks/codex-steering.ts` 已在本机退出 0，覆盖 system/report 各 12 种时序，日志为 `.artifacts/product-completeness-20261114/codex-steering-portable.log`；新 Windows 结果继续补充。

已读取 Windows `native-clis` 完整日志，确认 `FILE_ATTACHMENTS_OK`、`FILES_WIRE_OK`、`DELIVERY_FAILURES_OK`、`PI_REASONING_OK` 与 `PI_REASONING_WIRE_OK` 均实际输出；不是仅凭 job 绿色推断附件检查成功。

run [43558667271](https://github.com/tt-a1i/hive-desktop/actions/runs/43558667271) 已完整通过 Codex 检查，另外两个 job 再次成功；随后在旧 Pi 账号夹具的关闭时序失败。生产 `PiAuth.close()` 会取消目录读取，而旧检查延后观察退出登录 Promise，并要求关闭后的读取成功。现以调度屏障控制真实 SDK 读取的时序，保留原 AbortSignal，立即观察拒绝，核对关闭原因、刷新/读取排空、实际落盘删除及另一提供商不受影响。清理先关闭屏障标志再释放已有等待，避免失败时新增阻塞。生产账号代码没有改动，未接触用户凭据。

`check:pi-auth` 三组及最终 `pi-provider-config.ts` 定向执行均退出 0；记录在 `pi-auth-regression.log`、`pi-provider-close-final.log`。Codex 写盘故障另补原两条投递的完整比较、故障路径核对、修复后以原线程重建连接并显式重试、再次冷读原完成 ID，24 场景退出 0（`codex-steering-recovery.log`）。没有将复用故障连接即时重试称为通过。独立审查发现的注入前误清理与清理期间新建读取屏障两处问题均修复并复核。

上述 Pi 关闭检查最终另对另一提供商的完整测试凭据作前后 `deepEqual`，不只比较类型，定向执行退出 0（`pi-provider-close-exact.log`）。Codex 全组回归一次命中本地夹具的 1 秒初始化超时；隔离定向与随后完整回归均退出 0，未改变超时或产品代码，记录在 `codex-regression.log`、`codex-transport-isolated.log`、`codex-regression-final.log`。

run [43559892506](https://github.com/tt-a1i/hive-desktop/actions/runs/43559892506) 越过 Pi 账号和模型检查后，在置顶检查暴露陈旧的返回值期望：现在的全局命中包含 `workspacePath`，旧期望漏了该字段。已补充与实际创建目录独立 `realpath` 的精确比较，不删除返回字段或放宽 deepEqual；该问题也影响 Mac，不属于 Windows 产品缺陷。`session-pins.ts` 本机退出 0，保留四后端元数据、真实 HTTP/SQLite/JSONL、写盘失败/冷恢复及零 Agent 请求检查（`session-pins-final.log`）。新 Windows 结果继续记录。

run [43560938873](https://github.com/tt-a1i/hive-desktop/actions/runs/43560938873) 已通过置顶与归档，图片受控对端检查遇到 `spawn EFTYPE`：临时生成的 `.cjs` 被官方 SDK 当作原生程序，在 Windows 直接启动失败。夹具改为其支持的 `.js` 入口，并明确临时目录为 CommonJS；保留原始图片字节/历史/完成回执/无效输入拒绝断言，不修改 Claude 适配器，也不登录或调用真实 Claude。Mac 定向检查退出 0（`image-peer-launch.log`），Windows 后续结果待记录。

后续 [43561583272](https://github.com/tt-a1i/hive-desktop/actions/runs/43561583272) 已越过启动，夹具历史未被 SDK 读回，原耐久性断言如实失败。当前再将临时目录先 `realpath`，使传给 SDK 的目录与子进程实际 cwd 使用同一真实路径，避免 Windows 短路径差异；这是待 Windows 结果确认的路径修复，未删除历史或完成回执断言。Mac 定向退出 0（`image-peer-canonical-path.log`）。

run [43562367240](https://github.com/tt-a1i/hive-desktop/actions/runs/43562367240) 已完整通过上述图片对端/历史检查，路径修复得到 Windows 实机确认。下一项上传检查则暴露另一个跨平台陈旧期望：Pi 显式用户消息已保留为原生 `role=user`，旧检查仍找 `customType=hive-delivery`。现按附件 ID 与原生用户角色定位，首次及冷重开均要求恰好一条，保留原始图片/来源/HTTP/SQLite检查，并完整比较冷恢复的正文与展示元数据。Mac `check:attachments`（含编译后 JSONL）和收紧冷读数量后的定向检查均退出 0；未修改生产消息契约，也不是 Windows 产品丢失消息。

run [43563561289](https://github.com/tt-a1i/hive-desktop/actions/runs/43563561289) 已通过全部上述 Node 检查、Windows 优化版编译、位置/终端测试、部署/便携 ZIP/解压、包内 runtime/PTY、Codex 与 Windows 终端桥。原生窗口也已打开保存的项目并恢复失效会话选择；上传的 1040×760 窗口图显示真实应用。后续 UIAutomation 切换外观时仍比较旧 JSON 字符串，实际应用现在保存 `{mode,font_size}`，因此最终原生检查失败。当前修正脚本按模式及未变的16px字号核对，并保留三个原生 Toggle 状态检查；最终重启检查同样完整比较对象。没有修改产品设置格式、跳过窗口动作或放宽保存断言。

独立检查还发现 `--language en` 与中文 Windows 截图动作不兼容，现于创建临时状态/启动窗口前明确拒绝该组合；英文仅启动检查可不设置截图环境变量运行。Node 语法检查通过；本机没有 PowerShell，实际 Windows 交互结果继续由下一次原生运行确认。

run [43566600333](https://github.com/tt-a1i/hive-desktop/actions/runs/43566600333) 已确认三种外观保存/原生 Toggle、Pi 目录搜索及模型表单切换/放弃草稿通过。后续在外部程序页找不到可用的 Codex 浏览按钮，失败日志未区分缺失、禁用与裁切，尚不能据此断定唯一根因。

Mac 原生调查复现了另一个真实配置问题：1040×760、16px 下，同一程序卡片底部的输入/浏览被自身圆角边缘裁切，底部保存区却可见，滚动无法到达被裁切部分。仅给 Programs 的卡片增加 `flex_shrink_0()`，避免纵向弹性布局将它压缩后由 `overflow_hidden` 裁掉字段，不改变共享卡片或配置逻辑。调试复验中，16px 宽窗和真实20px中英文822×503均能滚到完整末行与底部操作；未运行账号、CLI或模型检查。Mac构建及MSVC交叉检查通过，不冒充Windows或release验收。

Windows脚本在检查浏览按钮前先聚焦实际Codex路径输入，让已有键盘显露逻辑生效；原输入/保存/恢复断言保留。任何 `Wait-Action` 失败会额外记录本测试窗口内固定控件的缺失/启用/离屏/位置状态，并保存失败截图，不复制输入值。新原生Windows结果继续补充。

不将 Mac 检查或 MSVC 交叉检查视为 Windows 原生成功，也不将本次部分原生步骤通过称为完整 Windows 验收。现有 Mac 开发包无需因 test-only 改动重新组装。

## 本轮账号范围

用户明确 Claude 尚未登录/配置，本轮无需验收 Claude，Antigravity 同样无需验收。不尝试登录、配置或调用它们的真实模型；保留接入及未验证说明，继续其他实施。此前请求允许 Claude 原生模型测试的事项已取消。
