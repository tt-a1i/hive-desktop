# 成员配置与生命周期开发包

2026-11-10；优化release源码 `3339ce1cfba8f1879ab366975a7fd33a47bdbdde`，已更新current/candidate。

- ZIP `.artifacts/hive-desktop-macos-arm64-current.zip`，candidate为同字节硬链接；143,419,916字节。
- ZIP SHA256 `3cd54516e5314d2ca657f0e8bb0dc3b6305bd437bc0248e0cb49303f1adc0929`。
- 原生程序SHA256 `4383c24cf238a119d347559c9f0db0d922ba3a11615da2c19c5743ac2dfb9dfc`。
- 实际应用 `.artifacts/release-3339ce1/extracted/bundle/Hive Desktop.app`，收据在本目录receipt.json及 `.artifacts/current-package/receipt.json`。

## 纳入与打包校验

纳入[成员表单成功清空/失败保留](2026-11-10-member-form-reset.md)及[复用Hive事务创建成员和后端配置](2026-11-10-member-atomic-create.md)。相对ecad294仅原生程序、desktop-runtime/main/hive-host三个编译模块及其源码映射变化。112个runtime文件匹配生产dist，独立解压28,863个内容/权限/链接条目匹配，29个链接均在包内。生产依赖、锁文件、patch、Cargo配置不变；runtime package.json排除检查scripts后全部字段精确一致，沿用冻结依赖而非声称fresh production deploy。

`cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`通过。包内Node执行六组gate全通过：check-package、check-packaged-models、check-packaged-dispatches、check-packaged-codex、check-packaged-remote、check-packaged-claude。

额外把已有member-creation.ts检查转译为独立JS，用包内Node且cwd为包内runtime执行，真实production JSONL/HTTP/SQLite验证四后端、非法输入、默认职责语言、配置失败事务回滚/原名重试、唯一launch配置、零执行和冷身份。没有绑定源码dist或mock网络/数据库。见member-gate.log和check-members.mjs；新事务路径不只靠构建过关。

## 实际包连贯流程

新建自有native-project/state/Pi目录，文件member-input.json固定为values=[7,19,23,31]。仅自有桌面设置指定原始 `/Users/tsk/.npm-global/bin/codex`；版本实查0.167.0，不改原CLI、全局默认或用户凭据。English/Dark20，实际解压应用使用包内Node/runtime，无源码覆盖。

1. 空团队→Add your first member→填写名称与中文职责→Testing/Codex→创建成功。成员stopped且未选模型时Start禁用。
2. 从成员实际目录搜索并选择openai/gpt-6.1-terra，Start启用；点击Start后Idle、会话入口出现。
3. 进入同一成员的统一聊天，实际原生档位Medium并显式保存。通过聊天框发送一次读取项目文件并计算sum/count的请求，不给预期答案、不要求其他成员/派单/修改文件。
4. 原始Codex实际完成team ready和commandExecution两工具；后者cwd为本项目、命令cat ./member-input.json、exitCode0、输出原样values。结果唯一为 `MEMBER_SETUP_OK sum=80 count=4`。正常commentary与最终结果分开，未当作两次用户回合。
5. 输入两行未发送草稿，返回团队、Stop，状态Stopped、启动模型仍为Terra。正常CmdQ/exit0。
6. 完整重开实际包，恢复同成员历史、两工具、唯一结果和精确未发送草稿；打开模型/推理显示Terra Medium。再次正常CmdQ/exit0，没有再点Send。

证据native-tools.ax.txt、native-tool-details.ax.txt、native-stopped.ax.txt、native-cold.ax.txt、native-cold-effort.ax.txt、native-cold.jpg。实际工具和后台结果证明本轮超出静态UI；不把成员直聊当作新的一对多派单验收。

## 独立冷读

包内Node导入包内runtime执行check-cold.mjs，正常退出：

- 仅1位tester/Codex成员，原ID、名称、完整职责、启动provider/model保持，status stopped、pending_task_count0。
- 同一Codex thread，Terra Medium；1条原始用户投递、1个完成ID，pending0/busy false/runId null。唯一commandExecution输出与原文件字节一致，唯一最终结果80/4。
- 两行草稿与空附件精确恢复，项目输入仍为准备时已知字节；未改用户全局模型。
- SQLite messages1、dispatches0、dispatch_messages0、agent_runs1、workflow_runs0，唯一run为exited/exitCode0；恢复没有新增运行/投递。见native-receipt.json、cold-snapshot.json。

本轮新增1条真实用户投递及其模型回合，模型仅Codex gpt-6.1-terra Medium；不统计厂商内部HTTP请求数。冷恢复没有新增推理。其余后端本轮为无推理配置/包检查，完整真实模型矩阵仍未达成。

## 产物与剩余边界

旧ecad294 ZIP/收据归档且保留可运行应用。无lsof引用后清除本批重复base，回收538,431,488字节（约513.5MiB）。另外确认6148801和6edac4c归档ZIP哈希及解压清单匹配、lsof无活跃引用后，只移除两份可重解压应用；保留当前3339ce1及上一ecad294可运行应用。总计回收1,615,282,176字节（约1.50GiB），见old-bundle-cleanup.json及各旧release的extracted-bundle-cleanup.json。旧ZIP、项目、状态、聊天工作目录与证据保留；旧脚本若引用已清除应用路径，需要先从对应归档ZIP重解压，或改用保留包内Node。

未签名/未公证macOS ARM64开发包；Windows仍只有源码目标检查，原生机器缺失。首轮全新状态的项目/CLI保存由准备脚本完成，GUI验证范围从空团队开始，不声称重验首次项目选择/账号登录。完整引导、所有厂商真实模型、完整辅助技术/语言/主题/平台矩阵和完整产品目标继续开放。按本批成员事务风险运行六组gate＋新事务专项，不默认重跑全部无关suite。
