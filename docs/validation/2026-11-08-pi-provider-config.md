# Pi 提供商配置与凭据移除

## 使用场景

只有订阅登录入口时，使用 API 提供商的用户仍需离开桌面配置 Key；已经保存的账号也无法在桌面移除。现在“设置 → 内置 Pi”按 SDK 能力展示订阅登录、API 配置或沿用现有配置，支持名称/ID/配置方式搜索、结果数量和无结果提示，已保存项优先。选择其他方式时，会提示完成后替换原有凭据；中途取消保留原配置。

API 表单由 Pi 1.4.0 原生 `ModelRuntime.login(provider, 'api_key', interaction)` 驱动，支持 secret/text/select 等多步输入；Key 默认遮罩，仅敏感步骤提供显示切换。非空 Key 检查由桌面执行，文本步骤允许 SDK 所需的空输入，避免阻断“按 Enter 继续”等步骤。提交、结束、取消和后台断开后清空授权输入，不进入桌面聊天草稿。

“移除登录信息”先显示原生确认框，说明它影响 Hive 与共用配置的 Pi、保留会话、不会移除环境变量等其他来源。确认后调用 SDK 的 `logout()` 删除对应凭据；取消不写入。配置与移除互斥，后台退出等待已开始的移除完成。SDK 已删除凭据但目录同步失败时，显示实际删除结果和刷新提示；文件写入失败则保留原状态并报错。凭据存取继续使用 Pi 的原有文件和锁，未新增存储、依赖或权限审批。

## 实际验证

| 检查 | 结果及证据范围 |
| --- | --- |
| `pnpm --filter @hive-desktop/runtime check:pi-auth` | 通过，包括原 OAuth 检查、新 API 配置检查和生产 JSONL 检查。均使用临时凭据文件和合成值，没有创建真实 API Key、修改用户凭据或发送厂商/模型请求。 |
| `checks/pi-provider-config.ts` | 真实 SDK、真实文件与锁、多字段及可选空文本、取消保留原值、删除后重新打开、模型可用性随配置变化、环境来源保留、只读文件删除失败、删除成功后同步失败、另一提供商不受影响及关闭等待移除。只读权限检查在本机非 root macOS 执行，Windows 分支不包含这条 POSIX 检查。还运行安装版本原生 Anthropic API 配置处理器，填入无法用于真实请求的夹具值，验证其存储；并未验证服务端认证。 |
| `checks/pi-auth-wire.ts` | 生产 JSONL 进程读取两种认证方式，真实 Anthropic 原生 API 表单接受夹具值并写入/移除；拒绝空 Key、非字符串、旧步骤和无效方式。返回 DTO 不含夹具 Key，默认模型文件未变，后台正常退出。 |
| GPUI 原生夹具 | 760×700 浅色和深色均通过：英文大小写搜索、中文搜索、无结果、多字段实际 Enter 动作、空文本继续、Key 遮罩、输入清空和焦点释放、移除弹窗的实际 Cancel/Confirm 动作及真实凭据文件结果、未保存程序路径保留。截图已查看，位于本地 `.artifacts/pi-provider-config/{light,dark}-provider-*.png`。 |
| 长列表定位 | 实际滚动到 `-1600px` 后开始配置，发现表单仍在屏幕上方不可见；改为在开始配置和移除时回到页首。修复后两色夹具均断言实际滚动偏移归零，截图确认表单可见。原始截图与日志保留为 `light-provider-long-list-start.before.png` / `native-light.before-scroll-fix.log`。 |
| 团队回归 | `pnpm --filter @hive-desktop/runtime exec tsx checks/team-runtime.ts` 通过真实 Hive HTTP/SQLite、team CLI、派单/seen/report、并发输入隔离及持久队列检查，无模型执行。 |
| Rust 和跨平台类型 | Mac 格式检查、生产 build、既有 15 项检查通过，1 项子进程夹具入口按既有设计忽略。完整 Windows 默认功能 `cargo check --target x86_64-pc-windows-msvc` 通过，包含 GPUI manifest；日志 `windows-check.log`。没有链接/运行 Windows exe。 |
| 打包检查 | 冻结部署生产 runtime，正式 Mac 二进制重建后打包。包内 Node、真实 PTY、Pi 提供商两种认证方式元数据、Hive HTTP/SQLite、任务冲突、会话改名/搜索、退出及原生窗口恢复均通过；zip 完整性通过。 |
| Windows 脚本接线 | 原生预览加入提供商搜索、订阅/API 入口和清除搜索检查。PowerShell 语法及 JS 语法检查通过，Windows 新增交互尚未执行。 |

原生夹具位于 `/tmp/hive-provider-config.JTi3C1/native`，后台为生产 JSONL + 真实 SDK + 本地合成提供商，使用临时 GPUI 显式帧接口。截图不是 HTML 页面，也不代表物理鼠标/快捷键验收。首次夹具运行的 JS 括号错误已修正并保留失败日志；弹窗截图等待实际动画结束后采样。没有为通过检查改变生产凭据语义，未删除既有测试。

Mac 开发包：`.artifacts/hive-desktop-macos-arm64-provider-config.zip`，约 163 MB，未签名、未公证。包内二进制 SHA-256 为 `89fb3afa10b826a836711f4b71a890149bebc00636698d51973d633321e2f601`，与正式构建相同；`main.js` / `pi-auth.js` 与构建输出相同。原生/凭据夹具未进入生产包。收据在本地 `.artifacts/pi-provider-config/receipt.json`。

## 未完成范围

Mac 本轮仍锁屏，物理操作、真实 Key 的服务端认证及真实浏览器授权没有验收。配置页只呈现本机保存状态，不声称 Key 有效。Windows 新增脚本和包内交互仍待平台运行，现有 CI 账单限制尚未解决。

自定义提供商/模型的图形编辑、项目扩展额外认证入口，以及完整 Cindy/Maka 体验继续待实施；现有 `models.json` 配置继续有效。外部 Claude/Codex/Pi 的账号仍由各自后端管理。此页“移除”只删除本机保存信息，不撤销服务端令牌。

本次未执行所有 Node 套件；已跑认证与团队定向穿透、原生夹具、生产构建和打包检查，没有改 Hive 协议或数据库 schema。完整产品目标继续开放。
