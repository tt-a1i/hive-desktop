# 输入与退出体验开发包293afe3

## 纳入范围

Mac ARM64优化release，source `293afe3f8931884414e52b59fc5ec3f674e9a8ea`。相对1e68306纳入成员通知身份、汇报处理中/原生语义、无内容composer区域的多余间距、附件焦点显露及固定数量入口、后台退出原始错误/清理，以及固定错误入口。具体源码/原生验证见对应当日记录。完整目标仍未完成。

## 包与安装

- current/candidate ZIP同字节且硬链接：143,903,395 bytes，SHA256 `a67f7655aaeafbed06d16ae000ecebc42412110fb0b10ab2fad9a92e816a1ba5`。
- 原生程序SHA256 `f0dfed19a0433f1025378c034627b9f66e93df92f3c5c6ac595f033d3ded2cab`；release构建35.83秒。
- DMG 233,071,126 bytes，SHA256 `43fba838eea27b5c1de20513f3a564207dff25f24d7846839ee56ca0d100cb1a`，hdiutil verify、只读挂载及完整manifest匹配后已卸载。
- `/Applications/Hive Desktop.app`经实际Finder替换；安装前旧副本与1e68306一致，替换后28,869项与新包一致。

冻结依赖沿用1534986来源及1e已验证包；lock/workspace/patch/Cargo及生产依赖元数据一致，更新完整runtime dist、原生程序及开发scripts。116个runtime文件与源码产物一致，解压后28,869项及29个内部链接逐项一致，只有预期六项变化（两个JS及map、原生程序、runtime package scripts）。不是新鲜production deploy，不夸大为依赖升级验证。

## 实际验证

- `check-package`、`check-packaged-codex`、`check-packaged-dispatches`、`check-packaged-queue`均exit0。新增退出清理检查在旧1e包实际失败，在新包通过；既有Codex及队列/团队检查仍通过。真实HTTP/SQLite/PTY/SDK及本地JSONL/loopback peer，无vendor/model请求。
- 包内后台冷读既有真实Terra三会话：busy=false/pending=0、完成ID及投递身份保持；只读，不新增推理。
- 无Node/runtime路径override实际启动解压包，记录进程路径确认自有Node/runtime；中文20px窄窗恢复12行草稿、两份附件及暂停输入，附件数量入口实际定位两张卡，退出exit0。
- 安装副本以独立测试data/agent启动，实际窗口恢复相同内容；进程路径确认使用Applications内Node/runtime，退出exit0，native/backend PID均消失。此次未使用默认数据。
- 包内冷读再次比较两条原投递ID/正文、草稿、两上传副本原始字节、单一原生轮次，保持不变且无自动重发。

证据 `.artifacts/release-293afe3`，包括assembly/manifest/gate-results、五类log、cold-team-snapshots、cold-composer-receipt、native-attachments图/AX、安装manifest和原生进程/退出收据。current-package两份receipt已更新。

## 存储与边界

新包的base/bundle与manifest完全一致且lsof确认无占用后移除，回收539,029,504 allocated bytes（约514MiB）。解压包、测试项目和证据保留；1e68306 ZIP归档及可运行解压保留。0080304及其证据未动。

未重跑未改变的remote/account完整包套件，沿用对应历史记录，不视为全功能重验。无新的真实模型验证，包内固定错误入口的错误正向展示依照同源码开发二进制记录；新包本轮主要验证恢复及附件操作。Windows原生、英文错误入口原生、VoiceOver、签名/公证、下载隔离Gatekeeper及正式发行继续，不把本开发包当完整交付。
