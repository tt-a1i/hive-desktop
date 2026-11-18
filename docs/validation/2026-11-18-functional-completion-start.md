# 功能补齐首批验收

2026-11-18，从 `main` 的 `c6003ec` 开始，在 `codex/functional-completion` 开发。Hive 上游 `main` 当时为 `18861b80379bff0d658623a3aa6f1454159d6925`。本批沿用桌面锁定的 `@tt-a1i/hive@2.2.1` 及现有协作补丁。范围继续跟踪 [#39](https://github.com/tt-a1i/hive-desktop/issues/39)，桌面更新使用隔离测试渠道。

## 源码与测试

- `9c25929` 修复 Windows 本地时间 API 引用。本机交叉编译通过，原生构建和包内检查另见 [Windows 基线 CI](https://github.com/tt-a1i/hive-desktop/actions/runs/44334015186)。
- `9c2e3a7` 让跨会话正文搜索与只读打开共用固定 Claude 配置的官方离线读取器。关闭配置仍可搜索其原始历史。丢失配置会报告原因。用两份同原生会话 ID 的本地历史验证了默认目录隔离，修复前查不到指定配置正文，修复后命中正确会话且默认目录正文不可见。运行了 `pnpm check`、运行时构建和四组正文搜索检查；均无 Claude 登录或模型请求。
- `6c683e1` 将图片解码任务归入其消息或预览窗口，释放离开会话后不用的纹理，并直接把解码后的像素交给渲染器。macOS 桌面测试 113 通过、1 忽略，Windows 交叉编译通过。本地 Cargo 串行执行构建任务，每次最多使用两个编译作业。

## Mac 隔离包和真实窗口

`.artifacts/functional-completion-mac-JyGnF5/` 保留新构建的 debug 桌面程序、冻结依赖、编译后的搜索修复、50 项包内回归及原生验收收据。本批包内通过真实 Node、Hive HTTP/SQLite 和 PTY 检查。Windows 及 Mac 优化安装包是后续独立验收项。

在含 16 张原始历史图片的真实窗口中，打开并关闭同一张 2400 × 2400 图片 10 次。稳定阶段关闭时桌面进程 RSS 回落约 23 MiB，最后一次关闭后的 RSS 比第二次高约 0.36 MiB。数值是单次运行内的前端 RSS 采样。重新打开程序、切换到空白会话并切回图片会话，历史和缩略图仍可用。两次主窗口退出均为 0，观察到的后台进程均退出。参见 `image-rss.json` 和 `reopen-receipt.json`。

## 固定配置的真实 Codex 协作

`.artifacts/functional-parity/native-profile-retest-wagMiz/run-4cXnds/` 使用 `6c683e1` 的生产 JSONL 和同一现有 Codex 账号下的两份固定配置。守卫仅允许 `openai/gpt-6.1-terra`，实际 3 次开始回合、1 次引导、0 次账号操作。改变隔离环境中的应用默认配置后，主控和成员仍使用原配置。真实派单、成员阅读并按水位汇报、原主控自动收到报告并独立核对文件均通过。冷恢复没有新推理或账本重放。

命令行记录的总结果仍为失败，因为退出后立即采样发现三个后台后代短时存活，稍后独立检查均已消失。随后用同一测试数据在 Mac 包的实际窗口重开主控并恢复最终结果，原生守卫记录 `thread/resume` 和历史读取，零新推理。用窗口关闭退出为 0，受桌面进程监管的后台进程组已经为空。证据见该目录的 `receipt.json`、`owned-pid-exits.json` 和 `native-cold-lifecycle-receipt.json`。

`6c683e1` 的 [Windows CI](https://github.com/tt-a1i/hive-desktop/actions/runs/44336630371) 的 CLI 与进程监督独立作业通过，主作业在冷恢复后显式继续队列的检查中超时。已增加失败诊断。正式发布和安装替换均未执行。

## 桌面更新检查与下载

测试包可通过 `package-desktop.mjs --update-channel` 内置渠道地址与 Ed25519 公钥。桌面在“关于与维护”手动检查，核对签名、当前版本和平台，再在用户选择下载时重新确认清单。下载逐块写盘并核对长度及 SHA-256。失败或退出时移除未完成写入，保留已经验证的同一归档。源码回归覆盖错误签名、旧版本、目标不兼容、清单变化、归档损坏、重试及下载中退出。

Mac 隔离包 `.artifacts/desktop-update-staging-OUzXbY/` 连接本地回环测试服务。第一次包请求返回截断数据，窗口显示失败。点击重试后下载实际桌面 ZIP，写出的 161,354,479 字节与源 ZIP 的 SHA-256 一致。测试清单使用临时版本，验证范围截止到压缩包校验与正常退出。另有 51 项包内回归及真实 Node、PTY、Hive HTTP/SQLite 冒烟通过。验收文件为该目录的 `receipt.json`、`native.log` 和 `functional-receipt.json`。

更新代码通过 Windows 交叉编译。[Windows 原生 CI](https://github.com/tt-a1i/hive-desktop/actions/runs/44340284767) 跟踪当前源码的编译、原生进程及包。安装身份验证、替换、启动、恢复与回退仍待实施。
