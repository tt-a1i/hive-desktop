# 界面语言：偏好与首批文案

最新原生补验：已实际完成中文→英文→冷启动英文恢复→切回中文，原聊天草稿保持、中文搜索进入英文外观页、浅色设置与英文窄窗导出通过，见[原生补验](2026-11-09-native-resume.md)。下文各批锁屏记录为历史，不再是当前原生访问状态；全页面/输入法/键盘/平台矩阵仍未完成。

17d8ce7已纳入最新候选便携包，并实际验证包内中英切换、原草稿、主题与冷启动恢复，见[候选包证据](2026-11-09-native-candidate.md)。下文各批“未打新包”为当时状态；current仍未推广，完整语言验收未完成。

2026-11-09，对应 #17。首批实现，完整中英文界面尚未完成，#17不关闭。

## 已实施

- “设置 → 外观”增加中文/English选择。使用独立桌面数据目录的desktop-language.json保存zh/en；复用已有原子保存helper，不新增业务store或依赖。保存成功后才更改活动语言；失败保留原选择，无效配置保留原文件并显示错误。后台重连不覆盖当前本地选择。
- 沿用gpui-kit0.9.0已安装的component::set_locale/locale（中文zh-CN/英文en），让原生组件菜单使用同一语言。
- 首批114条英文目录，迁移导航静态标签/状态提示/菜单、设置侧栏与标题/搜索结果、外观主题与字号、语言及外观偏好通知、主输入提示、部分设置外层操作。设置搜索同时索引原中文、当前语言标题/描述与原关键词，可搜索English、中文、语言等进入外观页。
- 输入提示按活动语言变化更新，使用已有InputState/TextareaState.set_placeholder；没有重建输入状态、改写value或切换会话。主输入框仍按现有会话后端显示提示。
- 显式tr只接受应用拥有的静态文本，不对运行时会话正文、工具结果、用户名称/职责/模板、提供商数据及诊断原文做自动翻译。
- 同时补上远程接入侧栏入口。之前该页只加了设置搜索entry，侧栏固定列表缺失。

## 验证范围

读取已安装gpui-component/gpui-base0.9.0源码确认set_locale及set_placeholder接口。后者只修改placeholder并notify，不改value、selection或undo；这仍不能代替实际切换时的草稿/输入法走查。

