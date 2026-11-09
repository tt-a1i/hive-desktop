# 主控结束后的真实晚到汇报

2026-11-09，源码6ce1970，无产品代码修改；原始Codex及openai/gpt-6.1-terra，隔离状态/项目/本地Pi配置，无全局认证/模型更改。不是候选包或Windows验收。

## 补验的边界

此前实际Terra证据验证系统通知加入仍活跃的主控回合；本轮刻意让主控先结束，再允许Worker生成并汇报结果。主控结束后新开一次结果处理是有用的后续更新，不把它一概叫重复答复或静默丢弃。

驱动只向主控发送一次请求，观察初始final后创建owned文件gate；没有替主控启动workflow、替Worker计算/写结果/提交report，也没有伪造或注入系统通知。Gate只在此验证目录，以真实fs.watch文件事件放行，不是产品的固定等待。

## 已取得的真实证据

- 主控chat `7318dcec-6c83-4df1-b5bd-5d74f5ba2918`，native thread `01a1c087-47fc-7c42-ace8-163fd729432f`。亲自读workflow源码并调用team workflow run --stdin，run `575ecb8f-41e8-4079-bb76-a7386a610edd`。
- 放行前，驱动观察主控已非busy、pending0、唯一final为WORK_STARTED、workflow仍running，只有用户投递的一个完成ID；before-release.json保存该实际快照。随后才写release-worker.json，排除了把这次汇报当作原回合内通知的解释。
- Worker native thread `01a1c087-a324-7ff3-8f5a-7619d4ca3f83`，读取输入[5,13,19]、写result.json为numeric sum37/count3。真实messages/seen0及原生team report工具completed、success=true，唯一dispatch `90e55e7d-85e6-454c-9828-a3e151ce3241`为reported，run completed。
- 主控收到真实workflow-finished系统通知，独立read输入/结果和team workflow show后结束为LATE_REPORT_OK 37/3。完整history恰好两条final：WORK_STARTED、LATE_REPORT_OK 37/3。投递来源为user/system、两个不同完成ID；既没有重派也没有重复结果final。
- 完整runtime重开后，两段thread身份保持、主控完成ID保持，主控及Worker pending均0。Worker original任务完成ID为空但责任已显式reported并回收，未把这解释成原生用户回合自然完成。
- Worker按项目约定读取README/docs/implementation-plan.md时发现后者不存在，真实失败工具保留；随后计算与显式report成功，未声称所有工具调用均成功。

## 原生窗口

源码开发应用PID23242直接打开上述冷历史：同页两条主控final、团队通知、Worker gate/messages/已完成report、已完成工作流及result.json只读预览37/3均实际查看。工具组展开后首次滚动回到较早内容，再通过“回到最新消息”定位实际report，不把离屏AX节点当作可见点击。

Cmd+Q退出0，PID已消失；再独立runtime打开两会话，仍pending0和原主控完成ID/两个final，after-gui-receipt.json保存。GUI阶段未发用户请求或继续处理。产物内容以实际可见预览核对；文件预览AX仍仅有内容容器，正文语义的完整验收不包含在本轮，需继续改进。

证据目录`.artifacts/late-report-6ce1970/`：run.mjs/prompt/owned源码与输入结果、before-release.json、events、receipt、after-gui及native进程/退出、main/worker/workflow/artifact截图。无需重编译或全套测试，无本轮产品代码改动。

## 未覆盖

只覆盖一位Worker、一个工作流完成通知、明确放行且正常结束的晚到路径。多通知竞争、同时用户输入、断线/强杀、其他厂商后端、Windows原生、包内路径及一般exactly-once不作保证。未替换候选1534986或current，完整目标继续。
