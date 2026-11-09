# 冷启动输入与聊天可读性进入候选包

2026-11-09，源码基线 `06ee28b282d69fc98acd70da24cb0902cd7b1529`。

## 产物

- `.artifacts/hive-desktop-macos-arm64-candidate.zip`，171,872,618 字节（约164 MiB）。
- ZIP SHA256：`27f96972c4ded3a19b3e44fa30e327b94cbb2cdf913986fb09197fd3b7e02cb9`。
- 原生程序 SHA256：`b996210ce2aa28a7e4759075b7540feded0e8fabbc6182269e300e94f811932b`。
- current 仍为 1acd805，SHA256 `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`，未推广。

纳入显式输入冷启动时恢复事实仅进入上下文的修复、已接收输入持续可见及去重、中英文处理反馈，以及聊天/展开工具正文的辅助功能语义。源码说明分别见[输入恢复](2026-11-09-user-input-recovery.md)、[接收反馈](2026-11-09-received-input-feedback.md)、[聊天可读性](2026-11-09-conversation-accessibility.md)。

## 包构建与检查

以768d085候选部署为基线，严格核对差异后仅替换 chats.js/map、runtime脚本元数据和原生程序；没有改变生产依赖、锁文件或补丁，也没有重新执行依赖部署。独立解压匹配全部112个runtime文件、28,830个未变普通文件、29个内部链接、31,570个AppleDouble元数据条目，ZIP testzip通过。脚本及收据在 `.artifacts/reading-candidate/`。

四组包检查全部exit0：`check-package.mjs`、`check-packaged-dispatches.mjs`、`check-packaged-queue.mjs`、`check-packaged-remote.mjs`，均传入独立解压的bundle。真实Node/PTY/Hive/SQLite及退出、已汇报派单回收、Codex工具回收/同回合通知、空会话/显式输入恢复、内外部Pi队列/附件/冷恢复、远程加密HTTP/PTY/撤销路径通过。厂商协议fixture不冒称真实模型；检查器来自checkout，生产runtime及依赖来自包。

## 原生包真实 Codex/Terra

从之前包内真实47/3主控历史独立fork `01a1c038-3f6b-7123-b7fb-c02153db6b03`，继承1个原生回合。使用独立数据目录，直接运行本轮解压应用，无源码Node/runtime覆盖。

在实际窗口发送：`Reply exactly PACKAGE_COLD_INPUT_OK. Do not use tools or change any files, models, or global settings.`

- 真实AX观察到原文及“已接收，正在处理”临时卡片，随后卡片移除并显示正常用户正文。
- 原样Codex、`openai/gpt-6.1-terra` 回复唯一 `PACKAGE_COLD_INPUT_OK`，正文和回复直接出现在AX文本节点。未改全局模型/认证。
- Cmd+Q正常退出，PID75552，exit0。
- 再用包内Node/runtime独立冷读：pending0、busy=false、完成投递3条，来源user/system/user；原生回合共2个（继承1+新输入1），final分别为旧47/3及新标记，恢复事实留在上下文。没有多开的恢复回复回合。
- 本轮没有再运行主控+Worker协作；先前该路径证据仍按[768d085候选](2026-11-09-steered-candidate.md)范围保留。没有把这次单条真实输入扩大为完整协作复验。

原生截图/AX、进程与退出、四组gate、fork/恢复日志和live receipt均保留。完成后确认进程已退出、解压及live目录无lsof引用，清理解压副本，原子替换canonical候选；旧候选ZIP不额外保留，.artifacts约4.0GiB，旧聊天目录未删。

这是未签名/未公证的Mac arm64便携开发包。Windows原生、Pi重新登录、Claude真实模型、完整VoiceOver/流式读取及Cindy/Maka全部体验仍待；完整目标未完成。
