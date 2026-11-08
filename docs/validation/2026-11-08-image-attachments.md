# 桌面图片附件与恢复

日期：2026-11-08。后续粘贴/拖放与原图查看进展见 [图片输入验证](2026-11-08-image-input.md)。本轮接通用户输入与桌面原生显示，前置适配器证据见 [图片传输](2026-11-08-prompt-images.md)。完整产品目标仍开放。

## 场景与可观察变化

用户给主控或成员发截图时，原先桌面只能发文字。现在可以选择本地图片、看缩略图、移除，并将图片与文字或图片单独发送到原会话。切换会话及退出重开可恢复附件草稿；发送前失败保留正文和图片。没有这条链，后端的图片能力无法由桌面实际使用。

## 数据与边界

- 使用 Hive 现有工作区 upload HTTP、SQLite 记录和受管理文件，新增桌面方法 chat.attachments.add/get。上传后修改原文件不改变快照。此次未修改 Hive 源码、pnpm Hive 补丁或数据库 schema，也未创建另一套附件 store。
- chat.prompt 接受最多 10 个不同上传 UUID，发送前校验工作区与图片。内置/外部 Pi 在已知模型不支持图片时提前拒绝。图片大小限制沿用每张 base64 5,000,000 字符、合计 20,000,000 字符。
- 引用通过现有 user_input.text 的末尾版本元数据保存：换行两次后 `<!-- hive-desktop-images:v1 ["上传 UUID"] -->`。原生执行器仅解码显式 user 来源，校验上传归属后，将原始字节送入同一个持久投递队列；清理显示正文并保留原始 sourceText 与附件身份。Hive 附加上下文不丢弃。
- 保留格式按一层解析。桌面正文若本身含标记，编码器添加外层空引用保护用户原文。直接从 Web/HTTP 手写保留格式会触发原生解释；Web 自身没有对应的图片展示投影。本轮不是完整跨 Web/桌面附件协议。
- 原生界面解码 PNG/JPEG/GIF/WebP 并生成最多 144×112 缩略图，限制宽高各 8000、解码分配 64 MiB。预览在后台线程生成，模型收到上传原字节，没有使用缩略图替代原图。实际本轮图片夹具为 PNG，其他格式尚无桌面逐种走查证据；GIF 动画不作为本轮能力。
- 草稿 JSON 只加可选附件引用，旧草稿兼容。每个工作区/会话独立保存；只有后台确认接受才清除已发送引用。未知确认、跨存储崩溃窗口仍需用户核对历史，不承诺恰好一次。移除草稿图片不会删除共享的 Hive 上传，未新增文件垃圾回收策略。

## 当前验证

1. `pnpm --filter @hive-desktop/runtime check:attachments`：真实 Hive HTTP/SQLite、实际 Pi SDK、持久队列及本机拒绝对端。覆盖上传原文件修改隔离、外工作区拒绝、非法类型/路径/ID、元数据转义、数据库 user_input 正文关联、原图模型输入、失败队列/历史重开和图片单独发送。生产 JSONL 另覆盖上传/读回、重复 ID、空输入、非字符串输入、纯文本模型拒绝时不写用户历史及正常关闭。
2. 生产 JSONL 检查也在新 Mac 便携包内的实际 runtime/dist/main.js 上通过，使用包内冻结依赖；不是只测源码导入。
3. 三组实际 GPUI 应用夹具：浅色宽窗、深色宽窗、760×820 窄窗，每组独立启动三次。阶段一上传、缩略图、中文正文、跨会话隔离并退出；阶段二重开恢复，暂时移走该夹具独占的受管理图片复现发送失败，确认正文/引用保留，再恢复原文件，以真实 GPUI Enter 事件发送图片单独消息；阶段三重开确认历史缩略图和 paused pending，没有自动重发。每组本机 HTTP 记录均为一次，线上图片 SHA-256 与原图相同。
4. 上述应用使用复制源码和临时 GPUI 显式帧渲染；有实际应用状态、组件布局、截图和按键事件，但没有物理鼠标、操作系统图片选择器或 Windows 窗口证据。驱动和截图保留在 `.artifacts/image-attachments/fixture-source` 及同目录 native-* 日志、receipt-{light,dark,light-small}.json 和各阶段 PNG。临时驱动不进入生产二进制。
5. `check:images`、`check:team`、`check:codex`、`check:claude-interactions` 定向回归通过；其模型对端均为合成/本机检查，不是新增厂商推理证明。本轮也重新运行 check:images-native-pi，原始外部 Pi 1.4.0 CLI 的字节、失败与恢复检查通过。
6. `pnpm check`、Mac `cargo build --locked`、Rust 应用检查 16 通过/1 子进程夹具入口按设计忽略、完整默认功能 Windows MSVC `cargo check` 通过。Windows workflow 加入 check:attachments，但尚未取得这一提交的新 Windows 原生 CI 或窗口结果。
7. 冻结生产依赖的 Mac 开发包打 ZIP 后重新解压，包内生产 JSONL 上传/读回、bundled Node、真实 PTY、Hive HTTP/SQLite smoke 及原生应用启动/会话恢复均通过；仍是未签名开发包。

