# 远程接入原生设置与 Hive 复用验证

2026-11-09。对应 #18，仍开放；不表示完整产品或远程功能最终交付。

## 接入范围

设置增加“远程接入”，可查找入口、读实际连接状态与网关、开关连接、生成/取消配对码、核对安全短码后确认/拒绝、查看/撤销设备及最近100条审计（含原有字节数和预览字段）。配对等待提示位于全局，不要求设置页始终打开。复用现有3秒界面更新循环，过期码及连接代次切换不继续使用；读取失败保留上次内容并暂停操作。设备撤销保留明确确认。

登录/退出/状态命令通过初始化元数据提供，调用包内Node与Hive原生remote CLI，显式绑定当前桌面的 dataDir/hive。macOS命令按POSIX shell引用，Windows命令按PowerShell引用；不修改调用终端的环境变量。仅复制命令，不自动登录或授权用户设备。登录后可用“重新连接网关”沿用PUT enabled接口重新装配连接。

退出桌面仍停止sidecar，不引入托盘或后台daemon。没有修改模型、协作账本、原有端到端加密算法、手机协议，也没有增加内置Pi权限审批。

## 依赖包与源码差异

npm最新仍为2.2.1；其实际发布文件缺少本地Hive 5d87d4ba10d9d66cfca32941a16c60e7e52476b0源码已有的网关撤销队列和远程配置键访问保护。最初完整链路在网关撤销断言超时，而本地设备已撤销；没有删除该断言冒认成功。

扩展现有可重现pnpm补丁，复用该提交的remote-config-keys、remote-gateway-client、remote-pending-revokes、runtime-store-remote、hive-remote及其hive-data-dir依赖；从同一源码移植routes-settings的assertAppStateKeyAllowed及两个调用点，保留包内其余设置路由（包括自动组队）。源文件无本地改动。JS由TypeScript5.9.3的transpileModule、ES2022/ESNext生成；补丁和锁文件随源码提交，不直接修改已安装node_modules。

首次移植检查发现漏带hive-data-dir，真实启动检查报告ERR_MODULE_NOT_FOUND；补齐原生模块后以下检查全部重跑通过。没有增加运行时fallback。

实际加密HTTP对端构造时，第一次误用裸JSON代替原有带判别字节的body分块，触发了共享解码入口未处理的ProtocolError并退出后台；校准对端后正常协议通过，再把同一坏帧作为显式回归重复复现退出。现有补丁另在remote-frame-bridge的已认证帧共享入口只捕获ProtocolError，发送原协议Reset、释放该数据流并审计；其他异常继续抛出，不做字符串匹配或兜底执行。回归要求收到ProtocolError Reset、目标设置未写入，随后同一连接仍能请求设备列表。

## 实际检查

