# 外部程序设置验证

2026-11-07，macOS。应用现在可以保存外部 Pi/Codex 的可执行文件路径，不必依赖手工环境变量。设置写入现有 Hive SQLite app-state 的 `desktop_executables` 字符串值，不修改任何 Agent 的全局配置或默认模型。

优先级为环境变量、应用设置、PATH。页面同时显示保存值与实际来源；空值恢复 PATH。保存只影响之后创建的外部进程，正在运行的会话不被替换。

## 验证

- `checks/desktop-settings.ts` 运行真实 Hive HTTP/SQLite 与外部 Pi RPC，验证保存、完整运行时重启、空值清除、环境覆盖、空白/控制字符/过长输入拒绝，持久化失败后保留旧值。真实已存在的 Pi handle 在更新路径后仍保持；新 handle 会使用更新后的路径，失效路径报错，修正后可以打开。无模型请求。收据 `.artifacts/desktop-settings-H7yZ9m/receipt.json`。
- 编译后的 `main.js` 实际 JSON-RPC 子进程验证了 `desktop.settings.get/update`、无效请求错误、关闭和重启后读取保存值。无模型请求。收据 `.artifacts/desktop-settings-rpc-YSc7vA/receipt.json`。
- 原生 CUA 在应用包中输入并保存实际 Pi/Codex 路径，退出后重开仍保留。由于 macOS Launch Services 缓存旧测试包的环境变量，另用新 bundle identity 及相同独立数据目录验证：不设置任何 Agent/Node/runtime 路径环境变量，页面明确显示两个实际路径均来自“应用设置”，外部 Pi 原有会话可打开。测试应用位于 `.artifacts/portable-settings-darwin-arm64/Hive Settings Proof.app`。
- 独立审查确认数据来源优先级、新 handle 才读取配置、失败传播、页面 generation 防过期响应；无新增阻断项，范围评分 B。runtime typecheck/build、Rust build/fmt 与 diff 检查通过。

本页仅覆盖可执行程序设置，不代表完整模型提供商、角色模板、命令预设、工作流等配置已经迁移。Windows 原生页面和可执行发现仍需实际平台验证。没有新增模型回合，也未删除或跳过既有失败测试。

## 附：会话草稿切换

后续原生走查发现所有会话共用输入框，切换会话会把未发送内容带到另一个成员。现按会话/无会话工作区分别保存当前进程内的草稿，发送只清对应草稿，发送失败保留原会话的内容。草稿尚未跨应用退出持久化。

第一版使用嵌套窗口 handle 更新，在真实按钮事件中未生效；没有将构建通过当验收。修复后记录实际输入归属与目标，在 render 使用当前 Window 更新。CUA 实测两个既有外部 Pi 会话分别写入 A/B，往返恢复各自完整草稿；新增无会话工作区写入 C，往返项目也分别恢复 B/C。只切换和输入，没有点击发送或调用模型。两个验证会话均明确选为原生 Terra。
