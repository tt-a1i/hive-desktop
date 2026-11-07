# Windows 交叉检查与近期界面验证接线

## 取得的新证据

前一轮在 GPUI 构建 Windows manifest 时缺少 `llvm-rc`，尚未进入应用类型检查。本轮安装 Homebrew LLVM 23.1.5（及其依赖），只在检查命令中添加 LLVM 的 PATH，没有改全局 shell 配置、Cargo 依赖、默认功能或 GPUI manifest 要求。

Rust 1.98.0，macOS arm64 主机，`x86_64-pc-windows-msvc` 目标，以下命令通过了完整应用及依赖检查：

```sh
export PATH="$(brew --prefix llvm)/bin:$PATH"
cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc
```

- 应用基线 `abadeee`：通过，日志 `.artifacts/windows-cross-check/abadeee-llvm.log`。
- 补充外观按钮辅助技术状态后的应用：同一命令通过，日志 `appearance-toggle.log`。
- 未关闭任何默认桌面功能、没有替换资源编译器输出、没有用原型替代应用。

这只是 Windows 目标的 Rust 类型 / 依赖检查，不生成或链接 Windows exe，也没有运行 Windows GUI、PTY 或外部 CLI。

## 当前 CI 状态

已刷新并查看 [abadeee 的运行 42397660469](https://github.com/tt-a1i/hive-desktop/actions/runs/42397660469)。build、native-clis、native-supervision 三个 job 均未启动，注释明确指出账号付款或 spending limit 问题。这不是本轮代码编译失败的证据，也不是 CI 通过。

## 增补原生窗口检查

原有 Windows UI Automation 脚本继续只操作启动检查创建的隔离应用窗口和临时数据目录，未调用模型：

- 外观按钮原先只有视觉 `selected` 样式，现在显式提供 `toggled` 语义状态。组件库的 selected 不会自动添加辅助技术状态，Windows AccessKit 也会对这种按钮暴露 TogglePattern 而非 InvokePattern。
- 验证深色、浅色、跟随系统三种选择的实际配置文件，以及三个控件中恰有对应一项为 On。使用原生 TogglePattern 操作，不把保存文件单独当作界面成功。
- 当前会话查找从实际 Pi 历史中的隐藏 README.md 工具参数定位并展开；不存在的查询必须禁用前后导航；关闭查找后原中文草稿保持。结束前重新定位第一条消息，以便在虚拟列表可见区域操作工具组头。
- 截图包含主题和会话查找，上传路径改为 `target/windows-*.png`，同时补上之前遗漏上传的会话搜索、关联派单与后台恢复截图。
- 启动检查将隔离数据目录显式传给 PowerShell；确认最终保存系统模式。额外步骤的总超时从 90 秒调整为 120 秒，仍在原四分钟 job 上限内，没有添加固定等待来掩盖问题。

这些新增窗口检查**尚未在 Windows 运行**，不能列为通过项。

## 本机验证及边界

通过：

- 上述 Windows 目标 `cargo check --locked`。
- macOS `cargo build --locked`、`cargo test --locked`（15 通过，1 个子进程夹具忽略）、`cargo fmt --check`。
- `node --check scripts/check-desktop-startup.mjs`。
- 系统 Ruby YAML 解析器检查 workflow 语法，保留 PowerShell 脚本 UTF-8 BOM，避免 Windows PowerShell 5.1 把中文控制名按本地 ANSI 解码。
- 使用临时解压的微软官方 PowerShell 7.6.9 arm64 包，只调用 `Language.Parser.ParseFile` 检查脚本语法；没有在 Mac 执行 Win32 UI Automation。下载文件 SHA256 与官方 release API 的 digest 相同：`a1c7e98ac403da4be5ba75c1c5855af272de7787a91d478c4267466fe58fc1af`。解析通过不等于 Windows PowerShell 5.1 实际运行通过。
- 新的 macOS 开发包通过实际项目 / 会话启动恢复检查。

本轮只有一处辅助技术状态元数据变更及验证脚本更新，未改变 Agent 或 Hive 协议，因此没有重复所有模型 / 后端 suite。Windows exe 链接、运行、新增窗口检查以及真实用户交互仍待验收；正式签名安装包与完整产品目标也未完成。

更新的未签名 macOS 开发包为 `.artifacts/hive-desktop-macos-arm64-accessibility.zip`，zip 完整性检查通过；没有把 LLVM 或临时 PowerShell 解析器放进应用包。
