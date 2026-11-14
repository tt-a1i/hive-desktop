# T3聊天标题与单行输入工具栏

工作项[#37](https://github.com/tt-a1i/hive-desktop/issues/37)，属于完整产品目标[#1](https://github.com/tt-a1i/hive-desktop/issues/1)。参考继续固定为实际T3 Code 0.1.9 / `24e77473ecf41bc1f7142529c7911c1aae1d76`的ChatHeader、ComposerControlsBar和ChatComposer；不引入其他产品的界面方向。

## 行为

聊天标题从单击直接重命名改为单击会话菜单、双击重命名。菜单与侧栏复用原有置顶/归档及忙碌保护，重命名另检查捕获的当前会话ID；项目面包屑使用既有新建流程。当前标题按钮的无障碍名称与侧栏区分，Windows fixture先打开当前标题菜单再选择重命名，原保存/取消及草稿断言保留。

模型与真实推理强度采用同一个触发器，继续使用原选择面板、会话身份/连接代数检查和保存路径。没有真实可选强度时不添加假effort。输入工具栏保持单行，按实际布局宽度将Pi原生命令放入溢出菜单；附件和发送/停止/继续保留可见。现有用量入口仍为唯一入口。后台协议、账号和全局模型配置未改。

## 原生调试与真实Codex

源码提交`83d6ac1`在独立工作树编译，Mac和MSVC检查通过。使用独立Hive Product QA及再次复制的`.artifacts/t3-chat-ui/{state,agent}`，不是当前安装实例，也不是最终优化包。以下结果来自实际窗口，不是preview注入。

- 标题单击打开菜单、Esc关闭；双击可直接输入重命名并聚焦，取消保持原标题及两行聊天草稿。实际临时重命名后经菜单恢复原标题，侧栏同步。主线程核对原所有会话ID和标题保留。
- 项目面包屑新增恰好一个未发送会话，独立index确认位于原项目。新建继承原Codex默认Sol，但**没有向Sol发消息**；随后从合并入口实际改为Terra/中，独立持久化记录为`openai/gpt-6.1-terra`及`reasoning.level=medium`，只改该测试会话。
- 宽窗及约822×503、中文20px下，附件、模型/推理和发送同一行；同一弹层包含真实Codex模型与强度选项。主线程只读AX/截图核对了标题和输入工具栏。
- 仅向新测试会话发送一次受限Terra任务：运行本地60秒hold脚本，不做项目开发或账号/全局配置操作。主线程独立验证实际Python进程58078属于owned QA53942，且命令是指定脚本。
- 工具进行中输入第二条消息排队，然后实际点击停止：工具标为中断，队列暂停、继续处理出现，模型入口恢复可用。主线程在Stop之后立即确认该Python已消失。
- 只点击一次继续处理，真实Terra仅回复`HIVE_T3_FOOTER_QUEUE_OK_20261114`，队列及暂停消失，未新增工具。返回原会话，两行草稿逐字保持；独立比较原草稿文件SHA完全一致。
- 正常退出0，owned Node53950消失。两个全新的包内Node/runtime进程分别冷读：原真实三会话36条完整history/ID保持；新会话7条history/ID在两个冷进程间完全一致，只有一个中断commandExecution记录、只有一次预期回复、pending为0、Terra/medium保持。冷读没有新模型请求。

新会话ID为`db46e496-17f9-4e82-bd36-c6024de88e99`，原生Codex线程为`01a1d96a-51cf-79f3-bde4-32e75ecdeb88`。实际命令、停止进程、模型选择、原草稿及冷读证据保存在`.artifacts/t3-chat-ui/{lifecycle-*,model-selected-terra,breadcrumb-new-and-rename-restore,cold-pass-*,cold-receipt}.json`；进程日志/退出收据为`.artifacts/product-completeness-20261114/t3-chat-native*`。

初次冷读断言误按assistant.toolCall筛选Codex工具；既有Codex历史协议实际使用toolResult。修正核验后要求工具恰一条、名称/命令/ID正确、状态为interrupted且原始hiveInterrupted标记存在，并比较两次完整冷读；原失败日志保留，不将其误报为工具丢失。

## 复核与边界

四个独立角度：架构A、边界B、验证A−、范围B+。审查发现Windows fixture仍直接调用移入菜单的重命名，已修复并独立复核。构建不作为真实交互的替代。

整合后生产源码与已实际操作的83d6ac1逐项无差异；1375个唯一英文键，直接中文tr字面量无遗漏。原生Windows fixture的引用目标诊断修正另行整合，不改变运行时或放宽原草稿断言。

整合后Rust主程序44通过、0失败、1忽略，MSVC交叉检查及diff检查退出0；实际日志为`.artifacts/t3-chat-ui/{rust-main,msvc-main}.log`。这些检查不替代上文真实窗口和Terra闭环。

置顶/取消置顶与Pi命令溢出菜单本批尚未实际操作。新建Pi并非纯UI：会初始化原生SDK及用户资源，可能安装缺失包或执行扩展；只展开已经就绪会话的overflow本身不发RPC。不为补这项界面验证重试Pi登录、伪造会话或禁用用户资源。Claude/Antigravity不验。

此批源码已整合，优化包与Windows原生继续。当前开发包仍为0e478c2，只含上一批设置浮层；背景模糊、侧栏搜索/宽度等其他T3差距与完整产品目标保持开放。
