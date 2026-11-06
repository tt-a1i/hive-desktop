# 内置 Pi 团队协作验证

日期：2026-11-07。正式代码：`apps/desktop`、`packages/runtime`。这是完整产品目标中的一段实现，不是完整产品交付。

## 实现与依据

- 固定使用 npm `@tt-a1i/hive@2.2.1` 与 Pi SDK `1.4.0`。`patches/@tt-a1i__hive@2.2.1.patch` 开放 runtime factory 和派生成员的后端选择；普通 CLI 继续交给原 PTY 实现。
- `hive:pi` 明确选择内置 SDK。主控和 Worker 共用 Hive 的成员、SQLite 派单、消息、seen/report、outbox 和退出处理。team 工具通过真实 CLI 请求 Hive，不复制业务账本。
- 桌面投递文件先持久保存消息再确认接受，按会话串行执行；完成 ID 写入 Pi transcript 后删除待处理项。失败暂停并保留消息，支持明确继续。模型结束不等于任务汇报，停止回复也不关闭派单。
- 工作区主控绑定、输入投递与模型切换按工作区串行，避免并发切换将输入送入另一个会话。切换前检查实际持久待处理消息和 Hive 未结派单。
- 原生界面显示主控/成员、活动和待处理状态；工具默认折叠，可展开、收起、复制并显示错误。会话历史与流式工具使用同一展示方式。

## 实际证据

成功团队执行保存在 `.artifacts/team-live-DakK9E/receipt.json` 和 `events.json`：

1. 主控通过注册 team 工具创建 `proof-worker`，省略 `--cli`。
2. Worker 实际启动为内置 Pi，继承 `openai/gpt-6.1-terra`，与主控使用不同 Pi 会话。
3. 主控派单，Worker 创建 `project/team-proof.txt`，文件字节精确为 `HIVE_TEAM_TERRA_OK`，并通过 read 与 Python 字节比较核验。
4. Worker 读取派单消息，取得实际 `required_seen_seq`，显式 report。Hive 派单 `b94c3198-a3eb-42bb-830c-f14c64b06111` 的最终 `state` 是 `reported`，包含实际结果文本。
5. 主控接收汇报并独立核验文件，最终助手文本是 `HIVE_TEAM_COMPLETE`。两个 transcript 的全部 assistant model 字段均为 `gpt-6.1-terra`。

这轮中主控曾向错误派单 ID 查询消息而收到 409，随后查询真实 ID 并完成核验；错误保留在工具记录和界面中，不能声称所有工具调用均成功。

生命周期验证由 `.artifacts/lifecycle-9PS9p8/receipt.json` 的初始历史和 `.artifacts/lifecycle-FwmqH6/receipt.json` 的恢复/取消收据共同支持：首次真实 write/read 后关闭并重开后台，Pi session ID 与序列化后的历史一致，重开未自动调用模型；实际 `bash sleep 30` 开始后取消，观察到 `aborted=true`、`busy=false`、`pending=0`，没有被取消回合的最终回复；下一轮返回 `AFTER_CANCEL_OK`。这是 SDK/会话取消证据，本轮未独立追踪 bash 子进程 PID 的退出。

macOS 原生窗口的 CUA 走查：退出旧窗口、重开上述团队数据后能切换主控和成员，看到各自工具历史与最终回复；展开和收起真实 team 工具结果均有效。另从界面发起真实 `sleep 120`，观察 bash 执行中后点击“停止”，界面回到“已停止当前回复”，工具记录 `Command aborted`，发送按钮恢复，待处理数量清空。第一次 UI `sleep 30` 在操作前正常结束，不算取消证据。

## 检查命令与范围

以下检查通过：

```sh
pnpm check
pnpm --filter @hive-desktop/runtime check:runtime
pnpm --filter @hive-desktop/runtime check:team
pnpm check:hive
CARGO_TARGET_DIR=experiments/gpui-chat/target cargo +1.98.0 build --manifest-path apps/desktop/Cargo.toml
cargo +stable fmt --manifest-path apps/desktop/Cargo.toml --check
git diff --check
```

`check:team` 中的队列/生命周期检查是窄边界检查；`team-tool.ts` 使用真 CLI 与检查专用 HTTP 接收端，不能称为完整 Hive 集成。`team-runtime.ts` 另行使用真实 Hive HTTP、SQLite、Pi SDK、team CLI，穿透 spawn/send/messages/report、派单状态及投递文件；仅暂停 SDK 队列以避免调用模型，不替换 HTTP 或账本。输入切换检查延迟一次真实 HTTP 提交以复现竞争。

真实模型检查需显式执行：

```sh
cd packages/runtime
pnpm exec tsx checks/team-runtime-live.ts --live
pnpm exec tsx checks/agent-lifecycle-live.ts --live
```

