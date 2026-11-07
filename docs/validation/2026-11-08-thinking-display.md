# 原生思考过程展示（2026-11-08）

## 真实使用场景

1. 用户等待 Pi、Codex 或 Claude 回复时，后端已经产生 thinking 增量，但窗口只处理 text，既看不到过程，也会在纯思考历史中丢掉整条助手消息。
2. 窗口显示默认折叠的“思考中…”/“思考过程”行，可展开、选择和复制实际文本；最终正文独立展示，取消标识中断。历史重开后仍保留实际过程，查找可定位并展开对应段落。
3. 不做会造成后端有活动而界面没有对应可检查内容，恢复后的对话也缺少原始过程，达不到统一结构化聊天的目标。

参考 Cindy 固定源码 `4fd670b72af9d060785e29c553a14b6794dfe8eb` 的 [ThinkingCard](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/apps/desktop/src/renderer/components/chat/ThinkingCard.tsx)：默认折叠、紧凑摘要、左侧正文线和未公开状态。这里只借鉴展示层次，不复制其 React 状态、时间推断或持久化机制。

## 实现边界

- 复用四后端已有 `assistant_delta {kind,contentIndex}` 和原生 assistant content，Rust 现保留 contentIndex。没有新增模型请求、提示词、权限审批或协作协议。
- 以原助手消息 ID 和块序号维护独立思考段，正文仍走原 Markdown 路径。思考区按原块序号排列，使用已安装 GPUI Base 的 SelectableText 展示原文，不引入另一个 Markdown 解析及异步高度缓存。签名字段不作为正文；明确 redacted_thinking 只显示不可展开的未公开提示，不展示其 data 字段。
- 新段和历史段默认折叠。流式追加、最终快照替换使用同一消息，最终结果不会重复拼接；纯思考消息保留，不生成空的正文复制按钮。
- 文本块开始、最终消息或 settled 结束思考中状态；取消及 Codex 原生 hiveInterrupted 标记显示中断。没有准确时长来源，不显示虚构耗时，也不根据此标记结算 Hive 派单。
- 快照中的 activeMessage 经同一消息投影，再保留最后一个实际流式思考块状态。当前运行中的会话切换通过现有阅读位置记录保留用户展开选择；应用重开恢复原生历史，展开选择默认折叠，不新增磁盘 UI 状态存储。
- 会话内容查找覆盖思考正文，定位会展开命中段并通知虚拟列表重新测量。思考的流式和展开变化更新现有消息 layout_revision。

## 原生验证

隔离 GPUI 夹具使用真实 Hive HTTP/SQLite、SDK SessionManager 写入并重新读取的 Pi transcript，以及经过生产 serde 解析的合成 Agent 增量。不调用厂商或模型；测试配置仅选择隔离 `thinking-local/gpt-6.1-terra`。合成事件只能证明原生投影和交互，不能外推为四家真实模型新一轮端到端验收。

已观察浅色与深色流程：纯思考历史保留；多个段与正文分开；redacted 内容不进入展示正文；实际按钮鼠标事件展开；连续 thinking 增量拼接；最终消息替换正文和思考、保留展开状态；取消后标识中断；切换及进程重开恢复历史和中文草稿。最后增加“先折叠再查找”的检查，使用最终 SelectableText 展示实现重新运行浅色、深色和 760px 窄窗，各完成两次独立进程并通过。截图显示查找命中已展开内容，中文草稿保持。

夹具曾在持有 HiveApp 更新借用时主动绘制窗口，触发 GPUI 重入保护；已将焦点读取和绘制移出实体借用。生产代码没有相应兜底。截图复核后将思考头左对齐、移除展开时整行选中底色，并隐藏纯思考消息的空正文动作。测试和最终夹具源码保留在 `.artifacts/thinking/`。

外部适配器的既有定向检查通过：Codex 会话标准化、Claude UUID/块投影、外部 Pi 会话/恢复；这些检查各按自身范围提供证据，不等同真实四后端思考界面的厂商验收。最新 Windows x64 MSVC 类型检查通过，原生 Windows 窗口仍待验证。真实账号推理、系统文字选择/剪贴板、超长思考的资源性能和 Cindy 整体体验仍未验收。

## 生产包与定向检查

默认功能的生产二进制重新构建，确认没有 THINKING_NATIVE_OK、fixture-stream 或原生场景正文标记。冻结生产依赖打包后的 ZIP 已实际解压，包内二进制 SHA 与构建一致。bundled Node 22.23.3 完成真实 PTY/Hive HTTP/SQLite smoke，生产原生窗口完成保存项目与失效选择的恢复。新增思考 transcript 的种子写入/回读断言后，又运行一次该解压包的启动检查，通过；这不等同通过生产包完成一次真实模型思考推理。

- ZIP：`.artifacts/hive-desktop-macos-arm64-thinking.zip`（未签名、未公证）。
- ZIP SHA-256：`10ec1359e38c12b1c558deda2df34407426abb948d2e2e2787d19e8660700926`。
- 二进制 SHA-256：`339646201b5c0f5aa7a724c1f32a432c1348ffbfd0658af09b8a882f0f1f242f`。

实际通过：`pnpm check`、`cargo fmt --check`、默认 `cargo build --locked`、`cargo test --locked`（18 passed、1 ignored）、Windows x64 MSVC 默认功能 `cargo check --locked`。外部投影定向命令为 `tsx checks/codex-session-normalization.ts`、`tsx checks/claude-history-synthetic.ts`、`tsx checks/external-pi-session.ts`。三种原生变体各完成两次进程，最终 SelectableText 版本含先折叠再查找并展开的断言。

Windows 的既有原生检查新增思考 transcript、按思考正文搜索、等待展开/复制控件及截图步骤。公共 Node 脚本通过语法检查，种子历史与 Mac 启动实际通过；本机未安装 PowerShell，Windows 步骤未执行，不能称为 Windows GUI 通过。

没有新增或删除 Rust/Node 测试文件，没有修改 Hive、模型权限或投递队列。已有启动检查增加一条真实 transcript 回读断言，Windows UI 脚本增加对应操作。此轮主要为原生展示和消息投影，以默认构建、原生交互、适配器投影和打包检查为范围，未运行全量 Node suite。真实四后端模型验收、系统文字选择/剪贴板、Windows GUI 与正式发行继续待完成，完整目标保持开放。
