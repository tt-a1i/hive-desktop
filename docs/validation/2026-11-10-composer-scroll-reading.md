# 输入区滚动与短窗阅读

基于 252a573 源码开发二进制。只改原生展示和输入框最小行数，后台协议与滚动跟随规则不变。current/candidate ZIP 仍为 7ae27ca。

## 问题与改动

1. 排队卡片有自己的 144px 滚动容器，外面输入区也滚动，鼠标停在卡片时不容易到达下方草稿。删除卡片内的独立高度/滚动限制，统一沿输入区滚动；每张卡片复用既有 Reveal，键盘逐卡显露，替代整个列表的粗粒度 Reveal。
2. 65% 窗高限制只施加在卡片，外部 Agent 状态行仍占空间，短窗底部提示被裁。将限制移到整个输入区域，状态、卡片、固定底栏与提示共同受约束。
3. “最新回复消失”可通过将宽窗缩至约 592×456 复现：辅助功能树含正文，但画面只见消息操作。临时布局诊断证实 tail follow 仍为 true，消息行 157px、消息视口仅 70.5px，正文被短窗空间与底部操作行挤出，并非确认发生了尾部跟随状态丢失。短消息输入框最小两行改为一行，仍自动增长至七行；助手的复制/引用改在消息标题行，减少正文下方占位，保留原有处理函数与可访问名称。

未修改 ListState、强制跳到底部、添加等待或厂商推理；诊断日志代码已从最终源码删除。用户与协作消息仍保留原来的操作布局，工具消息继续走原工具渲染器。

## Mac 原生验证

证据目录 `.artifacts/composer-scroll-252a573/`，Dark/20，短窗约 592×456，较大窗约 822×700。使用上一轮真实 Codex/Terra 历史和隔离应用数据。

- 队列展示副本中，鼠标经过单层滚动可到达完整未发送草稿；追加 x 后删除恢复原值。从输入框 ShiftTab 依次到 B、A 的 Copy，每张卡片本身显露，Tab 返回草稿；固定 Resume 与底部提示可见。见 `final-copy-a.png`、`final-copy-b.*`、`final-draft.*`。
- 真实已完成历史中，宽窗缩短后最新 COLD_QUEUE_B_OK 画面可读；输入七行精确文本，再缩回原一行草稿，正文仍可读，底部发送操作保留。见 `latest-compact.png`、`seven-lines.*`、`shrunk.*`。
- 主动滚到原始工具请求，输入七行草稿后恢复一行，仍停在同一旧请求，“回到最新”继续显示；显式点击后回到最后回复。见 `reading-older.*`、`reading-preserved.*`。未用强制 tail follow 抢走阅读位置。
- 较大窗实际点击助手标题行 Copy，粘贴回编辑框得到精确 COLD_QUEUE_B_OK；实际 Quote 得到 `> COLD_QUEUE_B_OK\n\nCOLD_QUEUE_UNSENT_DRAFT`，再恢复原草稿。见 `copied-reply.ax.txt`、`quoted-reply.ax.txt`、`restored-draft.ax.txt`。

队列展示使用旧暂停时间点的本地数据副本，厂商原生线程已经前进；仅验证卡片/草稿布局，未点击 Resume，不作为恢复一致性或新增推理证据。真实恢复与唯一消费仍以[上一轮完整闭环](2026-11-10-cold-queue-resume.md)为准。

## 状态与编译验证

真实 SQLite 逐行比较 messages、dispatches、agent_runs、workflow_runs；两个展示副本的聊天索引与精确原草稿均不变。独立实际 runtime / 原始 Codex 冷读最终历史，与上一轮最终快照逐项相同，completed ID 不变、pending 0、busy false；本轮没有新模型请求。`check-state.py` / `state-receipt.json` 通过，最终两个展示进程均正常退出 0。

最终 Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop` 和 `git diff --check` 通过。纯 UI 修改未新增 UI 测试或运行全量后台 suite；没有更改或删除现有测试。

未重新执行实时队列清空过程，因此本轮证明静态缩窗、草稿伸缩与阅读位置，不能扩展成所有流式时序问题已解决。中文/Light/默认字号的完整第二轮、完整辅助技术/输入法、Windows 原生和新包验证继续开放。
