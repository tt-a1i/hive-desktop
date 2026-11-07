# 内置 Pi 账号连接

## 场景和结果

首次使用内置 Pi 时，之前只能提示用户去 Pi CLI 登录。现在可从“设置 → 内置 Pi”或聊天区的“连接账号”进入 SDK 支持的 OAuth 提供商列表，开始登录、打开或复制授权链接、复制设备码、选择账号类型、回填授权信息并取消。已保存账号排在前面；列表标注的是本地凭据状态，不冒充服务端有效性检查。没有账号入口时，新用户仍需离开桌面才能开始使用。

复用 Pi 1.4.0 的 `ModelRuntime.login()`、原有凭据文件和文件锁。桌面协议只传递提供商元数据和登录交互，不返回凭据对象；没有新增权限审批，也不写 Pi 全局默认模型。现有会话仍通过原生目录刷新和会话模型选择生效，未把目录换成桌面静态清单。当前会话是内置 Pi 时，登录成功卡片提供返回会话并刷新模型的入口。

后台仅保留一个进行中的登录，使用独立登录 ID 和步骤 ID 拒绝旧输入及重复提交。总等待上限十分钟；取消、后台关闭以及 SDK 的单步骤取消均有清理路径。浏览器回调先完成时，手动回填框可以独立撤销。凭据保存成功但 SDK 本地目录同步失败时，展示已保存及刷新提示。后台断开、步骤结束或提交后清空授权输入；已移除输入框释放原生焦点。设置页切换期间保留当前尚未提交的回填草稿。

## 实际检查

以下检查均未发送模型请求，也没有登录或修改用户的真实账号。

| 检查 | 结果与范围 |
| --- | --- |
| `pnpm --filter @hive-desktop/runtime check:pi-auth` | 通过。真实 Pi SDK 和临时锁定凭据文件，合成 OAuth 提供商；验证持久化及新 runtime 回读、拒绝授权、浏览器回调抢先、重复/过期输入、取消/关闭、目录同步失败后的保存状态，以及 DTO 不包含夹具令牌。另起生产 JSONL 进程验证目录、提供商列表、错误输入、默认模型文件保持原样及退出。 |
| `pnpm --filter @hive-desktop/runtime exec tsx checks/team-runtime.ts` | 通过。真实 Hive HTTP/SQLite、team CLI、启动、派单/seen/report、并发输入隔离及持久队列守卫，无模型执行。 |
| 实际 GPUI 渲染和输入动作 | 760×700 的浅色与深色窗口均通过：账号目录、取消后重开、选项、授权输入、实际分派 Enter、账号元数据刷新、输入清空及移除后的焦点、设置页切换和外部程序路径草稿保留。截图已查看，见本地 `.artifacts/pi-accounts/{light,dark}-accounts-*.png`。 |
| `cargo fmt --check`、Mac `cargo build --locked` / `cargo test --locked` | 通过；15 项通过，1 项子进程夹具入口按既有设计忽略。 |
| Windows 默认功能交叉检查 | `PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` 通过，含 GPUI manifest，日志 `.artifacts/pi-accounts/windows-check.log`。没有链接或运行 Windows exe。 |
| 打包与解压包检查 | 冻结部署生产 runtime，重建正式二进制后打包。`check-package.mjs` 通过包内 Node/PTY、隔离 Pi 账号元数据、Hive HTTP/SQLite、任务冲突、会话改名/搜索和退出；`check-desktop-startup.mjs` 通过原生窗口恢复。zip 完整性检查通过。 |
| Windows 验收接线 | CI 增加认证定向检查，原生 UI 脚本增加“内置 Pi”目录读取与截图；PowerShell 和 YAML 语法检查通过。新增 Windows 检查尚未运行，不能用交叉编译替代。 |

原生夹具在 `/tmp/hive-pi-accounts.00dmh7/native` 使用复制的生产视图代码，通过临时 GPUI 显式帧/图像接口绘制；后台为真实桌面 JSONL 和真实 SDK，OAuth 对端为隔离的本地提供商。夹具没有点击浏览器按钮、操作真实账号或生成任何厂商令牌。临时方法及 GPUI 补丁均未进入生产依赖。先前脚本直接发 shutdown 后强退产生过清理告警，改为应用正常退出后，最终两色运行均无该告警。

Mac 开发包：`.artifacts/hive-desktop-macos-arm64-pi-accounts.zip`，约 163 MB，未签名、未公证。包内二进制与生产构建 SHA-256 均为 `07c619e8238bff0d13a5aa24740d37d74f8c4b8cc93f61c033771779ff71ff63`。包内 `main.js`、`pi-auth.js`、`desktop-runtime.js` 与构建输出逐项一致，未含 OAuth/原生夹具入口；本地收据在 `.artifacts/pi-accounts/receipt.json`。

## 仍未验收和剩余范围

- Mac 当前锁屏，鼠标、物理快捷键和系统浏览器授权尚未走查。本轮不证明任何厂商账号能成功完成授权；这是后续真实账号验收项。
- 此入口读取内置 SDK 提供商。项目扩展注册的额外 OAuth 提供商没有纳入全局账号页，不宣称兼容所有扩展认证界面。
- API Key、自定义提供商和模型仍沿用 Pi 环境变量及 `models.json`；桌面图形编辑与退出账号功能仍待实施。外部 Claude/Codex/Pi 的账号管理仍属于各自后端。
- Windows 原生账号页、系统浏览器及打包运行仍需验证。现有 GitHub Windows CI 的账单阻碍没有被本次本机检查解决。
- 未执行所有 Node 套件：本次改动集中于认证协调和设置 UI，已执行认证穿透、团队定向回归、原生检查及打包检查，未修改 Hive 协议或数据库 schema。没有删除或修改既有测试来适配产品行为。

本项补齐一个配置入口，完整 Cindy/Maka 桌面体验及正式发行目标保持开放。