截图复核发现并修复图片单独发送的空文字气泡，以及失败回合重开后只剩 Agent 名称的问题。错误正文现在从原始 assistant.errorMessage 显示，并保留失败前的部分回复；三组原生夹具均补验错误显示、图片单独消息与部分正文保留。

本地证据目录：`.artifacts/image-attachments/`，未提交生成的图片、临时配置、凭据或运行状态到 Git。

## 真实模型尝试与失败记录

明确选择内置 Pi 的原生 `openai/gpt-6.1-terra`，上传自行生成的左右蓝/粉 PNG，通过实际 Hive chat.prompt 进行识图。OpenAI OAuth 刷新返回 HTTP 400 `invalid_grant`；没有成功模型回复，未通过识图验收，也未回退到其他模型或 Magpie。失败事件保留在 `.artifacts/image-attachments/live-Gd1WLb/events.json`。已请用户重新登录；可用显式 opt-in 的 `checks/image-attachments-live.ts --live <蓝左粉右 PNG>` 重跑。

首轮 JSONL 夹具错误地假设“没有 credentials 就没有当前模型”，但 SDK 仍可选择默认模型，因此预期的同步拒绝没有出现。夹具已改成明确的本机、纯文本 Terra 能力模型，并断言本机请求数为零；没有为测试修改产品模型选择语义。更早草稿夹具写错目录、DB 文件名和原生驱动错误假设所有 provider content 都为数组的问题均在夹具修正，未用生产兜底掩盖。

## 尚未完成

- 真实 Terra 识图成功；原始 Codex/Claude、外部 Pi 的实际图片推理分别验证。Claude 仍受只使用 Terra 的测试模型约束，不改用 Claude 模型冒充验收。
- 图片粘贴、拖放、通用文件、原尺寸查看与下载/复制；当前仅本地图片选择及缩略图。
- 操作系统文件选择器真机交互、Windows 原生 UI、实际大图/长图片历史的资源与性能、正式签名发行。

没有将本机拒绝对端、截图夹具或构建通过记为完整附件/产品验收。

## 开发包记录

本地 ZIP：`.artifacts/hive-desktop-macos-arm64-image-attachments-final.zip`。

- ZIP SHA-256：`7bcfcd78b82d00124c9da05f5ed3693bf6351b82982f371af0ad9ca04cf7b3cd`。
- 原生二进制 SHA-256：`6e247480ae5519cfc6984d3b2e736ab6dadf7a61951809ca0b6848beffd4ada8`。
- 包内 image-attachments.js SHA-256：`41a18e022a07d93aeaf9e97ad784a5a79d069aab69512b85f1d38228b889d85e`。

本轮没有运行不分风险的全量 Node suite；变更集中于桌面图片输入、投影和草稿，Hive 协议/schema/核心协作逻辑未改，采用上述定向真实存储/传输与适配器回归。没有删测试；既有草稿测试仅适配新增可选附件参数，新增持久化检查覆盖旧文件兼容及图片单独草稿。Windows 新工作流需独立运行，不能由 Mac 交叉类型检查代替。


2026-11-09补充：原生Codex Terra已取得真实图片推理及完整runtime重开恢复证据，见[Codex图片实证](2026-11-09-codex-image-live.md)。本页此前Pi OAuth失败与系统UI/Windows限制按当时证据保留。
