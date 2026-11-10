# Codex 配置与空会话修复开发包

2026-11-10，源码008030443769812a91e805d1ae50288fc786ca1b。current/candidate ZIP、DMG和/Applications/Hive Desktop.app已更新为同一批优化release。它是开发包更新，不是完整产品验收。

## 本批纳入

- b90c18d：默认关闭的本机提醒接线/偏好/设置。OS送达与点击授权仍待回复，未触发系统通知；不能将打包当作#25完成。
- c3aeb18：命名但尚未发送的会话显示正常会话标题，未命名空会话仍用欢迎页；提醒复用既有阅读页面判断。
- 9dc5fc4、0080304：独立Codex原生连接/脱敏账号只读检查、未登录时的准确路径登录命令及复制；复用远程终端命令转义。

Pi/Claude/Codex对话适配器、模型/派单模块与生产依赖没有变化。最近的原生Codex Terra真实回复证据仍归[提醒关闭复核](2026-11-10-local-notifications.md#后续关闭提醒时的真实模型与冷恢复)，本批没有新增模型回合。

## 组装、清单和包内检查

cargo build --release --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop、pnpm build:runtime通过。生产依赖/补丁/Cargo锁/应用资源及身份校验与021c162保持；runtime package.json仅更新检查脚本元数据。包中仅原生程序、main/desktop-settings/remote-commands/codex-account/terminal-command的JS及map和该元数据共12条改变。

116个runtime输出与本仓编译产物精确一致。ZIP独立ditto解压后，28,869个条目的路径/mode/SHA和29条内部软链接逐项匹配；不是只比较文件数。DMG生成UDZO、hdiutil verify通过；只读挂载后app同一清单，Applications链接为/Applications。安装后已卸载。

本批实际重跑：

- node scripts/check-package.mjs --bundle .artifacts/release-0080304/extracted/bundle：plist/图标、包内Node、真PTY、Pi账号元数据、Hive HTTP/SQLite工作区与关闭通过。
- node scripts/check-packaged-remote.mjs --bundle同上：共享命令helper改动后的包内CLI、特殊路径数据隔离、回环登录/退出、真实WebSocket加密配对/SAS、撤销故障及恢复通过。不是生产网关或真实账号验证。
- .artifacts/release-0080304/check-account-package.mjs：只将本仓两份行为检查编译并绑定包内生产模块/Node，检查真实argv执行/转义、合成JSONL账号变体/超时/取消/回收、真HTTP/SQLite路径优先级/不保存/关闭排空，以及原样0.167.0真实账号只读和0.46.0连接失败。合成对端与真实CLI证据分列，没有thread/turn或模型请求。

三组退出码均0。未改/删生产源码或自动化测试，本轮新增的绑定检查脚本存入artifact供复验。未重跑无改动的packaged-models/codex/claude/dispatches，沿用46373f6实际记录仅限未改变的模块和冻结依赖；收据unchangedAdapterReference标明rerunThisBatch=false、runtimeUnchanged=false，不能再称整个runtime不变。未跑无关全量suite；Windows仅沿用最近源码目标检查，非原生包验收。

首次并行准备中runtime日志重定向早于目录创建而失败；确认终态后先创建目录再执行，最终build:runtime退出0。没有因观察超时重启仍活跃任务或添加生产兜底。

## 最终ZIP解压app原生验证

直接运行最终ZIP解压app，仅传自有数据/Pi目录和离线标记，显式清除Node/runtime/Agent命令覆盖。原生PID93007，子Node93027，完整进程路径均指向该包Contents资源，父子关系已确认；Cmd+Q exit0，两PID消失。

- English/light/20px、约593×507：收起侧栏，命名空会话“未发送草稿”标题/重命名入口及精确ONBOARDING_KEEP · 保留未发送草稿可见。packaged-title.png/AX。
- 设置路径Tab显露，输入本轮隔离空Codex目录的原样0.167.0启动脚本，检查实际显示未登录。Tab到复制按钮、Return复制，回聊天Cmd+V逐字等于正确引用的检测路径加login；随后恢复原草稿，未发送。packaged-login.png/AX、packaged-copy.ax.txt。
- 原生Reload恢复原保存程序路径、Save禁用、旧命令隐藏。本机提醒实际off且测试按钮禁用，未开启或点击Test。Escape回原草稿再退出。packaged-notification-off.ax.txt、packaged-draft.ax.txt。
- 使用该包Node/runtime独立冷读：原程序配置、会话ID/标题/Terra、空历史、pending0/busyfalse、精确草稿保持；隔离Codex auth.json不存在，SQLite workspace1，其余messages/agent_runs/dispatches/workflow_runs均0。packaged-cold-receipt.json。没有resume或额外模型执行。

## Finder安装与默认数据

安装前确认旧/Applications app精确匹配021c162清单且未运行；旧可运行解压包保留。Finder从只读DMG复制，出现“较旧项目”提示后选择Replace，拷贝完成，安装目录28,869条逐项匹配。通过Finder打开安装app：品牌欢迎、四Agent配置、默认未检查、Codex直达正确字段与新增连接检查入口可见；未填写或保存默认配置。

实际原生14695、Node20645均来自/Applications/Hive Desktop.app资源。Cmd+Q后app清单无运行，两PID消失；Finder启动没有exec句柄，因此exitCode=null，不伪写0。默认数据前后workspace/messages/agent_runs/dispatches/workflow_runs均0。installed-manifest-receipt.json、installed-process.json、native-install-receipt.json及安装截图/AX留存。

## 固定产物与清理

- ZIP143,910,191字节，SHA256 0102f1be51ea2d8c77dd40ab8c8e84fa7c4bfa82c39362f80aef34012518a146；current/candidate同字节硬链接。
- DMG240,495,742字节，SHA256 bb8334340724c92c1ebadb6169aabb4d4c0372e52a31561fd50284f1a76beee4。
- 原生程序SHA256 35f991e10e90c9320e228249659ed29569658d3be8fe4464ba2c3f0b4929d069。
- .artifacts/current-package/{receipt,candidate-receipt}.json为当前收据，原始证据.artifacts/release-0080304/。

旧021c162 ZIP归档至其release目录，其完整可运行解压app保留为最近回退。新base重复目录和第三旧46373f6解压bundle在全清单校验且lsof无引用后移除，分配块合计1,077,944,320字节（约1GiB；APFS物理回收不保证等量）。旧ZIP/收据/测试状态/项目/日志/聊天工作目录均保留，对已移除bundle的收据标记bundlePresent=false。.artifacts本轮约8.7GiB，Rust target约4.9GiB，未删除仍在使用的构建产物。

本包未Developer ID签名/公证，未验证下载隔离Gatekeeper、OS提醒送达/点击、完整输入法/VoiceOver/性能/手机/Windows原生或所有混合后端组合。#25/#26与完整目标继续开放，不把本次安装更新当作这些边界已完成。
