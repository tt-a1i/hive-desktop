# 外部 Agent 推理设置与 Codex 分页历史

日期：2026-11-08。完整桌面产品仍在实施。本记录分别列出原生 CLI、合成对端和 GPUI 夹具的证据，不代表厂商推理或跨平台体验验收。

## 真实问题与可观察变化

1. 用户使用外部 Codex 或 Claude 聊天时，原先只有 Pi 能在输入框调整推理强度；同一原生历史线程使用分页契约时还会被桌面拒绝恢复。
2. 四种后端现在共用会话强度入口，显示原生能力及实际生效值；显式选择可重开恢复。分页线程通过原生分页接口恢复完整项目和消息身份。
3. 不做会迫使用户转去 CLI 调节，且已有分页历史无法在桌面恢复，不能满足统一聊天目标。

## 实现边界

- Codex 0.166.0：`model/list` 提供档位；`thread/settings/update` 写 effort；`thread/read includeTurns:false` 读取实际 reasoningEffort。空线程不能用 includeTurns:true 读设置。
- Claude SDK 0.3.317：`applyFlagSettings({effortLevel})` 仅修改当前会话层；`getSettings().applied` 读取实际模型/effort。固定版本的公开 JS runtime 有 getSettings，但 Query 的类型声明遗漏它；适配器采用一个局部、有能力检查的窄类型桥，没有调用私有 request/transport。后续升级需重新验证这个边界。
- 不硬编码厂商档位，不提供会误导用户的“恢复全局默认”按钮。原生空值显示“原生配置”。Claude 的组织、环境、模型限制可能使请求 high 实际生效 medium，界面显示 medium。
- Pi 沿用原生会话。Codex/Claude 的显式请求按 provider/model 保存在现有聊天索引，重开仅应用匹配模型；实际换模型清除，重选当前模型保留。没有新 store/schema，也没有全局默认或权限设置更改。
- 旧档位退休、读取失败时仍可打开会话，但发送前要求重新选择。恢复 setter 部分失败则释放并重开一次，保留错误状态，避免重复重试或沿用不确定配置。索引写盘失败同样退休原句柄，修复后重新读取/选择。
- 配置错误由 RPC/弹层处理，后台记录原始诊断；不再重复以原始文件系统错误污染其他会话的 Agent 错误横幅。意外进程退出仍走原错误事件。
- 不承诺原生配置与桌面索引跨文件事务。Codex 原生空线程/会话层配置不一定持久，因此桌面显式覆盖承担重开恢复。

## 验证

本机命令显式指定 `HIVE_CODEX_COMMAND=/Users/tsk/.npm-global/bin/codex`，避免 PATH 上旧 0.46.0；Claude 原始 CLI 为 2.1.317。新检查默认使用通用命令名，不嵌入此机器路径。

- `check:external-reasoning`：真实 Hive HTTP/SQLite、原始 Codex、官方 Claude SDK + 明确合成 CLI 对端。覆盖能力、严格字段/档位、同模型重选、空会话重启、真实索引文件改目录失败、原生拒绝、实际档位钳制、退休档位的读取/阻止发送/修复；无模型请求。合成对端只收到 effortLevel，未发 query。空会话案例不冒充非空历史证据。
- `external-reasoning-native-probe.ts`：原始 Codex 选择 Terra 后读取/改 high；原始 Claude 仅元数据控制，保持现有模型，不选择其他模型、不发用户消息，读取 high → low → 恢复 high。Claude 当前模型元数据为 claude-4.8-opus-google；本项不是 Claude 或 Terra 推理。
- `codex-reasoning-history.ts --receipt ../../.artifacts/codex-native-ui-proof/receipt.json`：仅恢复先前自有 Terra 验证线程；真实分页历史恢复 6 条消息，改 low 后原生重开，历史摘要/消息 ID 完全一致，原工作文件不变，最后恢复原 medium。原生重启后 effort 本身回到 medium，进一步说明桌面显式覆盖的必要性。没有新 prompt/tool/厂商推理。
- `check:codex` 新增两页 JSONL 合成子进程：验证完整分页、原始 ID、顺序、重启一致，拒绝 summary 冒充 full；原有传输、归一化、退出、交互检查保留。
- 解压后的便携包使用自带 Node/runtime，通过 `external-reasoning-wire.ts` 验证四种相关 JSONL 请求、Codex/Claude 两次重开后的 high、严格空值拒绝和无模型事件；另通过真实 PTY/HTTP/SQLite smoke 及原生窗口恢复启动检查。ZIP SHA-256 为 `3bd80b3431392a5712dc1267f1c99dc781dbfebffde9205a35c28254849f8d6c`，二进制为 `35d271bb41a6f3a8269400665720591a4afd4c28cceeb7e7a3896ba717752f1f`，解压后 runtime JS 与当前 dist 逐文件一致。该包未签名/公证。
- `check:pi-reasoning`、`check:claude-interactions`、`structured-runtime.ts`、`team-runtime.ts` 定向回归；Pi 检查把已失效的“Codex 不支持”断言替换为缺失会话及同模型不重置检查，其余真实 SDK/RPC/磁盘/忙碌状态检查保留。
- GPUI 浅色、深色、760px 窄窗各跨两次真实应用进程启动；原生布局坐标的注入鼠标事件验证 Codex 设置、真实写盘失败/重新读取/重试、Claude 合成钳制 readback、跨会话中文草稿保留及恢复。失败后断言没有残留的全局 Agent 错误。不是物理系统输入，不是 Windows 窗口证明。
- 默认 Rust 检查 18 通过 / 1 fixture 忽略；Mac 生产构建、Windows x64 MSVC 默认功能 cargo check 通过。没有无差别全量运行 Node 检查；仅修改相关配置与历史恢复路径，运行上述定向回归。

本地证据位于 `.artifacts/external-reasoning/`：core-receipt.json、native-probe.json、native-history.json、三种窗口日志/截图、fixture-source，以及定向检查日志。夹具源保留实际执行版本，生产 GPUI vendor 无新增夹具钩子。

## 未完成的验收

原始 Claude 模型协作/实际限制下推理、Terra 新请求（Pi OAuth 失效）、Windows 新界面的原生运行、系统物理输入、正式签名/安装/发行和 Cindy 全流程体验仍待完成。分页历史此次真实线程只有一回合，多页证据来自合成子进程；大型真实分页线程的时延和内存仍未测量。

界面待打磨：共用传输层目前仍把 `DESKTOP_ERROR` 代码及 JSON 引号拼进错误文案，窄窗错误截图可观察；本轮未将其算作 Cindy 级错误呈现。
