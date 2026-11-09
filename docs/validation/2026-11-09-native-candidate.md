# 最新原生改动的Mac候选包

源码17d8ce7877328cd219d121fa46b09bb1a7f3698f，包含界面语言、原生主题同步、导出AX提示与固定页脚。未签名、未公证的macOS arm64便携开发包，不是正式安装器，完整目标未交付。

## 归档及内容

- 候选：`.artifacts/hive-desktop-macos-arm64-candidate.zip`，约164MiB；SHA256 `006a8e689f020d505a5ac0be24765139f4ab0b345d247d8cc90ac2f429a0828e`。
- 原生程序：SHA256 `705ab9453aa49332cf4727dc01200c48af24934cb96d898d9bd7cbb38a1ce109`，与上一轮实际阅读验证的构建相同。
- 以d9e061b候选为冻结基线，仅替换原生可执行文件。Git确认runtime源码、清单、锁文件、patch及Cargo依赖未变；不是本轮fresh deploy。
- 独立解压后核对112个runtime编译文件与源码dist一致；28,833个未变普通文件逐一SHA256匹配，29个内部链接目标/字节匹配且解析在应用内。31,570个AppleDouble元数据条目在新旧ZIP中逐字一致，归档testzip通过。
- 初次检查误将ditto已合并为扩展属性的`._`条目当普通文件，因不存在而失败；修正分类后，跨归档复用旧ZipInfo偏移又导致读取Bad magic。改为按文件名查新归档，验证通过。ZIP完整性检查此前已通过，未把这些失败归为产品损坏，也未改生产fallback。

## 实际包内检查

Mac `cargo build --locked`通过。通过包内Node执行`check-package.mjs`：真实PTY、Hive agent-manager自然退出/停止、HTTP/SQLite、配置元数据、四后端会话记录及shutdown通过，无模型请求。

通过包内Node执行`check-desktop-startup.mjs --language en`：真实协作记录、保存项目及缺失会话选择恢复通过。脚本最后SIGTERM清理启动probe，不算正常用户退出；下面另有实际正常退出证据。隔离Pi配置固定openai/gpt-6.1-terra及不可达回环端点，不使用真实账号或厂商推理。

实际启动从ZIP解出的应用，删除环境中的Node/runtime覆盖。进程命令行确认子进程84062使用包内Node及包内dist/main.js，未通过源码目录兜底。

原生窗口取得：英文→中文→英文切换，原草稿保持；浅色内容与原生标题栏同步；1184×1104截图像素窄窗导出范围说明/按钮/AX复制与保存提示可见；真实系统保存的11,576字符、1200段及末尾与源码快照逐字一致（SHA256 `6095efbaac24b4aa75721a7e91849d498f6acccb583ba7fecbabc60377ab7a5e`）；成员产物review.txt可读到第200行及READING-END。

实际Cmd+Q，84058退出0、Node84062退出，shutdown成功；再次冷启动恢复英文、浅色和原会话草稿，恢复隔离偏好为System后点击关闭窗口，87033亦退出0，后台清理。SQLite messages仍为0，两个UI日志无chat.prompt。

## 保留范围与清理

本轮没有重复未变runtime的全部远程/CLI/模型检查；d9e061b的远程/加密终端记录仅作为内容不变的历史证据，不冒称新ZIP中重新执行。完整页面/输入法/键盘/字号/VoiceOver、远程原生操作、工作流最终删除、真实账号/模型与Windows仍未验收。不推广current。

正常退出后确认进程表及lsof无引用，再替换唯一候选ZIP槽位并删除本轮解压目录；不动旧聊天目录、其他应用或依赖缓存。`.artifacts`约3.9GiB，无额外候选/依赖树副本。current ZIP仍是1acd805，SHA256 `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`不变。旧d9候选的收据保留supersededBy及zipPresent=false，旧哈希不代表现存文件。

收据：`.artifacts/native-package/{receipt.json,runtime-process-receipt.json,export-receipt.json,ui-exit.json,ui-reopen-exit.json,cleanup.json,superseded-candidate-receipt.json}`；当前候选索引`.artifacts/current-package/candidate-receipt.json`。日志`/tmp/hive-candidate-latest-{build,package,verify,smoke,startup}.log`，原生日志在native-package目录。本轮无产品/测试代码改动，未运行无关全量测试。
