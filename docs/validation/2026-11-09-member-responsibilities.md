# 成员职责的键盘阅读与文本复制

2026-11-09，修复最新源码团队成员页的真实可访问性缺口。此项是源目录 runtime 的实际 Mac 原生验证，没有重打候选 ZIP，也不代表 Windows 原生验收。

## 发现与修复

在上一轮真实 Codex 团队记录的独立数据副本中查看成员。常规及窄窗口可找到搜索、状态筛选、会话、启动和职责入口；展开职责后，正文只是 overflow_y_scroll 的普通 div，AX 树没有阅读区域，点击正文再按 End 无法到达末尾。

成员展开状态原先只存 ID 集合，改为同一个按成员 ID 索引的 ScrollHandle 映射：打开创建、收起移除，现有工作区切换清理继续生效。正文复用已有 text_reading::region 和 SelectableText，提供 AX 名称、焦点边框、方向/翻页/Home/End、滚动条及局部选区复制；保留完整复制入口与原始职责文本。没有新依赖、新 store、协作协议或权限变化。

## 原生检查

在独立数据副本中将成员职责换为102行、3,531字符的中英夹具，首尾分别为 RESPONSIBILITY-BEGIN / RESPONSIBILITY-END。只改该副本的职责字段，没有构造聊天消息、派单或模型回答；上一轮真实证据保持不变。

- 正文新增带成员名和键盘说明的 AX Region。
- 点击区域，End 显示第93–100行及 END；Home 后 PageDown 显示第8–17行，PageUp 返回 BEGIN。
- 鼠标选取第1行、Cmd+C，再在隔离成员会话草稿中粘贴，实际得到 `职责 001 · 中文成员职责阅读验证 · task line 1`。
- 1184×1504截图像素的窄窗中，隐藏导航后职责区域、完整复制、成员操作可见；End 可到第100行及 END。没有把此高度下的结果推广到所有窗口高度。
- 从末尾点击完整复制再粘贴到未发送草稿，落盘草稿与夹具逐字相等，3,531字符、102行。AX 会截短长输入值，最初仅用 AX 查末尾得不到结果；使用实际草稿断言确认完整性，没有把 AX 截短误判为剪贴板丢字。
- 修复前与修复后应用均正常 Cmd+Q 退出0；原生日志没有 chat.prompt，真实会话展示投影与原副本逐字保持，无新增模型回合。

## 检查与范围

通过 `cargo build --locked --manifest-path apps/desktop/Cargo.toml`、`PATH="/opt/homebrew/opt/llvm/bin:$PATH" cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` 及 `git diff --check`。纯UI改动，没有新增/修改测试，不重复无关全量。只读检查确认原始真实团队的职责和历史未改。

证据 `.artifacts/member-layout/`：原职责、长夹具、完整复制收据、前后原生/RPC日志及退出记录、隔离数据、最终 receipt。构建日志 `/tmp/hive-member-reading-{build,windows}.log`。实际截图与点击留在本聊天工具记录。

英文 AX、VoiceOver实际播报、滚轮、多个成员独立位置及 Windows 原生矩阵仍未验证；不能据此宣称整体 Cindy/Maka 对标完成。当前 ZIP 仍是此前17d8ce7候选，本修复待后续统一打包。

## 后续候选包

上述源码修复已纳入dd0230a候选并取得包内验证，见[候选实证](2026-11-09-stop-candidate.md)。本页原先未打包的边界描述属于当时记录，原始结果保持。
