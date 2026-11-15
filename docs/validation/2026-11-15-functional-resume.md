# 功能实施恢复核验

2026-11-15，桌面更新后恢复用户的完整 #39 目标。开发分支基线 `f3d9cdd`；本文不代表完整产品、发行包或所有平台已经验收。

## 合并与回归

- 问题附件 `927bbdb` 已合入为 `8d6bec0`。Codex 非保密问题使用私有上传副本的路径引用，独立保存问题草稿和附件收据；不是原生图片输入，也不额外发送普通消息。
- 严格 TypeScript 构建、97 项 Rust 测试（另 1 项忽略）、问题附件专项和完整 Codex 回归通过。专项使用真实 Hive 上传、独立受控子进程以及存储/传输故障注入；不把它当真实模型读取证明。
- Windows 历史入口夹具 `93c73e5` 已合入为 `f3d9cdd`。运行 `43662995040` 的构建、打包、解包功能检查与终端通过；最终 UIA 操作不能用共同祖先把 Tester 的责任正文与同名按钮唯一关联，故整组仍失败。后续 `b2d7ff6` 为责任动作补充成员、状态和职责的可访问名称，保留实际按钮调用、派单来源、独立草稿及返回派单断言；修复后 Windows 原生重跑待完成。

日志：`.artifacts/functional-parity/resume-node-build.log`、`resume-rust-test.log`、`resume-interaction-attachments.log`、`resume-codex.log`。

## 真实原生问题附件

冻结 `f3d9cdd` 的独立原生 Codex 0.169.0 / GPT-6.1 Terra 验收通过。模型实际发出结构化非保密提问并暂停，经新增生产 RPC 回复私有上传引用后，工具用 Python 读取文本和 PNG 副本；原件已删除，随机文本及两份文件的字节数和 SHA-256 均匹配。主代理另从磁盘独立核对了这两份副本。仅一次 turn/start、零 steer，耗时 15.961 秒；没有额外普通消息。冷开历史 ID 与附件收据保持，未自动回复或重放；全部 Runtime/CLI 已退出并核对 ESRCH。

PNG 仅证明字节读取，不证明视觉识别或原生图片输入。首次验证脚本错误要求命令包含完整拼接路径字面量，失败证据保留；修正验证时只做离线证据检查和冷读取，没有增加模型调用。目录：`.artifacts/functional-parity/native-question-proof/.artifacts/functional-parity/native-question/run-3DYoqh/`，包含 `receipt.json`、`original-harness-receipt.json`、`root-copy-readback.json`。GUI 与 Windows 原生问题回复仍待验收。

## 初始化脚本和绑定核验

暂停前的真实测试应用使用 `315112a` 原生程序和 `d6bc61e` 包内后台。新成员会话 `e3c18345-659f-49b0-8656-daaa15acd623` 的原始归属、Pi 后端、创建意图与脚本版本吻合。脚本在该工作树以退出码 0 完成，标记内容为 `gui-setup`，原生历史没有聊天消息。

恢复检查发现 ChatIndex 选择 B，但 Hive `agent_sessions.last_session_id` 指向 A。独立核实该夹具先停止 A、直接选择 B、打开 B，最后保存退出，未执行 B 的 Hive Worker 启动。因此 A 是最后实际启动的原生会话，B 是桌面选择，两者不能仅因不同就判为路由故障。

独立生产 API 检查覆盖激活 B、创建/打开未绑定 A、停止/重新启动 Worker、完整冷恢复，以及先激活历史 A 再激活 B 后打开 A：实际启动始终选择 B，目录、原生会话和排队保持；没有模型请求。未为此改写产品代码或 QA 数据库。证据：`.artifacts/functional-parity/binding-consistency/`。

随后通过实际界面关闭了测试 setup 角色。原生 AXPress 在按钮中心位于设置浮层之外时返回成功，但浮层关闭且数据库未变；将按钮聚焦并滚入可见区域后，界面显示“项目脚本已保存，尚未执行”。独立只读数据库确认新版本 `e95fc99a…` 的 `runOnWorktreeCreate=false`，关闭并重新打开设置仍为未启用。没有直接修改数据库，也没有重启应用，故不称冷启动验收。现有证据不足以确定离屏 AXPress 的内部派发原因；该辅助功能边界保留。证据：`.artifacts/functional-parity/ui-qa/resume-checks/`。

## 检查点与历史分叉

检查点模块修复了未跟踪 FIFO 被 Git 文件清单遗漏，以及嵌套 FIFO `.gitignore` 可能先被 Git 打开的问题。独立复验确认在读取前拒绝；已随 `44ae0e8` 合入开发分支。Root 的检查点四组验证通过，覆盖真实 HEAD A / index B / disk C、特殊文件、存储损坏、源变化、失败与冷恢复。部分不支持的索引、过滤器和文件系统状态明确拒绝。

原生历史分叉、精确边界、继承产物来源、共享执行门禁及必要用户操作也已合入 `44ae0e8`。Root 完整 `check:history` 和 Rust 101 项测试（另 1 项忽略）通过；这是实际 Hive/SQLite/Git、受控原生对端与故障恢复证据。独立审查发现后续中断回合会阻止分叉更早的完整回合，修复中。真实 CLI/Terra、GUI、Windows 和打包仍待新增证明，详见[分叉范围](2026-11-15-native-history-forks.md)。原地回退、跨后端接续、多实例/ACP、更新、语音、隔离 HTML、密码库和完整平台验收持续开放。

current/candidate 仍为 `837abd1`，没有替换用户已安装应用。Claude/Antigravity 不做账号和真实模型验收；Pi 登录错误不重试；真实 Agent 模型验证限定为原生 Codex GPT-6.1 Terra。

## 后续包与监督测试

未推广的 `portable-6df2214` 使用重新优化构建、锁定生产依赖和官方校验通过的 Node 22.23.3。核心包检查、包内 Codex 生命周期及 22 项功能检查通过，其中新增了问题附件的实际包内副本/协议/故障检查。它仍不是完整桌面交付，也未替换 current/candidate。

新 Windows 运行 `43662995040` 的独立监督测试在编译阶段发现协议引用 UI 模块的初始化类型。修复 `121080e` 将三个纯序列化类型移入共享协议，保持监督入口和断言；Root 的同一独立 Cargo 命令通过 14 项测试、1 项忽略。Windows 修复后原生重跑仍待安排。该运行最终完成，但原生 UIA 步骤失败，不能记为整个平台验收通过。

清理了已结束的账号额度开发工作树中可再生的 `target-clone` Rust 缓存，保留源码、证据、安装包与回退包。清理收据位于 `.artifacts/functional-parity/cleanup-20261115-resume.json`；不能把目录的逻辑大小当作实际释放磁盘量。
