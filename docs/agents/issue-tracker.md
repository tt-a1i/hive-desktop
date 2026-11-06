# Issue tracker: GitHub

本项目的需求、规格和任务记录在 tt-a1i/hive-desktop 的 GitHub Issues。
使用 gh CLI；写入前用 gh repo view --json nameWithOwner 核对目标仓库。
显式指定仓库时使用 -R tt-a1i/hive-desktop。

## 常用操作

- 创建：gh issue create --title "<标题>" --body-file <正文文件>
- 读取：gh issue view <编号> --json number,title,body,labels,comments
- 列表：gh issue list --state open --json number,title,body,labels,assignees
- 评论：gh issue comment <编号> --body-file <正文文件>
- 标签：gh issue edit <编号> --add-label "<标签>" 或 --remove-label "<标签>"
- 关闭：先评论说明，再执行 gh issue close <编号>。

多行正文先写入临时文件，再通过 --body-file 传入。

## 层级与依赖

- 子任务：创建时使用 --parent <父编号>，或编辑父任务时使用 --add-sub-issue <子编号>。
- 阻塞关系：创建时使用 --blocked-by <编号>，或编辑时使用 --add-blocked-by <编号>。
- 原生功能不可用时，使用正文中的 Part of #<编号>、Blocked by: #<编号>，并在父任务中维护任务列表。
- 所有阻塞任务关闭后才视为解除阻塞。可查询 issue_dependencies_summary.blocked_by，或读取 blockedBy 并仅统计 OPEN 状态。

## Pull requests as a triage surface

**PRs as a request surface: no.**

## 技能约定

- “publish to the issue tracker”：创建 GitHub issue。
- “fetch the relevant ticket”：读取对应 issue。
- /wayfinder 使用一个标记 wayfinder:map 的父 issue 保存 Notes、Decisions-so-far、Fog。
- 子任务使用原生 sub-issue，标记 wayfinder:research、wayfinder:prototype、wayfinder:grilling 或 wayfinder:task；相关标签在使用时按需创建。
- 按父任务顺序选择未关闭、无未关闭阻塞且未分配的子任务；认领时分配给执行者。
- 解决后评论结论、关闭子任务，并在父任务 Decisions-so-far 中追加结论和链接。
