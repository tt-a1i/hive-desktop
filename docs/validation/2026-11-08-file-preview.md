# 从工具与汇报产物预览文本文件

2026-11-09补验：Mac成员产物入口实际打开202行中文文件，Home/End及双向翻页通过，鼠标选中一行后的Cmd+C与粘贴内容逐字一致。滚轮与完整平台矩阵未验收，见[原生长阅读证据](2026-11-09-native-resume.md#长预览键盘阅读与文件选区复制)。

## 使用变化

此前产物只能在文件管理器定位，Read/Write/Edit 等工具记录的文件路径没有原生预览入口。现在可从展开工具的路径或已有产物旁的“预览”打开只读窗口，查看磁盘中的 UTF-8 文本、重新读取、复制全文，或继续在文件管理器中定位。界面明确区分当前磁盘文件与工具执行时的历史内容，不伪造历史文件版本或差异。

沿用 Cindy 固定提交 `4fd670b72af9d060785e29c553a14b6794dfe8eb` 的 `ToolCallCard.tsx` 显式文件路径入口思路，使用现有 GPUI Dialog、SelectableText 和 Scrollbar，不新增桌面依赖或 Agent 权限审批。

## 来源与边界

- Pi/外部 Pi 的 read/write/edit、Claude 风格 Read/Write/Edit/MultiEdit 等显式 `path`/`file_path` 输入提供路径入口，不解析任意 shell 命令猜路径。
- Codex fileChange 保留原生 `changes[].path`；更新含 `kind.move_path` 时使用移动后的路径。字段按原始 Codex 0.166 生成的 `FileUpdateChange`/`PatchChangeKind` 类型核对。不是对任意 CLI 文件操作的自动识别承诺。
- 继续使用产物现有的项目路径校验，支持项目内相对/绝对路径与内部符号链接，拒绝项目外目标、远程/设备路径；不是新权限系统。
- 后台只读普通文件，最多读取 1 MiB + 1 字节以明确拒绝超限，UTF-8 解码失败和二进制 NUL 内容明确提示，不静默截断或把错误当空文件。空文件单独展示。
- 显式重新读取成功后回到文件顶部；失败保留上次成功的内容并提示，恢复文件后可重试。不会修改历史工具结果或源文件。

## 展开时的阅读位置修复

窄窗口走查实际复现：原内容较短时，列表留在尾部 sentinel；工具展开后视口仍停在尾部，文件入口被裁到首屏上方，点击不会到达。仅暂停自动跟随不足以修复。

复用原来的 ReadingAnchor，在用户展开/收起时暂停跟随；没有具体阅读锚点时以点击的消息为锚点，已有阅读位置则保留。工具、工具分组、思考、消息原文和历史错误详情共用入口，不改变自动流式更新或“回到最新”的动作。

## 验证

文件读取测试使用真实临时文件：中文/空格路径、CRLF/Unicode 原文、空文件、缺失、目录、非法 UTF-8、NUL 二进制、恰好 1 MiB 及有效 UTF-8 超限、项目外绝对路径和内外符号链接。超限使用有效文本，避免因二进制拒绝而掩盖大小检查。

真实 GPUI + Hive/Pi 原生会话文件预置 Read、Claude 风格 Read、Codex fileChange 形状及汇报产物，操作真实磁盘文件；这些是 UI 历史夹具，不是本轮原始 Claude/Codex 厂商推理或真实团队汇报的证明。浅色常规、浅色 760px、深色 760px 检查：工具/产物实际点击、长文件键盘滚动、实际内容与历史工具结果不同、磁盘更新/删除/恢复、缺失转空文件、原生 move_path、二进制提示和 Escape 关闭。原工具结果未被覆盖。收据与截图在 `.artifacts/file-preview/`，模型配置仅 gpt-6.1-terra，无厂商模型请求。

工具与文件预览共用 text_reading 组件；另复跑工具详情原生选择/滚动/更新位置与会话恢复回归。没有读写用户系统剪贴板做复制验收，也没有用程序调用实际唤起 Finder/资源管理器。

运行 `cargo test --locked --manifest-path apps/desktop/Cargo.toml`（21 通过、1 忽略）、`cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`（LLVM PATH）及原生窗口走查。没有改 Node runtime 协议或 SQLite schema，不重复无关 Node 全量测试。没有新增 UI 单测，新增测试只覆盖文件读取行为。最后的产物路径对齐改动另跑常规浅色原生流程，生产入口随后重新构建恢复。

Windows GUI、物理输入/复制、原始外部 Agent 文件工具真实推理、完整 Diff/编辑体验仍待验收。开发包更新到 `.artifacts/hive-desktop-macos-arm64-file-preview.zip`，复用已验证冻结依赖，逐一核对 104 个 runtime 文件与当前产物一致；打包与解压运行收据保存在 `.artifacts/file-preview/`。它仍是未签名的 macOS arm64 开发包，不能作为 Windows 或正式发行验收。

从新 ZIP 单独解压后运行 `node scripts/check-package.mjs --bundle PATH` 和 `node scripts/check-desktop-startup.mjs --bundle PATH` 均通过，分别穿透内置 Node/真实 PTY/Hive HTTP/SQLite/关闭，以及实际原生窗口项目与会话恢复。未使用夹具启动入口，没有模型请求。
