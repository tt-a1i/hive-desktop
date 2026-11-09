# 取消队列与停止展示进入候选包

2026-11-09，源码基线 `78721c7e397dd1cffd5900b3f94e79e9b8e24ec3`。

## 产物

- `.artifacts/hive-desktop-macos-arm64-candidate.zip`，171,876,030字节，约164MiB。
- ZIP SHA256：`50327b8bbd32ed64d1802061269df956aaf6ce84679fcfeece918ca1df526cbd`。
- 原生程序 SHA256：`97112068aea7df0d89121d33f25bd6fccd497055615b47cd8aba46ef3e02a862`。
- current仍为1acd805，SHA256 `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`，未推广。

纳入[取消队列](2026-11-09-cancelled-dispatch-queue.md)和[停止展示](2026-11-09-native-workflow-stop.md)修复。以06ee28b部署为基线，严格允许列表仅替换chats/hive-host四个JS/map文件及原生程序；依赖、锁文件、补丁和manifest均保持，不是重新执行依赖部署。

独立解压匹配全部112个runtime文件、28,829个未变普通文件、29个内部链接及31,570个AppleDouble元数据条目，ZIP testzip通过。证据在 `.artifacts/terminal-candidate-78721c7`，新程序与上一轮实际Mac中英文走查二进制SHA一致。

## 包内检查和实际窗口

`check-package.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`、`check-packaged-remote.mjs` 四组包检查均exit0。检查器来自checkout，生产runtime、Node及依赖来自独立解压包；新增取消状态协调亦在包内真HTTP/SQLite/Pi SDK检查通过。厂商协议peer不冒认真模型。

另建立隔离live数据，在实际包窗口输入自有控制标记。明确标识的本地JSONL peer通过注册team工具启动真实工作流，两位成员各保持原生回合执行中；不是模拟工作流状态或退出结果。窗口进入详情后点击真实停止按钮：

- 独立检查停止前run running、两派单submitted、两实际子进程存活。
- UI停止后run stopped、两派单cancelled；停止原因与取消原因正常展示。
- 两位历史成员会话保留原文，没有过期继续处理任务；实际进程退出。
- 原生PID1261 Cmd+Q正常exit0，无HIVE_DESKTOP_NODE/RUNTIME源码覆盖；CLI fixture进程也由包内Node执行。
- 再用包内Node/runtime独立冷读，两成员thread身份保持，pending0、busy=false，实际run仍stopped。没有重发任务。

截图、进程/退出、before-stop、cold-receipt、四组gate和摘要receipt均保留。没有在本轮重复真实Terra推理；之前一主控双Worker真实证据仍按[源码实证](2026-11-09-two-worker-live.md)范围，前候选真实单条输入按[06ee28b](2026-11-09-reading-candidate.md)范围。

确认进程退出及解压/live无lsof引用后清理解压副本，原子替换canonical候选，不额外保留旧候选ZIP，.artifacts约4.0GiB。旧聊天state未删除。

## 证据维护与边界

开始构建时误用了历史stop-candidate目录名，发现后将新产物转移至独立terminal-candidate-78721c7，并从model-candidate的superseded收据恢复历史dd0230a收据。旧state、GUI和退出记录保留，但该旧目录build.py/build.log/launch.py已被覆盖，无法冒称原历史辅助文件仍保留；原目录维护说明明确记录此事。没有覆盖用户项目或会话状态。

Windows源码已具备PATH/.cmd解析与Job所有权接线，但本机未发现现成Windows虚拟机。本轮已请求可用Windows环境，不重新要求处理CI账单；没有把源码检查当Windows运行验收。完整目标仍含Windows原生/CLI/模型/安装、Pi登录、Claude真实模型及完整Cindy/Maka体验。当前仅未签名/未公证的Mac arm64便携开发包，不是完整交付。
