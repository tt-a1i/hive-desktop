# 工作流键盘逐页阅读

源码基线 `d2ece47`，对应 #13。实际场景是运行详情超过一屏时，Tab 虽能找到操作按钮，但只读正文没有独立键盘阅读入口；键盘用户需要鼠标滚动才能看完整日志。本轮复用既有 `text_reading::region`，给列表与详情增加可访问名称、焦点边框和方向键、Home/End、PageUp/PageDown 操作。保留原 ScrollHandle、Viewport 焦点显露、刷新快照与运行操作，没有新增后端、状态或依赖。

## 原生验证

使用上一轮 `.artifacts/workflow-reading-2426222/` 的真实 HTTP/SQLite 完成、失败、停止记录，3条系统通知继续暂停，不发起模型推理。

- 修改前进入完成详情，Refresh 或结果复制按钮获得焦点后，PageDown 没有移动正文。实际工具的按键名称是 `Next` / `Prior`；一次不支持的 `PageDown` 工具调用不算产品失败。
- 修改后宽窗默认字号及英文20字号，Tab 进入阅读区后 `Next` 显示后续日志，`End` 可到 LOG_12 和来源参数；列表 `End` 后通过8次Tab到完成卡，Space确实打开对应详情。焦点与运行身份按原生截图核对。
- 英文20字号、约592×506窗口，收起导航后实际 `End` 到 LOG_12 与完整换行的来源参数、`Home` 回结果开头；`Next` / `Prior` 翻页，Down/Up 小步滚动。结果复制按钮可由Tab进入，Space调用复制；Shift+Tab回到有边框的阅读区，再Shift+Tab离开到Refresh。该次复制没有粘贴或逐字节核对剪贴板，不扩大为新增复制验收。
- 异步详情加载完成前发送Tab的初次尝试未移动，不计通过；最终加载完成后的截图才作为证据。窄窗缩放/导航切换后的最初两次Tab也未建立可见焦点，保留原图；实际点击正文后Tab到结果复制按钮，再Shift+Tab进入阅读区并验证键盘路径，不宣称导航切换后自动落在阅读区。

证据 `.artifacts/workflow-keyboard-d2ece47/`：`english20-page.jpg`、`english20-end.jpg`、`narrow20-end-actual.jpg`、`narrow20-home.jpg`、`narrow20-page.jpg`、`narrow20-prior-down.jpg`。早期 `page-down.png`、`narrow20-end.jpg` / `narrow20-end-ready.jpg` 是未成功建立焦点的尝试，不能作为通过证据。截图数据为JPEG，以实际编码为准。

## 构建与数据

最终Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` 和 LLVM PATH 下 `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop` 均 exit 0；最终原生实例Cmd+Q exit 0。纯Rust UI/英文名称修改，未新增/修改测试，未跑全量suite。

独立冷读沿用前轮断言：3条终态逐字段不变、完成日志12条/总日志14条、原精确日志＋错误草稿保留、3条原ID系统通知仍暂停、busy false、原模型历史不变，messages/dispatches/dispatch_messages均零。见本轮 `check-cold.mjs` / `cold-receipt.json` / `cold.log`。只读验证不恢复通知处理。

current/candidate仍是6edac4c开发包，未纳入本批源码。完整VoiceOver、所有语言/字号/动态刷新、计划删除最终按钮和Windows原生仍待；#13及完整目标继续开放。
