# Codex 显示日志恢复：终态合并与写盘一致性

2026-11-12。基线 51d3b49，源码修复；当前安装包仍为 3a18b94，尚未包含本批和上一轮快照索引。

## 根因与修改

上一轮已减少快照查找，但受控 5000 回合的 CodexSession 装载仍约 456ms。显示日志 recover 在每个导入回合重新构建/展开 completedTurns Set，缓存回合则逐次 includes 全列表，产生平方级工作。

同一代码还有可复现的写盘顺序问题：recover 在保存显示日志之前就改 this.completedTurns。实际 rename 失败后，entries 与原文件未提交，内存却带着完成标记。修复故障路径后再收到一个部分原生 item，会把未提交标记写出；之后 recover 错过“历史细节可能缺失”提示。这里是桌面显示日志完成标记，不是 Hive 派单完成或模型投递成功回执。

修改限于 codex-projection.ts：恢复时一次创建局部 Set，按原顺序合并并一次保存；共享 save 在 rename 成功后才提交 entries 与 completedTurns。complete 同样把下一份标记交给 save，去掉先改内存/失败再回滚。缓存读取未发生合并时保留原标记列表（含已有重复项）；实际合并仍按旧规则去重，避免改变成功记录的 JSON 形状。

没有持久化缓存、新 schema/store、协议或模型上下文变更。显示日志仍仅供 UI 恢复，不进入 thread/resume、turn/start 或模型。没有改本批以外的临时文件保留/清理策略。

## 性能样本

本地证据 `.artifacts/codex-recovery-completion-20261112/`。benchmark.mjs 直接运行生产 CodexProjection，真实构造/原子写入文件，前后各场景 4 样本，中位包含 JSON 序列化与文件写入；历史是合成数据，无模型请求。

| 回合数 | 场景 | 修改前中位 ms | 修改后中位 ms |
| --- | --- | ---: | ---: |
| 1000 | cached | 2.60 | 1.56 |
| 1000 | native-import | 15.83 | 1.25 |
| 1000 | legacy-import | 17.36 | 2.09 |
| 5000 | cached | 30.43 | 5.76 |
| 5000 | native-import | 386.07 | 4.06 |
| 5000 | legacy-import | 395.46 | 6.98 |

六场景每份完整落盘 JSON SHA256 前后相同，并验证二次恢复文件逐字节相同、冷建对象的条目相同。

session-check.mjs 另经真实 JSONL 进程、50 页原生形状的合成历史和元数据/显示日志装载 5000 回合/10000 条消息。单次完整会话创建从 455.93ms 到 78.12ms；它包含进程启动、分页和文件 I/O，仍不是 GPUI 窗口打开或整应用性能结论。初始完整快照与新增输入回合后的完整快照 SHA256 均与基线相同，动态关联/完成投递/关闭冷重开保持。样本含上一轮来源索引，不能把两项收益混算为本次单独的快照收益。

## 失败与语义验证

failure.mjs 使用实际 EISDIR：保留原 JSON 文件，故障时把提交目标置为目录，令真实 rename 拒绝；还原后逐字节核对原文件。分别覆盖 recover 与 complete 的失败，再接受一个部分 item 并恢复已结束回合，通过公共显示结果检查缺失提示。

- 旧 recover：后续写盘带未提交的 turn 完成标记，缺失提示 false。
- 新 recover：后续写盘完成列表仍空，缺失提示 true。
- complete 失败：新旧均保留原状态/正确提示；新实现依靠写盘后提交内存，而非先改再回滚。

32 组实际成功写盘差分直接加载 git 51d3b49 的旧类与新类，组合 native/legacy、completed/interrupted/failed/inProgress，以及空读/仅缓存/仅导入/混合；包含重复标记。第一次/第二次落盘、冷读与输入对象不变的断言通过，全部旧/新 JSON 相同。失败路径的差异是上述明确修复，没有用成功路径对比掩盖失败行为变化。

另只读打开真实旧 Terra 一主控/两成员，模型、逻辑投递 ID、派单原文来源、pending 0 与无待处理汇报保持。没有 prompt 或 Hive resume，没有新模型推理。

## 实际检查与边界

通过：`pnpm check`、`pnpm build:runtime`、`git diff --check`、以上复现/差分/样本/真实旧会话，以及以下现有检查：

- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-session-normalization.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-paginated-history.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-steering.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-session-failures.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-interrupted-tools.ts`

未新增、修改或删除正式测试；未跑全量测试：仅桌面 Codex 显示日志的局部终态合并与提交顺序，使用真实文件错误、完整成功差分、实际原生协议及相关回归验证。当前安装包不变，下一次统一纳入开发包。没有面板操作、厂商推理、依赖或用户全局配置变动；Pi 登录继续搁置，真实推理仅 Terra，Windows 真机与完整产品目标继续开放。
