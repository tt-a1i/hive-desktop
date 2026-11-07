# Pi 模型与接口配置验证

日期：2026-11-08。范围：原生常用配置编辑及常驻会话配置刷新；不是完整产品验收。

## 实现

设置新增“模型与接口”，支持添加提供商及首个模型、编辑接口 URL / 协议 / 显示名，添加或修改模型名称、上下文、输出上限、推理与图片能力。删除采用原生确认。草稿跨页面保留；外部文件变化时显示冲突并保留草稿，重新核对后再提交。已有标识不可编辑。

复用 Pi 的 models.json。新增 jsonc-parser 3.3.1 用于局部 AST 修改，保留注释、BOM、CRLF、尾逗号和未编辑的高级字段。修改同名 modelOverrides 中已有的公共字段，避免 SDK 覆盖导致编辑不生效。删除自定义模型时一并删除其同名 override。提供商删除保留凭据和会话；孤立的已保存凭据仍可在账号页删除。

文件更新串行执行，使用内容版本检查、同目录临时文件和原子替换；保持已有权限及符号链接目标，只读/无效配置拒绝保存。实际 SDK 负责验证，使用只读且不提供凭据的存储适配器，避免校验创建认证文件或执行配置中的凭据命令。接口新增支持 OpenAI Completions、OpenAI Responses、Anthropic Messages、Google Generative AI；其余已有协议保留。

常驻 Pi 会话刷新时重新核对所选模型；模型消失会阻止发送，恢复配置可重新读取元数据。会话刷新使用 persist:false，不改全局默认。

## 验证

- `pnpm --filter @hive-desktop/runtime check:pi-models`：真实 SDK/文件、JSONC 与高级字段保留、modelOverrides、过期/并发版本冲突、无效输入、只读文件、符号链接及孤立凭据；常驻 SDK 会话配合真实本地 HTTP 监听证明已移除模型在请求和历史写入之前被阻止，恢复后元数据可刷新。
- `pnpm --filter @hive-desktop/runtime check:pi-auth`：SDK 认证存储、提供商原生处理器及生产 JSONL 配置增删/目录刷新通过，无厂商请求。
- 团队定向回归通过；未改 Hive 协议或 schema，未重复全量 Node 测试。
- Mac Rust 生产构建与既有检查通过（15 passed、1 ignored）；完整默认特性的 Windows x64 MSVC cargo check 通过。Windows 配置页脚本完成语法检查，未原生运行。
- 浅色/深色的实际 GPUI 布局与绘制夹具通过：添加/编辑、能力字段、窄窗滚动到保存、确认框取消/确认、切页草稿保留、外部冲突及删除。夹具使用复制源码的显式帧驱动，不等于物理鼠标/键盘验收。
- 冻结生产依赖重新部署的 Mac 包通过 bundled Node、真实 PTY、Hive HTTP/SQLite、配置写入与原生启动恢复检查。未向厂商或模型发送请求，未使用真实用户凭据。

本机证据位于 `.artifacts/pi-models/`，包含检查日志、浅/深色截图和 receipt.json；均为本地未跟踪产物。开发包 `.artifacts/hive-desktop-macos-arm64-model-config.zip` 未签名、未公证。

生产二进制 SHA-256：`4ded0c228b61c45b8285f5a0fd1f2bde67536397a6a74cb443b8f1597f5f2a21`。
包内模型配置模块 SHA-256：`907aa946e7c8db992d4dd37555619afe29208d39ae135f52017dbee425963eed`。

## 尚未完成

- Windows 原生窗口/运行验收、真实账号或兼容厂商接口调用、物理操作走查。
- override-only 内置模型条目，以及 cost、headers、sampling 和高级协议选项的完整图形编辑；原文件中的这些字段保留，可外部编辑。
- 草稿仅在当前进程保留，不保证退出后恢复。
- 版本检查是乐观并发保护，不是对任意外部写入进程的 OS 级 CAS；未验证断电持久性或 SIGKILL 临时文件回收。
- Windows CI 最近受账单限制无法启动，交叉检查不替代 exe 链接、运行和体验验收。完整 Cindy 体验、安装发行与总目标仍开放。
