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
