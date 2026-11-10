# 计划删除确认显露与键盘恢复

2026-11-10，源码基线b13ea26，#13后续。当前开发包仍46373f6，本轮新源码尚未打包。

## 实际问题与修复

中文20px、浅色、约593×507逻辑像素窗口中，键盘激活计划“删除…”后，确认说明和按钮出现在原卡片下方、视口外；再按Tab才显露。用户无法立即看到操作反馈。确认删除后，焦点又随被移除的按钮消失。

沿现有工作流状态加一个复用的Cancel焦点句柄：展开后下一帧聚焦取消按钮，由现有Reveal显露整段确认操作；初始焦点不落在不可逆确认上。取消继续沿用原导航恢复；删除成功清除确认对象时，也用既有focus_navigation在读取结束后恢复刷新入口。没有新增对话框、调度器、后端协议、权限、依赖或存储。

## 真实流程

仅在.artifacts/workflow-delete-b13ea26创建本轮自有隔离项目、计划和运行；不删除用户项目、旧聊天目录或原验收状态。原样当前包内Node/runtime、真实Hive HTTP/SQLite/Worker，无mock。

1. 自建completed运行产生OWNED_HISTORY_RETAINED日志和{retained:true,value:42}结果；自建target/peer两个未来计划，均通过真实API暂停。
2. 基线窗口键盘定位第二条target删除，Return后确认区仍离屏，再Tab才显露；Tab到Cancel/Return后关闭。独立HTTP回读两条计划逐字段未变。
3. 修复后同一场景Return展开，布局完成后说明、Confirm和Cancel可见，Cancel有焦点框；不改变焦点再次Return，取消并保留target。鼠标重新打开也自动显露Cancel；Shift+Tab选择确认、Return后target消失，peer保留。
4. 原生重新打开完成运行，原结果、日志、源校验与参数均在。关闭窗口后独立真实HTTP冷读：peer逐字段精确等于创建时快照，run和logs也逐字段相等。
5. 删除后的焦点恢复修复，再自建第二个paused测试target，执行同样原生键盘确认删除；删除后刷新入口有焦点框，Return刷新、Tab进入阅读区、Home到列表首部可用。
6. 三个原生窗口Cmd+Q均exit0。最终独立冷读只剩原peer且仍暂停；原completed运行及日志逐字段不变、总运行数1、待处理投递为空。两次目标计划删除均由界面实际操作完成，不把仅出现确认区算作删除成功。

## 单独记录的准备偏差与模型回合

最初合成脚本漏了工作流meta.description，上游真实返回500；修正脚本并保留失败状态，没有给生产代码加fallback。首次原生恢复夹具未保存Codex路径，PATH命中了不支持app-server --listen的旧程序，界面正确报告初始化失败；修正隔离数据的外部程序配置，后续恢复成功。

准备第二个测试target时，驱动显式调用chats.resume，随后才queue.pause；这一间隔中消费了先前completed工作流的系统通知，确实产生一次真实模型回合。因此整个本批不能声称零模型请求。精确owned原生JSONL为01a1c4c1-fef8-7270-8fb9-b2ef81258e4e，turn_context固定gpt-6.1-terra、medium、当前隔离项目；task_started/task_complete各1。该回合用exec批量执行team list、team workflow show和pwd三个只读调用并结束。没有把这个准备动作视作用户输入或计划删除本身触发模型，也没有为夹具额外恢复的问题修改生产队列语义。

最终SQLite：workflow_runs1、workflow_run_logs1、workflow_schedules1、dispatches/dispatch_messages0、messages1（既有工作流系统通知）；原生投影completedTurns1。完整冷读不再调用chats.resume，避免重新唤醒暂停队列。notification-model-receipt.json与final-cold-receipt.json单列真实模型和删除状态证据。

## 验证与范围

- Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`：最终通过。
- LLVM PATH下Windows `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`：最终通过。
- `git diff --check`：通过。
- 纯UI焦点修复，没有新增、修改或删除自动化测试，没有运行无关全量suite；原有真实后台删除协议未改。
- 证据在.artifacts/workflow-delete-b13ea26：fixed-cancel-focus.png、deleted.png/AX、retained-history.ax.txt、final-delete-focus.png、final-list.ax.txt、cancel-receipt.json、deleted-cold-receipt.json、final-cold-receipt.json及日志。

已验证的是上述Mac中文20px浅色窄窗、取消/目标删除/冷读和键盘恢复。未覆盖删除写入失败/响应丢失的精确时序、所有语言主题、VoiceOver、Windows原生；#13及完整产品目标继续开放。当前安装版与ZIP/DMG未更新。

## 本轮存储整理

检查时.artifacts约8.6GiB、Rust target约4.8GiB，磁盘剩余约93GiB。对release-7ae27ca和release-edbbae1的两份旧ZIP重新核对原收据SHA256；各28,863个解压条目的路径集合、mode、文件SHA及内部软链接逐项与manifest一致，lsof无打开句柄，才移除两个extracted副本（文件分配块合计1,076,772,864字节，约1GiB；不把该数字当APFS即时物理回收保证）。保留各ZIP、收据、项目/状态/日志/脚本及失败记录；current46373f6和最近可运行回退db6fb20解压包保留。旧收据标记bundlePresent=false及恢复ZIP，不再把被移除路径称为现存可运行包。证据.artifacts/cleanup-2026-11-10-workflow/receipt.json。没有清除仍用于构建的Rust缓存或旧聊天目录。
