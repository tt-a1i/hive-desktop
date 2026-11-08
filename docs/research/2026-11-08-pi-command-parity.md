# Pi 技能和提示模板的输入语义差距

日期：2026-11-08；源码基线 `ace7531`。实施任务：[GitHub #3](https://github.com/tt-a1i/hive-desktop/issues/3)，主目标 #1 的原生子任务。本文保留原始缺口及后续校准；原生输入修复的当前证据见 [输入验证](../validation/2026-11-08-pi-user-input.md)，命令目录/选择体验仍未交付。

## 使用场景与影响

用户在同一项目已安装 Pi Skill 或提示模板，在统一聊天中输入 `/skill:名字 参数` 或 `/模板名字 参数`。预期沿用所选原生 Pi 的输入处理；当前桌面投递直接进入模型输入，技能正文和模板展开缺失。因此命令选择 UI 必须建立在真实调用语义上。

[Cindy 固定快照的 Skills 说明](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/apps/desktop/help-knowledge/skills.md) 把本地 Skills 与输入框 `/` 选择关联。这里借鉴本地能力的使用路径，不引入其云市场、组织发布或账号体系。

## 同条件实际对照

隔离项目包含 `.pi/skills/parity-skill/SKILL.md` 和 `.pi/prompts/parity-template.md`，各自只有正文携带不同的 UUID 标记；参数也按技能/模板区分。使用同一 agentDir、原生模型设置和本地 HTTP 服务，模型固定为 `gpt-6.1-terra`。服务记录请求正文后返回 400，拒绝模型执行，无厂商请求。

| 路径 | Skill 正文进入 input | 模板正文进入 input | 各自参数保留 |
| --- | --- | --- | --- |
| 原生 Pi SDK 1.4.0 `AgentSession.prompt` | 是 | 是 | 是 |
| Hive 真实 HTTP/SQLite → 内置 Pi | 否 | 否 | 是 |
| Hive 真实 HTTP/SQLite → 原样外部 Pi CLI 1.4.0 | 否 | 否 | 是 |

共六条请求。桌面两条路径均检查模型失败后的真实待投递队列及失败记录，未把本地错误视为成功。外部 CLI 使用原始 `/Users/tsk/Library/pnpm/pi`，无包装或 RPC 内容改写。脚本、六条请求、结果和日志在 `.artifacts/pi-command-parity/`；四条内置对照的早期收据另保留为 `builtin-receipt.json`。

## 源码链路

- `agents/pi.ts` 的 Hive 投递使用 SDK `sendCustomMessage(..., {triggerTurn:true})`。SDK 此方法直接启动 custom run，没有走 `prompt` 的 input handler、技能与模板展开。
- `agents/external-pi.ts` 将有投递 ID 的输入交给桥接的 delivery 分支；`external-pi-extension.ts` 使用 `pi.sendMessage` 启动 custom run，同样绕过输入处理。普通 automated user 桥接分支还显式传 `expandPromptTemplates:false`。
- 对照 SDK 实际源码，公开 `prompt` 管线先处理扩展命令和 input handlers，再展开技能及模板；内置 SDK 的资源已加载；外部 CLI 的项目资源是否加载还受其原生信任规则影响，不能仅凭同一 agentDir 推断。

直接把 custom 投递换成普通 prompt 会影响来源与历史映射：SDK 的公开 message_end 通知早于当前 message 的 SessionManager 持久化，回调内立即读取 getLeafId 得到的可能是旧 entry。因此需用可恢复、无歧义的原生 entry/投递关联，保留原始用户输入、实际上下文、图片和完成确认。

## 实施约束

复用原生公开输入 API，不复制 SDK 私有展开函数，不改全局用户配置，不增加 Pi 审批。显式用户输入、dispatch/report/recovery 继续分别处理；协作文本以 `/` 开头不能意外执行用户命令。失败、取消、输入被 handler 消费及扩展命令的完成判定需要定向验证，不能因为 SDK promise 返回就推断派单责任完成。

#3 的验收包括真实内外 Pi 展开、实时/历史与完整重启、稳定投递来源及 ID、取消/错误不误确认、自动协作不触发用户命令，以及调用语义完成后的原生命令目录/选择体验。Windows 验证单列；这轮没有原生 Windows 或新的系统窗口验收。

## 后续校准：外部项目信任

后续检查原始外部 CLI 的 `get_commands`，发现初始实验只列出全局命令，没有隔离项目中的 Skill/模板。Pi 1.4.0 原生 RPC 在项目默认信任为 ask 且无交互确认时不加载这些项目资源；SDK 默认资源加载策略不同。因此上表外部两条缺失结果同时受资源未加载影响，原实验不能单独证明外部展开缺失的全部原因。custom delivery 绕过输入管线的源码判断仍成立。

新检查只在测试自建 agentDir 将 defaultProjectTrust 设为 always，确保自建项目资源实际加载；生产不设置该值、不修改用户信任规则。内置 SDK 与原样外部 CLI 现都通过原生技能、模板及扩展输入处理，实际原生 user entry 与来源、冷重启及取消/错误均分别验证。完整内容见输入验证页。此前六请求收据保留，不将其重写为已通过修复。
