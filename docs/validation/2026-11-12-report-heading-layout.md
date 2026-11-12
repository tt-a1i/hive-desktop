# 汇报身份与操作分层

2026-11-12，基于ff4aa47。查看旧真实一对二协作记录时，实际Mac窗口把成员/汇报标题、处理状态、派单/成员/原文操作挤在一行；English20字号下，proof-alpha · Worker report被截为proof-alpha · …。成员身份和来源不应让位于操作按钮。

本次只调整共享聊天消息头：名称与处理状态在上行，名称允许换行；派单、成员会话和查看原文移到独立的自动换行操作行。查看原文沿用紧凑ghost样式。复用原按钮/回调/Reveal与虚拟列表，未改变正文、来源、状态判定、投递、工具或后端协议，也没有新增依赖或持久状态。

## 实证

`.artifacts/chat-visual-20261112/prepare.py`核对旧源数据无占用后，复制此前645c9a5真实Terra团队状态到本轮独立目录。前后都只读打开历史，不发送prompt或恢复执行；前图来自d644db9优化包，后图为本轮debug源码，配置/数据和宽窗尺寸一致，不据此比较构建性能。

- 2360×1706截图中，修改前标题被操作挤压；修改后proof-alpha · Worker report完整可见，处理状态与下方操作层次清楚。
- 实际拖小至1644×1104截图（约822×552逻辑窗口），标题仍完整；派单/成员与View original操作换行，无横向裁切。
- 实际展开/收起原文，点击成员会话，读取alpha历史后返回主会话，原未发送草稿完整保留。
- 两个窗口均Cmd+Q exit0。`check.py`逐字段比较原数据与隔离副本：workspaces1、workers2、messages7、dispatches2、dispatch_messages0、agent_runs3完全一致；所有原草稿文件字节一致。没有新增消息、Agent run或模型请求。
- Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`和`git diff --check`通过。纯UI改动，无正式测试增改删，未跑无关全量suite。

原生before/after AX、原始JPEG截图、构建与退出记录及receipt均保留。截图API实际返回JPEG，初次按PNG读取元数据失败；修正扩展名且不转码，尺寸检查另排除72×72密度元数据后通过。没有为检查修改产品语义。对所有tr字面量与英文目录的只读扫描未见缺键，但不当作动态文案或完整语言验收。

本次只验证这套English20历史/宽窄窗口及上述操作，中文、更窄窗口、完整键盘/VoiceOver、所有汇报状态和Windows原生仍待。3ed07c9之后的当前d644db9安装包尚未包含此布局修改，完整精美产品目标继续。
