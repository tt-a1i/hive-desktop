# db12698开发包：Unicode标题与旧数据恢复

2026-11-12，current/candidate ZIP、DMG及Applications安装副本更新至`db12698a02abca1e9a65178c79b73827eed68e9e`，纳入[自动标题根因修复](2026-11-12-unicode-auto-title.md)。原生二进制与3d4422f逐字节相同；本批只替换chat-index/chats两个runtime模块及map，不重复构建Rust，不改依赖/锁文件或生产元数据。116个runtime文件、28,869条完整清单及29个内部链接在ZIP解压前后保持。

## 实际包内验收

- `gates.py`实际运行四组现有包检查并保存returncode，全部exit0：`node scripts/check-package.mjs`、`check-packaged-codex.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`，参数统一为`--bundle .artifacts/release-db12698/extracted/bundle`。包含真实PTY/HTTP/SQLite、队列与协议；合成对端不算厂商推理。
- 包内Node/runtime实际prompt生成59个x+完整🐝的60 code point标题；保持原完整排队/附件/停止/冷开/继续断言。由原生同版本serde_json的Rust程序实际读取新响应，逐字及字符数断言通过。本地HTTP响应，无厂商请求。
- 复制旧版本四后端空记录，保留两条真实问题形态的残缺自动标题、30个完整🐝和自定义标题。包内只读API先验证标题归一化且原索引字节不变，再设置本批导航。原生两次冷启动均显示四条正确标题，自动标题残片去除、正常emoji和自定义记录精确保持；两次Cmd+Q均exit0。只打开内置Pi空会话，没有启动另外三种模型。
- 原生打开会话的正常元数据保存将内存中的修复标题一并落盘；原坏索引另存legacy-index-before.json。check-unicode.mjs对照原/现记录、两份完整AX、完整emoji及自定义记录，native-data-receipt核对仅workspaces1、workers/messages/dispatches/dispatch_messages/agent_runs均0。
- 两次进程树均确认使用包内Node/runtime且无开发路径覆盖，退出后记录的App及后台PID全部消失。无需重复页面巡检；没有恢复未授权的模型调用。
- 包内Node只读冷开此前645c9a5真实Terra三会话，模型/投递身份/汇报来源、pending=0和pendingReports空保持。该推理证据仍属于原645c9a5；未向这三会话发送新prompt或触发chats.resume恢复回合。
- DMG校验、只读挂载完整清单和Applications链接核对并卸载通过；旧安装副本完整清单/无占用核对后，通过临时副本校验再替换，新安装清单与已启动解压包相同。安装副本本轮未另启动。

ZIP为143,935,501字节，SHA256 `de79c9f9e7a0173ad38d5f9dd167c382400c32b9b3fdf057b1cf4589f511e1d2`；current/candidate为同字节硬链接。证据目录`.artifacts/release-db12698/`含组装、actual-gates、标题新输入、Rust解析、原坏索引、原生AX/进程/退出、旧三会话冷读、安装/DMG及receipt。

## 清理与边界

在完整清单和无占用检查后清理本批重复base和临时安装备份；3d4422f完整解压包及旧ZIP作直接回退，原状态/数据/日志和脚本保留，见cleanup.json。本轮未进一步清理共享Rust缓存或用户项目。

本批无新增生产源码或正式测试，按已修改模块执行定向包检查，未跑无关全量suite。仍是未签名/未公证Mac ARM64开发包；Windows真机、完整输入/VoiceOver/所有Unicode组合、下载隔离和正式发行、生产网关/系统通知送达继续开放。Pi重登暂缓，真实模型仅Terra，Claude真实模型未验收，完整目标继续。
