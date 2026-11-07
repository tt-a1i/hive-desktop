# Claude 交互与通知设置验证

2026-11-07，基线 `8c5ad98` 后的工作树。此记录覆盖新增接入与界面，不代表完整产品或 Claude 模型协作验收。

## 真实问题

1. 用户把 Claude 作为主控时，原生工具许可和补充问题会进入 SDK 回调；此前默认处理器直接报不支持，无法在桌面继续这轮工作。
2. 现在同一聊天中显示工具输入、单次许可按钮或原生问题；用户回应后，后台把结果交给当前 SDK 回调。切换会话仍能找到待回应项和未提交答案。
3. 不接入这条路径，外部 Claude 统一聊天会在正常原生交互处中断，无法满足已确认的主控选择目标。

通知设置另一个实际缺口：Hive Web 已能配置完成通知地址，桌面却不能读取、保存或清除。接入后可在通用设置完成这些操作；否则用户仍需额外打开 Web 管理同一配置。只复用已有通知机制。

## 实现与契约

- 固定官方 SDK `0.3.317`，默认 `canUseTool` 交给独立的 `ClaudeInteractions`。原生 `requestId`、`toolUseID` 和当前输入 UUID 绑定临时卡片，不伪造 Codex thread 身份。原生信号、回合结束、停止、进程死亡和释放都会清理回调。
- 工具许可只提供单次允许、拒绝和取消。允许返回原始输入；拒绝不强制中断，取消返回 `deny` 与 `interrupt:true`。不返回 `updatedPermissions`，不改变原生权限模式或全局设置；内置 Pi 不增加审批。
- `AskUserQuestion` 保留原生问题、选项、描述和多选语义，回答以完整问题文本为键；多项按原生约定合成字符串。界面无默认选择，支持其他回答，空答案不提交。预览作为普通文本展示，不执行 HTML。
- “回应已提交”只说明交给 SDK 回调，不声称原生进程确认或工具成功。SDK 的其他 `onUserDialog` 和 MCP elicitation 仍单独报告不支持。

