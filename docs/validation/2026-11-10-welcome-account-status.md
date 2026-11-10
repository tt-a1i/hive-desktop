# 欢迎页账号保存反馈与凭据读取失败

2026-11-10；基线efa5fc8。仅源码，当前开发包仍3339ce1。

## 使用场景

1. 首次用户进入内置Pi账号设置，后台已读取“已保存API配置”，返回欢迎页时却仍只有通用连接说明，不知道是否已保存，也缺少下一步选择模型的提示。
2. 欢迎页现在复用账号页实际读取的元数据：未读取保持引导、已保存显示提供商数量/下一步、空配置说明环境变量仍可生效、失败提示重新读取。没有持久化“Ready”、自动登录/探测或推断凭据有效性；重启后再次回到未读取。现有四张Agent卡片和共享配置/返回流程不变。
3. 不做会让首次配置之后的反馈中断，需要重复进入账号页确认。实际改前窗口的账号页已有保存状态，返回欢迎页仍为Connect a Pi account…，不是凭空增加状态面板。

本机保存状态仅说明保存元数据，不证明能登录或调用模型。页尾将这一点与原有外部程序版本检查范围一起说明。无新依赖或权限审批，也未复制参考项目源码/资产。

## 同时发现的真实后台缺陷

验证显式Refresh时损坏自有auth.json，Pi SDK 1.4.0的无signal读取会在读取失败后回退到上次有效缓存，因而账号页仍显示已保存。最初损坏models.json未取得账号读取错误，尝试留在attempt-invalid-models.json，不算通过；之后针对实际凭据文件确认根因。

PiAuth目录读取现在传入生命周期AbortSignal，让原SDK报告读失败。关闭时保持原有登录取消和正在执行的移除操作排空，再取消剩余目录读取；初版在排空前abort导致原有shutdown-drain回归失败，已改顺序，未改测试预期。直接复用SDK，不改其缓存实现或凭据文件。

严格读取又暴露JSON解析错误会引用文件片段。已有JSONL真回归增加损坏凭据输入后，先验证旧代码返回成功缓存而失败；再验证原始解析错误含BROKEN_CRE片段而失败。最终使用固定、可翻译的文件修复提示，原始cause只留在本地Error，JSONL消息/输出不带文件内容。错误仍被抛出，不吞失败、不自动修复/重写凭据。

初次读取失败时没有目录元数据，账号页现在使用后台initialize已返回的Pi目录作为原有定位按钮的fallback，不必先读取成功才能找到文件。不是另建路径发现机制。

## 原生与真实后台证据

`.artifacts/welcome-account-status-efa5fc8/`；只有自有state/Pi目录，使用明显无效且永不调用的合成API凭据、PI_OFFLINE=1。未启动登录、授权浏览器或模型。

- 改前3339ce1实际包：账号页OpenAI显示已保存API配置，Escape回欢迎仍只有泛连接说明（before-return.ax.txt）。
- 源码English/Dark20：未读→显式进入账号页→返回显示1 provider；独立修改自有配置并显式Refresh后显示2 providers、空配置显示No login information saved locally及环境变量范围。saved-return、two-providers、empty-return等AX记录。
- 损坏凭据：旧无signal刷新仍显示旧成功，生产JSONL新增断言先红。严格读取后原生账号页明确失败，返回欢迎显示could not be refreshed；恢复文件→重新读取→保存状态恢复。strict-read-error/strict-failed-return/recovered-return记录。
- 最终提示脱敏后：新开发进程第一次读损坏文件，原生English显示固定auth.json修复提示，没有BROKEN_CRE；目录按钮在首次失败就可见。点击后Finder在实际自有目录的父级选中pi文件夹，记录repair-folder.ax.txt；仅关闭这次Finder窗口，未打开/编辑凭据。恢复文件、Refresh、Escape又显示1 provider（redacted-error.ax.txt / final-repaired-return.ax.txt）。
- 中文Dark20、约593×507逻辑窗口：完整冷启动恢复为未读取；实际读取后显示“本机已保存1个提供商…”与下一步。Tab显露配置按钮及两行完整状态，Return进入真实账号页，再Escape返回；zh-narrow-saved.jpg、zh-return/zh-final/cold-unread记录。英文为具体状态/操作验证，不宣称完整字号主题矩阵。
- 五个原生进程正常CmdQ/exit0。独立Node22真实runtime/Pi SDK冷读元数据为1、无登录流程，原合成auth.json哈希精确恢复；JSONL目录元数据不含凭据值。真实SQLite workspaces/workers/messages/dispatches/dispatch_messages/agent_runs/workflow_runs均0。check-cold.mjs / cold-receipt.json。

## 验证命令与范围

- runtime build与typecheck通过。
- 最终Node22.23.3分别执行checks/pi-auth.ts、pi-provider-config.ts、pi-auth-wire.ts均通过：真实SDK凭据存取、取消/回调/移除、移除期间关闭排空及production JSONL损坏读取/安全错误/恢复。最终日志为*-verified.log，read-failure-before.log / read-redaction-before.log及首次关闭顺序失败日志保留。
- Mac cargo build、Windows x86_64-pc-windows-msvc cargo check与git diff --check通过。新UI不新增脆弱UI测试；后端读错误已扩展现有真集成检查，没有mock fetch或凭据存储。

不运行无关全量suite，风险验证集中在被修改的认证元数据与关闭语义。API配置进行中/加载中的短暂卡片分支未专门以受控延迟复演；真实厂商登录有效性、Windows原生、完整辅助技术/语言主题矩阵和新包继续开放。欢迎反馈不代表完整引导或完整产品目标已完成。