- `cargo test --locked --manifest-path apps/desktop/Cargo.toml language::tests::language_preferences_reopen_and_preserve_invalid_or_failed_targets -- --exact --nocapture`：1通过。实际组件locale/英文目录选择、真实文件写入后重读、zh/en反复替换、无效值保留、目标目录阻挡原子保存及临时文件清理。先前未带模块前缀的exact过滤得到0条，没有把它当通过。
- `cargo test --locked --manifest-path apps/desktop/Cargo.toml appearance::tests -- --nocapture`：原有2条外观持久化/错误回归通过。
- Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml`、现有LLVM PATH下Windows x64 MSVC `cargo check --locked`、`git diff --check`通过。
- `check-desktop-startup.mjs`增加可选`--language zh|en`，仅写测试自身临时数据。用当前原生构建硬链接、现有Node22和源码runtime链接建立轻量验证fixture（不是便携包）。隔离Pi目录设openai/gpt-6.1-terra、PI_OFFLINE=1，以--language en启动。实际原生程序恢复保存项目、打开会话并修正已删除会话选择，脚本exit0；未发生模型请求。脚本最后SIGTERM清理自有probe，不算正常用户退出。
- 原生工具仍明确返回Mac锁屏。没有看到实际英文文字，也没有完成点击语言切换、切换期间草稿/选择/输入法保留、深浅色/窄窗及系统剪贴板验证。

本轮不改生产runtime、Hive协议/模型配置或内置Pi审批。未跑全量测试；只运行UI偏好的实际文件逻辑、现有外观回归、平台构建与原生启动回归。没有删除现有测试。新pref检查不以源码字符串或mock证明行为。

## 未完成项

成员/协作/派单/任务/工作流/记忆、Pi登录/模型表单、远程页内部、文件与图片阅读/导出窗口、聊天工具详情与交互、其他加载/空/错误提示和可访问性标签仍待全面迁移。部分已有通知的原始诊断/校验文字仍保留，不把114条目录称为全覆盖。还需核对动态格式、复数、长英文排版和术语一致性。

新语言功能未纳入current或候选ZIP；候选仍为d9e061b，SHA256 74c1b29371010de375e058460ecdd751a98e0f1539ffd7f8eb2858f1ac841935不变。实际Windows语言/输入和两个平台完整体验待验收。

收据`.artifacts/language/receipt.json`明确区分偏好检查、启动证据和未观察的UI；日志native-startup-en.log。gate结束且lsof无引用、无自有probe/sidecar后，移除轻量fixture，保留两个旧owned进程21544/54534和其现场。.artifacts仍约3.8GiB。完整目标保持active，后续按#17原始全量要求继续迁移，不缩小到本批范围。


## 远程、聊天与通知设置文案补齐

第二批累计193条英文目录（本批新增79条）。已迁移远程页状态、登录命令说明、配对与安全短码确认、设备撤销、审计标题/结果、时间/倒计时/字节数等动态说明及全局待配对提示；同时迁移聊天发送快捷键页/输入提示、通知URL设置及其静态通知。

异步提示不预先保存翻译结果：远程和聊天偏好用本模块的提示类型保存状态/原始错误，在render时决定语言；通知当前全部是应用拥有的静态提示，存为静态key并在render时翻译。因此旧提示随语言切换更新，不改后端或用户数据。设备名、ID、安全短码、复制的命令、审计action/endpoint/preview和原始诊断仍保留原值；只映射已知结果的显示标签，未知结果原样显示。中文/英文时间都由同一实际时间戳计算；配对倒计时按整秒向上取整，与Web一致，避免最后不足一秒时提前标成已过期。

只改Rust界面和英语目录，未改PI审批、发送按键逻辑、通知URL字段、远程协议/守卫、原有配置文件格式或生产runtime。没有删除、修改或增加测试用例；沿用现有定向检查：

- language::tests 1、composer::tests 1、settings::notification::tests 1，均通过。
- Mac cargo check/build、现有LLVM PATH下Windows MSVC cargo check、git diff --check通过。
- 真实Node后台notification-settings的HTTP/SQLite保存重开，以及remote-settings的配对、加密HTTP/PTY、UTF-8、坏流隔离、实时撤销和断网重开补发均通过。这是后台回归，不声称穿透了本批Rust按钮操作。

未重复全量测试：本批为UI文案及提示表示迁移。当前原生工具仍明确报Mac锁屏；真实切换、旧提示刷新、草稿/输入法保留、英文长文排版及两个平台UI尚未验收。其他页面与动态/错误文案仍待按#17原始范围全面迁移，#17不关闭。新语言功能仍未纳入候选/current ZIP，也没有本轮重新打包。收据`.artifacts/language/settings-receipt.json`区分上述证据范围。


## 会话入口与聊天常用控件迁移

第三批累计340条英文目录（本批新增147条），覆盖会话行的角色/置顶/归档操作、置顶区、搜索和归档窗口、聊天工具栏、复制/引用/导出/重命名/停止/排队/继续、部分成员与协作通用控件、欢迎页内置示例。动态的会话/项目/成员名称、消息原文、ID和模型名保持原值；只翻译外围标签。默认状态的内部字符串比较保留原值，不把翻译后的文案用于控制判断。

搜索错误保存Request原诊断或WorkspaceMissing状态，显示时取当前语言；搜索框切换语言只更新placeholder，不改查询值、selection、分页和请求代次。内置欢迎示例只在用户点击时按语言创建新输入，已有草稿原文只原样拼接，原始消息、用户模板和已保存会话标题不翻译。部分全局错误快照、成员角色/状态细节、协作/工具内部文字仍待后续迁移，不能把本批当全界面完成。

验证：

- 现有language::tests 1通过，实际locale/目录和偏好文件路径；未新增UI测试。
- Mac cargo check/build、现有LLVM PATH下Windows MSVC cargo check、git diff --check通过。
- 真实session-search、session-pins和session-titles回归通过：HTTP/SQLite、四后端记录、JSONL、原始标题、排序/分页、完整重开及写盘失败回滚；没有启动Agent或模型请求。它们验证后台，不证明Rust按钮已实测。
- 当前原生构建以--language en重新跑启动恢复；轻量硬链接/资源链接fixture并非便携包。实际新程序启动并恢复项目/会话选择，probe exit0后SIGTERM清理；不是正常用户退出、英文文字或搜索窗口的系统操作证据。

原有owned进程21544/54534仍在，原生工具再次明确报告Mac锁屏；未强杀这些现场。当前对话弹窗的英文/中文切换、选中项与查询/草稿保留、重命名验证、菜单点击、长英文和窄窗均未验收。没有删除或修改现有测试、生产Node后台/协议/模型选择与Pi审批未改，未跑全量测试，未重打ZIP。收据conversations-receipt.json及conversations-startup-en.log在.artifacts/language；fixture已在gate退出、lsof无引用且无自有probe/sidecar后移除。完整#17范围及完整产品目标保持开放。


## 成员、职责与角色模板文案

第四批累计379条英文目录（新增39条），覆盖成员角色/状态/后端标签、搜索筛选、职责展开与复制、角色模板的列表/编辑/删除/选择及其提示。角色模板名称、职责正文、命令/参数/环境、成员名称和原始诊断保持原值；筛选仍使用原状态码。模板通知和成员刷新/模型读取错误在渲染时本地化，保留原诊断。部分创建/停止成员的全局错误快照仍待迁移。

将成员显示helper移出协议模块，复用角色模板的角色映射。验证无GUI的process-supervisor-check入口时，发现现有protocol.rs依赖UI模块中的AttachmentRef与DeliveryFailure，原HEAD同样存在，导致该入口不能编译。两个纯数据结构移回protocol.rs，原UI模块重导出同一类型；不改变字段、serde命名或运行路径，不引入GUI替身。新增一条实际JSON编解码回归，验证队列附件、中文原文、投递失败的camelCase字段及错误命名拒绝。

本批验证：

- `cargo test --locked --manifest-path apps/desktop/Cargo.toml --no-default-features --features process-supervisor-check --bin process-supervisor-check`：13通过，1个既有子进程fixture入口按设计忽略，主测试实际启动该入口。包含真实进程树/并发关闭/Node启动与协议回归。
- `cargo test --locked --manifest-path apps/desktop/Cargo.toml language::tests`：1通过。
- Mac默认构建、Windows MSVC默认及无GUI监督器入口类型检查通过。Windows是交叉类型检查，不是Windows真机运行。
- `git diff --check`通过。未跑无差别全量测试；本批UI文案与纯数据结构归属调整使用上述定向检查，未改动/删除既有测试。没有模型请求。

用户回复解锁后再次选择仍运行的原生应用，工具依然明确报告Mac锁屏；实际英文排版、切换、草稿/输入法及窄窗未验收。未重启或删除运行中的验证应用，未制作新ZIP，完整语言覆盖和#17均未完成。收据`.artifacts/language/members-receipt.json`。


## 记忆页语言

第五批累计426条英文目录（新增47条），覆盖记忆状态/类型/来源、搜索/范围/注入设置、详情及操作、动态数量与相对时间。正文、标签、作者、来源摘录、引用标题/类型/编号、未知状态码和原始错误仍为实际原文。切换只更新query的placeholder，不改搜索值、筛选码或selection；记录的成功提示与加载/变更失败在render取语言，不增加请求或改变跨工作区操作结果归属。

现有language::tests 1通过，Mac构建、Windows MSVC类型检查及git diff --check通过；现有`checks/workspace-memory.ts`在包内Node22下跑真实Hive HTTP/SQLite回归成功（合成隔离记忆，无模型请求），收据`.artifacts/workspace-memory-xdsD8j/receipt.json`。没有新增或删除UI测试，未跑全量测试，本批改动限UI与错误显示；未改后台语义。新增目录项也供共用标签复用，但不据此宣称其他页面迁移完成。

本批原生语言/草稿/输入法/长英文排版及两个平台GUI仍待验收，继承上文工具明确报告的锁屏阻断；没有新ZIP或完整界面交付结论。阶段收据`.artifacts/language/memory-receipt.json`。


## 协作状态、派单卡片与任务编辑语言

第六批累计465条英文目录（新增39条），接入派单/消息/投递状态、派单卡片负责人/产物数/无障碍标签，以及任务编辑、冲突版本、复制/取消/保存和空/加载提示。加载/刷新错误保存静态提示key与原始诊断，render时取当前语言；刷新失败继续保留上次结果。任务草稿写盘失败提示同样按渲染取语言。全局打开派单/草稿同步错误仍是发生时快照，不把本批描述为所有错误都能动态刷新。

任务编辑器在既有locale更新入口只set_placeholder，遍历已有各项目草稿；不改input/base/conflicting_content/selection、不触发保存、刷新或冲突裁决。派单/消息正文、负责人、来源、产物路径、任务Markdown及冲突原文保持原值；状态helper只映射已知码，调用处仅用于显示，没有以翻译结果控制行为。

验证：现有language::tests 1通过，Mac默认构建与Windows MSVC类型检查通过，git diff --check通过。未新增/修改/删除测试，未跑全量测试；本批没有后台、存储或协议改动，定向检查覆盖现有偏好行为，未把类型检查当交互证明。任务冲突编辑、复制及语言切换中的中文草稿/输入法保留、英文窄窗仍需原生验收，当前锁屏阻断没有解除。未请求模型、未生成新ZIP。阶段收据`.artifacts/language/collaboration-receipt.json`。


## 工作流配置、计划与详情语言

第七批累计529条英文目录（新增64条），覆盖工作流实验开关/执行器策略、筛选、运行与计划列表、暂停/恢复/删除确认、运行详情/关联派单/日志/结果的操作和无障碍文案。相对时间基于同一个实际时间戳计算，英文使用s/m/h/d与in/ago；已知状态仅映射显示值，未知状态保留。

运行名称、父运行名、阶段值、脚本路径/hash、Cron、参数、结果、错误、日志、派单与汇报正文和产物保持原始值；空值显示沿用原有“无”占位并提供英文None。运行结果与日志复制仍使用既有内容。执行器草稿/允许列表、选中运行、筛选、滚动状态和操作generation不因语言切换而更改。刷新/操作失败及结果未确认提示保存类型和原诊断，渲染时取语言。没有修改请求、后台工作流、停止/删除/取消规则或新增权限流程。

验证：`cargo test --locked --manifest-path apps/desktop/Cargo.toml language::tests` 1通过；Mac默认构建、Windows MSVC类型检查和`git diff --check`通过。没有增删测试，未跑全量测试，本批只改UI显示和提示表示。静态目录核对保证本批显式标记的key有英文条目，该核对不代表排版/交互验收。工作流删除最终按钮等原生缺口仍保留；英文窄窗、长日志、切换中草稿/操作未实测。没有模型请求或新ZIP。收据`.artifacts/language/workflows-receipt.json`。


## 外部程序与团队设置语言

第八批累计554条英文目录（新增25条），接入程序检查/浏览/路径来源、重新检查/版本兼容提示和对应无障碍标签；程序设置及团队自动组队设置的成功/失败提示保存静态key与原诊断，在render取语言。文件选择对话框标题按打开时语言构建。非UTF8路径转换的内层固定错误目前仍保留原始中文，未当完整迁移处理。

版本探测只本地化当前桌面runtime拥有的8条固定消息，以及实际version与原“已检测到版本…”一致的成功提示；未识别消息和RPC诊断原样显示，不按错误内容吞异常，也不改状态/权限/版本判断。命令路径、版本号、来源码、环境变量覆盖优先级、尚未保存的路径与现有探测请求保持原值。未修改后台或增加探测请求。

验证：language::tests 1通过，Mac构建、Windows MSVC类型检查、git diff --check通过。既有executable-probe-wire.ts跑编译后JSONL后台，验证实际进程版本/未保存路径、输入校验、关闭期间取消和子进程退出；team-autostaff-settings.ts跑真实HTTP/SQLite持久化，两项均通过，无模型请求。测试文件无增删改，未跑全量测试；本批只改显示与提示表示。

再次原生选择现有仍运行应用，工具明确报告Mac锁屏，未观察新英文界面/文件选择/语言切换/草稿与输入法保留；不重启已有工作应用。#17仍需Pi账号/模型配置、文件/工具/导出内部及更多动态错误文字和实际布局验收。没有新ZIP。阶段收据`.artifacts/language/programs-receipt.json`。


## Pi账号、模型配置与草稿语言

第九批累计670条英文目录（新增116条），覆盖Pi账号/提供商搜索、登录/配置/移除确认、模型/接口/能力覆盖表单、校验/保存/冲突/草稿恢复提示和无障碍标签。账号运行时的17条明确归属Hive的固定提示做精确匹配后显示译文；未识别消息、SDK授权说明/输入选项/placeholder、设备码、URL、原始错误和用户数据保留。模型草稿底层io错误的内层诊断仍为原文，不宣称所有错误完全本地化。

账号本地操作失败/断连和模型静态通知/失败的显示按render取语言。既有locale更新入口只set_placeholder到账号搜索、当前登录输入和模型编辑器；不清空凭据、不改masked/focus、不重启登录或发请求，不改value/selection/初始值、配置revision、冲突base或已保存草稿。保留既有“新prompt才清空输入”的处理和断连清理逻辑。模型名称/ID、URL/API码、Token数、继承三态、配置路径和高级字段保持实际值。

另将模型能力覆盖按钮和程序浏览按钮的id改为能力值/后端固定码，避免显示文字翻译时控件id跟着变化。原先覆盖按钮id包含中文label；程序浏览id包含可能本地化的外部Pi标签。未改变按钮动作或权限。

本批验证：

- language::tests 1通过；现有settings::pi_models::draft::tests 3通过，涵盖原冲突base/其他配置草稿保留、不可读文件不覆盖/丢弃、原子写入失败保留旧表单。
- Mac默认构建和Windows MSVC类型检查通过，无编译警告；git diff --check通过。
- 既有pi-auth.ts：真实Pi SDK/隔离凭据文件，合成OAuth provider，无厂商请求。覆盖重开、状态脱敏、prompt回调时序、重复/过期输入、取消/关闭和写入后目录同步失败。
- 既有pi-auth-wire.ts：编译生产JSONL、真实Hive与隔离Pi配置，验证API配置/移除、非法/过期/空输入、默认模型保持和退出，无厂商API请求。
- 既有pi-model-config.ts与pi-model-overrides-wire.ts：真实SDK/JSONC文件、生产JSONL和HTTP/SQLite，覆盖高级字段/BOM/CRLF保留、过期/并发写入/只读/符号链接及继承/重置/移除/冲突/默认值保留；无模型请求。

没有新增、删除或修改测试；未跑全量测试，本批改动为UI显示/输入提示/控件id，认证与模型后台、协议及配置文件格式未改。测试通过不证明语言切换时凭据/草稿/selection/输入法保留，也不证明实际英文表单/移除对话框和窄窗可用；这些需要原生操作。再次选择现有仍运行应用，工具明确报告Mac锁屏，尚未观察当前界面。没有重启或删除已有验证应用，没有新ZIP。完整#17仍开放，工具/文件/导出内部、其他动态错误及最终GUI验收继续推进。阶段收据`.artifacts/language/pi-settings-receipt.json`。


## 文件、工具与导出语言

第十批累计746条英文目录（新增76条），迁移工具分组/状态显示/输入输出/改动与复制、只读Diff视图/来源说明、文件与图片阅读操作、草稿附件控件、文件管理器/预览入口，以及导出窗口和保存/取消/复制提示。内部工具状态及完成分组/失败颜色判断继续使用原固定值，render才翻译；Diff解析/来源、原始补丁、文件内容、工具名称/输入/输出和附件名称路径保留原值。

导出Snapshot新增捕获时语言标记，结构性Markdown标题/元数据/说明使用该语言，消息/工具输入输出/补丁/诊断正文仍为实际内容；capture形成的条目标题/元数据、后端显示标签同样按当时语言生成。之后切换导出开关只重建同一份快照，不取当前界面语言；已有投递投影heading仍按原值收录，可能保留原中文。文件名清理、字段筛选、代码围栏/工具与思考开关规则不变。导出保存错误分开保留提示key与原诊断，显示时取当前语言，取消后保留快照。

本批不是所有错误本地化完成：文件读取、附件导入与图片保存/解码的部分错误仍是原始或发生时快照；导出的已有投递标题及其他聊天/全局提示仍待续。没有改变读取大小/编码/路径边界、复制/保存的原文件字节或后台接口。

验证：

- 现有导出序列化检查1条，扩展英文元数据/结构标题断言，同时验证中文正文、实际命令/诊断、补丁和含反引号工具结果的围栏保留。修改这一条既有产物逻辑检查，未新增测试文件或删除测试。
- 现有工具终态检查1条、Diff数据2条、文件读取1条、图片原文件保存1条、产物路径边界1条、语言偏好1条通过，共8条。初次tool_message::tests筛选没有命中（0条），随后正确执行tool_message::status_tests 1条；未把0条称为测试通过。
- Mac默认构建、Windows MSVC类型检查和git diff --check通过。未跑全量测试：本批为UI显示和导出产物格式语言，使用上述定向路径/解析/保存/序列化回归。没有模型请求。

再次选择仍运行的原生应用，工具明确报告Mac锁屏；实际英文长文本/Diff并排/窄窗、切换中滚动/查找/快照固定、导出复制/保存对话框与正常退出仍待原生验收。没有重启或删除旧验证应用，没有新ZIP，.artifacts仍约3.8GiB。完整语言和产品目标继续开放。阶段收据`.artifacts/language/reading-receipt.json`。


## 预览错误与外部Agent交互语言

第十一批累计811条英文目录（新增65条），补齐文件预览的普通文件/大小/二进制/编码/附件路径提示及读取错误前缀、图片加载/选择位置/保存失败前缀；静态原因保存为类型，渲染时取语言。原始io/路径/SDK诊断仍保留；artifact_path生成的部分动态中文原诊断尚未全面迁移。读取逻辑、1MiB边界、UTF-8/NUL校验、工作区及符号链接边界和原图字节保存不变。

接入Codex/Claude/外部Pi交互卡片、单次批准/拒绝/取消、问题/多选/其他回答/秘密回答说明、Diff与权限详情、提交/重试提示。静态错误和缺失原生身份提示在render取语言，未改变has_native_identity、availableDecisions、没有原生Diff不能批准、提交代次、默认拒绝和秘密输入不回显原provider错误的规则。按钮id、请求方法、决定码、confirmed/cancelled与回答原值不变。内置Pi没有新增审批。

已有交互输入切换locale时只set_placeholder：保留外部Pi给出的placeholder原文，否则用桌面当前语言；不改输入/多选/其他回答、masked/focus、editor预填、超时/请求或提交。SDK/Agent问题、选项、工具输入、路径/命令与解释原文不翻译。主状态栏的部分交互摘要仍为发生时原中文，完整覆盖继续开放。

验证：

- 现有文件读取1条、图片保存1条、路径边界1条、语言偏好1条通过；将既有文件测试中二进制/编码/超限三个笼统is_err断言细化为错误类型匹配，其他行为断言保留。未新增/删除测试或测试文件。
- Mac默认构建、Windows MSVC类型检查、git diff --check及本批显式key目录核对通过；未跑全量测试，本批为显示和错误表示，读取/交互后台协议不变。
- codex-interactions.ts：真实子进程JSONL+Hive HTTP/SQLite，对端为合成协议server，无原生模型/批准执行验证，退出0。
- external-pi-interactions-transport.ts：真实子进程与合成对端，验证人类等待不计prompt deadline、其他操作有界、重复关闭与无迟到写入，退出0。
- claude-interactions-sdk.ts：官方SDK+合成CLI对端，7项canUseTool/manager场景，未用原始Claude CLI或Hive HTTP，退出0。

后两项首轮启动失败，生成fixture的shebang直接包含当前process.execPath，指向名称含空格的缓存app路径（peer存在但spawn返回ENOENT）。没有修改测试或生产fallback；将同一Node22.23.3硬链接到不含空格的临时路径，验证execPath、同device/inode后原命令重跑通过。临时入口为`/var/folders/v1/6vx82zzx32q7y_s1w4sx84j00000gp/T/hive-i18n-node-dgjo9hza/node`，没有复制Node/依赖/安装包。旧运行应用仍使用原路径；lsof按共享inode也会列出临时别名，保留两者，不据此重启进程或删除来源。成功与首轮失败边界均记入收据。

再次原生选择仍运行应用，工具明确报告Mac锁屏。交互卡片实际英文布局/键盘/秘密输入/多选/语言切换、文件失败重试与原图保存对话框未验收；合成协议和构建通过不代替这些验收。无模型或厂商登录请求，未打新包，.artifacts仍约3.8GiB。完整#17和产品目标继续开放。收据`.artifacts/language/interaction-errors-receipt.json`。


## 资源、思考、推理、用量与终端语言

第十二批累计908条英文目录（新增97条），接入命令选择/资源浏览、思考块、推理强度、原生用量及工作区终端按钮/静态状态/已知错误。命令目录/资源错误与用量解码/请求错误保存本地提示类型，render时取语言并保留原诊断。资源窗口只在locale变化时set_placeholder，不改查询/筛选/滚动；搜索保留中文分类/范围关键词并加入当前译文。实际资源名/说明/路径/命令与source_info原值、查找逻辑的原生code保持原样，不执行“补入草稿”。

思考正文及复制内容、redacted/interrupted/running标志保持原值；只翻译外围提示。推理已知level的标签本地化，发出的level仍为原code，未知值/原生description保留；未动失败后读回实际状态的逻辑。用量只翻译外围文案，原生source/report/counter标签、数值、估算标志和通知保留。

终端状态保存为静态Message、实际退出码Exited或原始断连原因Detached，渲染时本地化。九条桌面自有固定错误做精确匹配后显示译文，其他错误和Shell title/name/output原样，部分动态原诊断仍可能中文。序列/ACK、输入启用、身份/工作区/连接guard、detach/kill、启动/停止、重连规则与请求均未改；没有把翻译结果用于状态控制。主应用状态栏的其他动态摘要仍待迁移，不把本批当完整覆盖。

验证：

- language::tests 1通过；Mac默认构建、Windows MSVC类型检查、git diff --check及本批显式key目录核对通过。没有增删改测试，未跑全量测试，本批仅UI标签/提示表示与搜索显示词索引。
- pi-reasoning.ts：真实HTTP/SQLite、SDK/外部Pi RPC，能力、转录/重启、写盘失败后退休/重试、全局默认保持通过。仅两个被阻断的本地loopback请求，无厂商推理。
- agent-usage-wire.ts：编译生产JSONL读取实际Pi统计，缺失会话/重复读取/原生计数与全局默认保持、正常后台关闭通过，无prompt。
- terminal-ack-wire.ts：生产TerminalConnection与真实回环WebSocket、合成Hive对端通过（不是PTY集成）；terminal-input-bytes.ts：真实Hive POSIX PTY原始字节输入通过（不是Windows运行证据）。
- 当前新Rust原生程序通过带英文偏好的check-desktop-startup.mjs，实际连接源码runtime、恢复保存项目并修复失效聊天选择，SQLite导航写入确认。只使用本轮新fixture：可执行文件硬链接、Node/runtime目录软链接，没有复制依赖或打ZIP。探针用SIGTERM收尾，不能当正常用户退出。进程退出后lsof目录为空且没有残留fixture进程，已仅移除本轮native-bundle链接目录，保留日志。

原生CUA再次明确报告Mac锁屏，未看到英文文字，也未验收资源搜索/语言切换/滚动/草稿/输入法、思考/推理/用量浮层和终端键盘/窄窗/正常退出。源码启动检查不代表上述交互已通过。旧验证应用及current/候选包保留，无模型请求，无新ZIP；.artifacts仍约3.8GiB。阶段收据`.artifacts/language/catalog-terminal-receipt.json`。


## 主状态栏与全局错误表示

第十三批累计969条英文目录（新增61条）。主状态保存静态key或外部交互的后端/phase，render时翻译启动/连接/工作区/会话/退出及交互等待/提交/结束摘要。原先唯一的“后台已连接”隐藏判断改为Status.is_background_connected，仍匹配原静态key，与译文无关；运行中的回复提示规则不变。状态更新时机、连接generation、退出/清理失败保留owner与窗口、重连与会话身份检查未改。

全局错误由String改为ErrorNotice：Message保存静态key，Detail保存桌面自有前缀和原诊断，Raw保留原String。会话/成员/草稿/导航/部分生命周期错误按render取语言；消除成员空名称等在发生时保存译文的问题。未知后台/系统错误原样，避免把外部诊断当译文key。接收Option<String>的工作区移除结果只map到Raw，分支与结果不变。新增app/status.rs集中这两种显示表示，没有新store、协议字段或依赖。

仍未完成所有错误迁移：带动态补充说明的退出失败完整错误、部分工作区移除结果、附件批量错误、原生/系统诊断等继续保留Raw，可能含中文。没有把这些隐藏或概括成统一错误；完整#17继续开放。原始诊断、用户/路径/会话内容不翻译。

验证：

- 扩展既有language::tests，在同一测试原有zh/en切换中验证连接状态显示变化但隐藏判断始终一致，静态错误跟随locale，原始中英文/换行/路径诊断逐字保持。仍是1条检查，未新建/删除测试；未新增单独UI测试。
- Mac默认构建、Windows MSVC类型检查与git diff --check通过。
- 现有无GUI监督器定向suite 13通过，1个既有子进程fixture入口按设计忽略；实际覆盖拥有的进程树/并发关闭、终端入队、Node启动和协议字段。该suite不包含主界面错误渲染，不能用其代替GUI验收。
- 当前新原生程序带英文偏好启动并连接源码runtime，恢复项目/失效会话选择，SQLite导航保存得到确认。使用本轮新硬链接/资源软链接fixture，未复制依赖。脚本SIGTERM收尾，不是正常用户退出；退出后lsof目录为空、无残留fixture进程，已只删除本轮链接目录并保留日志。

未跑全量测试：本批跨UI模块的类型/显示迁移，保留原操作与诊断，采用上述语言/真实生命周期及原生启动检查。没有模型请求、后台协议/存储改动或新ZIP。CUA再次明确报告Mac锁屏；未实际点击语言切换、失败重试、错误关闭/滚动或正常退出，完整桌面/跨平台体验未交付。收据`.artifacts/language/main-status-receipt.json`。


## 带说明的错误、批量提示与聊天残项

第十四批累计1027条英文目录（新增58条）。ErrorNotice增加本地Context（标题/原错误/操作说明）和Batch（按原顺序的条目）；退出清理失败、工作区移除部分失败和附件批量错误不再在发生时拼接译文。操作结果判断、workspace_removed/缺失身份/刷新失败分支、错误正文、条目顺序、附件名/计数/20MB限制及退出时保留owner/窗口规则不变。系统/provider原错误仍作为Raw；本地静态失败和下一步说明在render选语言。项目启动/会话创建提示保留实际目标/后端及原诊断。

同时补齐历史/当前/待处理失败卡片和原消息重试操作、Diff解析失败提示、模型选择器、排队消息与聊天角色/空/加载/原文按钮。Diff解析只保存NoFiles或原解析错误，到渲染才选语言，原补丁和raw视图条件不变；没有把App/locale依赖引入解析模块。模型ID/名称/搜索、队列过滤/投递、复制正文、预览图字节、消息原文和原生错误均保留。原生无文件名图片的默认保存名按打开时语言生成，真实附件名不改。

成员汇报标题从缓存的整段String拆为静态heading key与原worker_name，render才拼接；原成员名即便与文案key同名也不翻译。DeliveryOrigin/角色/派单与报告身份规则不变。导出capture调用当前heading_text后冻结为原有Snapshot条目，之后仍使用捕获语言；此前“已有投递标题无法随语言更新”的缺口在该结构化路径已修复。缺失来源的原始fallback正文及部分原生/文件诊断仍可能中文，完整覆盖仍未证明。

本轮对源码中文literal做启发式候选扫描，由此发现并修复模型选择器入口与动态scope的“项目”英文目录遗漏。候选包括中文fallback分支、原生正文和诊断，不是缺陷数量、自动覆盖率或验收结果；原候选收据`.artifacts/language/source-literal-candidates.json`保留以供后续人工分类。

验证：

- 扩展既有language::tests，逐字验证zh/en的Context/Batch保留原诊断/换行/顺序及操作说明，汇报成员名与key同为“成员汇报”时仍保留原名；1通过。Diff原有2条通过，细化坏补丁错误类型断言；导出原有1条通过。共4条，未新增/删除测试文件或测试，修改的是既有真实格式/数据检查。
- Mac默认构建、Windows MSVC类型检查、git diff --check和本批显式文案key目录核对通过；未跑全量测试，本批为显示表示与UI元数据，不改后台协议或持久化格式。
- 既有file-attachments-wire.ts生产JSONL上传/解析/路径、删除源文件后副本字节、跨工作区与非法ID及正常关闭回归通过。
- 既有workspace-removal.ts真实HTTP/SQLite、原样Pi进程、实际工作流/目录lease、原进程退出、其他项目继续可用、SQL回滚/部分失败/转录与待处理spool保留通过，无模型回合。其首轮在空隔离Pi目录因Unknown model失败，补目录后又因No API key失败；两次均保留失败记录，不称产品回归通过。最后使用本轮一次性正常models.json登记openai/gpt-6.1-terra，端点为127.0.0.1:9，凭据为明确的合成占位值，原检查重跑通过。该配置只用于注册/切换/启动上下文，不是Terra能力/规格/真实认证或推理验证，没有请求厂商。未改测试代码、断言或生产校验/fallback。
- 配置准备中曾因SDK已创建auth.json触发“文件不应存在”的保护断言；没有覆盖该文件。实际生效的是models.json中的合成apiKey，已确认其值；随后检查退出0。夹具路径`/var/folders/v1/6vx82zzx32q7y_s1w4sx84j00000gp/T/hive-removal-locale-agent-2jirdu34`，成功业务收据另存`.artifacts/language/context-workspace-removal-receipt.json`。
- 新原生源码带英文偏好启动恢复与SQLite导航保存通过。仅用硬链接/资源软链接，脚本SIGTERM结束自有探针；退出后目录lsof为空且无fixture进程，只清理本轮native-bundle，保留日志。不等于正常用户退出或实际UI语言切换。

CUA再次明确报告Mac锁屏，未实际观察聊天/错误/模型浮层的英文布局、切换/滚动/复制/重试/窄窗或输入法，也未取得完整Cindy对标验收。无模型请求、无新ZIP，旧运行应用和候选/current包保留。完整#17和产品目标继续开放。阶段收据`.artifacts/language/context-chat-receipt.json`。


## 会话查找、移除确认与剪贴板提示

第十五批累计1046条英文目录（新增19条），补齐会话查找的输入提示/计数/上下一条/关闭快捷键，工作区移除的标题/按钮/完整数据保留与不可撤销说明，待处理汇报状态，以及动态key“设置已保存”。确认框仍按原按钮索引判断，取消默认和移除分支不变；工作区名/路径原样。待处理汇报仅在显示处翻译status()的静态值，协议模块不依赖locale，processing布尔值不改。

查找输入接入已有locale更新入口，只set_placeholder；不改query/hits/selected/返回焦点或滚动，搜索正文、思考/工具、原始投递与诊断的规则和键盘行为不变。未将源码候选等同缺陷：本轮保留已有双语fallback、语言自名“中文”、搜索中文关键词及原始技术诊断。

Diff内部changes函数接收打开时捕获的english_labels，合成“改动片段”及多片段标题可显示英文。数据模块仍不读取全局locale；原生patch、真实路径前缀、old/new文本保持原样，只有本机合成片段标签按打开语言生成；已打开文档的合成标题随该次内容固定，外围控件仍按当前locale显示。所有内部调用显式传入该值，没有为测试增加生产fallback。

剪贴板prepare以ClipboardTooLarge/UnsupportedFormat/EncodedTooLarge/原始解码错误返回，显示层才转为静态Message或原Raw；格式列表、64MiB源限制、3,750,000字节编码前限制、PNG原字节保留及BMP/TIFF转换不变，没有把App/locale依赖引入错误类型本身。

验证：

- 现有image_input::tests 1条通过，真实PNG/BMP/TIFF像素/原字节与损坏/超限/不支持格式；将两个笼统is_err断言改成类型匹配。
- 现有Diff数据2条通过，补英文合成标题、保留中文真实路径、原生patch逐字不变和原中文模式断言。现有语言偏好1条通过。共4条，未新增/删除测试文件或测试。
- image-attachments-wire.ts生产JSONL与真实Hive上传/SQLite检查通过，无模型请求。
- Mac默认构建、Windows MSVC类型检查、git diff --check通过。未跑全量测试：本批为UI文案与本地图片错误/合成标签表示，未改runtime或持久化协议；没有重复其他无关集成检查。

本轮没有重跑原生启动探针或生成新ZIP。CUA再次明确报告Mac锁屏，未实际点击查找/确认框或验证语言切换中的查询/输入法/焦点、真实Windows剪贴板和Diff窗口布局。原生/系统原诊断及少数源记录fallback仍按原值保留，完整语言覆盖与Cindy体验尚未验收。旧运行应用与包保留，.artifacts约3.8GiB。阶段收据`.artifacts/language/find-clipboard-receipt.json`。
