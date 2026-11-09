# 派单详情可读性与会话草稿往返

2026-11-09，基线9fc29a9，源码开发窗口；候选ZIP仍1534986，未纳入本轮UI改动。

## 问题及修复

实际原生窗口从成员会话打开已取消派单，界面可见任务与取消原因，但AX树只有按钮，读不到责任内容。现为派单状态/参与者、完整任务、汇报或取消原因、关联成员责任、协作消息参与者与种类/投递状态、正文、回复引用提供稳定ID及Label/Group语义；投递错误可见分支同步提供文本标签。不改协作协议、状态、存储或派单投递。

## 实际证据

`.artifacts/conversation-navigation-9fc29a9/`保留修改前后截图、AX文本、隔离状态和夹具源码。

- before窗口PID79361：主控草稿两行中文→成员独立草稿→返回主控，两个草稿各自恢复；成员→关联派单→返回来源会话仍保留成员草稿。首次粘贴被本机输入法/跨设备剪贴板替换，未发送或读取该路径文件；后续通过原生settable文本入口写入预定成员草稿。该干扰不算产品已复现bug，也不作为焦点准确性或所有输入法验收。
- dispatch-before.ax.txt缺失截图中任务/取消正文。修复后相同旧取消派单的dispatch-after.ax.txt含Task、Cancellation reason及No messages，按钮仍保留。
- 另通过真实Hive HTTP/SQLite/team CLI创建两位非临时成员、关联责任、question/answer及reply_to、带产物report。原生Codex协议控制使用本地peer，队列在派单前暂停，无网络/模型推理、无原生prompt。不是模型自主决策证明，也没有直接插入伪造report/消息行。
- 夹具最后误将空report成功应答解析JSON，seed-messages.log保留；没有重派或重报，独立SQLite和真实HTTP确认一项reported、一项submitted、两条消息与正确reply_to，再通过runtime保存导航。finalize-seed日志/位置收据保留，不修改产品迁就夹具。
- fixed窗口PID80512：实际从主控汇报卡打开详情，中英文AX均可读取多行任务、汇报、两个成员责任、问题、回答和回复引用。状态/成员及文件预览按钮仍可读。messages-zh.ax.txt及messages-en.ax.txt保存。
- 同一窗口跨项目打开旧成员，重开后的成员草稿仍为预定中文句子；经派单往返再打开主控，多行主控草稿仍完整，main-draft-cold.ax.txt保存。没有点击发送/继续处理，无新用户回合。两窗口Cmd+Q均exit0，所有owned native PID已退出。

## 检查及边界

Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`和`git diff --check`通过，日志在证据目录。纯显示语义改动不新增UI测试，不重复后端全套。

验证的是AX文本和列出的真实导航/草稿内容，不等于完整VoiceOver语音顺序、焦点/光标选区、窄窗/字号矩阵、Windows原生、所有加载/失败状态验收。投递错误分支仅源码与编译核对，本轮未制造新失败；其他网络错误/加载提示的完整语义仍需后续核对。候选包不更新，完整产品目标保持未完成。
