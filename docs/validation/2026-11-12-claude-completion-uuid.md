# Claude 完成回执的批量 UUID 核对

2026-11-12。基线 bda5722，源码优化；当前 78fef23 开发包尚未纳入。

## 实际开销与修改

Claude 元数据 persist 已先写文件再改内存，没有 Codex 上轮的写盘顺序问题，本批没有修改其存储。发现的实际开销是 completedDeliveryIds 对每条完成记录分别调用 hasNativeUuid(user/assistant)，每次都复制并扫描所有显示行。

在现有 ClaudeProjection 增加一个按当前可见行构造的临时原生 UUID 集合，完成回执每批只构建一次并查询。集合读取 source.nativeUuid 与合并分块的 source.nativeUuids，忽略 UI 别名/行键。每次调用读取当前行，不保留跨事件缓存，因此行替换、clear/refresh 后直接使用新视图。无完成标记时直接返回空列表，不增加旧历史的扫描。

原单 UUID 查询方法保持原实现。完成结果仍按原 metadata.completed 顺序过滤，必须同时存在 userUuid / assistantUuid；没有推断新完成标记、改写元数据、协议、新 store、依赖或账号配置。每批增加 O(当前原生 UUID 数) 的临时集合引用，不复制正文；未测全应用内存。

## 实证与测量范围

`.artifacts/claude-uuid-index-20261112/` 保留脚本和收据。check.mjs 使用生产 Projection 与明确合成的原生形状来源；前后各 5 次批量核对，计时包含当批集合构造/查询，表为后 4 次中位。

| 显示行数 | 修改前 ms | 修改后 ms |
| ---: | ---: | ---: |
| 2000 | 29.21 | 0.56 |
| 10000 | 757.05 | 3.02 |

已知 UUID 全部命中、未知/native API ID 全部排除；完整显示快照哈希前后相同。10 个公开变更阶段验证：同一 toolResult 行替换后旧 UUID 移除、另一行仍引用时保留、清空后不残留、流式 UI 别名不算原生 UUID、同一 assistant API 消息两个原生分块均可识别、父工具内子消息排除、刷新后新行可识别。前后所有阶段快照 JSON 逐字节相同。

sdk.mjs 使用官方 Agent SDK、真实受控 CLI 子进程和真实本地 transcript/元数据文件，预置 500 条完成记录、1000 行原生历史及两个缺失 UUID 的无效回执。直接调用生产 completedDeliveryIds，后 4 次中位 10.566→0.223ms；500 条结果及顺序正确，缺失 user/assistant 两条继续排除。实际 SDK 输入/原生成功终态完成后新 delivery ID 只加入一次；整个进程/会话关闭重开，完整快照与完成 IDs 保持。

另一个官方 SDK 夹具保留同样 1000 行历史但 metadata.completed 为空；完成列表仍空，不从成功回复外观猜完成。接收一个新的受控输入后，只返回新 delivery ID，冷恢复一致。

以上 CLI 明确为仓库既有 claude-reasoning-peer.cjs，协议模型名 Terra 是合成元数据，不是 Claude 运行 Terra 的声明。没有厂商模型调用。这些测量是来源核对/完成 getter 组件耗时，不包括历史加载、SDK 启动、模型时间、GPUI 渲染或全应用延迟。

## 现有回归

通过 `pnpm check`、`pnpm build:runtime`、`git diff --check` 和：

- `pnpm --filter @hive-desktop/runtime exec tsx checks/claude-history-synthetic.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/claude-context-placeholder.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/claude-write-loss.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/claude-runtime.ts`

前三者验证分块/UUID、无查询占位、真实 Hive HTTP/SQLite/官方 SDK/受控 CLI 的四类写入丢失，保留/拒绝完成回执、原输入队列/冷恢复/显式继续。最后一项使用真实原始 Claude CLI 与官方 SDK 的本机控制/隐藏上下文/冷读，没有激活成员或发起模型查询；没有把它扩大为 Claude 厂商推理验收。

本批未新增、修改或删除正式测试，未跑全量测试：仅批量成员存在核对，采用实际 Projection 变更/完整快照一致性、官方 SDK 原生协议/文件及相关现有回归验证。无面板操作、Claude 推理或真实凭据变更。当前安装包仍 78fef23；Pi 登录继续搁置，真实模型验证仍仅 Terra，Windows 真机及完整产品目标继续开放。
