# 原始 Codex/Terra 一主控与两位 Worker 实证

2026-11-09，生产源码基线 `7714122`（生产代码与06ee28b一致）。本轮不修改生产代码，不冒称包内或Windows验收。

## 执行链路

隔离项目 `.artifacts/two-worker-live/project` 的输入为 `{"a":[7,11,31],"b":[2,13,17,29]}`。预存工作流源码用 Promise.all 启动两项不冲突的工作：Branch-a只写result-a.json，Branch-b只写result-b.json。三个Agent均为原始Codex及 `openai/gpt-6.1-terra`，不改用户全局配置/认证。

验证驱动仅向生产chat接口发主控请求、观察真实workflow terminal并打开自有fs.watch gate；主控读取原文并通过原生team工具启动工作流，驱动没有替主控启动，也没有替Worker创建文件或提交report。该gate仅用于让主控等待真实结果，不是产品固定等待。

- 主控会话 `047a28bb-ad3f-4a26-807f-8e14cc492557`，原生thread `01a1c03c-26ab-7b60-a34d-9ce30a722834`。
- 单一workflow run `5ce0f9d1-458f-46e1-aca5-a5c098aebd41`，两条派单实际状态reported。
- Branch-a thread `01a1c03c-9047-7bc1-af1f-389baeb5c756`，实际读取输入、写49/3、messages/seen并显式report带result-a.json；原生team report工具completed且success=true。
- Branch-b thread `01a1c03c-9047-7ba3-90d5-50f754fde501`，对应61/4及result-b.json，同样显式report成功。
- 两派单创建相差107ms；原生工具活动区间A为1794226562113–1794226578677，B为1794226562160–1794226577230，确有重叠。该结果不用于推算相对串行的加速比。
- 主控再独立读取原输入、两产物及实际run，唯一final为 `TWO_WORKERS_OK a=49/3 b=61/4`。用户/系统通知为两个不同原生item，同回合结算；输入原字节保持。
- 完全关闭runtime后新建runtime，三个thread身份、完成ID及来源保持，三会话pending均0。

初始主控/Branch-a按项目习惯读取README和实施文档时遇到文件缺失，原生工具如实保留失败；隔离输入目录没有这些文档，后续实际读取/产物/汇报及核验成功。没有把中途每个工具调用都描述为成功。

## Mac原生恢复

直接启动源码开发应用，数据指向上述隔离state：主控最终回复、两位Worker各自派单/工具/显式report记录、工作流详情的两条关联派单和两个report返回值均核对。滚动后打开result-b.json真实只读预览，内容61/4。Sidebar保留两个成员的历史会话；临时workflow成员结束不等同删除会话历史。

首次尝试点击离屏产物操作没有打开预览，正常滚动使按钮进入视口后成功；不把AX树列出按钮当作已经点击成功。PID80143 Cmd+Q正常exit0，GUI阶段未再次发送模型请求。

证据 `.artifacts/two-worker-live/`：run.mjs、prompt.txt、workflow源码/两产物、events、receipt（含native intervals/冷恢复/工具记录）、parallel-observation.json、main/worker-a/worker-b/workflow/artifact-b截图、原生进程与退出日志。

这是实际主控+两位Worker的独立分支闭环，未覆盖混合厂商组合、复杂代码冲突、所有取消/失败组合、Windows或正式安装。没有改生产代码或自动化测试；本轮运行上述真实模型穿透及原生恢复，未重复全量suite。候选ZIP仍为06ee28b，完整目标继续开放。
