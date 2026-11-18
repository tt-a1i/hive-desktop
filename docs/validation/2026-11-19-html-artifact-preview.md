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

Windows 版使用 WebView2，并在载入产物前设置资源与子框架导航拦截。CI 增加了从真实报告打开 WebView2、验证隔离并操作按钮的原生检查。主作业仍在运行。独立进程监督作业失败于 `leader_exit_is_detected_while_descendants_hold_stdout` 的强制回收断言。该夹具已调整，详见[听写与进程监督验证](2026-11-19-voice-input.md)。没有安装到用户目录或发布开发包。[Windows CI](https://github.com/tt-a1i/hive-desktop/actions/runs/44358925746)
