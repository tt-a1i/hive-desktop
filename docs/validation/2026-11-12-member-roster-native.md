# 成员列表：原生标签复验与读取失败呈现

2026-11-12，源码基于 f6528fe，另修改 `apps/desktop/src/members/view.rs`。这是成员读取与展示的定向修复和验证，不是完整产品验收。

## 问题与修正

续接原有成员标签修复，真实 GPUI 窗口发现：首次成员列表读取失败，页面虽显示红色错误，却同时显示 `0 members`、零计数筛选及 `No team members yet / Add your first member`。错误正文也未出现在 AX 控件树中。读取失败因此被呈现为空团队。

成员页现在只在读取成功且列表为空时显示首次创建入口。首次读取失败不显示零成员计数或空筛选，原始错误以稳定 ID 的原生 Label 暴露；有缓存时保留原成员与筛选。复用既有错误文本和刷新逻辑，无新状态、接口、依赖、后台分支或重试定时器。

## 原生证据

现场：`.artifacts/member-roster-native-20261112`。源码 debug 二进制置于独立 App 容器，显式指定本项目 runtime/Node，独立 data/agent。这不是便携包验证，未替换 Applications。

复制此前真实 Terra 一对二验证的已退出数据，不复制认证；原三会话只读恢复，未点击 Send、启动成员或恢复待执行投递。先检查现存 alpha/beta 的派单列表、详情、成员会话及返回，再仅在数据副本通过真实 Hive HTTP 删除 alpha，保留聊天和派单。

验证 JSONL relay 仅对 `team.members` 注入显式失败或扣留真实响应，其余请求/事件透传生产 runtime。故障和延迟是受控注入，不冒称自然后台故障；列表、删除、SQLite、会话读取是真实后台。relay 只在 ignored artifacts，未入生产代码或包。

| 场景 | 结果与证据 |
| --- | --- |
| 列表失败或响应延迟 | alpha/beta 均用中性名称，无历史后缀；`roster-error-neutral.ax.txt`、`roster-loading-neutral.ax.txt`及relay日志 |
| 真实列表返回 | 已删除alpha加Historical conversation，仍存在beta无后缀；`roster-success-historical.ax.txt` |
| 修正前首读失败 | 错误与空团队引导同时出现；`before-fix-error.png` |
| 修正后首读失败 | 完整AX可读诊断，无首次创建入口/零计数筛选；`after-fix-error.ax.txt`及PNG |
| 重试成功 | beta卡片和真实Stopped状态恢复；`after-fix-retry.ax.txt` |
| 缓存后再次失败 | beta卡片、筛选与错误同时保留；`after-fix-cached-error.ax.txt` |
| 真正空团队 | 真实HTTP仅删除副本beta，中文保留“添加第一位成员”；`after-fix-empty-success.ax.txt`及PNG |
| 中文读取失败 | 创建引导/零计数筛选隐藏，AX可读错误；`after-fix-error-zh.ax.txt`及PNG |
| 退出与冷恢复 | 未发送草稿逐字保留；`cold-reopen-draft.ax.txt`，两轮受监督退出收据exit 0 |

默认AX差异输出不能当完整快照：首次保存的无变化摘要已通过明确 `disableDiffing: true` 的原生重走补齐。补存时一次ScreenCaptureKit捕捉错误随后重读同一进程成功，没有因此重启或判为应用故障。`member-roundtrip.ax.txt`仍只是无变化摘要；往返结果是当轮原生观察，不用该摘要证明全文内容。

## 实际检查

- `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`：通过。
- `PATH="/opt/homebrew/opt/llvm/bin:$PATH" cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`：通过，仅类型检查。
- `python3 .artifacts/member-roster-native-20261112/final-check.py`：通过。检查完整AX、退出码和只读SQLite；原验证状态仍两成员，副本最终零成员，两边各保留两条reported派单和两条report outbox。三个原生日志的turn_context及task_started/task_complete与原基线一致，新增模型回合0。
- `git diff --check`：通过。纯UI改动，未增改删测试，未运行无关runtime全量suite。

日志、截图、relay、现场脚本和receipt在上述目录。原始验证数据与ZIP保留。正常退出后进程核对未发现本轮App、relay或runtime残留。

## 边界与下一步

加载/读取失败/成功后历史名称已补原生复验；正在执行的新真实Worker动态名称仍未重新跑模型验证，不把闲置恢复或受控延迟等同于该场景。VoiceOver实际播报、Windows原生、全部窄窗/输入法矩阵继续开放。源码尚未入current/candidate/Applications的293afe3包。完整四后端产品验收、账号与正式发行边界仍按总表保留。
