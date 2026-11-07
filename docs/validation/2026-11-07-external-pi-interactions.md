# 外部 Pi 扩展交互

桌面已接入原始 Pi 1.4.0 的 select、confirm、input 和 editor 请求，共用聊天卡片、会话归属、快照和回应入口。内置 Pi 未新增审批。Pi 的取消仅回应当前请求；空字符串是合法输入，多行编辑保留预填内容，选项没有默认选择。

真实场景：用户已有外部 Pi 扩展在工作中请求选项或输入，过去会被自动取消。现在同一原生请求在桌面显示并把用户回答交回扩展，可通过回调结果直接核验。不做这一项，用户的现有扩展流程会在桌面中失败。

## 已验证

- [原始 Pi 检查](../../.artifacts/pi-interactions-evidence/receipt.json)：真实扩展命令的四类成功/取消、空值、唯一非空回答哈希、超时、停止与进程死亡。工作区和设置经过真实 Hive HTTP/SQLite；命令通过 handle.prompt、回应通过 Chats 方法，不能据此声称 chat.send 或桌面发送闭环。实际进程 PID 的扩展注册在每次命令前核对，模型固定原生 `openai/gpt-6.1-terra`，夹具会在推理前退出，未产生模型调用。
- [JSONL 对端检查](../../.artifacts/pi-interactions-evidence/jsonl-receipt.json)：等待用户时不消耗 prompt 活跃超时预算，其他请求仍有超时，重复原生请求导致关闭，失效回应不再写入。对端为合成，不执行模型。
- [macOS 桌面检查](../../.artifacts/pi-interactions-ui-proof/receipt.json)：原始 Pi 加载测试扩展，通过受控文件触发闲时请求；原生窗口选择“选项乙”、回应 false、提交空字符串和中文多行回答，均与扩展收到的哈希一致。切换另一会话后编辑草稿保留；取消得到原生 undefined；760×583 窗口中内容与操作区可滚动到达。未发送聊天提示，两个原生会话均没有助手消息。
- 回答不进入 Hive 的事件、快照和持久状态；这不保证用户自己的扩展或外部 Pi 不会记录回答。桌面测试扩展仅记录哈希。

## 失败与边界

早期夹具未正确注册扩展命令，slash 输入落入用户默认 Magpie 路由并连接失败，报告 0 token；这是一次不应发生的模型路由尝试，不能记为无推理验证成功。[失败记录](../../.artifacts/pi-interactions-evidence/failed-preflight.json) 已保留。后续改为显式扩展、Terra、实际 PID 注册检查及推理前退出护栏，未改全局认证或默认模型。

Pi 1.4 在安装 stdin 读取器前等待 session_start。启动扩展若等待对话框，客户端无法解锁；本次明确快速失败并关闭受管进程，原始会话和模型目录路径均已验证有界失败。见 [启动收据](../../.artifacts/pi-interactions-evidence/startup-receipt.json)。不宣称支持该上游限制中的对话框。

Pi 没有原生回应确认或 AbortSignal 取消通知。界面“已发送”只表示管道写入完成；本地超时从收到事件计算，不能保证与外部取消完全同步，迟答可能被 Pi 忽略。未验证 Windows 原生窗口、真实模型触发这些扩展、或所有第三方扩展。

## Self-Review

四角度独立复核最终为 A- / B+ / A- / B。架构项中 Codex 身份可选化曾减弱界面校验，已按后端检查必要身份，错误卡保留提示和停止入口。测试项中单独预检不足以证明实际进程加载扩展，已改为实际 PID 注册证明并增加禁推理护栏；原生超时值和回答不落盘断言已加强。未留下严重项；覆盖边界保留，不以绿色检查替代真实界面验证。

实际运行 TypeScript check/build、check:codex、check:external-pi、新增原生与 JSONL 交互检查、Rust fmt/check/build。原 Codex 检查因回应方法变为异步而调整为 await/assert.rejects，未放宽语义；未删除检查，纯 UI 未新增测试。完整产品仍在实施。
