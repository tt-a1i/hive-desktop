# 补充输入入队写盘失败的实际恢复

2026-11-12，3ecfbbb源码。沿chat.prompt → Hive authenticated user-input/结构化投递 → DeliveryQueue原子spool写入，补验正在回复时入队被操作系统拒绝的情况；没有发现需要修改的生产代码。

`.artifacts/queued-input-write-rejection-20261112/prepare.mjs`以现有queued-user-inputs检查为基础生成独立可运行程序，保留其完整SDK/RPC、HTTP/SQLite、附件、暂停及冷恢复断言，仅加入真实只读目录故障阶段。内置Pi和原样外部Pi分别在独立数据/配置及本地HTTP对端运行，两个进程exit0。使用包内Node22，导入当前源码dist；不冒称本批重新测了所有安装路径。

## 新增失败阶段及断言

1. 第一条输入实际进入原生会话，由本地HTTP对端保持当前请求，不让它结束；保存忙碌状态和pendingUserInputs。
2. 将本测试投递spool目录chmod为0500，再通过真实chat.prompt提交中文多行补充输入和已上传文件。实际请求返回HiveHttpError/status500，finally恢复目录0700；不mock filesystem/HTTP/SDK，也不在生产代码加入fallback。
3. 被拒绝后，第一条请求仍在执行；pendingUserInputs完整对象等于失败前，模型对端请求仍只有1条。按原上传ID重新定位文件，读取精确原字节。
4. 恢复权限后显式重试同一原文/附件，继续原检查的排队展示、非法附件拒绝、系统来源隔离、停止保留、完整runtime关闭/冷开、显式继续和顺序消费；冷开不自动唤醒，原补充输入在最终原生历史中对应投递ID恰好1条，附件及原文保持。

本地HTTP仅返回合成响应，没有厂商调用或用户账号变更。只断言这一显式重试场景的队列/原生历史，不将它推广为跨网络重试exactly-once，也不声称Hive底层消息表在所有部分失败中不留记录。

原始源码化检查、转译后的程序、两组日志、500拒绝类型和分组收据保留在上述目录；临时测试数据由原检查finally清理。本轮无生产/正式测试/安装包变化，无面板操作；没有重复无关全量suite。Codex/Claude、用户实际系统提示与磁盘故障矩阵仍各自保留边界，完整目标继续。
