# 后台凭据/交互/关闭边界加固与命令及搜索选择器焦点体验打磨

2026-11-13，基线 `a466407`（当前开发包 `992aae6`）。

## 改进范围与问题定位

围绕后端逻辑正确性最高优先级（推进 `#1`、`#3`、`#14`、`#17`、`#20`），复核 `packages/runtime/src/` 全量适配器/宿主边界与 `apps/desktop/src/` 输入区及筛选选择器，修复六项后台边界问题并完善桌面端焦点显露与卡片层次：

1. **内置 Pi 退出登录凭据脱敏读取与关闭信号立即取消（`pi-auth.ts`）**：
   - 原 `logout(providerId)` 在校验未注册提供商凭据时直接调用无信号、无脱敏包装的 `models.listCredentials()`：若本机 `auth.json` 损坏，可能返回旧快照或将 `JSON.parse` 引用的凭据片段透传到桌面协议层；同时 `close()` 在等待 `this.removal` 结束后才触发 `this.catalogAbort.abort(...)`，且 `assertIdle()` 未拦截 `this.closing` 状态。
   - 现统一抽出带 `this.catalogAbort.signal` 与本地 `cause` 脱敏包装的 `readCredentials(models)` 供 `catalog()` 与 `logout()` 共用，`assertIdle()` 同步拦截 `this.closing`，并在 `close()` 入口第一时间触发 `this.catalogAbort.abort(...)`。
2. **外部 Pi 原生交互取消并发写排空与超时句柄解绑（`agents/external-pi-interactions.ts`）**：
   - 原 `cancel(reason)` 使用 `Promise.all` 发送 `{ cancelled: true }`，若首个写帧拒绝会提前返回而遗漏等待其余兄弟交互的取消写帧；现改为 `Promise.allSettled` 排空全部待处理交互取消帧后再抛出单错误或 `AggregateError`（严格保持 `external-pi.ts` 在取消写失败时回收子进程的故障闭合契约），并为长时限交互定时器补齐 `timer.unref?.()`。
3. **内置 Pi 会话目录递归初始化与并发创建幂等容错（`agents/pi-session-manager.ts`）**：
   - `createDesktopSessionManager` 在以 `{ flag: 'wx', mode: 0o600, flush: true }` 写入初始会话头前补齐父目录 `mkdirSync(dirname(path), { recursive: true, mode: 0o700 })`，并对并发打开的 `EEXIST` 做幂等容错。
4. **会话装载期上下文注入后的关闭中断防护（`chats.ts`）**：
   - `Chats.load()` 在异步 `reasoning` 恢复与多段 `handle.addContext(...)` 完成后、写入 `ChatIndex` 及创建 `DeliveryQueue` / 挂载 `this.live` 前再次调用 `this.closeAbort.signal.throwIfAborted()`，防止关闭期间晚到的打开流程改写索引或残留队列微任务。
5. **宿主导航关闭态防护与工作区移除期 `/api/ui/workspaces/*` 写操作拦截（`hive-host.ts`、`desktop-runtime.ts`）**：
   - `hive-host.ts` 为 `getNavigation` / `saveNavigation` 补齐与其他宿主入口一致的 `if (closing) throw new Error('Hive host is closed')`，避免关闭竞态访问已关闭的 SQLite 连接；
   - `desktop-runtime.ts` 的 `requestHive` 工作区移除写拦截正则扩展为 `/^\/api\/(?:ui\/)?workspaces\/([^/?#]+)(?:[/?#]|$)/`，覆盖 `/api/ui/workspaces/<id>/...` 变更路径。
6. **原生命令菜单、搜索筛选重置与输入区/终端卡片细节打磨（`command_picker/view.rs`、`app.rs`、`settings/pi_accounts.rs`、`queued_inputs.rs`、`view.rs`、`terminal/view.rs`）**：
   - 为 `/` 原生命令菜单结果列表（`#native-command-results`）接入 `Viewport` + `Reveal` 键盘焦点自动显露，并将命令来源（技能/模板/扩展）升级为高对比圆角胶囊徽标；
   - 为会话模型搜索、成员列表搜索、成员模型搜索与内置 Pi 提供商搜索在输入变更时补齐滚动位置归零（`(0, 0)`），消除长列表滚到底部后筛选空白偏移；
   - 为 `#composer-body-scroll` 中的失败草稿卡片与断线复制按钮补齐 `Reveal`，统一排队消息卡片（`#received-input-*`）、顶部错误横幅与可复制终端错误框（`#terminal-error`）的边框与表面对比层次。

## 验证证据与边界

- **后台六项边界专项验证**（`.artifacts/backend-hardening-20261113/receipt.json`）：
  - `piAuth`：`logout` 传入 `AbortSignal`、拦截 `SyntaxError` 中的模拟密钥片段且保留本地 `cause`；在途 `logout` 目录读取收到 `close()` 后 `0 ms` 即时中止；
  - `externalPiInteractions`：首项取消写抛错时第 2 项兄弟取消写仍完整排空后再拒绝，双路拒绝聚合为 `AggregateError(2)`；
  - `piSessionManager`：深层缺失父目录自动创建并生成 `0600` 会话头文件；
  - `desktopRuntime`：工作区移除期间 `POST /api/ui/workspaces/<id>/...` 精确返回 `WORKSPACE_REMOVAL_BLOCKED`，`runtime.close()` 后 `getNavigation` / `saveNavigation` 干净拒绝。
- **运行时与桌面端回归检查**：
  - `pnpm check && pnpm build:runtime` 及 `check:runtime`、`check:workspace-removal`、`check:terminal`、`check:workflow-shutdown` 全部 `exit 0`（零厂商模型推理）；
  - `LOCALE_AUDIT_OK`（1,301 键 / 1,195 处引用）、Rust 36 项单元测试、13 项进程树监督测试、`x86_64-pc-windows-msvc` 主程序与进程监督两项 Windows 交叉检查及 macOS release 构建均 0 警告通过。
