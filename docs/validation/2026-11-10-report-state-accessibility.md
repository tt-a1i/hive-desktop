# 汇报状态与待处理卡片的可访问语义

2026-11-10，[#28](https://github.com/tt-a1i/hive-desktop/issues/28)后续，完整目标#1。

## 实际问题与改动

已有已消费汇报卡片显示处理状态，但AX树未暴露该文字；待处理卡片连成员标题和正文也未暴露。仅给状态添加Label后，实机仍只得到两条无法区分归属的“待处理”。现在复用既有标题为待处理卡片建立Group，正文及两处状态建立Label，并设置稳定元素ID。显示值直接复用同一状态/正文变量，成员名和用户内容不翻译；不增加后台字段或状态，也不改变队列语义。

初稿在无ID Div上调用role编译失败；增加稳定ID后Mac构建、Windows目标检查通过。原始失败日志保留，不修改依赖或为测试增加生产分支。

## 原生受控验证

目录`.artifacts/report-state-accessibility`。仅创建拥有的独立项目/数据/设置，使用生产GPUI程序、source runtime、真实Hive HTTP/SQLite、Codex适配器及磁盘队列。CLI使用本地JSONL合成对端（已有steering探针的协议实现，去掉动态工具并由文件事件显式结束回合）；无真实模型或外部网络请求。

准备阶段按现有集成检查方式暂停队列，放入两份明确标注“合成”的report投递。它们不是team report业务验收：数据库workers/dispatches/report_outbox均0。真实Worker/显式汇报/Terra证据仍单列[真实协作](2026-11-10-report-processing.md)。

实际GUI操作与AX/截图结果：

1. 重开后两份报告均“已收到 · 待处理”，AX分别归属于fixture-alpha/fixture-beta，正文可读。初稿缺少归属的AX另存`before-group.ax.txt`。
2. 点击原生“继续处理”，第一份显示“正在处理”，第二份仍待处理；已进入消息历史与仍在队列尾部的两个渲染路径均可读。
3. 文件事件只结束第一个合成native turn：alpha“处理回合已完成”，beta“正在处理”，来源与正文不交换。`mixed.ax.txt`有明确归属。
4. 原生设置中切换本次独立数据为English，20px字体保持，状态变为Processing / Processing turn completed；成员名及中文正文不变。
5. 1186×1014物理像素窄窗（约593×507逻辑尺寸），收起导航并手动滚动显露beta标题，Processing及View original均可见；结束第二个对端回合后显示Processing turn completed、停止消失、发送恢复。长的来源类别视觉省略，AX保留完整标题。见`en-narrow-{processing,complete}.{ax.txt,png}`。
6. Cmd+Q后应用不再运行，所属启动会话exit0；两次原生窗口均正常退出。独立后台仅chats.open冷读，busy=false/pending0，两报告各一次历史、原投递/完成ID保持，native存储字节及2个已完成合成回合不变。`receipt.json`记录零真实模型请求与业务表边界。

稳定等待由协议对端的显式文件事件控制，没有在生产代码加等待或延长超时。此验证覆盖状态值到原生组件和AX的映射；活动steer报告的队列processing与停止/阻挡/冷恢复仍由24模式生产适配器/磁盘回归覆盖，不将本次预排队头处理冒称实际同回合追加。

## 检查与边界

`cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、`cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`、`git diff --check`通过。纯UI改动不新增/修改测试，不跑无关后台全量；约1.5MiB证据，无新开发包或大型依赖副本。

VoiceOver实际播报、Windows原生及完整字体/平台/在途矩阵仍待；AX语义不等于已验证播报。current/candidate/Applications仍为1e68306，不含本批及3a41948/3770b3b后续源码。完整目标继续。
