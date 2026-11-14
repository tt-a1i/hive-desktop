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

Windows 新运行结果待补，不将 Mac 检查或 MSVC 交叉检查视为 Windows 原生成功。现有 Mac 开发包无需因 test-only 改动重新组装。

## 本轮账号范围

用户明确 Claude 尚未登录/配置，本轮无需验收 Claude，Antigravity 同样无需验收。不尝试登录、配置或调用它们的真实模型；保留接入及未验证说明，继续其他实施。此前请求允许 Claude 原生模型测试的事项已取消。
