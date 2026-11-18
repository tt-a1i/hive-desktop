# Codex 原生问题的附件回复

2026-11-19 已补充 [macOS 原生操作、真实 Terra 副本读取和冷恢复验证](2026-11-19-codex-question-attachments-desktop.md)。下文保留 2026-11-15 当时的验证状态。

范围：Codex `item/tool/requestUserInput` 中非 `isSecret` 的问题。普通输入框附件草稿独立保留。其他后端的问题及保密问题不开放附件入口。

Codex 的原生回复仍是 `answers: { questionId: { answers: string[] } }`。当前协议没有图片或文件字段。桌面把用户选择的文件或图片上传为 Hive 私有副本，并把 JSON 编码的副本名称、类型与绝对路径加入对应问题的答案字符串。这些引用明确标为用户数据，图片路径不表示原生图片输入，也不代表模型已经读取附件。没有额外发送普通聊天消息。

## 生命周期与保存

- 新增 `chat.interactionAttachments.save/list`；`chat.respondInteraction` 可接受独立的 `attachmentsByQuestionId` 上传 ID 映射。上传 ID 必须属于原会话工作区，每个交互最多十个不同附件。
- 提交前先验证原始原生答案，再定位全部副本，重新检查原会话、运行中 handle、问题及原生请求、thread、turn、item 身份。已结束或更换的问题不能收到迟到的回复。
- 附件收据只保存来源身份、上传 ID、名称、原副本路径和状态，不保存自由回答或保密回答。写入 `prepared` 并同步到磁盘后才交付原生回调。`callback-handed` 仅表示交付回调；原生确认与模型实际读取分别判断。
- 同一交互的保存/提交互斥。结果未知的已准备或已交付收据阻止重试，旧 RPC 形式也不能绕过。重开只显示原附件收据，不恢复回调或自动提交。
- 原生选择器、导入与保存回调绑定工作区、会话、连接和原生交互身份；不把迟到结果写入其他问题或普通输入草稿。提交前失败保留编辑。交付结果未知时保留收据并停止自动重试。

## 验证

受控检查使用真实 Hive HTTP、SQLite、私有上传副本及独立 Codex JSONL 子进程；不调用模型或账号。运行：

```sh
pnpm --filter @hive-desktop/runtime check
pnpm --filter @hive-desktop/runtime check:interaction-attachments
pnpm --filter @hive-desktop/runtime exec tsx checks/codex-interactions.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/image-attachments.ts
pnpm --filter @hive-desktop/runtime exec tsx checks/file-attachments.ts
CARGO_TARGET_DIR=/tmp/hive-question-attachments-target cargo check --offline --manifest-path apps/desktop/Cargo.toml
PATH=/opt/homebrew/opt/llvm/bin:$PATH CARGO_TARGET_DIR=/tmp/hive-question-attachments-target cargo check --offline --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc
```

以上 TypeScript、既有普通交互/图片/文件回归和 macOS / MSVC 静态编译已通过。Rust 使用独立写时复制缓存，不写原工作树缓存。新检查覆盖文件与图片原文件删除后的副本、普通图片 payload 不变、跨工作区与未知上传 ID、保密问题、非法答案、重复附件、并发提交，以及导入期间取消/回合结束/断线、收据冷读不重发。另以失败注入验证：部分上传不可用、收据写入失败后保留原问题并可重新准备、原生回调交付后 CodexTransport.write 拒绝时收据仍保留且禁止重试；子进程未收到答案。此处是受控写入失败注入，不是实际操作系统 broken-pipe 验收。具体结果见检查脚本及其收据。

独立只读审查发现提交后收据缓存刷新遗漏，已修复。图形操作、Windows 原生流程、真实 Terra 对问题附件的读取仍未验收。Claude/Antigravity 真实验收按本轮范围跳过；未重试 Pi `invalid_grant`。没有把本次 Codex 路径声明为所有后端问题附件均已完成，也没有更新开发安装包。