- `pnpm --filter @hive-desktop/runtime check:remote-settings`：真实Hive HTTP、SQLite和原生remote CLI；全程仅用自有临时数据及本地HTTP/WebSocket网关。未发模型请求。
- 临时路径含中文、空格、单引号及命令替换文本；执行实际复制状态命令，没有命令注入，也没有触碰指定的另一Hive数据目录。相对路径被拒绝。
- 默认未登录且关闭；原生login经真实fetch写入同一DB，状态接口看见登录与开关变化；原生logout后重开仍关闭/未登录。原始token未打印。
- 真实daemon WebSocket收发配对hello/ACK。手机对端使用原生X25519算法从实际ACK和票据独立派生SAS，与pending HTTP结果一致；确认前设备列表为空；确认后收到confirmed、网关收到绑定jti，设备列表有同一设备与名称。
- 撤销时网关首次503：本地设备立即不再有效、审计出现revoke，持久队列包含该设备和原网关；完整关闭/重开后网关恢复，同一设备撤销补发且队列清空。未知确认404、未登录创建配对503、绕过专用接口写remote_enabled返回403。
- 配对后通过真实会话盐/连接密钥/加密Hello及HTTP流请求设备列表，收到200；逐项读取/改写remote_daemon_token、remote_gateway_url、remote_pending_revokes、remote_enabled均收到403，响应不含token。坏body协议帧被重置且后续合法请求仍200。
- 同一完整远程检查再用现有开发包Node22.23.3执行通过；常规Node为26.12.0。仍不是新开发包内部验证。
- 相关回归：`pnpm check`、`check:runtime`（真实sidecar初始化/会话/重开/关闭）、`pnpm check:hive`、notification-settings及team-autostaff-settings通过。
- Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml`、现有LLVM PATH下Windows x64 MSVC `cargo check --locked`、`git diff --check`通过。

没有跑全部测试；本轮为现有远程接口和依赖修复移植，采用定向真实链路及设置/sidecar回归。未删除或改写既有测试。

## 明确待验收

用户回复解锁后，原生工具仍明确报告Mac锁屏，无法实际点击新页；保留两个旧owned进程54534/21544，未据此判断Hive死锁或强杀它们。新原生页的窄窗、键盘、剪贴板、实际配对按钮及退出流程仍未验收。

本地网关是实际HTTP/WebSocket对端，不是生产Cloudflare网关、账号或实体手机。本轮已验证配对密钥/SAS，但已验证原有加密HTTP流及配置边界，但未验证所有加密数据流（包括终端/WebSocket）、生产网关失败恢复或Windows原生体验。Windows命令本机没有实际PowerShell执行验收。本轮配置键的真实加密隧道403矩阵已通过；实体手机和生产登录不由回环对端替代。

本轮改变生产runtime及Hive依赖补丁，不能沿用“只替换原生程序”的旧候选构造方式。当前ZIP与66ef8f9候选均未包含此功能，未推广新current包。后续需重新部署依赖、包内验证和真实原生走查。完整目标保持active。


## Mac候选包补验

生产源码d9e061b234bcc7c36dc8a5f71c5cde8c4c015f50。重新执行`pnpm deploy --prod --frozen-lockfile --node-linker=hoisted`，不是复用旧依赖树；由package-desktop下载并校验官方Node22.23.3（arm64归档SHA256 23b25245dcfb9af7262f8ff142e9e2e0af025368117329e7a7458a51e5922f53）。

最终ZIP重新解压后，比对112个编译runtime文件、28,863个整包文件/链接条目、29个包内链接及原生程序。候选为`.artifacts/hive-desktop-macos-arm64-candidate.zip`，ZIP SHA256 `74c1b29371010de375e058460ecdd751a98e0f1539ffd7f8eb2858f1ac841935`，程序SHA256 `97a8899edc0e7f281fd05832905a1943f0e4787ef895f5051e7fce4d667bd6cf`。

实际从该解压包运行：

- `node scripts/check-package.mjs --bundle <extracted/bundle>`：包内Node、原始PTY自然退出、Hive agent-manager自然退出/停止、HTTP/SQLite、Pi账号/模型元数据、任务/四后端会话记录及关闭通过；没有模型请求。
- 新增`node scripts/check-packaged-remote.mjs --bundle <extracted/bundle>`：只对源码检查擦除类型并重定向所有生产导入、WebSocket、加密库和原生CLI到包内。包内Node运行完整配对/SAS、加密HTTP与配置403、坏协议流恢复、撤销503/重开补发、原生登录退出及路径隔离，全部通过。旧的已验证设置包被同一检查以缺少remote-commands.js拒绝，没有修改旧包。
- `scripts/check-desktop-startup.mjs`：隔离Pi目录设openai/gpt-6.1-terra、PI_OFFLINE=1；通过真实Hive/team准备派单、提问/回答、汇报和持久化位置，实际启动新包原生程序。程序读回第二项目、打开其会话并替换已删除的会话选择，SQLite位置更新通过。脚本最后SIGTERM清理自有验证进程；这不是用户正常退出或系统点击验收。

此次原生工具仍返回锁屏。启动恢复是原生程序的实际状态证据，与CUA点击/截图证据分开；没有因此声称新远程设置页已完成GUI走查。真实网关/手机、其他加密流、Windows与完整产品继续开放，#18未关闭。

candidate已用新依赖包替换旧66ef8f9候选，旧收据另存`.artifacts/remote-settings/superseded-candidate-receipt.json`。current ZIP仍为1acd805，SHA256 f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6不变；没有推广为current。所有gate句柄均已确认exit0，lsof无引用后移除本轮部署、中间整包和解压副本，只留一个候选ZIP及证据；.artifacts仍约3.8GiB。原owned进程21544、54534保留。收据/清单/日志在`.artifacts/remote-settings/`，当前候选收据同步到`.artifacts/current-package/candidate-receipt.json`。

本轮只添加包内行为检查及文档，没有修改生产逻辑或删除既有测试。实际运行上述风险对应gate及`node --check scripts/check-packaged-remote.mjs`、`git diff --check`；未重复全量测试，未声称Mac交互、Windows或完整发行通过。
