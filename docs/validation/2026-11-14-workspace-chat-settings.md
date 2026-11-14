# 固定协作入口与聊天阅读设置

2026-11-14，[#29](https://github.com/tt-a1i/hive-desktop/issues/29)。从 main `26ebdea` 开始，在 `codex/product-completeness` 实施。本文记录源码与隔离 Mac 原生窗口验收；安装包另列。完整目标 #1 继续开放。

## 已实现

- 普通空项目保留团队成员、协作概览和工作流入口。团队/协作为两条紧凑全宽文字行，保留既有事件、选择器 ID、窄窗返回和移除/断线禁用条件。
- 聊天设置新增成功工具合并/逐项、思考默认展开/收起。失败、中断、执行中工具不参与成功分组，详情仍可展开。
- 原子保存本机偏好，兼容原 `"enter"` / `"modifier_enter"` JSON，保留发送快捷键；写入成功后才更新生效值，失败保持旧设置。设置不改 Agent 原生配置。
- 流式最终快照保留手动展开/收起；返回会话时恢复所有已见块的选择，后台新块使用当前默认。默认改变后旧缓存不覆盖新默认，未公开内容始终收起。
- 保存结果固定显示在滚动视口外，窄窗真实写盘失败和恢复重试均可见；设置搜索包含新控件的完整中英文名称。
- 开发文件预览路由使用现有只读预览，移除覆盖项目 README 的行为。

## 检查

| 检查 | 结果及范围 |
| --- | --- |
| `pnpm check`、`pnpm build:runtime` | exit 0；本批未改 Node 后台生产源码 |
| `cargo test --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` | 42 passed / 0 failed / 1 ignored；ignored 为既有子进程夹具。原快捷键检查扩展为两项设置/历史格式检查，新增三项思考状态回归，未缩减检查范围 |
| macOS debug `--features ui-preview`、默认 release 构建 | exit 0；发布构建不启用预览特性 |
| Windows MSVC 交叉类型检查 | exit 0；普通 PATH 首次因缺 llvm-rc 失败，将 `/opt/homebrew/opt/llvm/bin` 加入 PATH 后通过，不代表 Windows 原生验收 |
| 英文词条闭合 | 1,344 键，源码显式 `tr(...)` 无缺项 |
| `check:claude` | exit 0；官方 SDK/原生 CLI 初始化、目录及受控协议、生命周期、持久化和交互检查，无模型请求 |
| `check:external-pi` | exit 0；真实外部 Pi RPC、Hive HTTP/SQLite 和受控投递/生命周期检查，无模型回合 |

四份独立复核覆盖持久化/边界、阅读状态/性能、测试质量、目标与可用性。发现并修复了后台新思考块被旧缓存收起、兼容测试须使用固定历史字面量、控件名称无法搜索、流式状态转换覆盖不足。修改后逐条复核；实际界面另由 Computer Use 代理点测。

## Mac 实际窗口

独立 `Hive Product QA.app` 仅用于区分用户已运行的同名安装实例，执行本批原生二进制并连接真实 Node runtime / Hive HTTP / SQLite / Pi SDK。项目、数据和 Pi 目录均隔离；SDK 本地历史夹具含两项成功工具、一项失败工具、思考和两条会话，这不是新的模型协作证据。

实际通过：空成员/派单时进入团队、协作、工作流并返回；旧 ⌘Enter 保留；工具合并/逐项与失败独立；思考全文、手动收起及跨会话保留；两行中文/emoji 草稿逐字保持；完整退出/冷开恢复语言、字号与偏好；中英文搜索；约 822×503 窄窗及 20 px 下末项 Tab 自动显露。首轮英文团队标签截断已修复并复验。

真实故障：配置已加载后，仅将测试偏好文件移至独有备份，以同名目录制造 EISDIR。点击未选模式后原值保持，红色提示在当前窄窗视口可见。主代理独立核对备份哈希，恢复后重试成功；固定成功提示可见，聊天实际恢复成功工具分组。

预览路由实际自动打开隔离项目真实 README，正文和只读标注正确。关闭后草稿保持；主代理独立核对 README 与草稿逐字相同。两个最终自有原生进程均 exit 0，Node 子进程清空；独立 SQLite 核对 messages、dispatches、dispatch_messages、agent_runs、workflow_runs、workers 均为 0。未操作用户原安装实例或原项目数据。

证据：`.artifacts/product-completeness-20261114/` 下准备/启动脚本、`ui-location.json`、`native-first-pass-receipt.json`、`fixed-notice-fault-baseline.json`、`fixed-notice-fault-receipt.json`、`native-final-receipt.json`、`*-process.json`、Rust/Windows/运行时日志。截图仅在 Computer Use 工具输出中观察，未保存 PNG。

## 边界

本批完成固定入口与阅读设置，整体功能完善继续按 [完整产品计划](../product-completeness.md) 推进。流式回归覆盖状态转换，不代替真实端到端模型流式验证。四后端及混合角色、Claude 厂商推理、Pi 账号、通知系统授权、Windows 原生、签名/公证和全部输入/辅助技术/性能矩阵分别验收。
