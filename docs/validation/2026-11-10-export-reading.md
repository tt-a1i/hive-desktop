# 导出预览阅读、选择与系统保存

源码基线`4c13d37`，对应#16。真实Mac窗口发现：预览、会话标题、固定快照范围和10,000字限制可见，但AX只暴露开关/阅读区域/操作按钮，没有正文和说明。`.artifacts/export-reading-4c13d37/before.ax.txt`来自6148801实际开发包。

## 修复

`conversation_export/view.rs`为标题/范围/预览限制加现有Label语义，预览正文复用`text_reading::selectable_text`，沿原阅读区ScrollHandle及10000字符上限工作。完整复制、选项、快照序列化和保存路径未改；无新依赖、后端或权限逻辑。

最终原生AX出现标题、固定5条记录说明、预览正文标签及截断说明，见`readable.ax.txt`；工具展示正文值有截断，不把这等同于完整VoiceOver逐字阅读验收。

## 实际数据与操作

复制原有合成Pi原生历史/隔离模型配置到新自有state，改写副本index的sessionFile指向副本；不打开原会话文件进行写入。数据包含真实Pi SessionManager记录的合成用户/思考/工具/1200段正文/汇报与附件元数据，不是模型生成结果。提供商是不可达回环地址，本轮没有发送或运行模型。

- 实际鼠标在预览滚动两页，标题/元数据换为正文后段，取得明确位移，见`readable.jpg` / `wheel-down.jpg`。
- 系统Save→Escape取消后，原快照与选项保留并显示取消状态。再次Save，通过系统Go To Folder选择本轮自有目录，保存中文命名`默认导出.md`；实际33,333字节/11,576字符，与旧完整英文快照逐字节相等，超出预览的尾部未丢失。系统路径字段最初两次粘贴没有实际值，随后通过AX setValue写入并核对后确认；不称前两次粘贴通过。
- 复制全文→另一自有会话未发送草稿，磁盘input逐字等于系统保存全文，见`full-copy-receipt.json`。再从预览拖选实际一行，Cmd+C粘贴仅为`Workspace: 导出验证`，见`selection.jpg`、`partial-copy.ax.txt`；原导出会话的`DO_NOT_EXPORT_DRAFT · 保留未发送草稿`始终精确保留。
- 英文16字号592×506窄窗，预览End到末端，Home/Next/Prior实际翻页；Tab从阅读区进入可见Copy按钮，反向Tab回阅读区/工具开关，Space开启工具和思考两开关，AX均on。见`narrow-end.jpg`、`narrow-page.jpg`、`options-keyboard.ax.txt`。
- 窄窗系统Save将两选项开启版本保存为`完整导出.md`；完整保存路径换行、说明和两操作均可见，见`narrow-saved.jpg`。独立读文件验证思考/工具输入/结果只出现在完整版、嵌套四反引号围栏保留；两版本均1200段正文、END_EXPORT_BODY及可见汇报/附件信息完整，排除隐藏上下文、原始注入、另一会话和未发送草稿。见`content-receipt.json`。
- 正常退出再开应用，原草稿保留，重新打开导出仍是原5条记录并默认关闭两过程选项，符合原有临时选项行为，见`cold-export.ax.txt`。基线包、修复后、冷开三个原生进程均Cmd+Q exit0。

## 数据核对与边界

`cold-receipt.json`：原始状态/配置文件逐项SHA不变，副本两个Pi原生JSONL也与原始字节一致；原会话草稿与另一会话实际局部复制草稿精确，真实SQLite messages/dispatches/dispatch_messages/workflow_runs/agent_runs均0。没有把导出、复制或保存误变为发送或执行。

最终Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、LLVM PATH下Windows MSVC target `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`及`git diff --check`通过。纯UI修改未新增/修改测试或跑全量suite；快照/原子保存代码不变，其原有测试与失败重试证据按原范围保留，本轮不冒称又做了失败保存。

范围仅上述英文16字号、主题及窗口；完整中文/字号/主题、VoiceOver、系统对话框纯键盘全流程、Windows原生与包内修复后GUI尚待。current/candidate仍6148801，尚未纳入这次预览修复及前两批后台修复；#16与完整目标继续开放。
