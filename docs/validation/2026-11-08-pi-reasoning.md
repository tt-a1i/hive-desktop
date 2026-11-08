# Pi 会话推理强度与设置失败恢复（2026-11-08）

## 真实使用场景

1. 用户在内置/外部 Pi 会话中使用同一模型处理不同难度的工作，桌面原先只能切模型，不能读取或调整原生推理强度；用户还需要保留其他会话及 Pi 全局默认。
2. 输入框旁可打开“推理强度”，读取当前会话及原生可用档位、选择并重新读取。切换/重开保留会话的实际选择，中文草稿不变；生成中禁止修改。写入失败明确报错，释放不能继续信任的原生内存状态，修复文件后可重读和重试。
3. 不做只能离开桌面操作 CLI，或修改会影响其他会话的默认值；设置失败后若继续复用原生内存分支，还可能让后续持久化引用不存在的父节点。

## 实现与边界

通过已有 Pi SDK 1.4.0 `thinkingLevel/getAvailableThinkingLevels/setThinkingLevel({persist:false})`、外部 Pi 1.4.0 RPC `get_state/get_available_thinking_levels/set_thinking_level` 接入。不构造统一的假档位，不改 SDK、用户全局模型/推理默认，不新增权限审批。后端返回自己支持的档位；提供商仍可能在请求内部按自身能力处理，界面不承诺精确推理 token 数或耗时。

`chat.reasoning {sessionId}` 返回 `{level,levels}`；`chat.setReasoning {sessionId,level}` 严格校验文本及当前原生能力，使用现有工作区操作屏障和真实投递队列忙碌检查。未知档位拒绝，生成中的会话拒绝修改。仅内置 Pi/外部 Pi 接入；Codex 和 Claude 的推理控制还未加入桌面，不显示假选项，也不声称四后端强度设置均已完成。

Rust 使用现有 Popover/Button 放在模型选择旁，按真实返回状态显示；打开/重读时请求原生状态。切换会话、模型或后台连接时清除旧结果，晚到结果用连接代数与打开请求身份过滤。请求期间阻止同时切模型，保存失败后重新读取实际原生状态；文件仍不可读时保留错误和重读入口。没有新增 index 字段或第二份会话偏好存储。

## 修复的真实持久化问题

首次检查曾用原生 setter 回写旧值尝试恢复，重开检查失败。源码与实际文件错误共同证明：Pi SessionManager 在 append 失败前已经推进 fileEntries/byId/leafId，仅回写旧强度会继续引用未落盘节点；这种恢复不可用，已移除。

现复用 Chats 既有 failLive 的 pause/abort/dispose 路径，任何原生配置变更失败后释放该 handle；下一次读取从持久记录恢复。推理设置和原有模型切换共用 `changeNativeSettings`。模型写盘失败前测试明确复现引擎仍选中未提交提供商，修复后读取恢复原提供商。外部 Pi 初始化只有选中的模型确实不同时才调用 set_model，避免重复选择同一模型重置其会话强度。

这是一致性恢复措施，不是原生文件、桌面 index 与 Hive 的跨存储事务。发生错误要按界面提示核对并重试，不承诺任意崩溃位置下的跨文件回滚或 exactly-once。后台清理失败仍向调用者报告，未吞异常或增加测试专用 fallback。

## 后端与协议验证

`pnpm --filter @hive-desktop/runtime check:pi-reasoning` 已通过：

- 真 Hive HTTP/SQLite、内置 SDK 和外部 Pi RPC，实际模型元数据统一为隔离 `gpt-6.1-terra`，读取能力、修改、非法输入拒绝、会话重开与默认文件逐字节不变。
- 两种后端分别将真实 transcript 替换为目录来制造原生 append 错误；失败不改持久文件，释放旧 handle 后恢复原模型/强度，再次保存及重开正确。
- 先新增失败检查复现切模型后引擎留在未提交提供商，再修共享配置失败路径。外部重开会追加当前进程的 Hive 工具绑定，检查按两个失败时刻各自的新文件快照比较，没有误把合法绑定变化当成写盘回滚失败。
- 两个真实用户输入进入已有队列，分别到本机 HTTP 挂起端点；生成时拒绝设置、保持原 runId 和强度，实际 abort 后恢复空闲。只发生两次回环请求，未调用厂商或其他模型。
- 单独的生产 JSONL 子进程检查原生读取/修改、严格参数、原生 transcript 和正常 shutdown；不发送模型请求。

