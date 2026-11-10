# 成员模型读取失败后的配置修复

2026-11-10；源码基线 be96578。仅源码变更，当前开发 ZIP 仍为 6148801。

## 真实问题与改动

1. 用户给已有成员选择模型，外部 CLI 路径失效时只能看到错误；没有正在打开的聊天也会遇到这个问题。
2. 错误下直接提供该成员后端的配置入口与“重新读取模型”。修正路径、保存、返回后保留成员筛选与模型面板，用户显式重读即可恢复目录。原来需要自行寻找配置，再关开模型列表。
3. 不做会把成员配置卡在不可操作的诊断上，尤其首次设置时无法顺着当前任务完成。

复用已有聊天修复按钮和设置焦点/返回机制，仅将修复按钮渲染提取为显式后端参数；旧聊天调用仍从当前会话取后端。成员错误使用失败请求保存的后端，不借用当前聊天。重读沿用原目录请求和失效响应检查，完成后可直接输入搜索。未改变后台、权限、认证或就绪判定；程序版本检测不代表账号登录或模型调用成功。

## 原生与冷读证据

自有副本 `.artifacts/member-repair-be96578/`，源夹具 `.artifacts/member-model-4d53de5/` 保持原哈希。English、Dark、20px；无主聊天、13名已停止成员，目标 Added-by-UI 原有 openai/gpt-6.1-terra。

- 改前实际窗口：错误为 `spawn …/missing-codex ENOENT`，只有诊断，无修复/重读操作（`before-error.ax.txt`）。
- 改后同错误出现 External program settings / Reload models。进入正确 Codex 字段后无需再点输入框即可完整替换为原始 `/Users/tsk/.npm-global/bin/codex`；实际检测版本 0.167.0。检测后保存值仍旧，未把检查误当保存。
- 首次点击屏外 AX Save 无效果；Tab 到可见 Save 焦点后 Return，实际显示 Settings saved 与新生效路径。Escape 返回同成员、原 Added-by-UI 筛选和打开的模型列表，旧错误仍保留，等待显式重读。
- 缩为约593×507逻辑像素，折叠导航并滚至错误。路径折行、两个操作完整可见；模型搜索框 Tab 两次到 Reload models，焦点圈可见（`narrow-retry.jpg`）。Return 后真实加载目录，获得10个 Codex 模型；无需再点搜索框输入 gpt-6.1-terra 后仅显示对应 Terra（`recovered.ax.txt` / `recovered.jpg`）。未点 Start、未改选模型。
- 正常退出（进程 exit 0），独立 Node22真实 Hive HTTP/SQLite 冷读：13成员完整对象与准备记录一致，目标原始 launch command/args 仍为 hive:codex / openai / gpt-6.1-terra；仅自有 Codex 程序路径按操作更新，Pi/Claude覆盖仍为空。`cold.mjs` / `cold-receipt.json`。
- 独立SQLite对照：messages、dispatches、dispatch_messages、agent_runs、workflow_runs均0；其他非app_state表数据相同，但内置command_presets的updated_at在每次启动刷新，比较时只排除该列。最初逐字比较因此失败，查清差异后限定排除；不宣称所有数据库字节不变。原源夹具文件哈希精确不变（`integrity.json`）。无新增模型推理。

验证：macOS `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`；Windows目标 `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`；`git diff --check` 通过。纯原生UI改动，不新增测试、不跑全量测试；真实后台与冷读验证如上。

## 参考与剩余边界

本轮实际只读查看已安装T3 Code项目/会话与输入区、Cindy侧栏/聊天；Beluga进入Agent选择引导，观察Codex Ready与其他Agent的不同状态，随后Back恢复原欢迎页，未登录、安装或修改用户配置。Cindy一张截图受系统捕获失败影响，不作完整视觉验收。源码参考仍按 `docs/research/2026-11-09-desktop-source-references.md` 固定快照；本改动未复制上游源码/资产。借鉴的是如实说明状态、就地修复的流程。

本轮原生复演覆盖Codex成员失败；其他后端映射复用已有修复入口，未声称全后端实机通过。新会话引导就绪展示、完整辅助技术/语言主题矩阵、Windows原生和新包仍开放，完整产品目标未完成。
