# 项目脚本、成员独立工作树与 Codex 账号额度

日期：2026-11-15。源码基线：`bbdf81d700dfcb8f884634d241a8f0cedb17d0a1`。这些功能已合入开发分支，尚未推广为 current/candidate，也未替换用户安装实例。完整目标仍按 [#39](https://github.com/tt-a1i/hive-desktop/issues/39) 实施。

## 项目脚本

设置中的“项目脚本”支持按项目保存名称与完整命令，手动选择项目目录或当前会话目录运行、查看实际目录及命令快照、分页读取输出、复制和停止。保存和执行是两个动作。后台只接收已保存脚本 ID 和版本，不接受调用方指定命令或执行路径。

配置使用 Hive SQLite 的整行比较更新；并发保存、工作区删除与失败回滚均经过真实数据库检查。运行先持久化请求身份和原命令快照，再启动 Hive 管理的独立 PTY。相同请求返回原记录；脚本后来被修改或删除、原目录移走后，也不会因此换命令或重复启动。

切换会话和关闭设置保留运行。停止、删除工作区和正常退出等待原生进程退出；目录或运行清单写入失效时仍按内存中的可信所有权清理。退出码、信号、取消、写盘失败及冷启动中断分别保留。输出最多保留 16 MiB；界面按字节分页，单次显示最多 1 MiB，截断明确提示。

本批修复了三个实际缺陷：删除等待期间的新启动窗口、读取运行目录失败而未停止静默进程，以及 RPC 错误分类丢失导致界面无法正确核对请求。独立复验覆盖真实 PID 退出、延迟启动、SQL 删除失败、其它项目与交互 Shell 保持，以及同请求仅执行一次。完整回归日志位于 `.artifacts/functional-parity/integrated-project-scripts.log`。

当前自动 setup/settle 尚未完成；新工作树初始化在独立实现中。手动脚本的真实进程验证为 POSIX；Windows 原生脚本另有验收任务。崩溃后的记录不会自动重跑，亦不承诺操作系统强制结束后台时能回收任意已脱离进程树的子进程。

## 成员独立工作树

成员卡片新增创建独立会话、查看其独立会话历史的功能。`member.worktree.create` 继承实际 Worker 后端和模型，固定原始成员归属。新会话不会立即替换成员当前绑定；显式发送或继续时才使用原 Worker，并检查运行、排队和未结派单。

创建意图在 Git checkout 和聊天索引发布前保存。索引写入失败后，可以恢复原成员、后端、模型及代码目录。缺少或损坏的新格式创建身份会明确失败。旧记录没有这些信息时，恢复入口明确说明会创建新的主控对话，不冒充恢复旧对话。

创建 API 本身不启动原生 Agent。界面创建后会打开并初始化原生会话，可能启动原生程序；这与模型推理和成员绑定分别核验。历史显示可使用不可变归属，但它不能作为停止其它运行的凭据。

合并后真实 Hive/Git/SQLite 与受控后端检查通过，包含四后端创建、A/B 绑定保护、忙碌与派单保护、索引失败和冷恢复：

- `.artifacts/functional-parity/integrated-member-worktrees.log`
- `.artifacts/functional-parity/integrated-member-worktrees-wire.log`
- `.artifacts/functional-parity/integrated-session-identity.log`

此前真实 Codex 工作树协作的 SDK 预绑定证据保留；新正式创建入口的真实模型协作正在单独验证，不能用旧入口代替新入口的验收。

## Codex 账号额度

外部程序设置中的 Codex 账号区域增加手动额度读取。会话 token 用量继续单独显示。探针沿用当前有效程序、关闭取消与自有进程管理，调用 `account/read {refreshToken:false}` 和 `account/rateLimits/read`，不登录、不刷新凭据、不启动模型。

只展示 Codex 额度桶实际返回的窗口百分比、明确的窗口时长、重置时间和读取时间。没有窗口、时长或重置值时显示不可用或未知；不假设五小时或一周。原生 `ordinaryUsageAllowed` 保留允许、受限和未知三态，不从百分比推断模型一定可用。程序路径或连接变化后，旧结果不能显示成当前程序的结果。账号标识、邮箱、原始错误及凭据不会传到额度界面。

受控原生协议检查覆盖多桶、空值、非法数值、账号类型、程序来源、超时、断开、关闭和 PID 回收。本机 Codex 0.169.0 仅执行一次真实只读检查，返回一个主要窗口；脱敏收据为 `.artifacts/functional-parity/account-limits-code/.artifacts/account-limits/official-receipt.json`。该收据确认启动 wrapper 退出，没有单独枚举真实 CLI 的所有子孙进程，不扩大其证明范围。

### 2026-11-19 原生窗口与读取时间

从解包后的 macOS 应用，在实际设置页使用现有安装的 Codex 0.169.0 读取一次额度，核对普通使用状态、剩余比例、窗口时长和重置时间。旧版英文窗口通过原生辅助功能可读到账户状态和额度窗口，读取时间缺少可访问的文本。`7afb169` 为读取时间补上原生辅助功能标签。

新版在 760 像素宽的中文窗口通过同一设置入口调用官方原生可执行文件。后台先执行 `account/read` 并明确禁止刷新令牌，再执行 `account/rateLimits/read`。辅助功能读到一个主要窗口、普通使用状态，以及与后台响应相同的读取时间。验证中未启动模型、登录或写入全局设置。应用和记录的后台及 CLI 子进程均退出。脱敏证据在 `.artifacts/current-native-codex-quota-JTTYbl/` 和 `.artifacts/checkpoint-preflight-and-native-quota-eUMYFA/quota-native-executable/`。

同一新版包的 760 像素宽英文窗口也使用已安装的常规 Codex 启动命令成功读取额度。该轮直接查看设置页和实际后台请求日志，窗口显示主要额度窗口及可读的 `Checked at`。先前带协议观测的两次常规启动请求失败，已保留其失败状态和可访问的读取时间。正常英文验收位于 `quota-default-english/`。Windows 原生额度窗口仍需运行验证。

## 合并后检查与仍待验证

Rust 桌面测试为 94 passed、1 ignored；Node 严格构建、项目脚本、成员工作树、工作树执行来源、历史归属、团队投递、Codex 生命周期与会话终端回归通过。日志前缀为 `.artifacts/functional-parity/integrated-*` 与 `quota-*`。定向 Windows MSVC 交叉检查通过，不等于 Windows 原生操作通过。

Windows 运行 `43651145657` 在会话终端夹具失败：真实原始流含 80×24 绝对光标坐标，夹具请求较大镜像后漏发真实桌面客户端的 resize。已补正确协议顺序、真实 VT 解析和有界原始流/快照诊断，完整目录、文件、PID 和恢复断言保留；仍待新的 Windows 实跑。早先 `43613855387` 的通过仅覆盖当时的输入/搜索/引用开发包。

Mac 目前锁屏，新增脚本、成员入口、额度面板及后续历史/远端入口的实际操作、窄窗和中英文布局待手动解锁后继续。Claude/Antigravity 按用户要求不做账号和真实模型验收；Pi 当前 `invalid_grant` 不重试。未推广的 `f545105` ZIP/DMG 不含后续路径、搜索和本批源码，保留作可复核产物；current/candidate 仍为 `837abd1`，回退 `9f010e2` 保留。
