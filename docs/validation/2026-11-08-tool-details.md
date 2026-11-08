# 工具参数与长输出的阅读体验

## 问题与参考

工具展开后原先将 JSON 参数和输出作为普通文本渲染，无法逐段选择，也没有独立的长输出阅读区；几百行日志会把错误和最终回答推到很远。参考 Cindy 固定提交 `4fd670b72af9d060785e29c553a14b6794dfe8eb` 的 `ToolCallCard.tsx` 中等宽参数和受限高度输出区，复用应用已有的 gpui-base SelectableText 与 gpui-kit Scrollbar。

现在工具参数和输出使用原生等宽可选文本，共用最多 320 逻辑像素高的独立滚动区，显示滚动条；复制完整详情的既有入口位于滚动区之外，仍保留全部参数和原输出。失败状态及折叠时的错误预览沿用原规则。

滚动区具有 Region 名称、Tab 顺序和可见焦点边框，支持方向键、PageUp/PageDown、Home/End。Ctrl/Cmd/Alt/Shift 组合键不被本地滚动处理器截获。工具渲染更新原位修改 name/status，保留 expanded 和 ScrollHandle，避免每次内容更新后重置阅读位置。

## 原生验证

隔离 Pi 配置与真实 SessionManager 原生文件，预置一条 240 行 bash 工具结果、一条 read 失败结果和最终回答；这些是持久化历史夹具，不是本轮真实模型执行工具的证明。

真实 GPUI 窗口通过鼠标/键盘事件验证：

- 折叠与展开、等宽参数、最大高度和完整输出末尾可达。
- 从聊天输入框建立焦点后，实际 Tab 导航到工具区，PageDown 确实改变该工具的 ScrollHandle；End 到达 2000 像素以后，Home 返回顶部。
- 夹具注入一次渲染层工具更新，滚动偏移保持；这是 UI 流式更新状态检查，不是原始 Agent 端到端流式推理验证。
- 鼠标拖动产生实际 TextSelection，选中内容来自工具的原参数或输出；未读写用户系统剪贴板进行复制验收。
- 切换会话再返回后，完整持久化结果逐字一致，失败状态保留；通过“回到最新消息”回到最终回答并检查实际渲染。不宣称内部日志滚动位置跨进程持久化。

原生浅色常规、浅色窄窗及深色窄窗证据在 `.artifacts/tool-details/`，含 screenshot、receipt、fixture-main 源码与 seed。GPUI 注入事件不等于物理键鼠、VoiceOver 或 Windows GUI。模型配置仅 gpt-6.1-terra，最终检查没有厂商模型请求。

验证校准保留：开始时没有窗口焦点，直接注入 Tab 无效；改为先建立输入焦点并验证完整导航。滚动区按组件规则补齐 tab_index/tab_group。选择命中查询必须在鼠标移动后执行，才能使用该位置真实 hitbox；未给生产代码添加为测试工作的替代分支。

## 检查与边界

纯原生呈现和内存阅读状态修改，无协议、数据库 schema 或依赖变更。运行完整桌面 `cargo check --locked --manifest-path apps/desktop/Cargo.toml`、Windows MSVC 默认特性同等编译及上述原生走查；不新增 UI 单测，不重复无关全量 runtime suite。最终恢复生产入口运行 `cargo build --locked --manifest-path apps/desktop/Cargo.toml`。

本轮未重新生成分发 ZIP。Windows 实际图形界面、真实系统输入、完整文件差异/文件预览体验及整体 Cindy 对标仍未完成。
