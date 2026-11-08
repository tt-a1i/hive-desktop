# Codex 设置应用确认与原始排队验证

任务 #7，相关 #6；不改变 Hive 协作协议、权限或全局 Agent 配置。

## 根因证据

原始 Codex 0.166.0 显式路径 /Users/tsk/.npm-global/bin/codex，模型固定 openai/gpt-6.1-terra。绕过 Hive 控制器的直接 RPC，8个独立进程/40次设置更新复现2次成功响应后读回旧值，一次第二读仍旧。先前一次回归失败保留，未用后续通过抹除。

官方 [app-server 文档](https://learn.chatgpt.com/docs/app-server)未描述实验 thread/settings/update 的完成语义；现场CLI生成schema确认 effort及thread/settings/updated。官方仓库 main 在 748ab55ca7de0b647d9a7a9d9a8e70e00f28666d 的 request_processors/turn_processor.rs:976-1027显示提交 Op::ThreadSettings(reply:None)后返回空响应，与实测“接受不等于应用”一致；main并非本机0.166.0构建来源，未冒称精确版本源码。

独立通知探针6进程30次不同值更新，收到应用通知后读回均匹配；6次同值重复不产生通知。因此不能对所有设置盲等通知，也不加入固定sleep或重复读到成功。

## 修改

Codex推理强度与模型切换复用同一应用确认入口：不同值先订阅本线程设置通知，再发更新，等待实际应用后读回；已是同值直接返回，避免永等不存在的通知。原生拒绝、进程退出或dispose会终止等待，30秒上限仅用于错误收敛。订阅和计时器在所有路径清理。模型更新校验原生实际模型身份。

## 检查

- 新 check:codex 中 codex-settings.ts 使用生产适配器与真实JSONL子进程，模拟响应先于应用，验证同值无通知、其他线程通知隔离、原生clamp实际读回、RPC错误及进程退出；旧适配器被 medium != high 断言稳定拒绝。没有mock传输或弱化断言。
- 原始CLI8进程/48次设置（含同值）通过新适配器及再次读回；源码可复现：HIVE_CODEX_COMMAND=<原始CLI> pnpm --filter @hive-desktop/runtime exec tsx checks/codex-settings-native.ts。初次脚本只因收据相对cwd路径错误未落盘，校准输出路径后完整重跑，不计初次为成功收据。
- check:external-reasoning实际Codex/Claude官方SDK合成对端、生产JSONL/冷恢复/写盘失败回归通过；既有check:codex、TypeScript与排队peer回归通过。原始/合成范围分列。
- 包内Codex gate加入相同设置行为检查；开发包补验另记。仅runtime逻辑变动，不重复Rust构建或无关全量测试。

## 原始 Codex 的显式用户排队

checks/queued-user-inputs-native-codex.ts --live 明确执行真实原始CLI/厂商Terra：真实工具sleep期间排入补充+文件，删除源文件；Stop只中断当前回合，冷runtime重开仍暂停，保留原deliveryId及附件。明确继续后原生工具读取Hive上传副本，模型返回只存在于文件内的随机marker，补充投递历史只有一条。无自动接受审批、模型fallback或Pi转述。

收据 .artifacts/queued-inputs-native-codex/receipt.json：chat ecb4c2e8-891c-4269-b0d0-8c0b084bc54f，thread 01a1baf7-7063-7f81-94e5-c7ae99fe9655，delivery da2cfd11-167d-4105-b2ff-ab0aa7548d61；实际工具记录2条。不是新包或物理GUI的Codex验收，Claude真实模型与Windows继续开放。

此过程中另记录一次Hive任务文件watcher ready 15000ms超时；不影响本次聊天/上传断言，但不能认定任务文件监听正常。保留日志并单列核查，不通过增加固定等待掩盖。其余安全收据及日志 .artifacts/codex-reasoning-readback/。
