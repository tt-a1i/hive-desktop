# 3a3f873开发包：诊断复制与会话错误归属

2026-11-12。当前/candidate ZIP、DMG及Applications安装副本更新为`3a3f873e88f7dd1cbd8bab191842476a17ffc30c`优化Mac ARM64包。纳入原生设置失败原因、Web诊断复制入口和会话打开错误隔离；完整产品目标仍未完成。

## 验证范围

- 源码原生诊断复制与完整剪贴板JSON核对见[诊断入口](2026-11-12-remote-diagnostics.md)，失败A→健康B→失败A的原生提示归属见[会话错误](2026-11-12-conversation-open-errors.md)。两者只做对应路径抽查，不将局部流程当作全界面验收。
- `cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`通过。冻结依赖、workspace、锁文件、patch及生产元数据不变，116个runtime文件匹配当前dist。完整28,869条清单及29个内部链接在ZIP解压前后保持；相对645c9a5唯一变化是native、chats.js及其map。
- 新解压包四组gate并行通过：`check-package`、`check-packaged-codex`、`check-packaged-dispatches`、`check-packaged-queue`，参数均为`--bundle .artifacts/release-3a3f873/extracted/bundle`。包含包内Node、真实PTY/HTTP/SQLite和协议/队列检查；合成协议对端不冒称厂商推理。
- 包内Node22另执行`settings-wire.mjs`，对比旧645c9a5/新包的生产JSONL响应：新包带Unknown Pi model原始原因；拒绝后的模型、原生身份、原始文件及有效模型重试保持，零模型请求。
- 包内Node22执行`cold-team.mjs`，真实读取此前645c9a5源代码Terra验证的三会话：模型/身份、完成投递及原报告来源集合保持，pending=0、pendingReports空。真实推理证据仍归原645c9a5，不冒称本批新推理。
- 新解压包独立空数据原生启动，欢迎页及设置入口可读；进程树确认实际使用包内Node/runtime。仅Cmd+Q退出，exit 0，记录的App与后台PID全部消失。不再逐页重复点击；三会话历史本批只验证后台冷读，没有声称在此优化包中重新原生浏览三会话。
- DMG经`hdiutil verify`及只读挂载，完整App清单和Applications链接正确，正常卸载。Applications旧副本先核对645c9a5完整清单与无占用，再经临时副本完整核对后替换；新安装副本完整清单等同新解压包，安装副本本轮未另外原生启动。

权威现场`.artifacts/release-3a3f873`保存组装、每组gate、设置对照、三会话冷读、原生进程/退出、DMG、安装及receipt。所有新增模型请求为0。当前ZIP为143,918,864字节，SHA256 `716be407b71f3a8ce3bd4e462a660880533537fdec53cba7504b36b220470d90`；current/candidate为同字节硬链接。

## 清理与边界

完整清单且无占用检查后，删除本批重复base和临时安装备份；另删除本会话创建、无占用的四个可重建开发App壳。各fixture状态、日志、截图、源脚本、收据及所有ZIP保留；旧开发壳不再作为可运行产物引用。645c9a5的完整解压回退包及ZIP保留，清理清单见cleanup.json；未清理共享Rust依赖缓存或项目数据。

本轮无新生产源码/正式测试变化，未跑无关全量suite；Windows类型检查沿对应源码记录，非Windows原生验收。仍未签名/公证，未验证下载隔离Gatekeeper；Pi重登后的真实推理、Claude真实模型、系统通知授权/送达、真实混合推理、完整输入法/可访问性/性能及Windows原生继续开放。
