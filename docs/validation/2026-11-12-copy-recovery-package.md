# d644db9开发包：协作原文与历史恢复

2026-11-12。current/candidate ZIP、DMG及Applications安装副本已更新为`d644db9840a12b474393f8e5535761e9cbe341dc`的优化Mac ARM64开发包。纳入[协作原文复制](2026-11-12-collaboration-copy.md)和[Codex恢复查找优化](2026-11-12-codex-recovery-performance.md)，完整产品目标继续。

## 验证范围

- `cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`成功；对应源码的Mac原文复制到未发送草稿、逐字/尾部空格/代码块检查，以及Windows MSVC类型检查已沿上文记录。优化包本轮只抽查欢迎启动与退出，不重复各页面操作，也不声称已在优化包重新点击三类复制按钮。
- 四个现有包检查由`gates.py`并行执行并从真实returncode保存`actual-gates.json`，全部exit0：`node scripts/check-package.mjs`、`check-packaged-codex.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`，参数统一`--bundle .artifacts/release-d644db9/extracted/bundle`。含真实PTY、HTTP/SQLite、队列、退出及协议恢复；合成对端不代表厂商推理。
- 包内Node运行`check-recovery.mjs packaged`，1,000/5,000回合的完整恢复及真实写盘/重新读取断言通过，结果SHA256均与已验证源码相同。包内采样只用于确认执行和一致性，没有拿不同Node环境的计时扩大上一轮性能结论。
- 包内Node运行`cold-team.mjs`，只读冷开此前645c9a5真实Terra协作三会话，模型、完成投递身份与汇报来源保持、pending=0/pendingReports空；无prompt或resume执行。真实推理证据仍属于原645c9a5。
- 解压包使用独立空数据实际打开欢迎页，进程树确认包内Node/runtime且无开发覆盖；Cmd+Q exit0，App及后台PID消失。安装副本本轮未单独启动。
- 冻结依赖和生产元数据未变，116个runtime文件、28,869条完整清单及29个内部链接保持。相对3ed07c9只改变native、codex-projection.js及map（英文文案内嵌native）。DMG通过verify与只读挂载完整清单/Applications链接核对并卸载。
- 安装前核对旧Applications完整清单与无占用；临时副本核对新清单后替换，新Applications完整清单等同已启动解压包。current/candidate是同字节硬链接。

ZIP为143,913,734字节，SHA256 `d522768c994afe327f297ed29086e6bcb88e5a5908a1d568b811f71a29eda9ed`。证据目录`.artifacts/release-d644db9/`保留组装、包检查、恢复、冷读、原生AX/进程/退出、DMG、安装及receipt。

## 清理与仍待验收

完整清单及无占用核对后，清理本批重复base与3ed07c9临时安装备份，3ed07c9完整解压包和旧ZIP继续作为回退。保留状态、原生截图/AX、日志、脚本和收据，未清理用户项目或共享Rust依赖缓存，具体见cleanup.json。

本轮无生产源码或正式测试增改删，执行风险匹配的包内检查，未跑无关全量suite；无新模型调用。仍为未签名/未公证开发包，下载隔离Gatekeeper、Windows真机、完整输入法/辅助技术/大图性能、生产网关与系统通知实际送达未完成。Pi重登暂缓，真实模型继续仅Terra，Claude真实推理边界不变。
