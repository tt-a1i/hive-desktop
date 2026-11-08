# 界面语言：偏好与首批文案

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
