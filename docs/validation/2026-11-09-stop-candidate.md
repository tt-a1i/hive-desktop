# 纳入成员阅读与停止/恢复修复的Mac候选包

2026-11-09，源码 `dd0230a0d68869594aec9a98fffe635e7be8bf12`。Mac arm64 未签名、未公证的便携开发包，完整产品仍在实施。

## 当前候选

- `.artifacts/hive-desktop-macos-arm64-candidate.zip`，171,823,869字节，约164MiB。
- ZIP SHA256：`9d416c25e4d8f61097ec8abc6f862b78e5f58a58f94fd9679d798adb904e858a`。
- 原生程序 SHA256：`6b92d53be37f5fdd6f2cfb42dfdd285ac1e3515383edca823ab0e20124471759`。
- 纳入成员职责可访问阅读、恢复信息合入待处理队列、Codex停止后关闭本聊天独占app-server，以及旧停止提示复位。

在17d8ce7候选上更新；Git核对依赖清单、锁文件、patch与Rust依赖均未变，runtime源码仅chats.ts有变化。重新构建runtime与Mac程序，更新原生exe、chats.js及其source map三个归档条目，其余冻结依赖沿用，不是fresh production deploy。

独立解压后，112个runtime文件与本轮dist逐文件一致；28,831个未变普通文件、29个内部链接及31,570个AppleDouble元数据条目与旧归档一致，链接仍在应用内，testzip通过。目录盘点包含AppleDouble时数量多于实际dist，已分类，不将元数据当运行文件。

## 包内检查

全部从实际包内Node运行：

- `scripts/check-package.mjs --bundle <bundle>`：真实PTY/Hive agent-manager自然退出与停止、HTTP/SQLite、配置元数据及shutdown通过，无模型请求。
- `scripts/check-packaged-queue.mjs --bundle <bundle>`：内置Pi与原始外部Pi，真实SDK/CLI、Hive HTTP/SQLite、附件/队列、停止/冷恢复及有队列/空队列恢复规则通过。响应为本地合成，不冒称厂商推理。
- `scripts/check-desktop-startup.mjs --bundle <bundle> --language en`：原生启动保存项目/缺失会话选择恢复通过。该probe最终SIGTERM清理，正常用户退出另见下文。
- 将现有`codex-stop-live.ts`唯一的runtime import绑定到包内dist，用包内Node执行。原始Codex0.166.0 / openai/gpt-6.1-terra真实检查通过：后台probe在Stop返回前退出、原线程与两条队列保持、另一原始连接不受影响，继续后顺序得到STOP_QUEUE_ONE_OK / STOP_QUEUE_TWO_OK。绑定仅用于验证，没有产品fallback或模拟CLI。真实收据`.artifacts/codex-stop-live-QsXFmo/receipt.json`，原始数据不改。

## 实际候选原生窗口

复制上述真实数据到独立GUI数据目录，包内API保存导航，并创建autostart=false的layout-proof成员，职责为102行中英夹具；不启动该成员，不构造模型回答。原始真实检查数据保持不变。

实际启动解压应用，移除Node/runtime环境覆盖；进程表确认Native15014的后台15015实际使用包内Node及dist/main.js。窗口恢复正确Terra线程与两条真实结果；9条既有展示条目ID及completedTurns保持，GUI日志无prompt/resume，无新增推理。

成员夹具正文有AX Region，End可读第100行及PACKAGED-RESPONSIBILITY-END，Home/PageDown到第8–17行，PageUp回BEGIN；1184×1504截图像素窄窗隐藏导航后，正文末尾与成员操作可见。此项是未启动成员的阅读夹具，不能称真实Worker完成了任务。

实际Cmd+Q，Native15014退出0。确认含原始CLI在内的8个自有进程均消失、lsof无解压目录引用，才删除本轮extracted副本。没有以强制结束或探针SIGTERM替代这条正常退出证据。

## 归档、证据与剩余范围

新ZIP原子替换唯一candidate槽位，旧006a8e候选收据标记zipPresent=false及supersededBy，旧验证记录保持。current ZIP仍为1acd805，SHA256 `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`不变；未推广current。`.artifacts`清理后约3.9GiB，没有增加长期解压依赖副本。

收据`.artifacts/stop-candidate/{receipt.json,bound-check.json,gui-seed.json,gui-runtime-process.json,gui-history-receipt.json,gui-owned-processes.json,gui-exit.json,cleanup.json,superseded-candidate-receipt.json}`；候选索引`.artifacts/current-package/candidate-receipt.json`。日志`/tmp/hive-candidate-dd-{runtime-build,native-build,package,smoke,queue,startup,native-stop}.log`及GUI日志保留。

本轮无产品/测试代码改动，运行打包风险匹配的实际包内/原生/真实模型检查，未重复无关全量。完整页面/输入法/辅助技术、远程网关、工作流、Pi认证、Claude真实模型、Windows原生、正式安装/签名/公证及整体Cindy/Maka级体验仍未完成；不以本候选局部成功替代完整目标。
