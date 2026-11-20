# 首次会话与日常操作

本轮从 Hive Desktop 的 `5308bfc` 开始。上一轮已完成首次引导、文件浏览、协作收件箱、历史分页和 PR 评审入口。本轮从仍需多一步操作的地方入手。任务记录在 [#55](https://github.com/tt-a1i/hive-desktop/issues/55)。

| 参考 | 本轮改动 | 剩余差距 |
| --- | --- | --- |
| T3 Code `7085c47` | 沿用可跳过、可重开的单页引导。Hive 可在引导中选择项目和已保存的 Agent 配置，并直接打开空白会话。根据本机检查结果推荐默认配置，用户也能主动选择其他已启用的配置。 | T3 Code 支持勾选多个项目并导入近期 Codex、Claude Code 和 Pi 对话。Hive 当前发现并添加项目文件夹。 [欢迎页](https://github.com/pingdotgg/t3code/blob/7085c477768d0986956d113d072bc301b2841823/docs/user/welcome-page.md) |
| Cindy `053a901` | 每个工作区保存协作事项的暂缓状态，重启后可查看和取消暂缓。新消息的 ID 与已暂缓事项分开处理。 | 跨项目统一收件箱、按紧急程度排列和定时恢复仍有差距。Cindy 文档中的跨设备验证也仍有待办。 [收件箱规则](https://github.com/makecindy/cindy/blob/053a9012748113aff20ecb1ec9a5746297700202/docs/product-rules/task-inbox.md) |
| Beluga `45a484b` | 文件名和文件正文搜索支持方向键切换当前结果，回车打开对应文件及命中行。 | Beluga 的文件树可用方向键逐层展开，文件面板可持续与聊天同屏。 [文件面板源码](https://github.com/kayraucklnc/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx) |
| Apache Maka Rust `64a5c6b` | Hive 的 JSONL 尾部读取逐块扫描，每条跨块记录只拼接一次。已有的关闭信号可中断读取。超长工具文本的预览保留完整 Unicode 字符，续读可接上原文。 | Hive 某些旧历史页仍需展开底层历史。Maka 可以在被固定的历史范围内按存储游标读取。 [存储来源与游标](https://github.com/apache/maka/blob/64a5c6b3a6c2f0354b011f5ccb8527a7f1bce874/plugins/agent/state/src/session_history/sql.rs#L245-L277) |

## 验证

- TypeScript 检查、运行时构建、Agent 配置的生产 JSONL 检查、离线 Pi SDK/RPC 历史回归均通过。
- 桌面 Rust 测试 164 项通过、1 项原有忽略。团队和项目文件回归通过，Windows x64 MSVC 交叉类型检查通过。
- macOS 原生窗口用空数据目录进入欢迎页，经系统目录选择器选择临时项目，并显式选择 Codex 后开启空白会话。无需点击输入框即可打字，冷启动恢复到同一会话及草稿。隔离数据库中消息和 Agent 运行数均为零。
- macOS 原生文件浏览用普通键盘事件完成文件名搜索、向下选择及回车预览第二个结果。正文搜索先按回车打开第一项，再按向下和回车切到第二个文件的命中行。验证时选中的项目目录仍由上级 Git 仓库忽略。运行时回归同时确认从父项目浏览时遵守其忽略规则。原生收据位于 `.artifacts/onboarding-first-chat/receipt.md`。
- 工具输出回归覆盖预览在 emoji 中间截断的原始复现。对比 720 组 JSONL 读取用例，新旧实现返回相同记录和游标。单条约 12 MiB 的合成记录在同进程五轮交叉测试中，中位读取时间从 1,670 ms 降至 10.9 ms。
- 收件箱测试覆盖重启后恢复、取消暂缓、工作区隔离、旧消息状态变化与新消息出现、损坏文件保留和写锁重试。隔离的 Hive 后台实际重开后，同一派单的新旧问题各自返回独立消息 ID。收件箱的暂缓按钮此次由自动化测试验证。
