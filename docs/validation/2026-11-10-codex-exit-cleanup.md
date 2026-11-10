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

## 后续：固定错误入口

不新增错误面板，复用 Agent 状态行。有会话错误时，将可能误导的“已停止当前回复”等状态替换为“错误详情”按钮，点击重置既有 composer 与 failure 两个ScrollHandle到错误顶部。错误关闭后自动恢复原状态；无错误时不增加控件。不增加翻译键、依赖或后台行为。

独立 `.artifacts/composer-error-entry` 使用生产后台/真实上传API预置12行草稿、两附件及两条暂停投递（并非UI Send的新证明）。在实际窗口点击继续后，只终止核对父子关系的自有合成对端；中文20px窄窗1188×1014下，错误内容在视野外时固定按钮仍可见；鼠标点击显示错误标题与详情/关闭操作，实际关闭后按钮和错误区域消失，恢复状态。原生退出exit0；cold脚本核对两条投递、草稿、附件引用/原始副本及单一原生轮次保持，没有继续或自动重发。error-visible/dismissed图与AX、cold-receipt保留。

最终 Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows目标cargo check及git diff --check通过。纯UI后续无新测试；英文原生这一入口及VoiceOver、Windows原生仍未验收。源码与1e68306包继续分别记录。

## 包内回归闸门

将同一退出清理检查接入check-packaged-codex：只擦除TS类型并将生产runtime导入绑定到解压包，复制已有本地queue peer。修复前1e68306包负对照实际在新检查处以原AggregateError失败，前四个既有包内检查仍通过；package-negative.log保留。因此旧包尚未包含这项修复。新包正对照仍待本批打包后核对，不能用源码green替代。
