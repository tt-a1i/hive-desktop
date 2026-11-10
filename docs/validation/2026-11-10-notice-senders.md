# 团队通知保留实际成员名

2026-11-10，完整目标#1的真实协作阅读修复。

在一对二Terra真实任务的主会话里，两个Worker分别提交就绪状态，原界面两张卡片都只显示“团队通知”，正文相同，用户无法从标题区分发送者。后台来源元数据已明确保存`workerName: proof-alpha/proof-beta`，前端`receive_message`却仅对`report`读取名称，丢掉正常system/dispatch已有的同类字段。

一处移除Report专用过滤，所有消息仅使用经过现有版本/来源契约解析的显式workerName；不从正文或Hive-looking标签推断身份，不新增协议、字段、依赖、状态或模型调用。缺少名称仍用原来源标题，任意实际名称不做翻译；复用原有`heading_text`、省略与AX标签。

## 实际验证

- 使用`.artifacts/team-two-report-fixed`的原生真实三会话历史，不发送新提示。当前源码原生程序、1e68306包内Node/runtime；生产后台无改动。
- 原生主控消息分别显示`proof-alpha · 团队通知`和`proof-beta · 团队通知`；汇报卡片仍显示对应成员，原文展开确认`kind="status" from="@proof-alpha"`及完整内容保留。
- Mac浅色17px宽窗及1186×1014物理像素窄窗（约593×507逻辑尺寸），收起窄窗导航后，成员名、换行正文与原文按钮可见。截图/AX保留为`notice-senders.*`及`notice-sender-narrow.*`。
- Cmd+Q、所属启动会话exit0。读取SQLite、文件、native JSONL和持久来源：三个会话仍各1轮，dispatch/outbox/文件/投递与完成ID全部保持，spool空。首次直接数组比较因会话索引顺序随导航改变失败；按chatId排序后逐字段一致，不把导航顺序变化误报成数据改写。
- `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、`cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`、`git diff --check`均exit0。

纯前端读取已有元数据的一行修复，不新增或修改测试，不跑后台/全量套件。Windows原生、英文/其他字号/完整辅助技术仍待，不能用类型检查代替。当前current/candidate/Applications仍1e68306，不包含此后续原生源码修复。

## 参考界面走查

本轮实际查看已安装T3 Code新会话输入/项目/模型控制结构、Beluga欢迎与Agent选择步骤、Cindy主界面的身份标题与消息操作；Beluga已返回用户原欢迎页。未发送消息、切换模型/账号或复制源码/资产。Cindy非绑定更新说明已关闭；T3首次截图为空白，后续得到缩小视图，只用可核对的AX/结构信息，不声称精确视觉对齐。Beluga的22步流程不直接套用到Hive；继续沿实际协作问题改进，完整设计体验尚未达成。
