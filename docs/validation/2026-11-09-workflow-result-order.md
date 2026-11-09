# 工作流详情优先展示结果

2026-11-09，源码开发窗口。

## 场景与调整

真实双Worker完成后，旧工作流详情先显示来源路径、完整源校验及参数，再显示结果。1184×1000截图像素窄窗里，结果卡从约720像素处才开始，首屏只有正文开头；非空参数还会继续把结果向下推。

现保留标题、状态、时间及阶段，然后显示结果/原因、关联派单和日志；完整来源、校验与参数移到页尾“来源与参数”。提供同区完整复制和AX文本语义。所有原字段仍在，不新增展开状态、业务store或后台接口。日志/成员/结果操作沿用原实现。

## 实际前后对照

- 在独立state副本中读取此前真实Codex/Terra双Worker的同一run `5ce0f9d1-458f-46e1-aca5-a5c098aebd41`，先运行旧二进制保存基线，再运行新二进制。
- 同样1184×1000截图像素、导航收起、详情滚动到顶部：结果卡约上移150像素，第一份报告能显示更多正文；宽窗结果同样提前。截图目录 `.artifacts/workflow-result-order-51fd122/`，文件为before-wide.png、after-wide.png、before-narrow.png和after-narrow.png。
- 页尾可正常滚动到达完整来源与64字符校验值，点击复制后实际粘贴到空聊天框核对原文；核对草稿清空，未发送。
- 英文窄窗补验证非空参数：只在独立副本注入明确标识的显示夹具，含中文换行、limit=0、enabled=false、数组；没有将这些参数冒充原真实模型运行参数。完整显示/复制/AX核对通过，截图metadata-en-narrow.png，复制证据copied-parameters-en.txt。退出后已恢复原args。
- baseline PID7734、中文PID41278、英文PID80292均Cmd+Q正常exit0。三次RPC trace没有chat.prompt/chat.resume，三个原生display投影与原历史逐字JSON比较保持一致；本轮无模型推理。

macOS `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop` 和diff检查通过。纯UI没有新增/修改测试，也未重复后台全量suite。

没有将此调整扩大为所有长内容/字号/VoiceOver/平台矩阵通过，Windows原生仍待环境。候选ZIP仍78721c7，尚未纳入本轮布局；完整目标未完成。收据、构建及原生日志在上述.artifacts目录。
