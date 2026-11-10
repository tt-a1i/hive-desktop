# 成员筛选后的真实删除与可读取确认

2026-11-10。补此前成员键盘验收只打开/取消删除的缺口。所有操作仅涉及本轮 `.artifacts/member-delete-native/` 新建的独立项目与三个无模型、未启动测试成员，没有修改用户原团队或历史现场。

## 原生操作

正式源码开发程序（基线261111e）使用空Pi目录、PI_OFFLINE=1。真实Hive创建Disposable target/Codex、Preserve reviewer/Claude、Preserve tester/外部Pi，三者reviewer、stopped、无启动模型。保留创建前后身份收据prepared.json。

- English、20px宽窗进入成员页，搜索Disposable；通过该成员菜单打开删除确认，屏幕写明具体姓名。第一次Cancel，目标仍在，搜索文本保留。
- 调整约593×507逻辑窄窗，收起覆盖式导航，重开该目标删除确认，实际滚动显露正文及Confirm/Cancel；姓名、确认按钮和换行的Cancel没有裁切。截图confirm-before.png。
- 点击Confirm，目标卡消失，状态数3→2，筛选结果0/2，空结果解释可见。搜索保持Disposable，后续直接⌘A输入Preserve进入原搜索框，显示另外两人及各自后端；不是误删列表索引或所有筛选外成员。见deleted-empty.png、preserved.ax.txt。
- ⌘Q正常exit0（句柄60603），未启动Agent、创建聊天或发送模型请求。

独立新runtime真实HTTP/SQLite冷读：目标worker及launch配置不存在；保留两人原ID、名称、两行职责、reviewer/stopped/pending0及hive:claude/hive:pi-rpc启动配置逐项相同。workers2、messages/agent_runs/dispatches/workflow_runs均0。见check-cold.mjs、cold.log、cold-receipt.json。

## 发现与修复

删除确认的姓名正文、筛选结果数、空结果/下一步和状态含义在屏幕可见，但旧辅助技术树没有这些文字；状态按钮的自定义标签也覆盖了可见计数。沿用现有Label/aria_label模式补齐这些实际文字，并将状态计数纳入按钮名称。没有增加新UI面板、焦点状态或后端逻辑，也未改删除/事务/协议语义。

最终Mac构建原生复验：

- 冷启动成员状态按钮读取 `All, 2` / `Working, 0` / `Idle, 0` / `Stopped, 2`，正文读到状态含义。
- 搜索已删除的Disposable，实际AX出现 `Showing 0 of 2 members`、`No matching members` 和后续操作说明。修复前同条件AX缺失，见deleted-empty.ax.txt与fixed-empty.ax.txt。
- 清除筛选，打开保留成员的删除确认，实际AX明确 `Delete member Preserve reviewer?`；点击Cancel后两位保留成员不变。没有再次执行删除。见fixed-confirm.ax.txt。
- 第二窗口⌘Q正常exit0（句柄54364）；再开独立runtime运行同一冷读检查仍通过，见cold-final.log。

## 验证与边界

`cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、对应Windows MSVC目标 `cargo check --locked … --target x86_64-pc-windows-msvc --bin hive-desktop`、`git diff --check`通过，无新增编译警告。只改UI语义，不加脆弱UI测试、不运行无关Node全量suite，既有测试不变。后台删除结果用真实HTTP/SQLite独立检查，非mock断言。

本轮只覆盖无历史、未启动、无派单测试成员的删除；运行中/未结任务删除的产品约束、带历史会话删除后的阅读、完整键盘与VoiceOver朗读及Windows原生仍待对应验收。当前ZIP/DMG/Applications保持db6fb20，261111e图片及本轮成员语义修复均尚未入包。没有新生产依赖或大型重复解压。
