# 恢复摘要与历史 JSONL 的 Unicode 修复

日期：2026-11-12。范围：源码验证；随后已纳入 a744821 开发包，见[包内验证](2026-11-12-recovery-unicode-package.md)。

## 问题与修改

Hive 恢复摘要对任务文档、派单进度和待答问题按 UTF-16 单元截断。在 emoji 边界可能留下半个代理对。真实 Pi 恢复将其写入 startupContext 和历史；Node JSON.stringify 能输出该值，但原生端 serde_json 1.0.154 拒绝整个响应。

- 既有 Hive 2.2.1 补丁中的三处摘要截断改为按 Unicode 码点计数，保持原有 500/1000/1536 限额及 HTML/XML 转义。
- 同类错误提示的 60 码点预览与省略号使用同一计数；40 个 emoji 不再被错误截短。
- 桌面共享 JSONL sender 仅在序列化字符串值时把孤立 UTF-16 代理项投影为 U+FFFD，供 Rust 读取已有损坏历史。有效代理对和普通文本保留；不改写 SDK 历史、数据库或模型输入。此处不声称修复任意对象键。
- lockfile 仅更新 Hive patch hash，依赖版本未变。未新增或修改正式测试。

## 证据

本地证据目录：`.artifacts/unicode-recovery-wire-20261112/`。

1. `recovery.mjs restart-before` 使用真实 Hive HTTP/SQLite、Pi SDK 和运行时重启，完整任务文档含 1535 个 ASCII 字符后接 emoji；以自有 queued dispatch 触发实际恢复分支。旧版在 startupContext、history.2.content 和 rawBackendItem.content 出现孤立代理项。最初 `before` 实验只做首次启动，未进入恢复分支，不能作为缺陷基线，已保留并以重启实验纠正。
2. `cold-wire.mjs before` 从旧 SDK 会话通过生产 dist/main.js 请求 chat.open，保存未重打包的 stdout JSONL。Rust 检查实际报 `unexpected end of hex escape`。
3. 修复后 `recovery.mjs after` 的同一重启流程无损坏字符串，截断处保留完整 emoji，原始任务文档逐字相等。受控 loopback 模型端点记录请求数为 0。
4. `cold-wire.mjs after` 读取修复前留下的同一 SDK 会话，实际生产 JSONL 被 Rust 接受；脚本逐字节断言原始 SDK 文件未变，退出码为 0。
5. `format-cases.mjs before/after` 直接调用生产摘要与错误格式化函数；覆盖三种摘要边界、错误预览边界和 40 emoji。此项是格式化检查，未冒充 HTTP report 集成。

## 已执行验证

- `pnpm check`、`pnpm build:runtime`
- `pnpm install --frozen-lockfile --offline`、`git diff --check`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/empty-session-recovery.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/user-input-recovery.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/team-runtime.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/pi-model-overrides-wire.ts`
- 本地复现脚本及同版本 serde_json 的 Rust 断言（见上）

全部通过。未跑全量测试：本次修改集中于已复现的文本生成和 JSONL 输出边界，以真实恢复、冷读、Rust 解析及对应现有回归验证。未进行新模型推理、面板操作、原生包发布或 Windows 真机验收。其他尚未证明有实际影响的截断路径不在本次修复范围。

Pi OAuth 登录继续搁置；真实推理仍仅允许 Terra，Claude 原生模型验收仍未进行。
