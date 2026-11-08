# 模型配置表单草稿

任务：[#4](https://github.com/tt-a1i/hive-desktop/issues/4)，完整产品目标继续实施。

## 实现

“模型与接口”的当前编辑表单复用现有 450ms 延迟保存与退出 flush，持久化到桌面数据目录中的 `desktop-model-drafts.json`。字段、接口类型、思考/图片开关及尚未填写完整的数字均按表单文本保存。未修改表单不新增草稿。

草稿按实际 models.json 路径隔离，保留原始 revision、原始字段值和当前编辑值。重新读取配置后自动恢复；配置已有外部修改时明确提示，保存继续经过原有后端 revision 冲突检查。不会自动套用新基线覆盖配置。实际配置路径改变时拒绝提交旧表单。

正常保存与明确放弃清除对应草稿，保留其他路径的草稿。读取、解析、临时写入及原子替换失败均返回错误；丢弃失败不替换当前表单。实际配置已经保存但草稿清除失败时明确说明两者状态。使用现有 tempfile，不增加依赖；不涉及 API Key 字段、登录交互或全局模型默认。

## 验证

- `cargo check --manifest-path apps/desktop/Cargo.toml` 通过。
- `CARGO_INCREMENTAL=0 cargo test --manifest-path apps/desktop/Cargo.toml settings::pi_models::draft::tests -- --nocapture`：3 项通过。实际文件往返保留原基线/未完成数字/能力与多路径草稿；损坏文件不会被改写；macOS 真实只读目录导致保存和丢弃失败，旧文件字节不变且可重新读取。
- 原生 GPUI 验证使用当前源码的隔离副本、真实 runtime 和独立 Pi models.json，无模型推理。窗口填写中文显示名称、接口地址和首个模型，点击 Responses，等待真实自动保存并正常退出；完整新进程恢复字段与接口类型。修改配置原文后完整重启显示外部修改提示，点击保存触发真实后端冲突，表单和草稿仍保留；明确放弃后新进程不再恢复。
- 同一原生检查另验证成功保存实际写入 models.json、清除当前草稿，完整重启不再恢复。收据、日志与恢复/冲突截图：`.artifacts/pi-model-drafts/`。
- 验证初始使用 `{}`，实际配置读取按现有约束拒绝缺失 providers，未达到模型表单检查；改为独立有效配置后重跑。未为此修改生产校验。
- `cargo test --manifest-path apps/desktop/Cargo.toml drafts::tests -- --nocapture`：原有聊天、任务、附件草稿 3 项回归通过。
- 最后 `CARGO_INCREMENTAL=0 cargo build --manifest-path apps/desktop/Cargo.toml` 通过，重新构建生产源码，替换根目录构建输出中的验证专用二进制。

## 边界

原生窗口检查通过 GPUI 输入状态和真实按钮事件执行，未将其当作系统输入法/物理键盘验收。Windows 原生交互与权限语义尚未验证；后续 Mac 开发包补验见下文。依用户要求不再等待或处理 GitHub CI 账单限制。未运行全量 runtime 测试：本次仅改桌面配置表单与本地草稿，不改协作协议、模型执行或 runtime 配置服务。其他配置表单草稿不计入本任务。


## Mac 开发包补验

开发 ZIP：`.artifacts/hive-desktop-macos-arm64-model-drafts.zip`，源提交 `05e0dbf6eee503635de7696de1670162e1dad2bd`。ZIP SHA256：`861e700ac1c7ecbc23e2c7739713eede3074bdeb1d06162cb35008617107b015`；二进制 SHA256：`2ed2808d8c8d94b36342a75a2a414864718b761a3546ac51f4ed6fb7da2c73c4`。仍为未签名、未公证的 Mac arm64 开发包。

核对冻结依赖元数据与基础 ZIP 后，刷新完整 runtime 和生产程序；新 ZIP 独立解压后核对全部 106 个 runtime 文件和二进制。runtime 与上一写入保护包完全相同，原生程序包含本次草稿修改。包内五组 Pi 行为 gate、真实 PTY/Hive HTTP/SQLite smoke、生产窗口启动/导航恢复均通过。

通过系统界面操作独立解压的生产应用，未插入 GPUI 验证代码：填写提供商 ID、中文显示名称、接口地址和首个模型，点击 Responses，Cmd+Q 退出，确认真实草稿落盘。完整新进程返回模型与接口页，全部字段与类型恢复，显示恢复提示。外部改变独立配置文件后点击保存，真实界面提示冲突且保留字段；确认放弃后再次退出、完整新进程不再显示表单。三个 owned 应用进程均正常零退出，外部配置字节未被草稿覆盖。

英文通过系统 typeText 输入；中文第一次 typeText 未填入，改用系统粘贴成功，不冒认中文输入法组合态验证。实际配置保存成功路径已由前述原生 GPUI/真实 runtime 检查覆盖，本次系统操作没有重复该分支。没有模型请求或真实账号凭据。

包、逐文件哈希、门禁与实际操作收据在 `.artifacts/pi-model-drafts-package/`。Windows 原生、系统输入法完整矩阵和正式安装发行仍待验收；CI 按用户要求暂不处理。
