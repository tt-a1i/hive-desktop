# Hive Desktop

Hive 桌面客户端，复用 Hive Web 已有的协作能力，为内置 Pi 和外部 Agent 提供统一聊天与团队工作界面。

## 当前状态

2026-11-06：已创建独立项目目录，整理了已确认方向和实施顺序。当前仅有项目文档，尚未包含应用代码、依赖或可运行安装包。

项目位置：`/Users/tsk/code/hive-desktop`。

私有开发仓库：<https://github.com/tt-a1i/hive-desktop>。

## 已确认的方向

- 以 Hive Web 当前能力为功能基线，复用团队、派单、消息、汇报、任务与配置逻辑。
- 内置 Agent 基于 Pi，首版不新增权限审批流程。
- 内置 Pi 和已接入的外部 Agent 均可担任 Orchestrator 或 Worker；选择外部主控时，由该 Agent 直接决策和处理汇报。
- 首版提供统一聊天界面，包括消息流、工具执行和成员会话；终端作为辅助能力。
- 接入层允许原生 SDK、原生 RPC 和 ACP 并存，具体后端按实际能力选型。
- UI、配置体验以 Cindy 为主要参考，结合 Raven 和 Maka 的合适设计；组件库与工程细节由实施者选择。
- 在本目录独立推进桌面项目，现有 Hive 仓库作为复用来源。

## 开始工作

先读 [实施方案](docs/implementation-plan.md)。后续实现前，应刷新并记录 Hive 权威仓库的基线，再决定具体模块的复用方式。

- Hive 权威仓库：<https://github.com/tt-a1i/hive>
- Hive 本地源码：`/Users/tsk/code/hive`
- 协作规则：Hive 仓库的 `CLAUDE.md`、`AGENTS.md` 与 `docs/superpowers/specs/2026-09-05-task-collaboration.md`。

当前没有开发或构建命令；应用骨架落地后再补充真实可运行的命令。
