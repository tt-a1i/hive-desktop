# 原始 Codex 同回合追加输入探针

显式选择 `openai/gpt-6.1-terra`，运行本机原始 `codex app-server`。这是会消耗模型额度的手动实验，不加入默认测试。使用项目现有 CLI 认证，不写全局配置。

```sh
pnpm --filter @hive-desktop/runtime exec tsx ../../experiments/codex-steering/probe.ts /absolute/new/output-directory
pnpm --filter @hive-desktop/runtime exec tsx ../../experiments/codex-steering/probe.ts /absolute/another/new/output-directory --interrupt
```

输出目录必须不存在；父目录需要已存在。`HIVE_CODEX_COMMAND` 可选择原始 CLI 路径，默认本机 `/Users/tsk/.npm-global/bin/codex`。

探针注册一个由本进程控制的 `hold_for_result` 动态工具，让真实模型调用后等待。在该边界先验证错误 turn ID 被拒绝，再对正确 ID 追加通知。普通模式释放工具、验证同回合两个独立 userMessage、仅一条预期 final、原生 completed、结束后 steer 被拒绝，并在完全新建的 app-server 连接中恢复验证。`--interrupt` 模式在追加成功响应后、释放工具前停止回合，记录中断历史及冷恢复；它不要求未消费输入已持久化。

所有原生通知、调用、完整回合历史、冷恢复历史、进程退出诊断与结果保存在指定目录。不会运行 Hive 工作流、派单或汇报，也不会把自有工具的返回称作 Worker 验证。

2026-11-09 实际结果见 [验证记录](../../docs/validation/2026-11-09-codex-steering.md)。关键发现：steer 成功响应不能代替消息已消费或回合完成；已接受的追加输入可能在停止后完全不出现在持久历史中。
