# Pi 原生输入、来源与确认

日期：2026-11-08。对应 [#3](https://github.com/tt-a1i/hive-desktop/issues/3)，当前为调用语义修复，命令目录/选择 UI 后续已实现并有独立 [验证记录](2026-11-08-pi-command-picker.md)；故障注入和平台验收未齐，票据保持开放。

## 实际行为

显式用户输入通过内置 Pi SDK `prompt` 或原样外部 Pi RPC `prompt` 执行原生扩展命令、input handler、Skill 和提示模板处理。自动 dispatch/report/recovery 保持原有 custom delivery，不把自动协作文本解释成用户命令。现有 Hive seen/report/accept 协议没有改动。

用户看到原始命令及附件，后端保留实际展开/转换后的原生 user 角色、正文及源 entry。状态专用 begin/result 记录保存来源和投递 ID，不作为伪造 user 消息加入模型上下文；本地消费/命令失败没有原生 user 时，桌面以明确的应用输入记录显示。SDK message_end 早于持久化，因此不在事件回调中猜测 leaf ID。

Hive 附加提醒及文件引用独立保存为隐藏应用上下文，原始用户文本原样传给原生 prompt。原因是无参数 `/command` 后追加换行提醒会破坏 Pi 的原生命令识别。文件路径仍进入实际模型请求，文件正文仍由 Agent 按需读取。图片仍使用原生 images 参数。

本地 handled 命令成功可确认这条输入投递；模型回合需通过原生最终回复和现有错误/取消检查。该确认仅代表输入已处理，不代表 Hive 派单完成或替代显式汇报。

## 验证

新增 `packages/runtime/checks/pi-user-input.ts`，真实 Pi SDK 1.4.0 / 原样外部 Pi CLI 1.4.0 + Hive HTTP/SQLite/实际投递队列，使用自建本地 HTTP 服务接收请求并提供受控成功、拒绝和等待响应。模型 ID 固定 gpt-6.1-terra；无厂商模型调用。

本轮收据记录 18 条本地请求、16 条重启后核验的原始输入：

- Skill 正文、模板参数及图片实际进入请求；input handler transform 保留真实原生 user entry。
- 无参数模板的正常最终回复完成投递，保留原生 assistant 正文。
- 无参数本地扩展命令与 handled input 不发模型请求，成功结算；抛错命令不误确认。
- 不存在的模板作为普通原文输入；本地 HTTP 错误保留待投递和失败原因。
- dispatch/report/recovery 中的 `/skill:` 原文不执行用户命令。
- 请求期间取消不误确认；完整关闭和重新创建 runtime 后，原始输入、角色、投递 ID、快照 ID 和源 entry 保持一致。

原生 Mac GPUI 窗口读取上述真实重启状态，内置 Pi 浅色 1180×820、外部 Pi 深色 760×583 均验证每条原始输入只出现一次，图片恢复、本地消费和命令失败记录可展示。窗口由测试驱动操作，不代表系统鼠标/输入法逐项验收。

证据位于 `.artifacts/pi-user-input/`：`input-contract-receipt.json`、`input-contract-success.log`、两份 `*-receipt.json` 与窗口 PNG。隔离数据目录以当前收据为准，临时路径不保证永久存在。

实际运行并通过：`pnpm check`；新增检查（运行 `pnpm --filter @hive-desktop/runtime exec tsx checks/pi-user-input.ts`）；`check:team`；`check:external-pi`；`check:images`；`check:images-native-pi`；`check:files`；Rust 定向检查 24 通过/1 忽略。构建和平台检查另存 `production-build.log`、`windows-typecheck.log`。没有重复无关全量检查。

修改现有图片检查以断言真实 native user + presentation，而非旧 custom message；文件检查改为分别验证原始 user 文本、隐藏上下文及实际请求中的上传路径。没有删除测试，没有给生产增加测试专用 fallback。

## 信任校准与边界

初始外部 Pi 对照没有实际加载项目资源；同一 agentDir 不代表 SDK 与 CLI 的项目信任策略相同。新检查只在自建 agentDir 设 defaultProjectTrust=always，让自建 Skill/模板实际加载。生产不改变用户配置、不绕过外部 CLI 信任策略，也不增加内置 Pi 审批。旧实验的校准见 [原始调研](../research/2026-11-08-pi-command-parity.md)。

后续已实现本会话原生命令目录与选择 UI，见独立验证页。仍未验证/交付：持久化失败的定向故障注入、compaction 后来源映射的专项检查、Windows 原生体验及正式签名安装。最新 Mac 开发 ZIP 仍为中断清理版本，不包含本次修改。上述本地协议成功不作为真实 Terra 推理质量或 Claude 账号可用性的证明。

后续已完成 Mac 范围的持久化故障、compaction 来源专项检查及新 ZIP 验证；见 [历史/持久化](2026-11-08-pi-history-persistence.md) 与 [开发包](2026-11-08-pi-native-package.md)，取代前文相应“待完成”状态。
