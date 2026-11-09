# 应用退出时中断工作流

2026-11-09，基线8ee5007；源码runtime补丁，不是候选ZIP验收。

## 真实问题与根因

上一轮真实Mac窗口在双成员工作流运行中Cmd+Q，进程确已退出、任务已取消，但账本变成failed/runtime closing。执行器只在冷启动把running行标为interrupted，关机时awaiter先拒绝让runner提前写failed，无法再恢复为中断。

Hive现有pnpm补丁新增runner.close：拒绝新的执行入口/完成加载后的启动；取消等待、终止活跃脚本Worker，并等待已经登记的执行/清理结束后才关闭存储。关闭打断的执行写interrupted，保留主动Stop和既有真实失败/成功；不通过错误字符串匹配改状态。只改现有Hive补丁，不另建桌面任务账本。锁文件仅更新补丁hash。

## 回归与原生实证

证据目录`.artifacts/workflow-shutdown-8ee5007/`。

- 新检查`packages/runtime/checks/workflow-shutdown.ts`运行真实desktop runtime、HTTP、SQLite、脚本Worker和原生子进程，厂商侧用明确的本地JSONL peer，无网络或模型推理。
- 最初夹具误读team启动返回值，before.log保留；纠正到真实run_id及端点后，before-corrected.log明确在failed != interrupted失败，未先改产品让错误夹具通过。
- 修复后双Worker任务、嵌套父/子工作流和已进入while(true)的纯计算脚本关闭后均interrupted；完成/失败/主动停止仍分别completed/failed/stopped，重复close返回同一Promise。
- 检查等待真实peer输入文件后才关闭，所有owned PID退出；独立冷runtime确认双任务cancelled、临时成员回收、成员会话pending0，历史保留。final-check.log及临时证据收据保存。
- 原生Mac：PID41583窗口实际发送工作流请求，团队页两成员各1项未结责任；未按Stop，直接Cmd+Q退出0。独立SQLite立即核对run interrupted、两任务cancelled、仅非临时reviewer保留。PID71248冷启动实际打开工作流详情，中文“已中断/中断原因”及两项“已取消”可见，无红色运行错误；截图native-cold.png和AX文本保留。再次Cmd+Q退出0；全部owned native/peer PID已消失。

## 实际检查

以下均通过：

- `pnpm --filter @hive-desktop/runtime check:workflow-shutdown`
- `pnpm --filter @hive-desktop/runtime check:team`
- `pnpm --filter @hive-desktop/runtime check:workspace-removal`
- `pnpm check:hive`
- `pnpm check`、`pnpm build:runtime`、`git diff --check`

为包内dispatch gate加入同一shutdown检查，防止后续包遗漏Hive补丁；本轮未对旧ZIP声称通过该新增检查。此轮未修改Rust，不重复Rust编译，未运行无差别全套或新模型。日志仍含临时成员回收后取消提示转发409，未吞掉/改成成功；账本、原生进程和队列分别核对。

Windows真实执行、所有后端组合、异常强杀/断电、签名正式包及完整体验仍待。旧failed运行记录不批量改写；中断的流程不自动重启、也不宣称已经汇报或完成。候选78721c7尚未包含该新补丁，后续打包必须重新冻结与核对依赖，不能直接沿用旧依赖验收结论。
