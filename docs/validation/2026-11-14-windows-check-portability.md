# Windows 检查程序可移植性

2026-11-14，工作项 [#30](https://github.com/tt-a1i/hive-desktop/issues/30)。只修改检查程序，未修改应用或流水线配置。

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

不将 Mac 检查或 MSVC 交叉检查视为 Windows 原生成功。现有 Mac 开发包无需因 test-only 改动重新组装。

## 本轮账号范围

用户明确 Claude 尚未登录/配置，本轮无需验收 Claude，Antigravity 同样无需验收。不尝试登录、配置或调用它们的真实模型；保留接入及未验证说明，继续其他实施。此前请求允许 Claude 原生模型测试的事项已取消。
