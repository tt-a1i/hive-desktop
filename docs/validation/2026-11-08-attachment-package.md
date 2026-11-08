# 附件体验开发包

本包包含 `5582c43` 的附件只读预览/紧凑文件行及 `ef710bd` 的多图片限高滚动修复，源码基线为 `ef710bde64bc5c4bb81ddf6ca7e87da45ee0c37f`。

## 交付物与一致性

- `.artifacts/hive-desktop-macos-arm64-attachments.zip`，约 166 MiB；未签名、未公证的 macOS arm64 开发包，非正式安装器。
- ZIP SHA256：`0db4dee3de6b0727e3384d831b920a219768ae8166ec1b80a0decee0624d4a46`。
- 原生可执行文件 SHA256：`2b5c2114d451aeaf87197e41bc895434701be1c7ab5228a3fa6b3853518726c5`。
- 当前源码重新构建 runtime 与生产原生入口，未使用原生交互夹具入口。复用上一份已校验冻结开发包的同平台依赖和 Node；逐项确认 lockfile、workspace 设置、patch 和 package metadata 无变更，校验基础 ZIP 的既有 SHA256 后再提取。
- 完整替换 runtime dist 与原生二进制，104 个 runtime 文件与当前构建逐一校验；生成新 ZIP 后独立解压，再次逐一校验二进制及所有 runtime 文件。记录和构建脚本位于 `.artifacts/attachment-package/`。

## 解压包检查

均从新 ZIP 的独立解压目录运行：

1. `scripts/check-package.mjs --bundle PATH`：包内 Node、真实 PTY 自然退出、Hive agent-manager、HTTP/SQLite 工作区及关闭检查通过，无模型请求。
2. `scripts/check-packaged-codex.mjs --bundle PATH`：包内 Node/适配器与真实 JSONL 子进程，检查累计输出、中断、拒绝、后台工具结束、分页和重新打开；合成协议对端，无厂商推理。
3. `scripts/check-desktop-startup.mjs --bundle PATH`：实际生产窗口读取保存项目、恢复缺失的会话选择并写回导航，通过。它不证明附件控件已由系统操作逐一验收。
4. 包内 Node/适配器连接原始本机 Codex 0.166.0，恢复本项目先前实际 Terra 会话。工具 ID、原生失败状态及完整输出与原始收据一致，无新推理。

## 当前界面与平台边界

附件源代码的原生点击、浅深/小窗、键盘滚动、复制、源文件删除、副本恢复、退出恢复和实际发送入口证据分别见 [附件预览](2026-11-08-attachment-preview.md) 与 [多图片溢出](2026-11-08-attachment-overflow.md)。

这轮另启动了新 ZIP 的生产应用，用隔离的真实 Hive 上传/原生 Pi 历史/草稿准备系统界面走查，但电脑当前锁屏且自动解锁失败。已请求用户方便时解锁；尚未完成新包的系统界面操作，不能把启动成功或既有源码夹具结果当成此次系统点验通过。临时应用及数据保留用于解锁后继续，位置/PID 记录于 `.artifacts/attachment-package/native-ui-location.json`，操作前须核对进程仍存活，不能凭文件推断运行状态。

最新 Windows CI [42457358276](https://github.com/tt-a1i/hive-desktop/actions/runs/42457358276) 对应 `ef710bd`，三个 job 都无执行步骤，annotation 仍明确是账户付款失败或需要提高额度。源码 Windows MSVC 编译检查通过，当前 Windows 原生监督/CLI/GUI及正式安装验收仍未完成，不反复重跑同一账户阻塞。

本轮没有修改产品代码、协议或依赖，不重复无关全量测试。无新厂商模型请求。完整客户端与 Cindy 整体体验对标仍继续实施。

## 解锁后的系统界面补验

用户回复已解锁后，重新核对 PID 74549 仍指向本包解压应用，再通过系统 UI 打开历史/草稿中的附件预览：原文件已删除，上传副本仍可阅读，重新读取后返回聊天，原草稿与两个附件保留。实际打开图片原图与 macOS 另存为对话框，完成保存；所选路径中的 69 字节 PNG 与原图 SHA256 一致，为 `b1ff9c8ea3a780bad09b346c423d2d0e46815926879b18e841d928376a946640`。

第一次数值设置后保存按钮保持禁用，取消后改用键盘输入并离开文件名字段才完成保存；自动化键入的最终文件名与预期不同，因此本轮只确认实际所选路径的图片完整保存，不将精确文件名输入当作通过。实际路径和该限制记录在 `.artifacts/pi-user-input/package-physical-ui-receipt.json`。

通过 Cmd+Q 正常退出，原控制进程收到零退出码，重新读取磁盘草稿和两个附件均保留；收据 `.artifacts/attachment-package/native-ui-receipt.json` 已生成。不是厂商模型调用、Windows 验收或最新 Pi 输入/命令选择器的新包验收。本段替代前文的“仍等待解锁”状态，保留先前锁屏失败记录。
