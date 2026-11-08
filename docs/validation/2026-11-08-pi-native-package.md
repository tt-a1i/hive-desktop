# Pi 原生输入开发包

源码 `47ac3decb304e02a02c9fe6799104fe50492cc19`，包含原生输入/命令目录与选择器、压缩后的完整聊天历史，以及内置/外部 Pi 写盘失败清理修复。不是完整产品或跨平台正式发行。

## 产物和一致性

- `.artifacts/hive-desktop-macos-arm64-pi-native.zip`，约 166 MiB，未签名/未公证的 macOS arm64 开发 ZIP。
- ZIP SHA256：`fa7ff24e912efc9864df252b1604519764127ea432a596a426385b27d5ce4783`。
- 原生二进制 SHA256：`d31b74e368f73fcc855e33e99761e8db1dc5348e30a36c6b5fc43576fc611b2b`。
- 在确认 lock/workspace/patch/Cargo 与冻结依赖元数据未变化后，复用上一份已验证 Node/依赖；完整刷新生产二进制和 runtime。106 个 runtime 文件及二进制与当前构建匹配，生成 ZIP 后独立解压逐一校验，系统操作后再次复核 ZIP 和解压文件哈希。
- `.artifacts/pi-native-package/package-receipt.json` 保存源提交、基础 ZIP 哈希、临时位置与各验证 verdict。临时目录以收据为准，不凭旧路径推断进程存活。

## 从新 ZIP 解压后运行

1. `scripts/check-package.mjs`：包内 Node、真实 PTY、Hive HTTP/SQLite、工作区及正常关闭通过。
2. `scripts/check-packaged-pi.mjs`：使用包内 Node、SDK 和 runtime，重用原生输入/目录、压缩与分支来源、两条真实权限故障和重试检查，全部通过。外部 Pi 使用本机原样 CLI；合成成功/失败响应限于回环服务，无厂商推理。
3. `scripts/check-packaged-codex.mjs`：包内累计工具输出、中断/拒绝清理、未确认投递恢复及原生分页协议检查通过；使用真实 JSONL 子进程对端，不宣称为新的原始 Codex 模型测试。
4. `scripts/check-desktop-startup.mjs`：实际生产应用恢复已存项目和缺失会话选择，通过。

旧附件包运行新 Pi gate 时被真实原始输入断言拒绝，history presentation 为空，而非以模块存在或源码字符串判断通过。首次 gate 绑定曾被新 helper 常量的导入阻挡，随后把检查 fixture 的持久化标记改为独立协议常量，旧包才运行到实际行为断言；该过程不计作旧包功能通过。源提交 9de9648 的压缩和写盘具体失败另保留于历史/持久化记录。

## 实际系统界面与冷重启

用新 ZIP 的生产应用和隔离数据启动，核对实际进程/应用路径后操作：

- 外部 Pi 点击命令按钮，目录包含实际已加载资源；选择 `/local-input` 后仅补入草稿，发送按钮仍等待确认。
- 内置 Pi 实际输入 `/local` 打开筛选，Enter 仅补全；再点击发送执行本地扩展命令。
- 两条命令均在 JSONL 中找到各自唯一的新 delivery ID、handled 结果及完成标记；已有同名检查消息按旧 ID 区分，没有把它们误当成本次系统操作结果。
- Cmd+Q 零退出，使用包内 runtime 冷重开后，两条新输入各出现一次、已完成、无待投递。

外部会话初次发送先被独立的 Hive 自动恢复投递挡住，因为原隔离服务已结束。使用同一已配置回环端口返回一条明确合成的 Responses SSE，点击“重试原消息”后恢复投递完成，随后本地命令执行；未绕过/删除恢复消息，也不把该合成文本称为真实 Terra 输出。回环服务和本次应用都已停止，过程及两类 ID 分别记录在 `native-ui-receipt.json`、`native-ui-cold-receipt.json`、`ui-loopback-receipt.json`。

## 边界

macOS 的普通鼠标/键盘和上述实际包内流程已验证；输入法组合态、完整系统交互矩阵及 Windows 原生/ACL/安装仍未完成。Windows gate 不把 POSIX chmod 当作 ACL 验证，显式跳过这两项权限注入。Windows CI 的账户账单/额度问题未在本轮解决。Claude 真实模型协作与完整 Cindy 体验仍在实施，不以这份开发 ZIP 或检查通过宣称完整交付。
