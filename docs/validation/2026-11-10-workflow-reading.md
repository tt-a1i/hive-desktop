# 工作流运行阅读与刷新位置

基线 `2426222`，对应 #13。真实完成/失败/停止记录用于原生阅读和复制验证，不运行模型任务。本轮修改仅在 Rust 工作流视图与瞬时 UI 状态中。

## 真实运行与阅读缺口

`seed.mjs` 使用真实 Hive HTTP/SQLite/Worker：完成脚本写入12条中文日志并返回 `{sum:42,count:3,checks:[…]}`；失败脚本抛出原始两行中文错误；等待脚本在产生就绪日志后由驱动通过正式 stop API 停止。停止数据真实，但不称本轮操作了原生停止按钮。

原始 Codex CLI/Terra 只负责既有主控身份及原生元数据；队列显式暂停，三条真实完成通知保留待处理，不触发厂商回合或伪造模型回复。

原生基线 `before-detail.ax.txt` 只读到结果和来源/参数，没有运行名称/状态/阶段或12条日志正文。`details.rs` 补齐这些阅读语义、时间/计数/空状态，日志及结果复用现有选择文本能力；原结果完整复制按钮保留，错误与停止原因仍使用原后端原文。

## 原生实证

- 修复后 `completed-readable.ax.txt` 包含完整运行身份、状态、阶段、12条日志和原结果；失败与停止分别显示“运行错误”及“停止原因”，原始两行错误、错误前日志和 `Stopped by user` 保留，见 `failed-readable.ax.txt` / `stopped-readable.ax.txt`。
- 实际鼠标拖动结果局部选区，Cmd+C → 返回聊天 → Cmd+V 得到 `check-1 中文"`（包含所选的结尾引号），见 `partial-result-copy.ax.txt`，不冒称复制了整个 JSON。
- 原生“复制日志”得到12条真实日志的完整正文，不含时间标签；实际粘贴到未发送草稿后，与后台12条 `message` 逐字相等，见 `logs-copy.ax.txt`、`logs-copy-draft.json`。再用“复制工作流结果或错误”追加原两行错误，原日志保留，见 `error-copy.ax.txt`。
- 约592×506窄窗可实际滚到 `LOG_12` 和来源/参数，见 `narrow-log-end.ax.txt` / PNG。返回运行列表与重开原详情仍绑定正确运行。

## 列表位置的真实根因与修复

窄窗列表滚到底 → 完成详情 → 返回，列表确实没有回到顶部，但原卡片发生约40逻辑像素偏移。再次用实际坐标点击同一路径仍出现，不归因于 AX 自动化。见 `list-before.png` / `list-after.png` 和 `list-coordinate-before.png` / `list-coordinate-after.png`。

根因是重新读取期间用 `!loading` 隐藏已确认的空状态（本例为空计划提示）；列表瞬间变短，ScrollHandle 把原底部位置夹到新的范围。读取完成后正文恢复高度，位置不会自动回到原值。空运行列表提示也有同样入口。

现在以当前工作区 `loaded` 标记是否已有成功快照：默认/切换工作区时为 false，真实快照成功后为 true；刷新和详情往返保留上次确认的空状态，初次读取前不冒称没有记录。沿原 ScrollHandle 工作，没有补固定偏移、等待时间或重新定位循环。

最终构建重走相同窄窗坐标点击，返回后原失败/完成卡片及空计划提示的位置保持；再原生点击刷新，位置保持。见 `list-fixed-before.png`、`list-fixed-after.png`、`list-fixed-refresh.png`。这不是所有动态内容变化都保证像素位置不变的承诺。

## 构建、冷读与边界

最终 Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、LLVM PATH 下的 Windows MSVC target check 与 `git diff --check` 通过。三个原生进程均 Cmd+Q exit 0，最终构建也实际冷恢复完整日志/错误草稿，见 `cold-draft.ax.txt`。

独立 `check-cold.mjs` 用真实 runtime/SQLite 逐字段核对三条终态不变、12条完成日志不变（全部三运行共14条）、精确未发送草稿、3条带各自 run ID 的系统通知仍暂停、busy false / 原模型 history 不变、Terra 选择保留；messages/dispatches/dispatch_messages 均零。见 `cold-receipt.json` / `cold.log`。初次只读检查误用了 `workflow_logs` 表名，核对实际 schema 后使用 `workflow_run_logs`，未改生产 schema。

证据根 `.artifacts/workflow-reading-2426222/`。纯 UI 修改未新增/删除测试，未跑全量 suite；没有生产 TS/协议/调度更改。键盘逐页阅读、完整英文/字号/VoiceOver/大历史、窗口关闭期间在途操作、计划删除最终按钮及 Windows 原生仍待，#13不关闭；current/candidate仍为6edac4c开发包，未纳入本轮源码。完整目标保持进行中。

后续已补列出范围的键盘逐页阅读、英文20字号及窄窗实证，见[键盘正文阅读](2026-11-10-workflow-page-reading.md)；不扩大为完整平台/辅助技术矩阵。
