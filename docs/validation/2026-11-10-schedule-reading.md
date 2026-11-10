# 定时计划正文与键盘控制补验

基线 `02868f0`，对应 #13。工作流计划已有暂停/恢复能力，本轮发现其正文虽然可见，无障碍树却只有恢复/删除按钮；Cron、状态、路径和参数没有可读文本。原生基线见 `.artifacts/schedule-native-02868f0/before.ax.txt`。

## 修改

计划名称和启用/暂停状态增加稳定的语义标签；Cron/下次运行、上次运行、完整路径及参数复用现有 `text_reading::selectable_text`，保留原值并支持正文选择。长文件名标题省略显示，完整名称仍在语义标签、操作标签与路径正文中。

只改 `workflows/view.rs`，没有新增组件抽象、依赖、后端协议、定时器或业务存储；暂停/恢复沿用现有 PATCH，未改调度语义。

## 真实数据与原生验证

使用真实 Hive HTTP/SQLite、原始 Codex 0.167.0 的 Terra 元数据建立自有主控，从正式认证 API 创建两条未来年度计划，初始均暂停。脚本只返回 `{noModel:true}`，本轮没有触发工作流脚本或模型回合。

首次夹具使用全新 Pi 目录，SDK 不认识该空目录里的 `openai/gpt-6.1-terra`，明确失败退出。保留 `seed-pi-attempt.mjs`、`seed.log` 和原状态；随后另建 `state-codex`，通过原始 Codex 的实际目录选择 Terra。没有修改产品模型目录或增加测试 fallback。

- 修复后原生树可以读取两计划的完整文件名、暂停状态、`Cron: 0 0 1 1 *`、完整路径及中文 JSON 参数，见 `readable.ax.txt`。
- 鼠标恢复计划 B，UI 显示启用及下次运行；独立只读 SQLite 确认只有 B 启用，A 仍暂停，见 `resumed-sqlite.json`。从页面初始刷新焦点 Tab 到 B 的暂停按钮，实际看到焦点框后按 Space，UI 和独立 SQLite 均恢复为两条暂停，见 `paused-keyboard.ax.txt`、`paused-sqlite.json`。
- 通过应用设置切换 English/20px，原生状态/Cron 说明翻译，中文参数原值不变。在约592×506窄窗收起侧栏，长标题省略，恢复/删除按钮仍可见，完整路径和参数正常换行。
- 窄窗重新进入工作流，从刷新焦点 Tab 到离屏计划 B 的恢复按钮，实际自动显露其完整按钮/焦点框，Space 恢复、再 Space 暂停均成功，见 `narrow-keyboard.ax.txt` / PNG。没有把 AX 树中存在离屏控件单独当作可达证据。
- 拖动选区时页面位置发生变化，本轮实际选中的是 Cron 中的 `oes not trigger`，不是原先准备选的中文参数。真实 Cmd+C 后回到聊天、Cmd+V 得到该精确片段并保留为未发送草稿，见 `copied-cron.ax.txt`；只认这条实际复制结果，中文参数的精确选区复制仍待补验。
- 两个原生窗口均正常 Cmd+Q exit 0。`check-cold.mjs` 用独立真实 runtime/SQLite 冷读：两条计划的 ID/工作区/路径/Cron/参数/下次运行/创建时间保留、均暂停、上次运行仍为空；实际脚本内容精确匹配原自有源码。messages/dispatches/dispatch_messages/workflow_runs 均零，原 agent run 已 exited，主控 history 空、pending 0 / busy false。英文/20px/精确复制草稿保留，见 `cold-receipt.json`、`cold.log`。

## 检查与边界

`cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、LLVM PATH 下的 Windows MSVC target check、`git diff --check` 均通过。纯 UI 修改未新增/删除测试，未运行全量 suite；实际验证为上述原生窗口、真实 HTTP/SQLite 与冷读断言。

本轮无 last_run_at 非空数据，未把该条件分支标为实际窗口通过。计划删除最终操作、中文参数选择、跨连接在途时序、完整 VoiceOver/大历史与 Windows 原生仍待，#13 保持开放。current/candidate 仍为 `6edac4c` 开发包，未纳入本轮源码；完整目标保持进行中。
