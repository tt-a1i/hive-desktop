# 输入与错误修复：优化开发包

current/candidate 已更新到源码 `6edac4c0f1851459b721cabdc2eb2b60a8708597` 的 Mac ARM64 release 优化构建。ZIP **143,416,277 字节**，SHA256 `a475a4e6fd67ab03adcb4b9930ff10cd49dba93f2c97c50cc8c75386f168837d`。

- ZIP：`.artifacts/hive-desktop-macos-arm64-current.zip`，candidate 为同字节硬链接。
- 实际解压应用：`.artifacts/release-6edac4c/extracted/bundle/Hive Desktop.app`。
- 收据：`.artifacts/current-package/receipt.json` 与本轮 `.artifacts/release-6edac4c/receipt.json`。
- 仍是未签名、未公证的开发包，不是正式安装器或完整产品交付。

## 纳入的改动

纳入 7ae27ca 之后的错误修复入口、配置返回焦点、新建会话直接输入、暂停队列固定继续按钮、单层输入滚动、整体高度边界、一行最小输入与助手标题行操作。成员/工作流等既有能力保留，但本轮不冒充再次全量原生验收。

## 构建与包完整性

`cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` 通过，构建耗时 1m52s。锁文件、工作区配置、包元数据、补丁及 Rust 依赖声明与已验证基线无差异，因此复用同平台冻结依赖包，不宣称全新 deploy。

生成 ZIP 后独立解压，112 个 runtime 输出、28,863 个文件/模式条目、29 个内部链接逐项匹配；与前包唯一变更为原生可执行文件。原生二进制 SHA256 `8699d0dbae1970b0d4b5c1f9310b257ab5ef45194f0580e20400280c1cbabb44`。未设置源码 runtime/Node override，原生应用使用其包内后台与 Node。

包内 Node 执行 `check-package`、`check-packaged-models`、`check-packaged-dispatches`、`check-packaged-codex`、`check-packaged-remote`、`check-packaged-claude`，六组均 exit 0；各 gate 的真实 SDK/HTTP/SQLite/PTY 与合成协议对端范围沿既有脚本保留，不能解释为六个厂商真实推理或生产网关验收。

## 真实 Codex / Terra 与冷恢复

实际解压应用、Light/16 中文界面、新隔离项目，原始 Codex CLI 0.167.0、gpt-6.1-terra、Medium。

第一次原生 Send 要求读取 `values.json` 并求和。模型生成的 Python 对字典 values 求和，命令失败，却仍给出目标回复。UI 正确展示工具失败；保留 `live-first-failure.ax.txt`，不把该回复作为工具成功证据，也未为此修改产品代码或源数据。

第二次原生 Send 明确指定 `cat values.json`。真实 commandExecution exit 0，输出精确为 `{"values":[7,29,41]}`，助手回复 `RELEASE_6ED_READ_OK 77/3`。原生展开工具核对输入、项目 cwd 和输出，留下中文草稿 `包内未发送草稿 · RELEASE_6ED_UNSENT` 后正常退出；重开同一线程仍保留失败/成功工具、回复和草稿，再正常退出。

独立包内 runtime 冷读断言：两条精确用户输入、两个工具（一次失败、一次成功）、两个目标回复各一次、Terra/Medium、pending 0 / busy false、精确未发送草稿。两次原生进程 exit 0。见 `live-receipt.json`、`live-cold-snapshot.json` 与 `live-*.ax.txt` / PNG。本轮真实厂商输入数为 **2**，不能写成单次全部成功。

## 包内错误修复键盘流程

沿用隔离的真实 Pi SDK/回环 401 失败记录；本轮不重发、登录、改模型或运行新 Pi 推理。实际 Dark/20 约 592×456 窗口，滚到草稿，从输入框 ShiftTab 两次→账号设置→Escape→Tab/Return 打开模型设置；Escape→Tab 后可直接追加 x，再删除恢复原值。

包内独立 SDK/HTTP/SQLite 冷读通过：原请求/待处理 ID 和 spool、附件字节、配置哈希、精确草稿不变，pending 仍为原来的 1。本轮此流程新增提示/厂商推理 0，原生 exit 0。见 `repair-receipt.json`、`repair-models.*`、`repair-return.*`。

## 保留与边界

前版 7ae27ca 两个 ZIP 和收据已归档至 `.artifacts/release-7ae27ca/`，其实际解压应用继续保留。仅在 lsof 确认无引用后删除本轮重复 `base`，分配块统计清理 **538,415,104 字节，约 513.5 MiB**；源码、状态、失败证据、全部 ZIP 与上一可运行包保留。

Windows 仍只有前轮源码编译检查，未获得本包原生 GUI/CLI/安装验收；Claude 厂商推理、Pi 重新认证、生产远程/手机、完整视觉/键盘/辅助技术/输入法与性能矩阵、签名/公证和正式发行仍开放。未新增或删除测试，未跑全量 suite；本轮风险匹配检查为六组包 gate、实际原生流程及冷读断言。完整目标未完成。
