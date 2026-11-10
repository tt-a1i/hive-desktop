# 远程设备确认、撤销与窄窗焦点复验

2026-11-10，#18 的后续验证。完整产品目标继续开放。

## 场景与变化

用户在远程设置核对已配对设备后撤销访问，需要读到设备身份、确认说明和结果。原界面中设备正文和访问记录缺少 AX 正文；约593×507逻辑像素窗口中，用键盘激活撤销会增加确认说明的高度，但焦点没有变化，确认按钮被推到视口外。

复用现有 selectable_text 展示网关、设备身份、确认说明和审计正文；撤销与取消的 AX 名称包含设备名称和ID。共享 Reveal 额外比较被包裹内容的布局大小，展开时重新显露操作；不跟踪滚动后的位置，保留手动浏览历史记录的位置。没有更改协作协议、远程授权语义、存储结构或依赖。

## 真实操作与持久化

- 使用当前已安装46373f6开发包，从实际界面生成一次性配对码。仅本机127.0.0.1合成网关及自有测试对端，运行原样Hive远程daemon/密码学/store/CLI；未登录真实账户、连接生产网关或使用真实手机。
- 测试对端从真实pair-ack独立推导安全码914163，与原生界面显示一致；界面执行“Codes match: confirm”，网关收到一次pair/confirm及confirmed帧。设备ID为0727f7b3-496e-4a8c-ab40-617dca2b7077。首次展开撤销后取消，SQLite仍保留该设备且revoked_at为空。
- 修改后的源码窗口冷恢复同一设备，设备名称、ID、创建时间、最近活动及审计记录在AX可读。此测试设备始终Last active No activity yet。
- 593×507窗口，Search后8次Tab到Revoke，Return展开。修复前确认按钮在视口外；修复后说明、Confirm revoke、Cancel立即完整可见。手动滚到审计记录后保持位置；Tab能显露Cancel，Return取消后设备仍保留；再展开仍完整可见。
- 实际点击Confirm revoke后显示No devices paired yet及对应revoke成功记录。原Hive产生两条审计记录（一条含revoked原因），本轮没有去重或改写其含义。网关仅收到一次pair/revoke，目标ID与先前配对一致。
- 三个原生窗口均Cmd+Q正常exit0。独立SQLite确认唯一设备保留撤销墓碑、revoked_at非空、last_active为空；workspace/worker/message/agent_run/dispatch/workflow_run全部0。
- 关闭窗口后，用同一包内Node/runtime独立冷读，GET devices为空、两条目标revoke记录均ok；通过真实PUT enabled关闭此隔离数据的远程功能。最后精确关闭自有网关PID31030，exit0。未修改默认用户数据。

## 检查与证据

- `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`：通过。
- `PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`：通过。
- `git diff --check`：通过。
- `.artifacts/remote-confirm-native/`：pending/paired AX及图片、expanded-clipped.png、expanded-fixed.png、manual-scroll.png、revoked.png/AX、paired-receipt.json、revoked-receipt.json、cold-receipt.json、gateway-events.json、构建与进程日志。密钥和凭据不进入报告。

本轮为原生UI与共享显露修复，没有增加或删改自动化测试，没有运行无关全量runtime测试；真实后端链路由上述操作穿透。新源码尚未纳入当前46373f6 ZIP/DMG/Applications。未新增模型推理。该GUI流程没有建立活跃手机数据隧道，因此不声称本轮验证了活跃终端断开；加密隧道、HTTP/终端与撤销拒绝保持此前包内remote gate的独立证据。VoiceOver、完整语言/字号矩阵、真实手机及Windows原生仍待。
