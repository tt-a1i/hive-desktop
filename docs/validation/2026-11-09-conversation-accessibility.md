# 聊天正文及工具详情的辅助功能读取

2026-11-09，源码开发窗口，非候选 ZIP 验收。

## 实际缺口

此前原生走查的 AX 树只列出聊天复制、引用和工具展开等操作，用户正文与助手回复虽在屏幕可见，却没有文本语义节点。使用辅助功能浏览历史时无法直接读出内容。

在已有渲染节点补充语义：消息组按现有发送者/投递标题命名；普通正文使用可见文本；Markdown 使用组件自身 `rendered_text().as_str()`，不自行剥离标记；已展开工具输入/输出分别命名。团队消息完整原文只在展开后加入文本节点，保持折叠边界。没有增加后台事件、投递状态或存储。

## 原生验证

使用此前真实原始 Codex / Terra 的 47/3 主控和 Worker 历史，独立审计数据目录 `.artifacts/steered-native/state`，本次未发送输入、继续执行或运行新模型推理。

- Worker 派单正文及中文助手回复出现在 AX 文本节点，按派单/Codex 分组。
- 展开已完成工具组，再展开 `commandExecution`：AX 能读取真实参数 `/bin/zsh -lc 'cat input.json'`、cwd 和输出 `{"samples":[5,19,23]}`；截图与文本保存在 `.artifacts/conversation-reading/tool-details.*`。
- 主控显式用户正文、中文回复、最终 `MAIN_STEER_OK sum=47 count=3` 可读。
- 折叠团队通知没有完整 `<hive-system-reminder>` 原文节点；点击查看后出现真实工作流完成通知文本；再次收起后移除。截图/树在同目录 `main-folded.*`、`notice-expanded.*`、`notice-refolded.txt`。这是 AX 语义边界检查，不把历史原文当成新指令执行。
- Cmd+Q 正常退出，PID 72525，exit 0；RPC trace 没有 `chat.prompt` 或 `chat.resume`。
- macOS `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop` 及 `git diff --check` 通过。

本次只改 UI 语义，未新增/修改测试，也未运行后台全量测试。不等于完整 VoiceOver 导航、所有 Markdown/图片/思考块、流式读取或 Windows 原生验收。候选仍 768d085，current 不变，完整目标未完成。
