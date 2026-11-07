# macOS 内置 Pi：后台异常退出时清理正在执行的 Shell

## 问题与修复

实际 Pi 1.4.0 的 `createLocalBashOperations` 使用 detached shell，供取消与超时按进程组清理。桌面已有的 Node 进程组监督不能覆盖这个新组。无模型复现中，强杀 Node 组后 Node 27693 退出，但 Bash 27694 与 sleep 27695 仍然存活；夹具随后显式清理了它们。

macOS 现在通过同一个桌面可执行文件的非 UI 入口启动 shell：助手保持 Pi 的 detached 组，使用 kqueue 监听实际 Node 父进程的退出。先注册监听并复核父进程，再启动 shell；父进程退出时清理助手自己的组。拒绝父进程不符或非独立组的调用。现有 Pi 的取消、超时、输出、环境与退出码处理继续保留。桌面启动 Node 时自动提供助手路径；手工运行 Node runtime 不自动获得这项桌面监督。

改动固定在 Pi 1.4.0 的 pnpm 补丁中，macOS 未启用助手或其他平台继续使用原 shell 路径。没有新增权限审批、模型调用或依赖，也没有改写协作协议、会话和派单持久化逻辑。

## 实际验证

`node scripts/check-pi-shell-owner.mjs apps/desktop/target/debug/hive-desktop`：

- 实际 Pi Bash 执行器保持中文 stdout、stderr、cwd、自定义环境、非零退出码与信号退出码。
- stdin 传命令的 shell 路径通过实际 `/bin/sh -s`。
- 取消和超时后，助手、Bash、sleep 均退出。
- 分别强杀 Node 自身、Node 整个进程组后，工具组的三个进程均退出，无关 sleep 存活。
- 非 detached 助手调用失败，shell 没有执行。

生产依赖重新冻结部署，使用正式打包脚本生成 macOS arm64 bundle，以包内 Node、包内 Pi 与包内桌面二进制再次运行上述检查，并加 `--native`。native 检查从实际桌面 `Runtime::launch` 启动 Node，仅替换 runtime 入口以执行一个无模型 shell 夹具；没有手工给 native 环境提供助手路径。验证 Node 的直接父进程、实际助手路径与存活状态后强杀 Node：

| native 夹具进程 | PID | 结果 |
| --- | --- | --- |
| 窗口 | 50787 | Node 退出后仍存活 |
| Node | 50789 | 强杀退出 |
| 助手 | 50790 | 退出 |
| Bash | 50791 | 退出 |
| sleep | 50792 | 退出 |

命令（`bundle` 指正式脚本输出目录）：

```sh
HIVE_SHELL_CHECK_RUNTIME="$bundle/Hive Desktop.app/Contents/Resources/runtime" \
  "$bundle/Hive Desktop.app/Contents/Resources/node/bin/node" \
  scripts/check-pi-shell-owner.mjs \
  "$bundle/Hive Desktop.app/Contents/MacOS/hive-desktop" --native
```

另通过：`pnpm install --frozen-lockfile`、`pnpm check`、`pnpm build:runtime`、`cargo build --locked`、`cargo test --locked`（14 通过，1 个子进程夹具忽略）、实际 `checks/team-runtime.ts`、`scripts/check-package.mjs` 与 `scripts/check-desktop-startup.mjs`。后两项验证包内 PTY、HTTP/SQLite、会话改名/搜索、任务冲突、原生保存项目与会话恢复。没有运行所有后端的完整 suite；这次不涉及后端协议变更，集中验证工具进程与桌面生命周期。

## 边界与未完成项

- 本修复覆盖正在执行的内置 Pi shell。主动 `setsid` 逃离工具组的进程，以及工具已经正常返回后遗留的后台任务，保留 Pi 原有语义；不宣称管理任意 daemon。
- kqueue 清理异步发生，不表示 Node 监督的同步清理屏障已经纳入这些独立组。
- 没有修改用户外部安装的 Pi，不能把这份结果外推为外部 Pi CLI 同样被补丁修复。
- Windows 代码路径没有采用该 macOS 助手。新补丁的 Windows 冻结依赖构建与真机验证仍需继续；此前 CI 账单限制没有在本轮消除。
- 未重复视觉走查，没有声称 UI/UX 达到 Cindy 标准，也没有声称完整产品完成。

便携包 `.artifacts/hive-desktop-macos-arm64-shell-owner.zip` 已重新解压，比较原生二进制一致，再次通过包内 Node 的全部 Shell 检查（包括 native 启动路径）与 `check-package.mjs`。解压包 native 复验的窗口 71290 存活，Node 71292、助手 71294、Bash 71295、sleep 71296 均退出。该包仍为未签名、公证的开发包。
