# 保留 Codex 意外退出原因

## 已复现问题

上一轮原生故障走查仅终止自有合成JSONL对端后，错误详情被 `Backend exit cleanup failed` 覆盖。生产Codex adapter在断连后设置 run.error 并等待工具释放；`abort()`等待结束后重新抛出相同的Error对象。Chats.failLive将它再次算作清理失败，覆盖原退出事件，并使等待cleanup的调用者再次拒绝。

只从清理结果中过滤与已上报原始错误完全同一对象的拒绝，不做消息字符串匹配，不改变abort、dispose、队列暂停或原始错误事件。不同对象的异常仍保留原AggregateError路径。无新依赖、权限、模型或schema变动。

## 定向回归

新增 `packages/runtime/checks/codex-exit-cleanup.ts`，加入现有check:codex。真HTTP/SQLite/持久队列、生产Codex传输、自有本地JSONL子进程，零真实模型。

修复前断言 `await live.cleanup` 实际以相同原生EOF错误包装成AggregateError拒绝，red日志保留。修复后：自有对端SIGTERM，cleanup resolve且live移除；最后错误仍含原生EOF/SIGTERM；活动和排队两条输入以原ID及正文冷恢复，读取不自动执行；显式resume后两条原始ID完成，原生历史为原失败轮+两完成轮。这里的读取私有live只等待实际清理并检查是否被释放，不替换生产handle/PTY/HTTP。

检查通过：pnpm check、pnpm build:runtime、完整定向check:codex（含已有工具清理/interrupt/steering24模式）、check:queued-inputs-peers（Codex及官方Claude SDK本地对端）。未跑全仓所有suite，此改动仅排除同一已报告错误，定向检查覆盖故障、资源释放、队列与恢复。

## 原生组合复验

`.artifacts/composer-combined`：中文20px、真实系统文件选择器多选自有a.txt/b.txt、12行草稿、实际Send一个本地保持输入和Queue一条补充。核对app→后台→唯一对端父子关系后仅SIGTERM自有对端。实际错误展开后AX和画面均显示 `Unexpected Codex app-server stdout EOF`；无通用cleanup覆盖。退出exit0。

窄窗1188×1014物理像素，点击固定“附件 · 2”可跨过错误及排队内容直接显示两张附件卡，长草稿仍在，未点击继续或重发。冷读逐字/ID比较两条原始投递、12行草稿、两个附件引用及上传副本的原始字节；原生历史仍仅1轮且字节保持。cold-receipt.json、root-error及attachments-narrow图/AX、owned-processes及故障前投递/草稿保存。

仍有体验缺口：长草稿可把错误区挤出视野，键盘能显露但鼠标发现入口不够明显，继续修复。该合成进程验证不代表真实厂商崩溃或模型推理。Windows原生及完整目标未完成；尚未纳入1e68306包。
