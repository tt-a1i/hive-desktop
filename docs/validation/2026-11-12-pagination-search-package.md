# 3ed07c9开发包：协作分页与后端搜索

2026-11-12。当前/candidate ZIP、DMG及Applications安装副本更新为`3ed07c95262d5690bb38ffa48633368ef6b4bab9`优化Mac ARM64开发包。纳入协作整页分页和英文后端名称搜索修复；完整目标继续，未声明产品验收完成。

## 验证

- [分页证据](2026-11-12-collaboration-pagination.md)：真实HTTP/SQLite检查20/21/40条与完整升序身份；源码原生只抽查20条禁用、自动追加21后启用、第二页仅21及退出。Mac debug/release构建与Windows MSVC类型检查通过。
- [搜索证据及账号边界](2026-11-12-search-labels-and-auth-boundaries.md)：本批包内Node运行`check.mjs ABS_RUNTIME packaged-3ed07c9 --require-pass`，External Pi、Built-in Pi、外部 Pi、内置 Pi、Claude Code、Codex六组搜索各命中唯一正确后端；无Agent进程或模型请求。
- 四组包检查全部exit 0：`node scripts/check-package.mjs`、`check-packaged-codex.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`，均传`--bundle .artifacts/release-3ed07c9/extracted/bundle`。包含真实PTY、HTTP、SQLite、派单/关闭、队列及协议恢复。合成JSONL对端及本地响应只证明对应协议，未充当真实厂商推理。
- 包内Node只读冷开此前645c9a5真实Terra协作三会话，原模型、投递身份、汇报来源保持，pending=0、pendingReports空；没有发送/恢复执行或新推理。推理证据仍属于原645c9a5。
- 解压包使用独立空数据原生启动欢迎页；进程树确认包内Node/runtime，无开发路径覆盖。Cmd+Q退出0，App及后台PID均消失。没有重复全页面点击，本批未原生浏览旧三会话。
- 冻结依赖与生产元数据未变，当前116个runtime文件、28,869条完整清单及29个内部链接一致。相对3a3f873仅native、chats.js及map变化。DMG校验及只读挂载清单/Applications链接通过；安装前核对旧副本完整清单及无占用，新安装副本清单等同已运行解压包。本轮未另启动安装副本。

证据目录`.artifacts/release-3ed07c9/`保留组装、各组日志、搜索/冷读、原生AX与进程退出、DMG、安装及receipt。ZIP为143,919,457字节，SHA256 `4cc303698fb7d105d918db8da8cc6356ad696ab45bcc82213dcd6edbc1f82c87`，current/candidate同字节硬链接。

## 范围

本批无正式测试增改删；执行定向脚本及包检查，未跑无关全量suite。构建和协议检查不证明完整体验。仍为未签名/未公证开发包，Windows原生、系统通知实际送达、完整输入法/可访问性/性能及真实混合推理仍待验证。Pi OAuth重登由用户暂缓；真实模型继续仅Terra，Claude真实推理未验收。

经完整清单及无占用核对，清理本批重复base与上一包临时安装备份；保留3a3f873完整解压回退、旧ZIP、fixture状态、脚本、日志及收据。未清理共享Rust缓存或用户项目数据，清单见cleanup.json。
