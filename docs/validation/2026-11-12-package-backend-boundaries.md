# 当前包的后台边界补验

2026-11-12，针对已安装的3ed07c9同字节解压包，补齐此前本批未执行的五组现有检查。检查从包内Node、runtime及实际SDK/依赖执行，源码仅提供已有检查程序。没有生产代码、正式测试或安装包变更，也没有面板操作/厂商模型推理。

## 实际检查及结果

以下命令均在仓库根运行，参数统一为`--bundle .artifacts/release-3ed07c9/extracted/bundle`，exit 0：

- `node scripts/check-packaged-tasks.mjs`：真实Chokidar/文件系统；十次启动期间关闭、关闭后不再创建目录、stop/restart、直接及原子保存通知、真实文件路径错误。
- `node scripts/check-packaged-models.mjs`：原生SDK和JSONC；高级字段/BOM/CRLF保留、并发及过期保存冲突、只读/符号链接、override继承/编辑/移除、生产JSONL、热会话重新加载及会话/全局默认隔离。临时配置和合成凭据不代表真实登录有效。
- `node scripts/check-packaged-pi.mjs`：原生SDK/外部Pi RPC；历史压缩与当前分支、技能/模板/输入扩展、真实只读JSONL写盘失败、无虚假投递确认、保持同一投递身份、显式重试及冷恢复。上下文、模型、reload、推理强度及dispatch写盘失败均覆盖。仅本地loopback响应；部分自动恢复会连接本地关闭端口9失败，未伪称成功模型回复。Windows ACL未覆盖。
- `node scripts/check-packaged-claude.mjs`：官方SDK连接本地合成CLI，真实Hive/SQLite/transcript/spool；限定写盘失败作用域、终态后退出、完成身份持久化、暂停队列、冷恢复及显式继续。本次将`HIVE_CLAUDE_WRITE_LOSS_OUTPUT`定向到本批目录，保留新证据。不是Claude原生模型验收。
- `node scripts/check-packaged-remote.mjs`：真实本地WebSocket、X25519/SAS及加密HTTP/PTY；实际执行终端输入输出、UTF-8、畸形流隔离、配置敏感键拒绝、实时撤销后拒绝输入、网关撤销故障后的重启重试、审计及持久化。网关服务为本地对端，不证明生产账号、互联网链路或手机体验。

日志及带日志哈希的`extended-backend-receipt.json`位于`.artifacts/release-3ed07c9/`。执行后再次计算当前ZIP哈希，与原发布收据完全一致；原开发包及安装副本无需重新打包。进程快照未见本批检查残留进程。检查暴露的SQLite实验性提示保留，五组无失败退出。

## 范围与下一步

协作消息的桌面读取/自动刷新和拥有实现的100条升序游标契约已核对，未找到需要修改的页大小差异；未把源码检查提升为长消息原生体验验收。本轮没有为制造改动引入新机制。

这些结果补强当前包的配置、写盘、恢复和远程协议证据，不代表完整产品完成。仍需推进真实账号允许范围内的模型协作、长会话体验、原生输入与可访问性、Windows真机及正式签名发行；Pi重登继续由用户暂缓，真实模型仅Terra。按风险补这五组定向检查，未跑无关全量suite。
