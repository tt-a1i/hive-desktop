# 消息失败、诊断恢复与原投递重试

日期：2026-11-08。完整桌面目标仍在实施。

## 真实问题

1. 用户发送消息或附件后，Pi 的原始 API 错误曾同时出现在助手正文和全局错误条；重开应用后，其他后端的投递失败原因也缺少统一恢复入口。
2. 现在输入区显示一份简短失败提示，详情按需展开和复制；重开仍能看到当前待处理消息的失败原因，使用既有继续接口重试原投递和附件。历史失败回复保留可展开诊断，空正文也不会丢失失败记录。
3. 不处理会让用户难以区分历史错误与当前待处理状态，也容易误以为需要重新输入或重新附加文件。

## 实现与数据边界

- 在现有聊天索引保存 `lastDeliveryFailure {deliveryId,message}`。仅匹配持久投递队列头时才作为当前失败返回，不建新任务账本，不修改 Hive 派单或汇报语义。
- 队列仍为待处理消息的权威来源。成功处理或取消移除队列头后，旧诊断不会套在下一条消息上。重试沿用既有 `chat.resume`、deliveryId、正文、图片和文件副本。
- Pi 原生会话保留每次执行尝试；相同投递 ID 在桌面合并为同一条用户消息。此改动没有改写厂商原生重试上下文或删除历史失败尝试。
- 无法保存诊断时明确发送 `diagnosticWriteFailed`，保留已接受队列，显示临时告警，不声称错误原因已落盘。未保存的诊断无法在重开后恢复；原消息仍能继续。
- Pi `stopReason:error` 的 errorMessage 与真实回复正文分开，历史详情可折叠。当前失败使用显式投递 ID 关联，不匹配错误字符串；部分正文和思考保留。
- 会话切换清除当前视图的诊断，后台连接中断时禁用重试；普通 Agent 异常另有当前会话告警，不再写入全局错误条。

## 本次验证

- `pnpm --filter @hive-desktop/runtime check:delivery-failures`：真实 Hive HTTP / SQLite、Pi SDK、持久上传与队列，连接本地 Responses SSE 对端。覆盖失败→关闭→恢复→成功重试、原图片和文件副本、相同逻辑投递身份、真实索引原子重命名写入失败、明确告警、第二条投递不误用旧诊断、修复磁盘后继续、旧索引兼容和非法诊断拒绝。
- Mac 原生 GPUI：浅色、深色、760px 宽，各两个应用进程，真实 Node runtime 及本地 HTTP。覆盖恢复失败、详情展开/收起、第一次重试再次失败、同一错误不重复显示、第二次重试成功、运行中状态和成功后重开不自动请求。发现并修复空正文过滤误删失败历史。保留文字和文件卡片，逻辑用户输入只有一条。
- 原生走查使用 GPUI 注入鼠标事件和窗口截图，不等同于物理鼠标键盘；没有操作真实用户剪贴板，本轮复制按钮的系统结果未验收。合成 SSE 成功回复不是厂商推理证明。
- `pnpm check` / `pnpm build:runtime`；`check:team`；`check:workspace-removal`；`check:codex`；`tsx checks/claude-runtime.ts`。
- `cargo test --locked --manifest-path apps/desktop/Cargo.toml`：18 通过，1 个专用 fixture 忽略；Windows MSVC 默认桌面特性 `cargo check` 通过。没有跑全量 Node suite：本次修改集中于投递诊断与呈现，采用相关队列、团队、清理与适配器回归。

证据保留在 `.artifacts/delivery-failures/`：`core-receipt.json`、三份 `native-*-receipt.json`、原生截图与驱动脚本。fixture 源码目录由收据 data 的父目录定位，路径为临时验证数据，不能作为产品目录约定。

## 尚需完成

- Mac 开发包 `.artifacts/hive-desktop-macos-arm64-retry.zip` 已生成，解压后真实 Node / PTY / Hive HTTP / SQLite / 退出烟测和原生窗口恢复已通过。包未签名、公证，正式安装发行与 Windows GUI 不在本轮通过范围。
- 未进行新的厂商模型调用。Pi OAuth 失效、Claude 真实协作、外部 Agent 真实错误交互和 Windows 最新界面验收仍有独立边界。
- 未新增或改写 Hive 核心协议；未删除、修改现有测试。
