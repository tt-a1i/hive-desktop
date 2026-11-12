# 46659e2开发包：重新部署Hive依赖与团队预览

2026-11-12。current/candidate ZIP、DMG及Applications更新为`46659e28b2e343dbc27334d51843eca6aa516517`的Mac ARM64开发包，包含[团队任务预览Unicode修复](2026-11-12-unicode-team-preview.md)。本批按新补丁重新部署生产依赖，没有沿用1534986f冻结树；今后的冻结基线为本提交。原生源码/锁文件及二进制与前包保持，未重复构建Rust。

## 部署与完整性

- `pnpm --filter @hive-desktop/runtime deploy --prod --frozen-lockfile --offline --node-linker=hoisted .artifacts/release-46659e2/deployed-runtime`通过，离线复用381个缓存包，无依赖版本升级。原仓库构建脚本策略保持，部署日志中的忽略脚本提示保留，没有自动运行approve-builds或扩展后端支持声明。
- 正式`node scripts/package-desktop.mjs --executable apps/desktop/target/release/hive-desktop --runtime .artifacts/release-46659e2/deployed-runtime --output .artifacts/release-46659e2/base/bundle`通过。官方Node22.23.3 darwin-arm64重新下载并依SHASUMS256验证，archive SHA256 `23b25245dcfb9af7262f8ff142e9e2e0af025368117329e7a7458a51e5922f53`；所有链接留在包内。
- source/deployed/extracted的Hive routes-workspaces模块SHA256完全相同：`e728ba83da38a71f6236587db88006217281a868c0eabdcf2a6065f7fd1bdd18`。116个runtime文件与当前dist一致；28,869条完整文件/模式清单及29个内部链接在独立ZIP解压后保持。
- 相对db12698，完整清单精确只有六项变化：Hive团队路由、runtime/package.json、两个部署lock文件、.modules.yaml和.pnpm-workspace-state-v1.json。原生程序、Node、其他依赖及资源文件字节均相同；不是只检查顶层版本号。

## 实际验证

四个现有包检查由gates.py实际执行并记录returncode，均exit0：`node scripts/check-package.mjs`、`check-packaged-codex.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`，参数统一`--bundle .artifacts/release-46659e2/extracted/bundle`。包含包内Node、真实PTY、HTTP/SQLite、派单/退出/恢复及队列，本地合成对端不冒称厂商推理。

包内check-team.mjs启动正式main.js，经真实JSONL/team.members/HTTP/SQLite读取已有隔离queued任务，保存未经重封装的stdout行。任务预览精确59个x加完整🐝，同版Rust serde_json解析和60字符断言通过。原生窗口实际打开成员页，显示Unicode worker、已停止及1项未结派单；正常Cmd+Q exit0，记录的App/包内Node PID均消失。DB仍workspaces1/workers1/dispatches1，完整任务原文不变，messages/dispatch_messages/agent_runs均0。此任务是序列化夹具，不是模型新派单。

包内Node另只读冷开先前645c9a5真实Terra三会话，原模型、投递身份、汇报来源及pending=0/pendingReports空保持，未向这三会话发prompt或触发chats.resume回合。真实推理证据仍归原提交，本批无厂商模型请求。

DMG经verify及只读挂载，与已验证ZIP的完整App清单/Applications链接一致并正常卸载。旧安装副本清单及无占用核对后，经临时副本校验再替换；新Applications与已启动解压包完整一致，安装副本本轮未另启动。

ZIP为143,935,513字节，SHA256 `5fb26f76c6180c296c3310391341b35cb42071fda3607992c23af37cf21eaccb`；current/candidate为同字节硬链接。原始部署/官方Node/组装/完整差异、实际包检查、JSONL/Rust、原生AX/进程/退出、三会话冷读、DMG/安装及receipt保存于`.artifacts/release-46659e2/`。

## 清理与边界

完整清单及无占用检查后，清理重复base、临时安装备份，以及与包内runtime逐项相同的独立deployed-runtime树；保留db12698完整回退包、旧ZIP、fixture、源补丁/锁文件、日志和收据。部署树已删除，重跑该阶段需按保留的命令重新离线部署；没有清理共享Rust/pnpm缓存或用户项目，清单见cleanup.json/cleanup-deployed.json。

本轮无生产源码或正式测试变化，按依赖更新风险执行定向包检查，未跑无关全量suite。未签名/未公证，Windows真机、完整输入/VoiceOver、生产网关/通知送达及正式发行仍未验收。Pi重登暂缓、真实模型继续仅Terra，Claude真实推理未验收，完整产品目标继续。
