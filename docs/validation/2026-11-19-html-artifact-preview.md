# 隔离 HTML 产物预览

`fd1eac1` 为派单报告和会话工具的 HTML 文件提供“交互预览”。已有文本预览仍可读取源文件。首次打开交互预览时，Hive 沿用原派单或会话的执行目录校验，保存 2 MiB 以内的 UTF-8 副本。同一工作区和同一来源地址始终重开该副本。换名、修改或移走原文件不会改写已经保存的内容。

交互预览使用独立原生窗口与临时网页存储。页面运行在只允许脚本的沙箱框架内，没有 Hive 调用入口。支持自包含 HTML、内嵌脚本和样式，以及允许的内嵌图片、字体和媒体。外部站点、文件 URL、表单和宿主文件选择受限。关闭窗口释放 Rust 所持的网页实例。重开恢复文档，页面中的计数或表单等运行状态重新初始化。

## macOS 原生包

实际打包程序组装 arm64 开发包，使用冻结运行时和官方 Node。用真实 Hive SQLite、派单、显式报告及附件路径填充隔离项目，随后从报告卡打开文件和交互预览。两次原生启动均经过以下检查。

- WebKit 页面按钮实际改变计数。宿主框架和网页存储读取失败，Hive 调用入口及 WebRTC API 不可用。弹窗、本机假文件读取、文件选择、回环 HTTP 请求与图片加载受限，尝试跳转后原页面仍可交互，回环服务没有收到请求。
- 在第二次原生启动前移除源文件。仍能从原报告打开同一快照，再次操作计数。实际关闭预览和主窗口后，两个桌面进程均以 0 退出。无模型调用。

证据位于 `.artifacts/html-preview-mac-NbFliE/native-hardened/receipt.json`、`first-open.ax.txt`、`cold-reopen.ax.txt`、对应原生日志和 `.artifacts/html-preview-mac-NbFliE/package-smoke.log`。该包通过真实 PTY、Hive HTTP/SQLite 和退出冒烟，并通过全部 53 项包内功能检查。结果见 `.artifacts/html-preview-mac-NbFliE/functional-receipt.json`。

## 来源与平台

真实 Git 与 Hive 回归覆盖同名项目和工作树文件、跨工作区拒绝、重复捕获、工作树移除后的冷恢复、大小和编码校验，以及已存副本的摘要检查。类型检查、运行时构建、macOS 构建及 Windows 交叉编译通过。本机 Rust 构建使用单作业。

Windows 版使用 WebView2，并在载入产物前设置资源与子框架导航拦截。`aca54bb` 解压包在原生窗口从真实派单报告打开 HTML，UI Automation 在由 Hive 进程拥有的 WebView2 窗口内将计数从 0 点到 1，再关闭预览。同一包通过包括 `html-artifacts` 在内的 50 项运行时检查。Windows 上的冷重开和 macOS 原生验收中的各项隔离探针仍需对照验证。早期作业曾因进程监督夹具失败，调整后该作业通过，见[听写与进程监督验证](2026-11-19-voice-input.md)。没有安装到用户目录或发布开发包。[Windows CI](https://github.com/tt-a1i/hive-desktop/actions/runs/44363997413) · [功能收据](https://github.com/tt-a1i/hive-desktop/actions/runs/44363997413/artifacts/14458378371)

## 工作树移除后冷重开

源码专项先在独立工作树中保存 HTML 快照，再改动源文件并把新提交推送到本地 bare 仓库。Hive 原生工作树移除接口核对远端后移除了工作树。后台冷启动后快照仍是首次打开的版本，实时文件入口确认原工作树已失效。

macOS 签名 ZIP 解包实测使用真实 Hive 派单和报告。项目根目录和成员工作树有同名 HTML，成员报告指向工作树。首次预览后正常关闭应用，用 Git 移除成员工作树，保留项目根目录同名文件。冷重开仍从原报告进入同一工作树快照。两次都能操作计数并通过隔离探针，回环服务收到零个请求。两次窗口及其后台均正常退出。

Windows 构建作业的原生验收现在也包含回环请求、图片、文件选择和导航探针。它会关闭窗口，移除源文件，再用第二次启动核对报告和快照，最后保存进程退出及回环计数收据。Node 和 PowerShell 语法检查通过。新路径仍待 Windows 原生运行。[Windows 窗口验收脚本](../../scripts/check-desktop-startup.mjs)

本轮证据位于 `.artifacts/html-cold-reopen-nJbNHQ/`，包括 `native-worktree-proof/receipt.json`、两次辅助功能树、`html-runtime.log`、`artifact-origins.log`、ZIP 清单和包内冒烟。

## 同名工具产物的独立预览

GPT-6.2 Sol 用生产运行时代码复现了一个问题。同一会话的两次工具调用依次写入 `report.html` 时，第二张工具卡打开了第一张工具卡保存的 HTML。`55a8310` 为每份快照记录原始会话、工具调用和路径。分叉继承的工具调用继续使用原始会话。旧版只记录路径的快照可以从独立的“旧版路径预览”入口打开。打开已存快照时，文件详情仍显示实时源文件不可读的原因。

快照记录首次打开交互预览时磁盘上的内容。如果文件在首次预览前已经被覆盖，工具卡只能捕获覆盖后的版本。

- macOS arm64 的本地签名 ZIP 解包后，从真实 Hive 会话中两张已保存的离线 Pi 工具卡分别打开初稿和终稿。删除源文件并冷启动后，从原工具卡恢复初稿和终稿，并打开旧版路径快照。三份快照的摘要各不相同。实际 WebKit 窗口均能操作计数，回环服务收到零个请求。两次正常退出后，受测桌面和 Node 进程均已结束。第一次运行的验收脚本用原始 HTML 文字判断预览是否关闭，产生误报。脚本改用原生窗口的辅助功能标题并重新完成验收。没有模型调用。
- 包内 Node 与冻结依赖通过 HTML 快照和 Codex 分叉来源两项回归。便携包冒烟通过，包括真实 PTY、Hive HTTP/SQLite 和退出。Rust 测试通过 127 项，1 项忽略。Windows MSVC 目标通过交叉编译检查，当前改动仍待 Windows 原生窗口验证。

证据位于 `.artifacts/html-tool-versions-PONINH/`，包括 `native-window-iHhWwZ/receipt.json`、各版本的原生辅助功能树、`packaged-functional-receipt.json`、`package-smoke.log`、签名和归档清单。ZIP 的 SHA-256 为 `216405ffbc7a95d34c82825ba6428df393b2fc4ae2deea1351852a4101f32dd9`。
