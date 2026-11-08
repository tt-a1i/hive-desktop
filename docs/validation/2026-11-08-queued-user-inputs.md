# 回复期间排队补充消息

任务：[#6](https://github.com/tt-a1i/hive-desktop/issues/6)。完整产品、Windows 与系统操作矩阵继续实施。

## 行为

Agent 正在回复时，停止按钮保留，发送入口显示“排队”，当前发送快捷键也可排队。补充消息沿既有 Hive 用户输入、来源元数据与持久 spool 按顺序处理，不中断当前回合，不实现厂商 steer 或另建任务账本。附件可在回复期间添加，提交 RPC 未返回时仍防重复提交和附件修改。

输入区显示待处理用户原文、附件名及复制入口。state 和 chat.open 新增 pendingUserInputs，来源仅为显式 user，不返回隐藏的执行文本、自动派单或汇报。已在聊天出现或正在处理的输入不再重复显示为等待项；运行中不重复显示包含当前回合的旧总计。暂停或失败时仍需显式继续/重试，排入新消息不偷偷恢复暂停队列。

复用既有提交中的输入副本及失败恢复；错误不会清掉原本运行中的状态。提交中的纯文本也参与现有草稿 flush（此前只有带附件的提交进入该路径）。输入框沿原交互在提交时清空，后台确认前保留提交副本；失败恢复原文或保留在失败草稿中。不承诺跨 UI 草稿和后台投递的恰好一次发送，未确认提交仍须核对历史与队列。

## 验证

- `pnpm --filter @hive-desktop/runtime check:queued-inputs`：真实 Pi SDK、Hive HTTP/SQLite 和文件 spool，合成回环 Responses HTTP；测试配置固定 gpt-6.1-terra，无厂商推理。保持首个请求未响应，后续原文与附件可接受、只排队；不存在的附件提交被拒绝，原执行仍忙碌且队列不变。
- 自动 system 投递不会出现在 pendingUserInputs；事件与快照中的稳定 deliveryId/原文/附件一致。停止只丢弃中断的当前投递，后续输入保持；完整 runtime 冷重开仍暂停，明确继续后按原 ID 唯一进入历史。四次回环请求分别涵盖被中断的当前回合、补充输入、显式测试的 system 投递和冷启动恢复，不冒称全部为用户消息或真实模型回答。
- 浅色 760 宽窗口、深色常规窗口的原生 GPUI + 真实 runtime，分别验证：Enter 发首条、忙碌中排队按钮、Enter 排下一条、同帧连续 Enter 不重复、独立停止、两个排队项冷恢复、继续处理和各自历史仅一条。共四个进程正常零退出。
- 窗口验证另构造先前已验证但后台已无记录的附件引用，通过真实配置/上传校验拒绝提交；输入恢复、当前回复继续。首次夹具引用未标记已验证，被正确的前端 guard 拦住；随后夹具私有字段访问未编译却误运行旧二进制，相关尝试作废。修复夹具访问方法，并改为编译失败即停止后重跑全部。对原窗口恢复方式的额外改动已撤回，没有证据证明它有卡死 bug。
- `check:team`、`check:delivery-failures`、TypeScript check、已有草稿 3 项检查及 Mac 生产 cargo build 通过。既有队列写盘、停止和重试语义保留，没有改 Hive 协作协议、schema、厂商接入或模型默认。
- 最后重新构建生产程序，替换根目录输出中的验证专用二进制。相关收据、回环与窗口日志/截图：`.artifacts/queued-inputs/`。

## 边界

本次原生窗口检查为复制源码的显式帧驱动，不代表物理鼠标/系统输入法验收。Mac 锁屏仍阻止前一模型覆盖包的系统操作，该 owned 应用保持等待用户解锁；没有重复请求或重启。当前开发 ZIP 尚未纳入排队 UI；其版本和证据继续以模型覆盖包记录为准。

公共队列适用于四种后端；初轮实际穿透范围是内置 Pi，后续外部后端证据及界限见下文。Windows 原生未验收，CI 按用户要求暂不处理。不新增撤回、编辑、重排能力或权限流程。


## 外部后端补验

- `check:queued-inputs-external-pi`：复用同一实际队列/SDK/HTTP 检查，以原始 Pi 1.4.0 RPC 子进程运行；CLI 来源为 `/Users/tsk/Library/pnpm/pi`，版本与 launcher 哈希见 `.artifacts/queued-inputs-external/cli-version.json`。独立临时 Pi 配置、离线目录与 gpt-6.1-terra 回环模型，未使用 Magpie 或真实凭据。
- 外部 Pi 已通过忙碌中接受补充原文/附件、拒绝无效附件不影响原运行、自动来源过滤、停止只取消当前项、冷重开保持暂停、明确继续后原 ID 唯一消费。浅色 760 宽及深色常规原生 GPUI 另验证按钮/快捷键、连续 Enter、提交失败恢复、两条排队项的停止/冷恢复/顺序继续；四个应用进程均零退出。
- `check:queued-inputs-peers`：Codex 生产适配器连接真实 JSONL 子进程，Claude 使用官方 SDK 连接合成 CLI；实际 Hive HTTP/SQLite、真实 spool/历史文件。两种接入均验证当前原生回合未结束时第二条不执行、附件与来源事件、错误不清原执行、停止/冷重开暂停，以及明确继续后完成 ID 和历史各一份。没有替换生产适配器、Hive 或队列。
- 以上 Codex/Claude 子进程是明确标记的协议模拟端，返回合成响应。它们只证明本次排队接线和持久化行为，不代表原始 Codex/Claude CLI 或厂商模型实际执行。合成端元数据标为 gpt-6.1-terra，不代表 Claude 厂商提供该模型。
- 初版 Codex 模拟端缺少已要求的 historyMode，被生产适配器正确拒绝；补齐既有 legacy 契约后重跑两种接入通过，没有放宽生产解析。
- 既有 `check:queued-inputs` 与 TypeScript check 回归通过；显式指定 `/Users/tsk/.npm-global/bin/codex` 0.166.0 后，`check:external-reasoning` 的复跑通过，但保留了首次读回旧值的失败，见下文；Claude 合成端的自动完成为显式可选控制，原推理设置检查仍保留默认等待行为。最后再次构建生产程序。

本轮没有修改生产逻辑，也没有重复不相关全量测试。模型与 SDK、原始 CLI、协议模拟和 GPUI 帧驱动分别记录；物理输入、真实 Codex/Claude 新功能与 Windows 验收继续开放。收据和日志在 `.artifacts/queued-inputs-external/`，仍未将排队功能纳入新的开发 ZIP。


### 本机 Codex 回归的失败与界限

默认 PATH 的 `/opt/homebrew/bin/codex` 是 0.46.0，正确地被当前 transport 所需的 `--listen` 参数拒绝；没有修改全局 CLI 或增加不明兼容分支。已有文档指定的 `/Users/tsk/.npm-global/bin/codex` 经现场确认仍为 0.166.0。

显式 0.166.0 的首次设置 high 后，生产读回返回 medium，既有断言失败。随后同命令完整复跑通过；额外只记录、不替换请求结果的控制探针也通过。独立原生 RPC 探针读取三次均为 high，并收到 effort=high 的 thread/settings/updated 通知。现场 CLI 生成的 experimental JSON schema 仍声明 effort 字段有效。没有把失败改成“允许 medium”，也未靠重复到成功抹掉这个失败。根因仍待核查，不能把一次通过视为该不一致已修复；原始失败与探针在 `.artifacts/queued-inputs-external/`。
