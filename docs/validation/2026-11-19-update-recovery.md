# 安装程序中断后的恢复

取消安装的资源清理、交接确认及未重新下载的原生安装重试见[后续验证](2026-11-19-update-installer-ownership.md)。

`a72a0dc` 在替换前生成独立的 **Recover Hive Desktop** 入口。它和安装程序使用同一个 SQLite 文件锁。安装进程被终止，操作系统会释放锁，用户可以在关闭 Hive 窗口后手动运行恢复入口。恢复先核对持久化进度和对应程序的 SHA-256，再恢复未能启动的旧版路径，或补记已通过启动检查的新版。新版须收到安装程序写入的启动 PID，才会打开数据目录。

## macOS arm64 打包验收

用正式打包命令组装同一源码的 `0.1.0` 和 `0.1.1` 开发包，冻结运行时依赖，由本地回环渠道提供签名清单。三次均从原生设置页选择下载、安装和重启，观察原进程正常退出。

- 正常安装 `0.1.1` 后打开原工作区，输入框显示更新前的草稿。安装收据为 `installed`。
- 将隔离测试包的后台入口改为立即退出再签名。新版未通过启动检查，安装程序自动恢复并重开 `0.1.0`。原草稿可见。
- 实际移动旧应用后，在新版尚未移入时向隔离安装程序发送 `SIGKILL`。观察到原 `.app` 路径消失并确认安装进程退出。执行实际生成的 `.command`，恢复旧版的正常启动路径。重开后原工作区、草稿及 `rolledBack` 收据均可读取。

正常包 ZIP 为 163,032,775 字节，SHA-256 为 `e685cb15b4a6abc0e11fc43e5ed847511ffc4c9e0eabf8ed82e60767bdcb96c0`。逐次证据保存于 `.artifacts/update-recovery-mac-SxyNx3/receipt.json`、`verification.log`、`interrupted/first-rename.json` 及各测试的 `old-desktop.log`。新版二进制单独探测在缺少启动确认时退出，且没有创建隔离数据目录。所有测试都未调用模型。

## 回归与边界

真实子进程回归验证安装程序仍运行时拒绝恢复、终止后可恢复、重复运行可读取同一终态。更新后的实际应用通过全部 52 项包内功能检查，以及包内 Node、实际 PTY、Hive HTTP/SQLite 和退出冒烟。收据为 `.artifacts/update-recovery-mac-SxyNx3/functional-receipt.json` 和 `post-install-smoke.log`。类型检查、运行时构建、macOS 构建及 Windows 交叉编译通过。本机 Cargo 限制为单作业。

Windows 主作业通过编译、49 项便携包内功能检查及原生窗口。包内安装子进程测试通过，覆盖第一次目录移动后终止进程并恢复。Windows 桌面应用实际替换和 `.cmd` 恢复入口仍需实测。独立 CLI 检查在官方 CLI 启动前的进程枚举阶段失败。实际断电、重启以及迁移用户数据后的降级仍待验收。测试只使用隔离开发包，没有对外发布。[Windows CI](https://github.com/tt-a1i/hive-desktop/actions/runs/44352782083)
