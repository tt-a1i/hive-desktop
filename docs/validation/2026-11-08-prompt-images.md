# 图片传输与队列保真验证

日期：2026-11-08。范围：后端能力基础，尚不是可从桌面发送附件的交付。

## 真实场景与实施顺序

用户给主控或成员发送截图时，当前输入与 Hive user-input 链只携带文本，适配器也只构造文本内容。完整结果应是原图进入选定后端，取消、失败、排队和重启保持对应字节与来源；界面能选择、预览并恢复草稿附件。只做上传按钮无法证明上述行为。不做原生内容块和可靠投递，界面附件会停留在文件路径提示或丢失图片。

本阶段实现四后端和持久队列，后续继续通过现有 Hive 用户消息和工作区上传接界面。当前没有改变 Hive 协议、数据库 schema 或用户认证配置，也未添加一个绕过协作语义的图片发送入口。

## 已实现

- 内部 PromptImage 为 PNG/JPEG/GIF/WebP 的 base64 内容块。桌面层限制每张编码后 5,000,000 字符、合计 20,000,000 字符、最多 10 张；这是应用传输限制，不是所有厂商/模型的承诺。
- 检查规范 base64、MIME 与文件头匹配，创建独立快照；不联网下载、不按文件扩展名猜格式。该检查不是完整解码器或尺寸校验。
- DeliveryQueue 持久保存图片，重新打开验证旧 spool 与新图片内容，拒绝损坏图片或会丢失图片的 context-only 投递；pending 返回深拷贝。文件写成功后才接受进内存，沿用现有重试及完成确认。
- StructuredSessionHandle.deliver 可携带图片，Chats 将图片传入同一个真实持久队列，再进入选定后端；文本调用保持兼容。
- 内置 Pi 和外部 Pi 原生扩展通过内容数组发送图片；普通 RPC prompt 也携带 images。模型未提供图片能力时提前拒绝，外部模型公开摘要仍只包含原有身份字段。
- Codex turn/start 构造 data URL 图片项。Claude SDK 构造 base64 image source；两者历史投影将内嵌图片规范化，同时保留完整原生来源。

接口依据：固定安装的 Pi SDK/CLI 1.4.0 公共 sendCustomMessage/sendUserMessage/prompt 与 RPC 源码；[OpenAI 官方 app-server 文档](https://learn.chatgpt.com/docs/app-server) 的 image/localImage 输入项；[Claude 官方 Vision 文档](https://platform.claude.com/docs/en/build-with-claude/vision) 的 image source 和平台限制。实际模型是否接受、缩放或解读图片仍属于后续厂商端验收。

## 验证

`PATH=/opt/homebrew/opt/node@22/bin:$PATH pnpm --filter @hive-desktop/runtime check:images` 通过：

1. 图片规范化、MIME/文件头错配、非法 base64、数量/单张/合计上限；调用后修改原对象或 pending 快照均不改变队列原图。
2. 真实 spool 文件重开并投递相同字节，损坏 spool 和 context-only 图片明确拒绝。
3. Codex 适配器通过真实 JSONL 子进程连接合成 app-server，断言线上 data URL 字节、历史投影及成功确认；不是原始 Codex CLI 的图片推理。
4. Claude 适配器通过官方 SDK 和合成 CLI，断言真实 SDK 输入、原生格式 transcript 文件及完成关联；不是原始 Claude Code 的图片推理。
5. 实际 Pi SDK/provider 编码连接仅监听 127.0.0.1 的 HTTP 对端，对端记录图片后返回 400。验证原始字节、失败后未确认完成、持久历史重开、纯文本模型拒绝前不写历史或 HTTP。
6. 真实 Hive HTTP/SQLite 元数据与 Chats 结构化端口接入实际 SDK/队列，传入图片后本机对端收到原字节，失败队列保留一项。该端口输入使用合成 run 标识；不是从桌面 chat.prompt 经 Hive user-input 的完整用户流程。

`check:images-native-pi` 额外使用本机原始 Pi CLI 1.4.0 和实际桥接扩展，验证同一失败/恢复链路。所有 Pi 测试模型身份均为 gpt-6.1-terra，仅本机合成 HTTP 请求，没有厂商网络推理或真实用户凭据。全局模型默认文件保持原字节。

定向回归：check:team、check:codex、check:claude-interactions 通过，覆盖真实协作队列、历史投影、错误与原生交互。未改 Rust/UI，未重复原生窗口或 Rust 测试；未跑全量 Node suite。Windows workflow 已加入 check:images，仅接线与 YAML 检查，尚未在 Windows 运行。

初版结构化端口夹具缺少 worker role，实际 Hive 返回 400；补上合法 role 和 autostart:false 后通过。失败证据保留于 `.artifacts/prompt-images/fixture-invalid-role.log`，未为夹具修改产品默认行为。

本机检查日志位于 `.artifacts/prompt-images/`。临时测试目录和合成凭据均隔离，测试结束清理自己的目录。

冻结生产依赖重新部署的 Mac 开发包 `.artifacts/hive-desktop-macos-arm64-image-transport.zip` 通过 bundled Node 的编译图片模块检查、真实 PTY/Hive HTTP/SQLite smoke 和原生启动恢复，ZIP 完整性检查通过。原生二进制与前一提交保持相同，未签名、未公证；该包依然没有桌面附件入口。

二进制 SHA-256：`96f1ff5a4135d12090ee3c6475eb7cc9e8b6edb5fcacc79d0ca659e43d967d33`。
图片模块 SHA-256：`b8c7b71a8813e5ffc76414017dba8acd26865e8b62a951b197b25402d7f309f4`。

## 必须继续完成

- 复用 Hive 工作区上传，将附件身份持久关联到用户消息及恢复投递；当前 user-input/恢复文本契约尚未扩展。
- public chat.prompt、桌面选择/拖放/粘贴、预览、移除、附件草稿与失败恢复，图片历史缩略图。
- 原始 Codex、Claude 的实际图片推理；内置/外部 Pi 的真实 Terra 识图；平台和账号验收分别记录。
- 完整图片解码/尺寸约束、GIF 首帧行为、模型额外限制、大图和长图片会话的内存/性能验收。没有自动缩放或图片转换。

完整目标保持开放；本阶段不算用户附件功能可用。
