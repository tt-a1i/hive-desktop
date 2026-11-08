# 任务文件监听的启动取消

任务 #8，源于原始Codex真实排队验证中的一次任务监听超时；不是由模型输出推断。

## 复现与根因

固定Hive2.2.1依赖中的watcher先等Chokidar ready，stop/close只关闭watcher，没有结算ready等待；原生watcher关闭后不再发ready，待启动Promise直到15000ms超时才拒绝。在真实Chokidar/文件系统上连续3次复现，耗时约15013/15002/15003ms，日志和收据.artifacts/tasks-watcher-lifecycle/before.*。不需要厂商请求或mock watcher。

该固定版本start还会把closed重设false，正在await stop的start也可能在整体关闭后继续创建watcher。本地Hive当前源码已有部分关闭guard，桌面实际使用的发布依赖不同，因此以实际安装模块为准，不直接修改用户Hive源码。

## 修改与验证

沿既有pnpm补丁更新发布依赖的dist/src/server/tasks-file-watcher.js：按watcher保存ready的取消入口；stop/close主动结算并清理ready/error订阅及超时器，真正的error仍先拒绝启动，保留错误记录/重试。closed保持终态，在入口和await stop之后检查；关闭后不再安排变更通知。不提高超时阈值、不吞异常、不新增依赖。lockfile只更新补丁哈希。

check:tasks-watcher使用真实Chokidar验证十次立即/启动中的关闭、关闭后的start不创建.hive、启动中stop后重启、直接保存和原子替换实际回调、真实ENOTDIR/EEXIST错误。check:team、内置Pi真实Hive/SQLite/HTTP队列和TypeScript定向回归通过。真实HTTP/SQLite八次冷启动关闭另保留完整原超时窗口的观测；结果与开发包状态见后续记录。

本次没有修改桌面Rust、Hive任务协议、schema、厂商/模型或真实用户数据。未运行无关全量测试，Windows原生仍待验收。补丁改变部署依赖，因此开发包必须重新pnpm deploy，不能只替换本项目runtime/dist或沿用旧依赖。
