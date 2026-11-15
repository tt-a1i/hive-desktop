# 功能实施恢复核验

2026-11-15，桌面更新后恢复用户的完整 #39 目标。开发分支基线 `f3d9cdd`；本文不代表完整产品、发行包或所有平台已经验收。

## 合并与回归

- 问题附件 `927bbdb` 已合入为 `8d6bec0`。Codex 非保密问题使用私有上传副本的路径引用，独立保存问题草稿和附件收据；不是原生图片输入，也不额外发送普通消息。
- 严格 TypeScript 构建、97 项 Rust 测试（另 1 项忽略）、问题附件专项和完整 Codex 回归通过。专项使用真实 Hive 上传、独立受控子进程以及存储/传输故障注入；不把它当真实模型读取证明。
- Windows 历史入口夹具 `93c73e5` 已合入为 `f3d9cdd`。按 Tester 的唯一责任正文定位原成员历史按钮，并以派单来源及独立草稿核对目标，保留既有历史与草稿断言。新 Windows CI 已启动，尚未取得其原生成功结果。

日志：`.artifacts/functional-parity/resume-node-build.log`、`resume-rust-test.log`、`resume-interaction-attachments.log`、`resume-codex.log`。

## 真实原生问题附件

冻结 `f3d9cdd` 的独立原生 Codex 0.169.0 / GPT-6.1 Terra 验收通过。模型实际发出结构化非保密提问并暂停，经新增生产 RPC 回复私有上传引用后，工具用 Python 读取文本和 PNG 副本；原件已删除，随机文本及两份文件的字节数和 SHA-256 均匹配。主代理另从磁盘独立核对了这两份副本。仅一次 turn/start、零 steer，耗时 15.961 秒；没有额外普通消息。冷开历史 ID 与附件收据保持，未自动回复或重放；全部 Runtime/CLI 已退出并核对 ESRCH。

PNG 仅证明字节读取，不证明视觉识别或原生图片输入。首次验证脚本错误要求命令包含完整拼接路径字面量，失败证据保留；修正验证时只做离线证据检查和冷读取，没有增加模型调用。目录：`.artifacts/functional-parity/native-question-proof/.artifacts/functional-parity/native-question/run-3DYoqh/`，包含 `receipt.json`、`original-harness-receipt.json`、`root-copy-readback.json`。GUI 与 Windows 原生问题回复仍待验收。

## 初始化脚本和绑定核验

暂停前的真实测试应用使用 `315112a` 原生程序和 `d6bc61e` 包内后台。新成员会话 `e3c18345-659f-49b0-8656-daaa15acd623` 的原始归属、Pi 后端、创建意图与脚本版本吻合。脚本在该工作树以退出码 0 完成，标记内容为 `gui-setup`，原生历史没有聊天消息。

恢复检查发现 ChatIndex 选择 B，但 Hive `agent_sessions.last_session_id` 指向 A。独立核实该夹具先停止 A、直接选择 B、打开 B，最后保存退出，未执行 B 的 Hive Worker 启动。因此 A 是最后实际启动的原生会话，B 是桌面选择，两者不能仅因不同就判为路由故障。

独立生产 API 检查覆盖激活 B、创建/打开未绑定 A、停止/重新启动 Worker、完整冷恢复，以及先激活历史 A 再激活 B 后打开 A：实际启动始终选择 B，目录、原生会话和排队保持；没有模型请求。未为此改写产品代码或 QA 数据库。证据：`.artifacts/functional-parity/binding-consistency/`。

辅助功能操作未确认成功保存“关闭测试 setup 角色”；只读数据库仍显示启用。未将按钮操作成功等同于业务保存成功，未直接修改数据库制造界面验收。该测试配置清理及实际界面继续核验，不能标完整。

## 检查点与历史分叉

内部检查点模块 `958146d` 修复了未跟踪 FIFO 被 Git 文件清单遗漏的问题。独立实际文件系统检查确认原缺陷已经拒绝捕获；同时发现嵌套 FIFO `.gitignore` 仍可能先被 Git 打开并阻塞至超时。它不会产生 ready 检查点或修改源数据，但仍需要前置拒绝，当前未合入主开发分支。

代码检查点和用户可用分叉是两个范围。原生历史分叉、精确边界、继承产物来源、共享执行门禁及用户操作仍在独立分支实施。原地回退、跨后端接续、多实例/ACP、更新、语音、隔离 HTML、密码库和完整平台验收持续开放。

current/candidate 仍为 `837abd1`，没有替换用户已安装应用。Claude/Antigravity 不做账号和真实模型验收；Pi 登录错误不重试；真实 Agent 模型验证限定为原生 Codex GPT-6.1 Terra。

## 后续包与监督测试

未推广的 `portable-6df2214` 使用重新优化构建、锁定生产依赖和官方校验通过的 Node 22.23.3。核心包检查、包内 Codex 生命周期及 22 项功能检查通过，其中新增了问题附件的实际包内副本/协议/故障检查。它仍不是完整桌面交付，也未替换 current/candidate。

新 Windows 运行 `43662995040` 的独立监督测试在编译阶段发现协议引用 UI 模块的初始化类型。修复 `121080e` 将三个纯序列化类型移入共享协议，保持监督入口和断言；Root 的同一独立 Cargo 命令通过 14 项测试、1 项忽略。Windows 修复后原生重跑仍待安排；该运行的主构建在核验时尚未终止，不能记为新平台通过。
