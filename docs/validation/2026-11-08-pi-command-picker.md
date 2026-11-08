# Pi 会话命令目录与选择

对应 [#3](https://github.com/tt-a1i/hive-desktop/issues/3)。在原生输入调用语义修复后，统一聊天输入区新增当前 Pi 会话的技能、提示模板及扩展命令选择，不引入自己的资源扫描和展开器。

## 行为

- 输入 `/` 自动打开，继续输入按名称及描述筛选；输入区 `/` 按钮也可打开，刷新重新读取当前原生会话已加载的目录。
- 内置 Pi 读取公开 session extensionRunner / promptTemplates / resourceLoader，外部 Pi 调用原生 RPC get_commands，并仅过滤该子进程的 Hive 私有 bridge 命令。
- 菜单显示命令种类和说明，tooltip 保留原生来源路径。只列实际已加载资源，不把另一项目的本地资源混入。
- ↑↓ 选择、Enter 补入草稿、Esc 或点击外部关闭；选择不会自动发送。补全保留原有参数、正文、附件和撤销历史；多行参数前补足原生命令解析需要的 ASCII 空格。
- 菜单浮在输入框上方，通过 GPUI anchored snap_to_window 避让窗口边界，列表单独滚动。加载/错误/无匹配状态分别显示；刷新期间旧条目不能点击。无匹配时 Enter 沿用原有发送语义，不强行执行某个菜单项。
- 切换会话/工作区或重启连接会清空目录和菜单，异步结果核对会话、打开请求及连接代次。Codex/Claude 本轮不显示该 Pi 目录入口，不伪造跨后端命令兼容性。

## 当前证据

新增目录行为断言先因 chats.commands 缺失失败，实施后通过：真实 Pi SDK/原样外部 Pi CLI 1.4.0 + Hive HTTP/SQLite，校验项目技能/模板/命令的实际 sourceInfo 文件路径、隐藏私有命令、第二工作区不会继承本地资源、完整重启后目录恢复。原有 18 请求输入语义检查一并通过，无厂商调用，模型 ID 保持 gpt-6.1-terra。

目录路径首次检查还发现 macOS /var 与 /private/var 规范路径不同；测试改为 realpath 后核对具体资源文件，生产保留真实原生 sourceInfo，不改写资源来源。

Mac 原生 GPUI 夹具读取上述真实状态，通过窗口键盘/点击链路验证筛选、Enter 补全不发送、点击保留已有参数、方向键实际改变选择、Esc 保留输入，以及多行中文参数。浅色 1180×820 和深色 760×583 分别运行，截图在 `.artifacts/pi-user-input/*commands*.png`；同目录保留 `catalog-contract.log`、`*-commands-receipt.json`、Rust 与 TypeScript 检查日志。该选择器为源码原生窗口验证，尚未对打包后的新版本做系统鼠标/输入法逐项验收。

同日直接查看已安装 Cindy CN 0.1.128 的原生 UI `/` 菜单，参考其即时筛选、选择与说明布局；临时输入已清空，无消息发送或模型调用。没有将其云技能市场、账号或权限规则带入 Hive。

## 边界

会话原生资源更新须沿用原后端实际加载机制；“刷新”读取当前已加载目录，不承诺把磁盘新安装资源自动 reload。扩展命令可能需要尚未接入的原生扩展 UI，原生执行错误仍按既有投递错误显示。后台持久化故障注入、compaction 来源专项检查、Windows 原生交互和新开发 ZIP 仍待完成，#3 保持开放。

最终源码验证：`pnpm check`、`check:pi-user-input`、Mac `cargo build --locked`、现有 Rust 检查（24 通过/1 忽略），以及 Windows MSVC `cargo check --locked` 均通过。未改 Hive 协议/schema，也未重复无关全量运行时测试；新增核心目录检查复用已验证的真实输入链路，纯 UI 使用原生窗口走查，没有新增 UI 单测或删除现有测试。

重新查询 GitHub 确认 [Windows run 42462561419](https://github.com/tt-a1i/hive-desktop/actions/runs/42462561419) 对应 a2744ca：build/native-clis/native-supervision 三个 job 都为零步骤，annotation 明确说明近期付款失败或额度需要提高。该失败不是 Windows 编译/原生运行已经执行的证据；本轮交叉类型检查不能替代这些检查。

后续持久化/压缩来源检查及新开发 ZIP 的实际系统输入验收已完成对应 Mac 范围，见 [历史/持久化修复](2026-11-08-pi-history-persistence.md) 和 [Pi 开发包](2026-11-08-pi-native-package.md)。Windows 等未验收项继续保留。
