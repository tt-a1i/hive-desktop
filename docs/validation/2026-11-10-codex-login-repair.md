# Codex 未登录修复入口（#26 后续）

2026-11-10，源码基线9dc5fc4。本轮把已检测出的未登录状态接到明确的下一步，不是内嵌OAuth登录或完整认证验收。

## 用户问题与实现

首次检查提示“请在对应CLI登录”，但配置可能包含多份Codex。用户仍需找出命令并处理路径引号，容易登录另一份程序。现在只有本次有效结果为signed_out时，显示该次实际检测命令的login参数、终端说明及复制按钮；Windows说明为PowerShell。改路径后沿既有过期判断隐藏整组旧结果和命令。已有账号、无需认证、失败及未知状态均不附登录命令。不会因复制而执行CLI或启动OAuth。

从已有remoteCommands提取原封不动的POSIX/PowerShell argv转义到terminalCommand，两个调用点共享，不新增依赖或Shell执行通道。程序仍由既有无Shell的OwnedCliProcess运行；helper只生成供用户复制的文本。未增加内置Pi审批或修改用户全局模型默认。

## 真正未登录与实际原生操作

证据目录.artifacts/codex-account-9dc5fc4。测试启动脚本仅为原样0.167.0子进程指定本轮独立空CODEX_HOME，并清除该子进程的两项API Key环境变量；用户真实Codex配置未被替换。独立包内Node调用只读account/read得到signed_out，signed-out-receipt.json。这是原生CLI未登录证据，区别于前一轮合成JSONL对端。

新建本轮独立工作区、一条未绑定Agent的Codex Terra空会话和未发送草稿。Mac最终源码程序、包内Node及本仓编译runtime：

- 中文/light/20px：输入未保存隔离CLI路径并检查，实际显示未登录、检测来源、完整被引用的命令；未保存提示可见。约593×507窄窗从路径Tab经过Browse/检查到复制按钮，焦点自动显露，命令和长路径正常换行。zh20-login.png。
- Return复制，Escape回聊天，Cmd+V实际粘贴的完整正文为同一被引用路径加login；copied-login.ax.txt逐字核对。未按发送，随后恢复ONBOARDING_KEEP · 保留未发送草稿。
- 改回保存的正常CLI路径后，完整AX中旧命令/复制入口消失，显示输入变化；保存仍禁用。stale-command.ax.txt。
- 切English后再检查隔离CLI，英文说明/未登录结果/命令/Tab到复制按钮通过，en20-login.png/AX。Shift+Tab回路径，恢复正常CLI并Return检查：实际ChatGPT账号记录、没有登录命令，Save禁用；signed-in-no-login.ax.txt。
- Escape返回精确原草稿，Cmd+Q exit0，应用已不在运行清单。没有执行Codex login或接受授权；隔离目录auth.json仍不存在。
- 独立包内Node及新runtime冷读确认原保存路径、一条原会话身份/标题/Terra、无Codex线程、空历史、pending0/busyfalse、精确原草稿。只读SQLite workspace1、messages/agent_runs/dispatches/workflow_runs均0；cold-receipt.json。无新增模型回合。

初始化夹具曾误写不属于产品恢复协议的desktop-navigation.json，已在启动前删除，未为夹具改生产恢复逻辑。测试壳只硬链接本项目程序并复用现有Node；无新增依赖树或整包副本。

## 定向检查

- pnpm --filter @hive-desktop/runtime check:terminal-command：先缺实现模块失败，再实现通过；执行生成命令调用自有argv记录程序，空格/中文/单引号/美元替换/反引号/分号/换行逐参数往返，未生成注入哨兵文件。macOS通过；PowerShell格式有静态输出断言，不能冒称Windows执行。
- HIVE_CODEX_COMMAND=/Users/tsk/.npm-global/bin/codex pnpm --filter @hive-desktop/runtime check:codex-account：原3类设置来源及错误/取消/回收/真实CLI回归继续通过；新增真实HTTP/SQLite的signed_out结果及精确登录命令契约。其他状态的原有精确输出断言确保未附登录命令。
- pnpm --filter @hive-desktop/runtime exec tsx checks/remote-settings.ts：提取共享helper后，原包内remote CLI命令、含中文/引号/命令替换字符的数据路径隔离、回环登录/退出、真实WebSocket加密配对/安全码/撤销及持久化通过。网关/账号是自有回环对端，不是生产远程账号验证。
- pnpm check、pnpm build:runtime、Mac cargo build、Windows MSVC cargo check和git diff --check通过。没有删除既有测试；只扩展账号契约检查并新增最小命令执行检查。未跑无关全量；未修改凭据/模型协议或schema。

## 当前边界

本轮补齐原生Mac未登录、中文/英文20px窄窗、实际复制、过期隐藏与已有账号分支。实际浏览器授权/重新登录不在本轮执行范围；账号记录仍不代表凭据有效或模型可用。无需认证/其他账号类型的原生矩阵、VoiceOver、Windows原生及新开发包仍待验收，#26与完整目标保持开放。当前021c162 ZIP/DMG/Applications未更新，本轮未触发系统通知或替代#25授权确认。
