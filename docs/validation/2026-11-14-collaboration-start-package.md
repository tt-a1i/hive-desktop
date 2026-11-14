# 当前 Mac 开发包 c24069c

2026-11-14。协作起步、窄窗任务编辑及长标题修复已进入 current/candidate ZIP 和 DMG。用户正在运行的 `/Applications/Hive Desktop.app` 未替换，完整目标继续。

## 包与来源

| 项目 | 结果 |
| --- | --- |
| 应用源码 | `c24069cf0d9db1357071b51e031535230d8a7e34` |
| 构建 | 默认功能、优化 release，无 ui-preview |
| ZIP | `.artifacts/hive-desktop-macos-arm64-current.zip`，144,128,996 字节 |
| ZIP SHA-256 | `6eb934e4aadf520fa8e44f555a574c21737592d653c303b124311e746f32a5c1` |
| DMG | `.artifacts/release-c24069c/Hive-Desktop-macos-arm64.dmg`，245,238,341 字节 |
| DMG SHA-256 | `8e1c00782f103ab5a13c7331a3340fddeae8b3fcc6de15694a6aaee215766902` |
| 原生程序 SHA-256 | `1da1b5ed82b48c242c0779eca5727dd7e71b0706462d62e34216faf5fb17abd5` |

以已验证的 f9f3c2b 包为底座。全部 28,869 项清单、权限及 29 个链接核对，仅原生程序改变；生产 runtime 116 份文件逐字节匹配。后续 ff9adab 只改检查与记录，独立确认生产源码/依赖与本包来源无差异。没有因 test-only 修复重新部署依赖。

四组包检查 `check-package`、`check-packaged-codex`、`check-packaged-dispatches`、`check-packaged-queue` 全部退出 0；包内 Node/runtime 的原始 Codex、Pi SDK/外部 Pi、HTTP/SQLite、PTY、持久队列与关闭检查按各自受控对端范围运行，不当成厂商模型验收。DMG 只读挂载核对清单与 Applications 快捷方式并卸载，原生窗口启动/恢复也退出 0。

## 最终 payload 原生与真实生命周期

使用同一原生程序和包内 Resources，仅独立 QA 身份用于区分正在运行的用户安装实例；移除 Node/runtime/preview 覆写。中英文、20px、822×503 及宽窄切换中，协作起步操作首屏、长标题省略/工具实点/重命名取消、任务编辑自动显露/输入/取消、原协作报告和草稿、Grouped 跨进程保留均通过。

在最终包中通过界面打开新的隔离项目，选择原始 Codex / GPT-6.1 Terra / Medium，实际运行托管的 `python3 hold.py`。独立确认 Python PID 及 ready 文件后，界面排入一条补充输入并 Stop。停止显示 Interrupted，队列等待显式继续；正常退出后独立确认 Python 消失且没有运行完成标记。

包内 Node 冷读确认：原线程/模型、原排队 ID `12c4cea9-4bce-4b41-b227-ce9d0c39a598`、完整后台文本/展示原文保持，`pending=1`、`busy=false`、无活动运行/令牌。再次启动最终包，实际等待约5秒仍暂停且没有自动重跑；只点击一次 Resume 后，原输入完成一次，唯一答复为 `HIVE_NATIVE_QUEUE_20261114_OK`，没有新增工具，旧命令继续显示 Interrupted。再次完整冷读，队列为空、原完成 ID/全部历史保持，探针没有重跑，原协作三会话和草稿不变；所有隔离项目均无拥有的残留进程。

该生命周期包含2个真实原生回合。进程消失是在 Stop 后退出应用时独立观察，未声称已在 Stop 返回瞬间采样。保留厂商原始 `inProgress` 字段，另核对 Hive 的 `toolStatus=interrupted`；也区分完整后台文本和展示原文，不用改写原始数据制造成功。

## 真实协作及边界

本轮从原生界面完成的一主控/一成员真实协作见[记录](2026-11-14-collaboration-start-and-live.md)。本包以包内 Node/正式 JSONL 冷读原三会话36条完整消息、ID、完成回执和角色搜索；不把历史重读称为本包又跑了一次派单。报告正文列出了文件，结构化产物数组为空，相关附件交互未据此验收。

Pi 当前原生账号 invalid_grant，不重复登录或推理；Claude/Antigravity 按用户要求本轮不验收。Windows 已通过 Node 检查、优化版编译、便携包、包内检查及窗口恢复项目；后续外观交互检查遇到陈旧的设置格式断言，继续修复复验，不将这些步骤等同 Windows 完整使用。中文/emoji 粘贴通过，直接 Unicode 的工具注入及完整输入法/辅助技术矩阵未验证。包未签名/未公证，系统通知授权与正式发行仍单列。

## 回退、清理与证据

f9f3c2b 保留可直接运行回退。未推广的 5a220f2 在实际窗口发现标题问题，其 ZIP 与失败/修复记录保留；移除其重复解压/组装目录与无用 DMG，以及新包已完成的组装 base，回收约1.62 GB逻辑文件大小。当前解压包、全部原数据/日志/收据及近期仍用于开发的 Rust 缓存保留。

包清单/检查/DMG/推广/清理记录位于 `.artifacts/release-c24069c/`；原生退出与真实生命周期证据位于 `.artifacts/product-completeness-20261114/` 的 `title-production-process.json`、`lifecycle-production-resume-process.json`、`lifecycle-receipt.json`、`lifecycle-cold-before-resume.json`、`lifecycle-completed-snapshot.json`。初始验证脚本中原始字段/JSON 边界的错误期望及修正日志也保留，未修改生产数据或放宽最终消息比较。
