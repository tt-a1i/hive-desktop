# Codex 长历史快照的投递来源查找

2026-11-12。基线 92e8d24；源码优化，尚未纳入当前 3a18b94 安装包。

## 实际开销与修改

桌面不是定时拉取整个聊天历史；原生端使用流式事件，chat.open 在打开会话等实际读取场景取完整快照。CodexEvents 为每条历史调用投递来源查找，原回调对全部 metadata.deliveries 执行最多两次 find。5000 回合/10000 条历史/5003 条来源记录的真实 JSONL 受控进程实测，已缓存会话的快照仍需约 118.6ms。

仅修改现有 codex.ts 的会话内查找：按 turnId、itemId 构建嵌套 Map，首次按需创建；元数据成功写盘、内存替换后作废。保留每个键的首个记录，查询先精确 item，再 undefined 的回合级来源，维持旧 find 的顺序和回退语义。来源关联、同回合追加更新后会重建，不依赖只在初始化构建的静态表。写入失败不会更换 metadata 或作废其对应的旧索引。

没有新模块、持久化缓存、store、协议字段或依赖。代价是每个发生来源查询的 Codex 会话增加 O(投递记录数) 的内存索引引用，随该会话对象保留，成功写盘后作废；不复制消息正文，未测全应用内存或引入淘汰策略。

## 真实进程与文件证据

`.artifacts/codex-snapshot-20261112/check.mjs` 实际创建 CodexSession、受控 Node JSONL app-server 进程、分页 thread/turns/list 和持久元数据/显示历史。数据为明确的合成原生历史，模型字段 Terra 只是协议元数据，无厂商推理。

- 5000 回合/10000 历史/5003 来源记录，前后各 6 次直接生产 handle.snapshot 调用。首个快照 221.3→4.3ms；后五次中位 118.6→2.0ms。包含快照构建，不含随后的 JSON stringify。
- 线程装载仍约 467/470ms，本批没有优化装载、分页、磁盘写入、JSONL 传输或原生渲染，不能推广为整个应用的加速比。
- 优化前后完整快照 JSON SHA256 相同：`a604b13ffd2550bd3dbc0f85bc981132c8d50d336a512aeb598401ac9ef3be41`。重复精确记录保留第一个；精确优先于较早的回合级来源；重复回合级记录保留第一个。快照读取前后 metadata 文件逐字节不变。
- 先读快照使索引存在，再通过实际 turn/start / item/completed / turn/completed 接收新输入并完成投递；流式 message_end、持久化来源及完成 ID 均正确。前后新回合完整快照 SHA256 同为 `0df5e3ee80c7cac8395dfab69e5a02372c98f8c6089e7054357f7c77926ffffa`。
- 关闭并重新创建进程/会话，完整新快照相同。没有把合成响应当作实际模型生成。
- 另使用真实已安装 Codex CLI 控制链路，只读打开旧 Terra 一主控/两成员会话，模型、完成 delivery IDs、原派单展示来源、pending 0 与无待处理汇报保持。没有调用 prompt 或 Hive resume，没有新模型回合。

初次 before 夹具未忽略 initialized 通知而退出，得到 stdout EOF；修正受控对端的通知处理后运行 baseline。旧失败日志保留，没有改生产传输或重启一个仍活跃的进程。有效数据为 baseline / after。

## 已执行

- `pnpm check`、`pnpm build:runtime`、`git diff --check`
- 上述实际 JSONL / 文件 / 快照对照与旧 Terra 三会话冷读
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-session-normalization.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-paginated-history.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-steering.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-session-failures.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-interrupted-tools.ts`

全部通过。尤其 steering 的写盘失败/冷恢复/追加消费与重试保留覆盖索引失效边界。未新增、修改或删除正式测试，未跑全量测试：改动限于既有查找回调的临时索引，采用原生协议、完整 JSON 一致性、持久化/动态追加及相关现有回归验证。无面板操作、新模型推理、协议/用户数据语义变化。当前安装包仍为 3a18b94；Pi 登录继续搁置，真实推理只用 Terra，Windows 真机与完整产品验收仍开放。
