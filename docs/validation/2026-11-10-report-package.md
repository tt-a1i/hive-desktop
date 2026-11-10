# 汇报队列与欢迎摘要进入 Mac 开发包

2026-11-10，源提交`1e68306651e5f9f43b04b832c8e1bc03a6ac3d76`。current/candidate、DMG与Applications均已更新。本轮是开发包验证，不表示完整产品交付。

## 纳入内容

- Codex成员汇报可在当前活动回合通过原生steer消费，避免正常一对二协作结束后额外运行两个空回合。源码真实Terra/medium三会话各1轮、显式汇报、主控独立只读字节断言、唯一最终答复及冷恢复见[修复证据](2026-11-10-codex-report-steering.md)。
- 欢迎页分别显示版本和连接/账号检查，复用设置页结果与过期条件。源码中英文20px/路径变化/未登录及冷恢复见[欢迎摘要](2026-11-10-welcome-account-summary.md)。

## 包一致性

优化`cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`实际exit0。复用0080304经验证的同平台冻结依赖，锁文件、工作区、补丁、Cargo及生产package元数据逐项不变；不是新的依赖部署。相对该包仅原生程序、`delivery-queue.js`和对应map三个条目变化。完整116个runtime输出、28,869个文件/模式/链接清单条目与ZIP解压结果精确一致，29个内部符号链接均保留。

- ZIP：143,911,374字节，SHA256 `a21ae2d846ef0c70e00f9253da7d7c44b75ae6b13002090e6e031b4c95102375`。
- 原生程序SHA256：`ac73d336d256abf2cc182cb87954112fe6f78f8a63f03da0ff068ef39c445ca1`。
- DMG：231,984,129字节，SHA256 `c19cb381da77dace64eaa007d986053f88b82044f1f3e787dc7d02a7b7afaa9f`；hdiutil verify、只读挂载完整清单与Applications链接检查通过，已卸载。

current和candidate是同字节硬链接，路径分别为`.artifacts/hive-desktop-macos-arm64-current.zip`与`.artifacts/hive-desktop-macos-arm64-candidate.zip`。收据在`.artifacts/current-package/{receipt,candidate-receipt}.json`与`.artifacts/release-1e68306`。

## 实际包内检查

以下全部实际exit0，运行对象为解压包的Node、生产模块与依赖：

```sh
node scripts/check-package.mjs --bundle .artifacts/release-1e68306/extracted/bundle
node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-1e68306/extracted/bundle
node scripts/check-packaged-codex.mjs --bundle .artifacts/release-1e68306/extracted/bundle
node scripts/check-packaged-queue.mjs --bundle .artifacts/release-1e68306/extracted/bundle
```

真实PTY/HTTP/SQLite/关闭、派单结算与原投递恢复、24组system/report原生追加、Codex设置/历史/取消失败回收，以及内外Pi队列顺序/附件/停止/暂停冷恢复通过。厂商JSONL部分使用明确的合成协议对端；Pi队列使用本地受控响应，不调用真实模型。本轮无新模型请求；源码同提交的真实Terra协作与包内冷恢复分开记录。未改动的远程/账号模块沿用0080304对应包内gate，不冒称重跑全部套件。

## 原生、冷恢复与安装

- 解压包直接启动，无Node或runtime源码override。独立零项目数据、中文20px，检查原样Codex0.167.0版本和只读账号，返回欢迎页两项结果同步可见；保存AX/截图。没有登录/凭据刷新/模型执行。捕捉偶发失败与一次输入冲突原样保留，重新取得状态再操作。
- 原生PID97288、包内Node97296，Cmd+Q后所属启动会话exit0，两进程消失。独立SQLite确认五张业务表仍全0。
- 独立包内后台仅`chats.open`此前真实团队的三个会话，不resume/不prompt；pending0，文件、台账、原生轮数、投递及完成ID逐字段保持，`baseline.json == after-package-cold.json`，没有额外模型唤醒。此项是实际原生历史读取，不是包内重新执行一轮模型协作。
- Finder从已校验DMG实际选择Replace更新`/Applications/Hive Desktop.app`；安装后的28,869清单项与新包一致。
- 通过CUA明确应用路径启动已安装应用（本轮不是Finder双击启动），欢迎页加载后可用；原生PID31017、安装目录内Node31032。Cmd+Q后窗口与两进程消失，LaunchServices启动不补记未知退出码。默认数据workspaces/messages/agent_runs/dispatches/workflow_runs前后均0，未改默认CLI路径或全局账号。

## 存储与边界

清单相同且lsof确认无占用后，删除本批重复base和第三旧021c162的解压应用，共1,077,923,840 allocated bytes（约1GiB）。021的ZIP/全部证据与项目数据保留，收据注明bundlePresent=false；0080304的ZIP、收据及可运行解压应用作为上一版回退保留。本轮后`.artifacts`约9.1GiB，Rust target约4.9GiB，不删除活跃目录或作为旧聊天cwd的项目数据。

未签名/未公证，下载隔离属性/Gatekeeper发行路径、Windows原生、系统通知授权/送达/点击、完整UI/输入法/辅助技术/性能及全部厂商组合仍待验收。源码定向回归与本轮四组包内gate匹配改动风险，不跑无关全量suite；完整目标#1继续开放。
