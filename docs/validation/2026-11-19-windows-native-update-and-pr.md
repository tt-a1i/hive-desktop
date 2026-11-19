# Windows 原生更新验收与同名 fork PR

## Windows 更新验收入口

Windows 构建作业现在使用解包后的便携应用作为更新起点，并用同一份源码和锁文件编译一个更高补丁版本的测试候选。候选构建复用原来的依赖构建，一次执行一个 Cargo 作业，完成后恢复原有版本声明和可执行文件。测试候选只在本机回环地址提供，使用临时 Ed25519 密钥签署更新清单。[Windows 构建作业](../../.github/workflows/windows-build.yml) · [验收脚本](../../scripts/check-windows-native-update.mjs)

验收脚本对两个独立安装目录使用 Windows UI Automation。从实际设置页检查、下载和确认更新，再等待安装收据和窗口重启。健康候选需要显示安装成功并与候选可执行文件摘要一致。故障候选使用同一个新版可执行文件，后台在启动时主动退出。此时窗口需要显示恢复旧版，并与原始可执行文件摘要一致。两种情况都核对原会话的未发送草稿及安装进程退出。[原生控件脚本](../../scripts/windows-update-ui.ps1) · [Windows UI Automation 接口](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.automationelement.findall?view=windowsdesktop-10.0)

本轮通过 Node 语法、PowerShell 解析、工作流 YAML、候选版本锁文件检查和 Windows MSVC 交叉检查。使用便携包内的独立 Node，实际通过签名下载、篡改拒绝、安装程序所有权、文件替换、启动失败回退和安装进程中断恢复专项。直接使用依赖系统动态库的 Homebrew Node 复制测试运行时曾失败，使用包内 Node 后通过。新的 Windows 原生入口仍需在可交互的 Windows 运行器执行，当前尚无成功或回退的 Windows 窗口收据。

## 同名 fork PR

GPT-6.2 Sol 独立核对总目标时，在真实临时 Git 工作树和受控 `gh` 对端复现了误判。GitHub CLI 的分支筛选使用分支名称，跨仓库的同名 head 因此可以出现在列表里。Hive 现在从列表中筛出同仓库目标，再校验仓库、分支和提交。仅有 fork、同仓库匹配多条或达到查询上限时仍会拒绝。受控回归另外验证过期预览和确认后读回。[GitHub CLI 查询源码](https://github.com/cli/cli/blob/v2.103.3/pkg/cmd/pr/shared/lister.go#L50-L66) · [Hive 实现](../../packages/runtime/src/github-prs.ts)

macOS 原生窗口用真实 Git 把会话工作树推送到本地 bare 仓库。受控 GitHub 对端随后同时返回本仓库 PR #17 和 fork PR #18，两条使用同名分支。原生预览只显示 #17。窗口成功更新 #17 的标题和多行正文，保留草稿状态。对端只收到一次对 #17 的编辑，正文临时文件已清理。桌面进程以 0 退出，记录的 Node 后台也已退出。

本地验证 ZIP 复用了 `b4dfbc0` 的 Rust 可执行文件和同一份 Node，后台来自 `f6875ce`。ZIP 解包严格验签和逐项清单核对通过。包内 Node、PTY、Hive HTTP/SQLite、退出及 Git JSONL 检查通过。此次 macOS 验收使用受控 GitHub 对端。实际托管 GitHub 写入及当前 Windows 原生操作仍待验收。

本地证据位于 `.artifacts/windows-native-update-plan-INkbVx/`，包含 `mac-replay/mixed-preview.ax.jsonl`、`mac-replay/mixed-confirmed.ax.jsonl`、`mac/packaged-wire.log`、`mac/archive-manifest.json` 和更新专项日志。ZIP SHA-256 为 `4429378d2ce2285110a048c13e81ba40902e4690306158b8cbd2e2b6a9d7a3c7`。
