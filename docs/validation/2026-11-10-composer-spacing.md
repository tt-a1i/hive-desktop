# 输入区空白与内容保留

源码基于 bf96b2a。纯原生 UI 调整，不改后台、队列、输入语义、依赖或测试。

## 问题与修复

20px 字号的窄窗口中，无错误、无待显示输入、无附件时，三个空区域仍作为 composer `gap_2` 的子元素参与布局。将三个 renderer 的空状态改为 `None`，调用方只挂载存在的区域。正向内容保持原实现，Textarea 的 1–7 行自动高度、65% 窗口上限及固定底栏不变。

先尝试的 Viewport fit-content 假设实际没有改善，已完全撤回；未修改 focus_scroll。不能将这次修复解释为改变整个滚动策略。

## 实际 Mac 验证

证据目录 `.artifacts/composer-intrinsic-height`，独立数据、生产后台及本地合成 Codex JSONL 对端，零真实模型请求。1186×1014 物理像素、英文20px同尺寸前后图中，空 composer 从约280物理像素降至220，节省约30逻辑像素；宽窗也核对。before/after-wide、before/after-narrow 及 AX 均保留。

- 12 行中文/英文/emoji 草稿增长至7行上限；Cmd+Down 可到末行，Cmd+Up 可回开头，清空后收缩，Cmd+Z 恢复原文。首次粘贴后未单独证明光标自动滚至末尾。
- 实际系统文件选择器添加自有 README，清空长文本后附件预览可见；移除最后附件后空区域消失、空文本 Send 禁用；撤销恢复12行文本，不恢复已移除附件。
- `cold.mjs` 冷读生产后台：草稿逐字一致、附件为空、历史两轮仍完成且字节不变、报告完成 ID/归属保持、无待处理消息。
- 长草稿与附件同时存在时，附件可能在 composer 可视区上方；此前鼠标尝试未完成该组合的滚动/键盘可达性验收，继续核对。

另用 `.artifacts/composer-state-check` 独立项目，从实际窗口发送一条合成输入、排队一条补充，窄窗显示接收状态及内容。仅 SIGTERM 本次拥有的合成对端后，实际错误区、详情和继续处理按钮可达。详情原文为 `Backend exit cleanup failed`，未新增生产兜底，也未据此宣称正常清理。退出后两条原始投递逐字/ID保持；冷读 busy=false、pending=2、原生历史仅原来的一轮且字节不变，无自动重发。保存 accepted-before-fault、after-exit-pending、cold-snapshot、cold-receipt、queued/error-narrow 图与 AX。该模拟不代表真实 Codex 崩溃或模型验收。

前三次 UI 进程观察 exit0；最后故障场景已在 CUA 观察原生应用退出且进程不存在，终端 session 已失效，不追认为已取到 exit code。

## 检查与边界

- `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`：通过，最终 build-clean.log 无新增警告。
- `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`：通过，windows-clean.log。
- `git diff --check`：通过。
- 无新增/修改测试；纯 UI 使用原生正向/空状态与实际持久化校验，未跑全量测试。

尚未纳入 current/candidate/Applications 的1e68306包。Windows 原生、VoiceOver播报及完整目标继续，不以此项作为完整交付。

## 后续：附件键盘显露

实际复现：12行草稿加README，在1186×1014物理像素窗口，编辑器定位末尾后，Shift+Tab 虽切到附件按钮但附件仍在 composer 可视区外。给附件区域复用已有 `focus_scroll::Reveal`，仅一处调用方变化，无新滚动抽象。

新二进制实际验证：编辑器 Cmd+Down → Shift+Tab 显露带焦点环的移除按钮；再次 Shift+Tab → Enter 打开README只读预览；关闭后回编辑器末尾 → Shift+Tab → Enter 实际移除附件，正文12行保持。保存 long-attachment-before/revealed/removed 图与AX；应用正常退出exit0，冷读脚本再次通过。Mac build/Windows目标check通过。鼠标从正文滚回附件的便利性仍需改善，本次不宣称已完成整个组合体验；尚未验证多图/多行附件与VoiceOver。