本轮只运行与新增接入、队列及原生界面有关的检查，没有运行 Hive 上游全量测试或 Windows CI。Pi 全局默认配置未修改；测试会话显式选 OpenAI / Terra，后续成功验证没有使用 Magpie。

## 失败尝试与修复

- `.artifacts/team-live-6lNeva`：最初补丁漏了已发布包中的 spawn 选择接缝，成员落到默认外部 CLI；同轮检查又错误地将用户输入中的完成标记视为助手完成。该尝试失败，不计作 Terra 团队闭环证据。补丁现作用于实际安装的 dist；检查只匹配助手消息，并检查真实派单状态及双方模型。
- `.artifacts/team-live-wmIx69`：最初适配器参考了更新的源码契约，返回 `{write,payloadBytes}`，已发布 2.2.1 实际要求 `Promise<void>`，导致派单被取消。停止该失败试跑，改为逐个核对安装包契约，并增加真实 send/report 集成断言。
- `.artifacts/lifecycle-9PS9p8`：恢复检查误将序列化丢弃的 undefined 属性差异当失败。保留原收据，修正比较后复用同一数据继续验证，没有重复首轮模型写入。
- 生命周期验证出现一次 Hive tasks watcher 的就绪超时警告；没有阻断对话验证，本轮未证明该 watcher 的可靠性。

## 独立复核与处理

- 后端复核发现新主会话继承旧 resume 提示而缺少启动上下文：已按实际打开的 Pi 上下文判定恢复，真实 SDK 切换检查通过。
- 后端复核复现输入与切换交错造成串会话：已将激活和提交放入同一个工作区操作队列，真实 HTTP 时序检查通过。
- 后端复核发现未加载会话的持久投递被切换检查漏掉：已读取原会话队列再判断，重开检查通过。
- 安装包契约复核发现恢复路径缺少桌面 team 工具指引：已在 Pi 加载时独立安装隐藏桌面上下文，不依赖 Hive 首次启动分支。
- 原生复核发现暂停入队和取消删除消息后状态不更新：队列现在通知待处理数变化，窄边界检查通过；暂停时禁止再次发送并保留输入，提供“继续”。
- 证据复核确认真实团队与生命周期记录支持上述限定结论；发现恢复收据缺少前次证据链接、精确内容断言使用 trim，均已修正检查脚本，保留原始收据。子进程终止边界在上文明确列出，没有将 SDK 取消扩称为进程清理证明。

## 消息来源与恢复的追加验证

新增投递元数据独立记录用户、派单、团队通知和恢复来源；模型仍收到完整 Hive payload，界面读取单独的展示文字，不靠删除或识别字符串判断来源。Pi 的 custom delivery 和元数据同条保存，首个助手回复前也会写盘；旧 user 历史显示为“对话输入”，不会猜测其来源。Worker 输入仍经过共享执行器的就绪、串行和停止检查。

`.artifacts/team-live-JStiXX` 再次完成两个真实 Terra 会话的 spawn/send/执行/report/主控核验。记录包含 user、dispatch 与 system 来源；派单为 reported，`delivered_at` 仍为空，因此不声称验证了 delivered 状态。状态通知没有误标为显式报告，当前通用报告投递仍显示为团队通知。

`.artifacts/lifecycle-DCjoGa` 验证新消息格式下的实际模型路径：含 Hive 标签的用户原文保持 user 来源，完整历史和 delivery ID 在关闭重开后相同；实际 bash 先记录自身 PID，取消后该 PID 已不存在，随后对话继续成功。这只证明该次拥有的 shell PID 退出，不推广为全部后代进程或任意取消场景的保证。

追加 macOS CUA 走查确认：派单卡片可展开完整原始 payload；恢复消息显示为“会话恢复”；用户粘贴 `<hive-system-reminder>` 文本仍显示为“你”，文字完整，真实助手返回 `UI_ORIGIN_OK`。发送失败后切换会话或已经输入下一条草稿时，新增按会话保留失败内容及复制/移除入口；该失败界面分支已构建并源码复核，尚未实际走查。

四个独立复核分别检查运行时与持久化、原生显示、验证证据及 Codex 通信层。发现的 Worker 停止检查绕过、新会话首次 custom 消息未落盘、失败输入丢失均已修正。补充真实恢复/取消证据；未把队列单元检查当作模型取消证明。待处理队列重启后的实际模型消费、取消时后续团队消息的保留仍需专项验证。

## 未完成范围

外部 Claude Code、Codex、Pi 的完整桌面接入，成员管理/配置/协作主页，桌面后台意外断开后的重连，Windows 真实运行与两端安装包均未完成。

崩溃可能发生在工具副作用完成与完成标记写入之间；持久队列不会把该窗口变成 exactly-once。当前检查没有证明系统强杀、机器掉电、磁盘损坏或全部上游工作流的恢复行为。基础阶段与完整目标均保持开放。
