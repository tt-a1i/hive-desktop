# 队列结算与远程界面修复进入候选包

此候选已由[768d085候选](2026-11-09-steered-candidate.md)替代；本页保留0e72137的历史证据，原ZIP不另存，canonical路径现在指向新候选。

2026-11-09，源提交`0e721373c860d7b829cbbc771d115908a64567db`。候选包纳入649de4f内部通知折叠、b3e4b36已汇报派单队列结算、a9b6d64远程配对信息可访问性及过期状态提示修复。

- ZIP：`.artifacts/hive-desktop-macos-arm64-candidate.zip`，171,842,913字节（约164MiB）。
- ZIP SHA256：`a55599325b28049f3f519d43cf350b59c8e3d4cb79c0817027482df77997bfee`。
- 原生程序 SHA256：`5b793a733a6c6a2b87a75689d80351bac19c726df144ec474823fe7d428a3907`。
- current仍1acd805，SHA256仍`f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`；没有推广候选。

## 可复核构造

以原7c28580候选为基线，先核对原ZIP哈希和Git干净状态。确认lockfile、pnpm补丁、Node生产依赖及Cargo依赖均未变化；runtime package.json只改scripts。替换原生程序、chats/delivery-queue/hive-host各自JS与source map，以及部署清单中的scripts；保留部署后清单的依赖转换和其他字段，不重新复制依赖树。

独立解压新ZIP后，112个编译runtime文件与本轮生产dist逐个一致；28,826个未替换普通文件、29条解析在包内的链接及31,570个AppleDouble元数据条目逐项保持，ZIP testzip通过。正式原生程序匹配上述哈希。元数据未改变不等于原生代码未变；本轮确实替换了原生程序。

构造脚本及收据位于`.artifacts/settled-candidate/build.py`、receipt.json；该脚本记录一次特定基线构造，不能对已经替换的canonical ZIP盲目重跑。

## 包内检查

以下gate均已核对实际进程句柄exit0：

- `node scripts/check-package.mjs --bundle <extracted/bundle>`：包内Node22.23.3、真实PTY及Hive agent-manager自然退出/停止、HTTP/SQLite工作区和正常关闭，无模型请求。
- `node scripts/check-packaged-queue.mjs --bundle ...`：真实内置Pi及原始外部Pi SDK/HTTP/SQLite队列、FIFO、附件、停止与暂停冷恢复；模型响应为自有回环对端。
- 新增`node scripts/check-packaged-dispatches.mjs --bundle ...`：仅擦除检查文件的TS类型，把生产导入及typebox全部绑定解压包，由包内Node执行。真实Hive HTTP/SQLite/SDK/CLI验证已汇报任务结算、未汇报任务/用户输入/report类/不同成员投递保留及旧spool冷恢复；另跑原生工具确认、直接停止、缺确认超时和断线的合成JSONL边界。未使用源码runtime替代包内逻辑。
- `node scripts/check-packaged-remote.mjs --bundle ...`：包内Node/CLI/加密库/HTTP/SQLite实际配对与独立SAS推导、加密HTTP配置键403、坏流Reset、真实PTY终端IO/control、设备撤销拒绝旧输入及网关503后的冷恢复补发；仅自有回环网关。

本轮生产代码未另改；新增上述包内检查脚本并记录实际运行。`node --check scripts/check-packaged-dispatches.mjs`和`git diff --check`通过。没有重复全量suite；采用打包边界、队列和远程实际链路gate，源码Mac/Windows类型证据保持其原范围。

## 原始Codex/Terra与冷恢复

由本次解压包Node及runtime执行`.artifacts/settled-candidate/live/run.mjs`，实际原样CLI0.166.0、模型openai/gpt-6.1-terra：

- run：d001d5d8-98a7-4260-a339-839e63a2d374，completed。
- dispatch：be766006-37f1-4801-b592-08831aad11ec，reported。
- Worker自己读取[7,11,13]，计算并写sum31/count3，实际messages/seen/report带产物，原生report completed/success=true；输入保持，回收后busy=false/pending=0。
- 独立包内runtime冷读同一thread、run、dispatch及工具成功状态，pending仍0；未resume/prompt。

驱动启动workflow，Worker模型自己执行/汇报，不冒称本轮主控模型编写启动。主控亲自从原生聊天创建workflow的源码证据见[原生主控](2026-11-09-native-workflow-orchestrator.md)。

## 实际原生窗口

工作流窗口PID51588：确认实际包内Node/runtime进程路径；同一Worker无遗留待处理项，展开report工具为完成；工作流同一结果、reported成员、两条日志及产物预览31/3均实际可见。Cmd+Q exit0，记录的8个自有进程全部不在进程表。

远程窗口PID80518：自有回环网关和测试登录也使用本包Node/runtime/remote CLI/加密依赖。实际打开远程页，关闭/恢复连接、读取配对码、设备名称、说明及SAS的AX标签、拒绝请求；状态刷新后没有遗留Submitted提示。SQLite为1次user_rejected、0配对设备，网关没有确认请求。没有授予设备访问。Cmd+Q exit0，2个自有窗口/后台进程退出，网关确认daemon断开后显式结束自有网关进程。本轮没有GUI模型输入。

上述为工作流与远程具体操作；通知折叠已纳入相同源码原生程序，本轮没有另回放历史主控通知。中英文、窄窗完整矩阵仍按前轮[源码远程实证](2026-11-09-remote-native-ui.md)范围区分，不把哈希或本次宽窗扩大成全部UI重验。

## 保留与边界

所有gate退出、已记录进程不在、lsof +D无引用后，只删除本轮可重建的解压目录；所有真实工作区、会话、日志和收据保留。原子替换canonical candidate ZIP，旧7c收据标注zipPresent=false/superseded；没有另留一份旧ZIP。.artifacts仍约3.9GiB。

这是未签名/未公证的Mac arm64便携开发候选，仍非正式安装包。Windows原生、生产远程网关/手机、确认/撤销GUI、完整可访问性/键盘矩阵、Claude真实模型及Pi失效登录等边界仍在[验收总表](../acceptance-status.md)。异步结束通知重复答复未解决。完整Cindy/Maka体验目标没有完成，current未推广。
