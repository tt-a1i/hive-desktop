# 3d4422f开发包：汇报分层与图片解码

2026-11-12。current/candidate ZIP、DMG及Applications安装副本已更新到`3d4422f8cafd356ae1a3f51cc3d6b927ddbc09f8`的优化Mac ARM64开发包，包含[汇报身份分层](2026-11-12-report-heading-layout.md)与[图片预览并发限制](2026-11-12-image-preview-pressure.md)。完整产品目标继续。

## 包内验证

- `cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`通过。依赖/锁文件/生产元数据未变；116个runtime文件逐项保持，与d644db9相比只换native。28,869条完整清单、模式及29个内部链接在ZIP解压后保持。
- 四组现有包检查由gates.py并行运行、按实际returncode收据记录，均exit0：`node scripts/check-package.mjs`、`check-packaged-codex.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`，参数统一为`--bundle .artifacts/release-3d4422f/extracted/bundle`。含真实PTY、Hive HTTP/SQLite、投递/停止/恢复；本地合成对端不算厂商推理。
- 解压包使用重新定位到本批data内的独立Pi原生历史副本，实际重开16张2400×2400图片，末尾13/14/15缩略图可查看；点击最后一张，原图对话框实际显示2400×2400及原图/复制/保存入口，再关闭并正常Cmd+Q exit0。本轮没有实际复制或保存原图，也未逐张打开16图。
- `check-images.py`逐一核对全部16个原生消息ID/文本、PNG字节及哈希；Hive workers/messages/dispatches/dispatch_messages/agent_runs均0。真实进程树使用包内Node/runtime，无开发覆盖，退出后App和后台PID均消失。没有测本批release的RSS，不沿用debug指标冒称优化包实测。
- 包内Node执行cold-team.mjs，只读冷开此前645c9a5真实Terra三会话，原模型/投递身份/汇报来源保持，pending=0/pendingReports空。无prompt或resume执行；本轮新增模型请求0，真实推理仍归原645c9a5。
- DMG经verify、只读挂载完整清单和Applications链接核对并卸载。安装前核对旧Applications清单与无占用，新临时副本清单核对后替换；新安装副本与已运行解压包完全一致，本轮未另启动安装副本。

ZIP为143,935,301字节，SHA256 `e6678475ae339b4310121da905493a6980ed956a6354375a401979134d6dd13c`；current/candidate为同字节硬链接。`.artifacts/release-3d4422f/`保存组装、各组检查、冷恢复、原生AX/退出/进程、DMG/安装及receipt。汇报分层在本批源码English20宽/窄窗已有真实记录，本轮未重复包内点击此路径。

## 存储与边界

清理本批重复base及d644db9临时安装备份，保留d644db9完整可运行解压包与ZIP作为直接回退。另将更早645c9a5、3a3f873、3ed07c9的重复解压副本退役：每份均先核对完整manifest、归档ZIP哈希及无占用；原ZIP、源脚本、fixture、日志和历史收据保留，需要时可用ditto从ZIP重新解压。各旧目录storage-status.json与本批prune-superseded.json记录实际状态，历史文档中“保留完整解压包”是当时状态，不能继续当作当前路径存在保证。未清理用户项目、账号或共享Rust缓存。

本批无生产源码或正式测试增改删，运行上述风险匹配的包检查，未跑无关全量suite。仍未签名/公证，Windows真机、下载隔离/安装体验、所有图片格式/坏图/草稿/并发操作矩阵、完整输入法/VoiceOver、生产远程网关及系统通知送达仍待。Pi重登暂缓，真实模型继续仅Terra，Claude真实推理未验收。
