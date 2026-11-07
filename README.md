# Hive Desktop

Hive 桌面客户端，复用 Hive Web 已有的协作能力，为内置 Pi 和外部 Agent 提供统一聊天与团队工作界面。

## 当前状态

2026-11-08：采用 GPUI + gpui-kit 构建 macOS / Windows 原生界面，复用 Hive Node 后台。macOS 已分别跑通内置 Pi、外部 Codex 和外部 Pi 主控创建 Worker、派单、文件执行、显式汇报、主控核验及会话恢复。外部 Pi 的范围见 [团队验证](docs/validation/2026-11-07-external-pi-team.md)，Codex 见 [原生接入验证](docs/validation/2026-11-07-codex.md)。成员管理、Claude 官方 SDK 接入、账号与模型配置、全局置顶和便携开发包已加入；Claude 真实模型协作、完整配置、界面打磨和跨平台正式发行仍在实施。

项目位置：`/Users/tsk/code/hive-desktop`。

私有开发仓库：<https://github.com/tt-a1i/hive-desktop>。

## 已确认的方向

- 以 Hive Web 当前能力为功能基线，复用团队、派单、消息、汇报、任务与配置逻辑。
- 内置 Agent 基于 Pi，首版不新增权限审批流程。
- 内置 Pi 和已接入的外部 Agent 均可担任 Orchestrator 或 Worker；选择外部主控时，由该 Agent 直接决策和处理汇报。
- 首版提供统一聊天界面，包括消息流、工具执行和成员会话；终端作为辅助能力。
- 接入层允许原生 SDK、原生 RPC 和 ACP 并存，具体后端按实际能力选型。
- UI、配置体验以 Cindy 为主要参考，结合 Raven 和 Maka 的合适设计；组件库与工程细节由实施者选择。
- 桌面界面使用 Rust、GPUI 与 gpui-kit；macOS 和 Windows 均为首版目标。保留 Node 后台，避免因 UI 选型重写已验证的协作业务。
- 在本目录独立推进桌面项目，现有 Hive 仓库作为复用来源。

## 开始工作

先读 [实施方案](docs/implementation-plan.md)。后续实现前，应刷新并记录 Hive 权威仓库的基线，再决定具体模块的复用方式。

- Hive 权威仓库：<https://github.com/tt-a1i/hive>
- Hive 本地源码：`/Users/tsk/code/hive`
- 协作规则：Hive 仓库的 `CLAUDE.md`、`AGENTS.md` 与 `docs/superpowers/specs/2026-09-05-task-collaboration.md`。

需要 Node 22.19+（22 LTS 或 24+）、pnpm 10.30.3 和 Rust 1.98.0。

```sh
pnpm install
pnpm dev
```

应用使用独立的桌面数据目录，读取 Pi 的现有模型与认证配置。“设置 → 内置 Pi”支持 SDK 原生订阅登录、API 多步配置、提供商搜索和移除本机登录信息，复用 Pi 凭据；“设置 → 模型与接口”可添加兼容接口、编辑自定义模型及能力，保留 Pi 配置中的高级字段；见 [模型配置验证](docs/validation/2026-11-08-pi-model-config.md)。桌面内切换会话模型不会改写 Pi 的全局默认。运行方式与环境变量见 [桌面应用](apps/desktop/README.md)。

```sh
pnpm check
pnpm check:hive
pnpm --filter @hive-desktop/runtime check:runtime
pnpm --filter @hive-desktop/runtime check:team
```

这些检查使用临时数据目录，其中团队集成会运行真实 Hive HTTP / SQLite、Pi SDK 和 team CLI，但暂停模型执行。真实模型验证固定使用 Pi 原生 OpenAI / ChatGPT 接入的 `gpt-6.1-terra`，不依赖 Magpie；见 [协作验收记录](docs/validation/2026-11-07-structured-team.md)。

图片选择/粘贴/拖放、原图查看、复制/保存、图片单独发送、附件草稿和历史恢复已接入现有 Hive 用户消息及投递队列，验证边界见 [图片附件](docs/validation/2026-11-08-image-attachments.md)。真实 Terra 识图本轮因 OpenAI OAuth grant 失效未通过，系统保存对话框、通用文件、长图片历史性能和 Windows 原生体验仍待验收，见 [图片输入验证](docs/validation/2026-11-08-image-input.md)。

进展与剩余范围见 [进度记录](docs/progress.md)，实际验收证据见 [基础阶段收据](docs/validation/2026-11-07-foundation.md)。早期 [选型原型](experiments/gpui-chat/README.md) 仅用于保留选型证据，正式应用位于 `apps/desktop`。
