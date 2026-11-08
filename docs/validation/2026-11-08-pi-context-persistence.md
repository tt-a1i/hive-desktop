# Pi 非回合上下文与原生写入故障

后续检查 [历史/投递状态修复](2026-11-08-pi-history-persistence.md) 的所有调用入口，真实复现 `addContext` 写盘失败后仍可继续写入原句柄。首轮补齐非回合方法后，又用自动 dispatch 输入复现相同遗漏；两个修改前日志保存在 `.artifacts/pi-context-persistence/before.log` 和 `dispatch-before.log`。

## 修复边界

内置 Pi 共用一次性的原生变更失败处理：上下文追加、模型切换/刷新时的原生模型写入、推理强度写入，以及应用投递状态。失败后不允许继续修改该句柄，由已有 owner 清理，从磁盘重新打开。原生回合自己的文件写入异常以 Error 的 path/syscall/code 确认属于本会话 JSONL，不根据 message 字符串猜测，也不把普通模型/扩展错误视为存储故障。

回合内的故障通知移到 prompt.finally 结算后；非回合修改另排通知，避免在仍执行的 SDK 回合上提前清理。模型配置的读取/解析失败不走此写入故障分支，用户修复 JSON 后可以在同一健康句柄刷新。

## 实际验证

新增 `checks/pi-context-persistence.ts`，不替换 SDK、文件系统或存储：

- 对自建实际 JSONL 设置只读，用真实 SDK 运行 context/model/reload/reasoning/dispatch 五条路径；验证原始 OS EACCES、原文件未变、失败句柄拒绝继续写、owner 仅收到一次通知。
- 恢复权限后重新打开真实会话，成功追加上下文；实际记录没有失败内容、没有不存在的 parent，成功内容只有一份。
- 临时破坏自建 models.json 再修复，验证读取错误不会冻结健康写入句柄。
- 穿透真实 Hive HTTP/SQLite 与 context-only spool：失败仍保留同一队列项及诊断，owner 释放旧 SDK，修复权限后重试成功，完整 runtime 冷重开后上下文仅一份、队列为空。此路径不启动 Agent 或模型回合。

`check:pi-models`、`check:pi-reasoning`、`check:team` 和新增检查通过，runtime 构建通过。模型/推理相关检查均使用 gpt-6.1-terra 的本地配置或拒绝/合成回环服务，无厂商推理。没有改全局认证/模型默认，也没有新增权限审批或依赖。没有删除测试，纯 UI 未改；不重复无关全量检查或未变的 Rust/Windows 交叉构建。

包内 Pi gate 纳入同一检查，POSIX 权限注入不作为 Windows ACL 验收。源码修复与最新已打包版本须分开核对，开发包记录以其源提交和收据为准。

## 已刷新开发包

`.artifacts/hive-desktop-macos-arm64-pi-write-guards.zip` 纳入源码 `bde9d6c80ca96df65c16d49e48de6708b5cfb377`，仍为未签名/未公证的 Mac arm64 开发 ZIP。SHA256：`b0dead1908b0c37dd64045c834631e44eab5d0b2ce16c498e13582702b5b9789`。

从上一份已验证 Pi 开发包复用相同冻结依赖与 Node，校验元数据和基础 ZIP 后完整刷新 runtime。106 个 runtime 文件与源码构建及新 ZIP 独立解压文件匹配，只有 agents/pi.js 和 map 相对前包变化；生产二进制哈希相同。包内五组 Pi 检查、真实 PTY/Hive/SQLite 和实际窗口启动恢复通过。

上一份 47ac3de 开发包通过前四组 Pi 检查，但被新增上下文写入检查的真实“失败后仍可复用句柄”断言拒绝；新包相同检查通过，负向日志在 `.artifacts/pi-write-guards-package/old-package-negative.log`。新包所有记录见同目录 package-receipt.json 与 gate 日志。

没有重复未改的 Codex 二进制/协议或整套系统鼠标/输入法矩阵；上一包的命令选择实际系统操作记录仍单独保留，不当成新包重测。Windows 原生/ACL 和正式安装仍待完成。
