# 工作区辅助终端

基线：`e1dde8c`，2026-11-07。此文是当前实施范围，未通过的项目不能据此称为已实现。

## 真实问题

1. 用户在桌面与团队协作时，需要在项目目录运行命令、查看交互式程序或中断 Shell 命令，目前必须离开 Hive Desktop。Hive Web 已有 Shell 和终端能力。
2. 完成后，可在当前工作区显式打开真实 Shell，输入命令并读取 ANSI 终端屏幕；切换后重新连接同一运行可恢复屏幕，明确停止后进程退出。
3. 不做这项，桌面无法独立完成已有 Web 的日常项目操作，已确认的辅助终端目标也缺一环。

## 当前范围

- 当前工作区的一个 Shell 面板，提供启动、连接已有 Shell、停止。隐藏面板只断开 viewer；停止才结束 Shell。
- 复用固定 Hive 2.2.1 的 Shell HTTP 接口和双 WebSocket 终端协议，不重新实现 PTY，不接管结构化 Agent 的原生终端。
- Node 侧保存现有 Hive UI 认证并连接终端流；Rust 侧只接收本地 sidecar 事件与响应，不取得认证 cookie。
- 保留原始终端字节、restore 顺序和流控。界面在实际终端 parser 消费后 ACK，不能以进入未消费队列作为已消费。
- attachment、工作区与 run 身份共同约束输入和异步事件。工作区移除、后台退出和停止释放对应 viewer；其他工作区继续可用。
- 不增加多窗格、搜索或新的 Agent 调度语义。

## 组件选择

当前固定 `gpui-kit 0.9.0` 使用 `gpui-pre 0.3.11`，组件库没有可直接使用的终端。采用已发布的 [gpui_xterm 0.2.1](https://docs.rs/crate/gpui_xterm/0.2.1) 作为最小源代码适配基础，保留 MIT 许可、来源提交和修改说明，使用其 `alacritty_terminal` 屏幕/ANSI 处理。原包依赖旧版 Kit，必须编译适配并实际验证，不能将源码具备相关接口等同于可用产品。

本地组件不启动 PTY、不拥有后台读取线程，只暴露同步 feed/reset/resize 和输入事件。GPUI 负责组合输入、候选框位置、选择与用户主动复制粘贴；默认不处理终端输出请求自动修改剪贴板。macOS Cmd 快捷键、Windows 操作和 bracketed paste 属于实际验收范围。

便携开发包随应用资源一并包含 `licenses/hive-terminal/LICENSE` 和 `NOTICE`，保留这份源码的许可及修改来源。

## 验证

- 真实 Hive HTTP/SQLite/PTY：项目 cwd、输入输出、UTF-8、ANSI、resize、Ctrl-C、自然退出与显式停止。
- 重新 attach 的 restore 与后续输出有序；两个工作区/旧 attachment 不串线，隐藏面板不结束 Shell。
- 原生窗口：颜色、光标覆盖、备用屏幕、方向键/Tab、中文 IME、选择与复制粘贴、窄窗口。
- macOS 与 Windows 分别记录；构建及 ConPTY 检查不代替 Windows 原生窗口/IME 验收。
- 无需模型请求，不使用 Magpie，不改用户全局 Agent 设置。若后续有模型验收，仍只使用原生 `openai/gpt-6.1-terra`。

后端、组件和宿主 UI 分别实施；完成前进行四角度独立复核，修复问题后保留真实运行收据。

## 实施中确认的恢复边界

固定 Hive 2.2.1 的 IO 与 control 是两个连接。先接通 IO、让真实 Shell 输出唯一标记并确认收到，再连接 control，旧协议返回的快照也包含同一标记。此时简单地在快照后重放等待期间的实时输出会重复。这一触发已通过真实 Hive/PTY 和原生 WebSocket 确认，先前持续输出时未观察到重叠的运行不能否定它，也不能估算日常发生频率。

因此桌面侧采用显式选择的快照边界：在建立当前 IO viewer 时捕获它的快照，随后新输出只走实时流；control 取走同一 viewer 的快照。原 Web 客户端默认协议不变。修复需同时验证边界前的历史保留、两连接之间的输出只出现一次、恢复后的实时输出不丢，不能通过丢弃全部等待输出或匹配文本去重来实现。
