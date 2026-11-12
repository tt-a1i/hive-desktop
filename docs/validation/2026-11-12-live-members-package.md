# 活动成员实证与645c9a5开发包

2026-11-12。源码/包固定点 `645c9a55f177a353725629cd92a6c6531be2c9af`。本批补上活动新Worker验证，并把成员标签与读取失败修复纳入优化Mac ARM64开发包；完整产品目标继续开放。

## 真实协作

现场 `.artifacts/team-source-645c9a5`。原始Codex CLI 0.167.0、gpt-6.1-terra、medium，独立状态与Pi agent目录，不改用户全局模型或认证。隔离项目建在 `/tmp`，避免继承Desktop仓库指令导致此前的不存在文档读取失败。

源码原生窗口只发送一次768字符任务，先逐字校验磁盘草稿。主控创建proof-alpha/beta并派单；在两者工作时捕获原生完整控件树：协作页均Submitted，成员页两名Working、各1条未结派单，成员会话正在执行且名称正确，无Historical conversation后缀。证据为`live-collaboration-first.ax.txt`、`live-members.ax.txt`、`live-alpha-conversation.ax.txt`。这是实际新成员执行，不是旧历史恢复或合成事件。一次ScreenCaptureKit捕捉失败后重读同一窗口成功，未重启任务。

两派单均reported，两条显式汇报投递至原主控；三会话各1轮，未因两汇报额外启动空回合。alpha/beta分别为精确27/26字节且无末尾换行。主控独立Python同时断言两文件，原生工具日志中同call_id的真实输出为exit_code 0、`BYTE_ASSERTIONS_OK`；原始调用与输出另存`independent-assertion.json`。只有一条主控最终回复`HIVE_TWO_WORKERS_COMPLETE`。

源码正常退出exit 0。用新包内后台独立冷读三会话，随后由包内原生程序重开，报告、最终回复、成员名称及未发送草稿`Source 645c9a5 · 未发送协作恢复草稿 🐝`恢复，未发送/resume，再正常退出exit 0。按chatId逐字段核对原生轮数、完成投递、台账、文件保持；数组顺序变化不当作数据丢失，也不声称JSON数组逐字相同。进程树证明Node/runtime来自新包，退出后记录的所有拥有进程均不存在。

真实推理3轮发生于同提交源码；包内重开是只读恢复，未新增模型回合。不能将该验证说成已安装新包的新推理，或完整后端组合兼容性证明。

## 打包与代码验证

优先通过代码检查协议、SQLite、原生会话日志、进程与归档；原生操作只覆盖活动标签及关键恢复/退出边界。无需为每个后台场景反复操作面板。

- `cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`通过，优化release。
- 冻结依赖代码刷新：对293afe3的锁文件、workspace、patch、Rust依赖及生产package元数据无变化；116个runtime文件逐项匹配当前dist，28,869清单项和29个内部链接匹配。ZIP解压前后完整清单一致，唯一变化是原生二进制。
- 新解压包四组命令并行执行且均退出0：`node scripts/check-package.mjs --bundle .artifacts/release-645c9a5/extracted/bundle`、`check-packaged-codex.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`（后3命令同一`--bundle`）。包含真实PTY、HTTP/SQLite、派单、Codex生命周期及内外Pi队列；协议合成对端不冒称厂商推理。
- DMG由`node scripts/package-macos-dmg.mjs`生成，`hdiutil verify`通过；脚本只读挂载后核对完整App清单和Applications链接，再正常卸载。
- Applications旧副本先核对293afe3完整清单及无占用，再经独立临时副本完整校验后替换，安装副本与新解压包完整清单一致。此副本本轮未再启动原生窗口，不宣称Finder安装/启动交互验收；同字节解压包的原生启动、包内后台路径和正常退出已验证。
- 沿用上一轮同提交的Windows MSVC目标类型检查。纯UI源码已做定向原生检查；本轮无生产代码/测试变更，未跑无关全量suite或重复TS检查。

## 当前产物与边界

current/candidate ZIP为同字节硬链接，143,903,643字节，SHA256 `68fea2bb936e2fd0b6be4e63e3b6c29d33e47e97898854140d68d2088c513291`。DMG、Applications与ZIP均为645c9a5；权威收据在`.artifacts/release-645c9a5/receipt.json`及`.artifacts/current-package/`。旧293afe3 ZIP及可运行解压包保留，清理仅限核对完整清单且无占用的本轮base和临时安装备份，详见cleanup.json。

包仍未签名/公证，未验下载隔离Gatekeeper；Windows原生、Pi OAuth重登、Claude真实模型、系统通知授权/送达、全部组合与完整UI/IME/辅助技术/性能验收继续开放。活动成员标签已有本轮新实证，但一次正常过程不证明任意刷新时序无缺陷。