两类交互共用 `canUseTool`，以及 `updatedInput.answers` 的格式，已对照 [Claude 官方文档](https://code.claude.com/docs/en/agent-sdk/user-input) 和 [官方示例](https://github.com/anthropics/claude-agent-sdk-demos/blob/main/ask-user-question-previews/README.md)。本地固定版本的声明另行核对，不只依赖最新网站。

通知地址复用 `/api/settings/app-state/notifications.webhook-url`。桌面校验空值或可解析的 HTTP(S) URL，保存时去除首尾空白；Hive 原有通用 app-state 接口的校验语义未改。输入默认遮蔽，说明通知会包含工作区标识、成员名和汇报或工作流摘要。保存和清除跨页面导航继续等待，完成后同步输入；失败保留草稿。未新增通知投递逻辑。

## 实际执行的验证

全部通过：

- `pnpm check`、`pnpm build:runtime`。
- 既有 `pnpm --filter @hive-desktop/runtime check:claude` 的八个检查：原始 Claude Code `2.1.317` 初始化、目录、控制、隐藏上下文、恢复和进程死亡，以及明确标注的合成历史和直接工具回调。未发送推理请求。新建清单与日志：`.artifacts/claude-regression-20261107-085001/`。
- `pnpm --filter @hive-desktop/runtime check:claude-interactions`：官方 SDK 的真实管道通信连接合成 CLI；验证单次允许的完整输入、拒绝/取消、问题答案、非法答案、重复/过期响应、原生取消信号和后续请求。另一组起真实 Hive HTTP/SQLite，验证会话归属、答案不进入 Hive 状态、进程死亡、实际 `chats.prompt` 队列成功交付和取消。
- `pnpm --filter @hive-desktop/runtime check:notification-settings`：真实 Hive HTTP/SQLite 默认空、保存、关闭重开、清除后再次重开。
- `cargo fmt -- --check`、`cargo check`、`cargo build`，以及 `cargo test accepts_empty_and_http_urls_only`。

新增交互检查已加入常规 `check:claude`。八个原有检查和两个新检查分别执行通过，没有为文案和纯 UI 重复跑整个项目测试集。未删既有测试，也未增加仅为检查服务的生产 API 或 fallback。

最终新检查收据复制到 `.artifacts/claude-interactions-evidence/{sdk-receipt,hive-receipt}.json`；原目录分别为临时目录下的 `hive-claude-sdk-interactions-gCOsjo`、`hive-claude-interactions-toYuTp`。首轮合成 peer 未正确解析 session 参数的失败保留为 `failed-preflight.json`。随后修正的是夹具参数、原生 transcript 的 parent UUID 链与中断结束事件，没有用生产代码绕过检查。

## macOS 原生窗口

冻结包：`.artifacts/claude-interactions-ui-proof/Hive Desktop.app`。`build.json` 记录源码散列和原生二进制；使用当前 runtime 输出与此前冻结的生产依赖，不是一次重新下载依赖的正式打包。控制与模拟 CLI 在 `.artifacts/claude-interactions-ui-xAk6Wc`。独立 `CLAUDE_CONFIG_DIR` 与 Hive 数据目录，不覆盖 HOME，不接触用户全局配置。

实际从桌面发送消息，经 `chat.prompt`、Hive 队列、官方 SDK 到模拟 CLI，完成：

- 工具原始输入可读；点击仅允许本次和拒绝本次，卡片移除，队列继续。
- 空问题答案显示校验提示；单选选项甲、多选选项一和选项二，加中文其他回答，切换到第二会话再回来仍保留全部草稿。原管道回执中答案精确匹配，每个请求只收到一次。
- 取消提问返回 `deny + interrupt:true`。全局停止另走实际 interrupt，卡片和待处理计数清除。
- 约 760×583 逻辑尺寸下，问题和长工具输入能滚动，底部操作可达。合成 peer 不完整模拟所有原生聊天回显，因此不据此声明完整 Claude 会话排版验收。
- 通知地址默认遮蔽，FTP 地址不能保存；隔离的 loopback URL 保存后已 trim。未保存地址跨页签保留；点击清除后立即切走再回来仍为空，另从真实 SQLite 读取确认已清除。没有人为延迟响应，精确的异步交错修复另经源码复核。
- 退出后原生应用、Node 与两个 GUI 合成 CLI 进程全部消失。

窗口收据：`.artifacts/claude-interactions-ui-proof/receipt.json`。唯一提交答案检查同时覆盖 UTF-8 和 JSON 转义，未出现在 Hive 数据文件；模拟 CLI 自己的回执有答案，不能推断外部原生 Claude 不保存答案。

## 边界与任务 verdict

| 项目 | 结果 |
| --- | --- |
| 默认许可与原生问题接入、统一聊天卡片 | 已实现；官方 SDK + 合成对端 + Mac 原生界面验证 |
| 当前会话归属、一次回应、停止与过期清理 | 定向检查通过；GUI 另验证待回应期间停止 |
| 既有原始 Claude 无推理控制与恢复 | 八项回归通过，未推广为真实模型证据 |
| 通知地址读取、保存、清除与草稿 | 真实 HTTP/SQLite 和 Mac 窗口验证通过 |
| Claude 实际模型审批、问答和团队闭环 | 未验证；用户要求所有模型测试使用原生 Terra，不擅自改用 Claude 模型 |
| 本次模拟 peer 的 MCP | 状态模拟为 connected，不证明真实 MCP 握手或工具调用；旧原生握手证据见适配器记录 |
| 实际 webhook 投递 | 未触发，也未向外部地址发送消息；此切片仅配置已有机制 |
| Windows 原生交互 | 未验收；`8c5ad98` 的 Windows CI 通过不包含本次改动 |

## Self-Review

四个独立角度并行复核，之后对改动复核：

- A 架构：A-；原 Codex 空快照状态和身份守卫未回退，未发现新增阻断。
- B 真实边界：B → B+；发现清除通知地址后立刻切走导致旧值保留。已修：请求跨导航继续，重开时保持 loading，成功更新隐藏输入。原生窗口及源码复核通过。
- C 验证质量：B+ → A-；新增检查漏入常规脚本已修，问题取消、非法多答/重复答案、真实队列完成/取消盲区已补。没有生产测试专用 API。
- D 契约：B → B+；“回应已发送”改为“回应已提交”，避免暗示原生确认；通知内容说明已补充。

没有未处理的严重项，也没有 reviewer 结论冲突。综合 B+：本切片有可检查证据，完整产品、真实 Claude 模型和 Windows 窗口仍未完成。
