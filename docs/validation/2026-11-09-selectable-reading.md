# 可选中文本的原生正文语义

2026-11-09，基线2f444ed；本轮是源码UI改动，候选1534986未更新，验证尚有明确未完成项。

## 根因与改动

上一轮实际打开result.json，屏幕显示37/3，但AX只有文件内容Region，没有正文。依赖gpui-base的SelectableText只提供窗口选区/绘制，没有应用层正文标签。检索所有调用点后，在已有text_reading模块增加selectable_text：用Label包裹同一个原生选择run，正文以共享字符串同时用于显示与AX，不替换选择机制、不新增依赖或读取路径。

文件预览、成员职责、展开思考、原始补丁四个未提供正文标签的入口使用该包装；工具参数/输出及工作流来源已经有Label包装，保持原样，避免重复语义。展开和redacted条件仍在原调用处，不把折叠或隐藏思考暴露出来。文件预览同时补可见的只读说明、路径、错误、保留旧内容提示和空文件的文本语义。

## 实际验证到的范围

证据目录`.artifacts/selectable-reading-2f444ed/`。从上一轮真实晚到汇报的状态/项目建立隔离副本，仅将副本工作区路径指向副本项目，不改原始模型证据。当前文件内容夹具只写副本result.json，检查后已恢复其原37/3字节；不把该夹具当作Agent生成的新产物。

- 源码原生PID56760实际打开工作流result.json；AX现在包含只读说明、完整副本路径和`{"sum":37,"count":3}`正文。short.ax.txt/short.png保存，对照上一轮只有Region的缺口。
- 在该副本替换为82行中英文长文本后点击重新读取，AX正文随可见内容更新（工具展示会截短长标签，不据此宣称完整VoiceOver长文验收）。点击原Region后End可见第80行与PREVIEW-END，Home返回PREVIEW-BEGIN，PageDown到第26行附近；long-end.png及本聊天操作记录保留。
- 选区拖动/复制步骤收到窗口状态已改变提示，随后ScreenCaptureKit报-3811。重新AX读取显示窗口已离开预览，不能把该步骤记为复制成功。停止本轮原生操作，未强行关闭用户正在操作的窗口；PID56760当时经进程表确认仍在运行，不补写exit0。
- native.log未发现chat.prompt/resume等新请求，本轮未发起模型推理。

## 检查及待补验

Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`及`git diff --check`通过。UI显示改动不新增测试，不重复后端全套。

必须继续：选区/全文复制回归；成员职责、展开/收起思考与原始补丁的原生阅读；空文件、读取失败保留旧内容的实际状态；英文/窄窗、完整VoiceOver/Windows。不能以共享包装和编译通过推断这些均已验收，也不把本项或完整目标标记完成。运行中的副本目录不可直接清理。