原有团队定向 suite 通过。外部 Pi suite 前五项通过，进程退出/恢复项在使用本机全局配置时两次 get_state 超时；保留失败日志。该检查现使用自己的临时 Pi 配置、虚拟凭据和回环模型，原有杀进程/身份/队列/后端断言保持，通过独立复验。原先环境中具体哪个配置导致超时没有确认，不以重跑成功声称根因已经查明，也没有为测试修改生产超时或降级。

## 原生窗口验证

浅色、深色和 760px 窄窗口分别运行真实 sidecar/HTTP/SQLite/SDK/RPC，每种两次独立进程：点击读取、选择高档位、实际文件错误、修复后重读/重试、中档位保存、切换到外部 Pi 并设置高档位、切回原会话、退出重开；两份会话设置与中文草稿保持，全局设置文件完全不变。截图与夹具源码保留在 `.artifacts/reasoning/`。

原生检查通过 GPUI 临时帧/AX 边界观察 hook 注入真实按钮鼠标事件，生产源码没有这些 hook。它不代表物理鼠标、系统输入法或 Windows GUI 验收。Windows 的 native-clis job 已接线安装固定 Pi 1.4.0 后运行定向检查；账单限制解除前，不能视为 Windows 原生通过。

## 生产构建与开发包

最终默认功能生产构建、18 项 Rust 测试（1 ignored）、Windows x64 MSVC 类型检查、runtime build/typecheck、源码空白及 rustfmt 检查通过。冻结锁文件部署生产 Node 依赖，重建的二进制不含验证夹具标记。最终 ZIP 已实际解压，二进制、chats.js 和两个 Pi 适配器 hash 与本轮构建一致。

解压包通过 bundled Node 22.23.3 的生产 JSONL 推理读写/拒绝路径、实际 PTY/Hive HTTP/SQLite smoke 和原生窗口保存项目/失效选择恢复。未签名、未公证，不是正式发行。

- ZIP：`.artifacts/hive-desktop-macos-arm64-reasoning.zip`。
- ZIP SHA-256：`d916f736634dd13b5bd771e0dbc130a4e723a4111c82b15186978609924f6228`。
- 二进制 SHA-256：`555571c4669286579a52ab2d50da4621a131cd8941a31cf6a49f1a1524bc2610`。
- chats.js SHA-256：`7c882a78903c00f149b6ae19e70c62c8efadd4521b474152c7f9e617b8d83277`。

实际验证命令：`pnpm check`、`pnpm --filter @hive-desktop/runtime check:pi-reasoning`、`check:team`、外部 Pi suite 的前五项与修正隔离后的 `tsx checks/external-pi-runtime-lifecycle.ts`；`cargo build/test --locked`、`cargo check --target x86_64-pc-windows-msvc --locked`、`cargo fmt --check`；三个原生主题/尺寸变体；`check-package.mjs`、`check-desktop-startup.mjs` 和包内 Node 执行 `pi-reasoning-wire.ts`。

新增两份真实后端/JSONL 检查，既有外部 Pi 生命周期检查改为隔离配置，原杀进程、原生身份、持久队列和后端约束断言保留；没有删除测试。此轮复用设置与已有清理生命周期，没有更改 Hive schema、协作协议、队列实现或权限边界，按风险运行定向回归，未运行全量 Node suite。Codex/Claude 会话推理控制、真实账号模型验收、Windows GUI 与正式发行继续待完成，完整产品目标不收缩为本轮功能。
