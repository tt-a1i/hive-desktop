# 定时执行、中文参数与确认说明

源码基线 `191aca4`，对应 #13。补齐之前未验证的真实 `last_run_at` 分支，并修复删除确认缺少可读说明及计划身份的问题。无调度/协议/存储改动，仅工作流确认文案语义和英文名称。

## 真调度与窗口实证

自有隔离数据 `.artifacts/schedule-fired-191aca4/state-valid/`，真实原始Codex0.167.0只建立Terra模型元数据和主控身份，不发送模型输入。正式认证API创建A（每秒Cron）与B（未来年度Cron，立即暂停）。由现有真实30秒调度器触发A，真实Worker返回 `{noModel:true,message:"原生计划甲 · 仅元数据",audit:1}`；驱动观察完成后通过正式PATCH暂停A。未改时钟、直接写库或合成运行记录。

`seed-receipt.json`保存两计划和唯一完成运行。只读SQLite核对A的非空last_run_at和B的空值、均暂停、唯一完成运行及精确中文结果。Mac实际列表显示A“上次运行”、B不显示；通过Tab到真实运行卡、Space进入详情，页面展示同一中文结果和参数、零派单/零日志，见`last-run.jpg`、`run-result.ax.txt` / JPEG。

第一次seed误用 `/workflow-runs`，真实API返回404后正常退出；保留`seed-first-attempt.mjs`、`seed.log`、`state-codex/`。核对现有桌面读取路径后改用 `/workflows/runs`，另建`state-valid/`，不在产品里增加兼容分支。失败尝试未形成运行。

## 复制与确认

实际鼠标拖选中文参数→Cmd+C→返回聊天→粘贴，完整得到 `原生计划甲 · 仅元数据",`（包含选中的结尾引号、逗号）；不冒称选中整个JSON或只选中文值。此前误选下一行得到 `audit": 1`，随后按实际坐标重选；两次AX均保留。最终未发送草稿是后一次精确字符串，见`chinese-selection.jpg` / `chinese-copy.ax.txt`。

删除确认基线AX只有“确认删除计划”“取消”，缺少“运行历史会保留”的说明，操作也不标识对应计划。现在说明用现有Role::Label暴露完整计划名及原文；两按钮的可访问名称包含计划名，取消名称提供英文翻译。未改变视觉布局或删除handler。

最终构建原生AX可读完整说明和两项具体计划动作，见`confirm-fixed.ax.txt`。实际Tab逐项到可见取消按钮，Space取消后确认区消失、Refresh获得焦点，两计划保留，见`cancel-focus.jpg`、`cancelled.ax.txt` / JPEG。较早一次离屏AX点击取消未证实成功，不计通过。**未执行最终删除按钮**，不把取消检查当作删除验收。

## 检查与限制

Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、LLVM PATH 下Windows MSVC target `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`、`git diff --check`通过。本轮两次带环境配置的原生进程均Cmd+Q exit0；第一次退出后的额外getAX自动唤起无环境wrapper，出现找不到嵌入runtime提示，随即关闭，未将其视为已打包应用失败。

独立`check-cold.mjs`验证两计划ID/参数/时间/源码均保留且暂停、唯一完成运行逐字段不变、原生复制中文草稿精确、messages/dispatches/dispatch_messages均0、全部agent_runs已exited、主控history空且pending0/busyfalse，无新推理。见`cold-receipt.json` / `cold.log`。

纯UI改动未新增/修改测试，未跑全量suite；没有新包。current/candidate仍6edac4c。此轮窗口仅默认中文字号，英文新增键名有编译检查但未实际切换本轮英文确认；全语言/字号/VoiceOver、Windows原生、最终删除和其他调度场景仍待，#13和完整目标继续开放。
