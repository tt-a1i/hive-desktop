# Codex 原生连接与脱敏账号检查（#26）

2026-11-10，源码基线 c3aeb18。首次设置只检查版本无法确认桌面连接：本机 PATH 的 /opt/homebrew/bin/codex 为0.46.0，能返回版本但不支持当前原生 app-server --listen；实际使用 /Users/tsk/.npm-global/bin/codex 为0.167.0。新用户此前需要到首次发送才发现连接失败。本轮补设置反馈，未自动选择或更新任何CLI。

## 实现与契约

保留既有版本检查及结果，增加独立“检查 Codex 连接与账号”。沿现有 command 校验、环境变量 > 应用路径 > PATH 优先级，支持未保存路径；调用已有 CodexTransport/OwnedCliProcess 建立真实原生连接，再 account/read({refreshToken:false})，最终排空并回收自有子进程。每次原生请求5秒上限；取消、设置关闭均复用既有 probeAbort/probes 排空。没有新增依赖、线程/模型请求、登录/刷新/移除操作、持久状态或后台轮询。

契约直接取自原样0.167.0的 app-server generate-json-schema，留存 GetAccountParams.json/GetAccountResponse.json。返回只包含配置命令/来源及封闭的 status/accountType；账号对象中的邮箱、计划、工作区、ID、凭据及错误诊断不进入前端/结果。原生连接失败或非法响应返回failed，未知未来账号类型返回unknown，保留进一步检查方向。检测到ChatGPT/API Key/Bedrock/GCP记录只描述本机状态，不宣称凭据有效或模型可用。requiresOpenaiAuth=false且无账号时单列无需认证，不误称已登录。

前端结果随输入变化标为过期；环境变量覆盖时按实际命令保留判断。后台代次变更丢弃旧结果，重连清空检查状态。沿既有设置字段Reveal/Viewport，未创建第二套导航或权限流程。

## 验证

- 新增 checks/codex-account.ts。先运行时缺待实现模块而失败，随后实现。真实自有JSONL子进程协议对端覆盖ChatGPT/API Key记录及脱敏、无账号、无需认证、未知类型、非法响应、5秒超时、程序不存在、请求中取消和PID回收；实际写出的请求严格只有 initialize / initialized / account/read，refreshToken=false，不调用模型接口。对端是合成协议，不冒认真实账号行为。
- 同一检查穿过真实 Hive HTTP/SQLite app-state，验证草稿/app/env来源、输入错误、不保存草稿路径及runtime关闭时正在检查的子进程完成取消/回收。
- 指定原样 /Users/tsk/.npm-global/bin/codex，真实初始化及account/read成功，返回account_detected/chatgpt；原样旧版 /opt/homebrew/bin/codex返回failed。检查前后应用保存路径完全相同。命令 HIVE_CODEX_COMMAND=/Users/tsk/.npm-global/bin/codex pnpm --filter @hive-desktop/runtime check:codex-account，最终退出0。原生CLI检查无模型或thread/turn请求。check-final.log。
- Mac Hive Connection Audit，使用最终账号检查源码debug程序、包内Node及本仓dist/main.js：有效配置路径显示 Connected to Codex. A ChatGPT account record was detected；输入改为未保存旧路径后立即隐藏旧结果、显示输入变化，再检查显示连接失败及Entered path。没有点击保存。native-signed-in / native-legacy 的PNG及AX。
- 英文/light/20px、约593×507窄窗：恢复原路径、Tab经Browse到新按钮，焦点自动显露，Return重新检查成功；正文/来源完整换行，未保存提示消失且Save禁用。Escape回聊天，“保留草稿 B”及精确未发送草稿保留。Cmd+Q原生exit0，应用已不在运行清单；native-narrow.png/AX、native-draft.ax.txt。
- 退出后独立使用包内Node和新runtime冷读：配置仍原0.167.0路径，三条原会话、命名空会话身份、原草稿、空历史、pending0/busyfalse及通知false保持。只读SQLite仍workspace1/messages1/agent_runs1/dispatches0/workflow_runs0；先前原样Terra线程仍仅1个既有回合，未新增模型执行。cold-receipt.json。
- pnpm check、pnpm build:runtime、最终Mac cargo build、Windows MSVC cargo check、git diff --check通过。另跑既有 executable-probe.ts，核对旧版本检查契约、超时及退出取消不被新检查影响。未改/删既有测试，未跑无关全量；本次测试覆盖新增只读协议、文件/进程及共享设置生命周期。

证据目录 .artifacts/codex-account-c3aeb18。仅保留使用的两份官方协议定义，已删除其他生成schema副本；验证壳硬链接本项目程序、复用现有Node，没有复制依赖树。

## 边界与后续

原生未登录/无需认证/其他账号类型目前仅有协议对端覆盖；中文、完整主题字号、VoiceOver和Windows原生未验收。连接成功不等于模型可用或完整登录引导；Pi与Claude未由本次检查推广。#26暂保留开放，完整目标继续。当前021c162 ZIP/DMG/Applications未包含本次源码；通知授权仍待单独回复，本轮未触发通知。
