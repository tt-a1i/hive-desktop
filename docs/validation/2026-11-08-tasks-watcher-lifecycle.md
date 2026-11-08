# 任务文件监听的启动取消

任务 #8，源于原始Codex真实排队验证中的一次任务监听超时；不是由模型输出推断。

## 复现与根因

固定Hive2.2.1依赖中的watcher先等Chokidar ready，stop/close只关闭watcher，没有结算ready等待；原生watcher关闭后不再发ready，待启动Promise直到15000ms超时才拒绝。在真实Chokidar/文件系统上连续3次复现，耗时约15013/15002/15003ms，日志和收据.artifacts/tasks-watcher-lifecycle/before.*。不需要厂商请求或mock watcher。

该固定版本start还会把closed重设false，正在await stop的start也可能在整体关闭后继续创建watcher。本地Hive当前源码已有部分关闭guard，桌面实际使用的发布依赖不同，因此以实际安装模块为准，不直接修改用户Hive源码。

## 修改与验证

沿既有pnpm补丁更新发布依赖的dist/src/server/tasks-file-watcher.js：按watcher保存ready的取消入口；stop/close主动结算并清理ready/error订阅及超时器，真正的error仍先拒绝启动，保留错误记录/重试。closed保持终态，在入口和await stop之后检查；关闭后不再安排变更通知。不提高超时阈值、不吞异常、不新增依赖。lockfile只更新补丁哈希。

check:tasks-watcher使用真实Chokidar验证十次立即/启动中的关闭、关闭后的start不创建.hive、启动中stop后重启、直接保存和原子替换实际回调、真实ENOTDIR/EEXIST错误。check:team、内置Pi真实Hive/SQLite/HTTP队列和TypeScript定向回归通过。真实HTTP/SQLite八次冷启动关闭后再观察16000ms，无延迟错误；收据runtime-receipt.json。

本次没有修改桌面Rust、Hive任务协议、schema、厂商/模型或真实用户数据。未运行无关全量测试，Windows原生仍待验收。补丁改变部署依赖，因此开发包必须重新pnpm deploy，不能只替换本项目runtime/dist或沿用旧依赖。


## Mac 开发包

ZIP `.artifacts/hive-desktop-macos-arm64-tasks-watcher.zip`，源62f906b6c48913910c90b4134f6a2092f84c0e92，SHA256 `c23844fac80c56a0aa5a257e72e3d24e92cc243ae47635a86a5b6589c643e183`。重新离线pnpm deploy --prod，完整替换部署runtime依赖树，未沿用旧Hive依赖。独立解压的108个runtime文件与生产构建匹配，包内watcher与checkout/全新部署文件哈希一致；全部1094个依赖链接目标都在包内。原生二进制未变（54307c9c3338a0d10f3d6bbefbfcc9cb3e44bc1100237eb5183e74517cf98bfc）。

包内Node的真实Chokidar生命周期检查、Codex设置/历史/中断gate、内外Pi队列、真实PTY/Hive/SQLite smoke及生产窗口启动恢复均通过。旧Codex设置包被“关闭后仍创建.hive”真实行为断言拒绝。收据及日志 .artifacts/tasks-watcher-package/，不以源码字符串断言替代行为。

本轮未重复系统鼠标输入，不声称新的物理UI验收；未签名/未公证的Mac arm64开发包，Windows原生仍未验收。#8本次复现的生命周期问题已完成源码及Mac包内验证，完整产品#1及其他未完成范围继续开放。
