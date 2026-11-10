# 成员修复、导出阅读与取消失败修复开发包

2026-11-10；源码 `ecad294232fb93493d9c1e1393559d6f61d06851` 的优化release，已更新 current/candidate。

- ZIP：`.artifacts/hive-desktop-macos-arm64-current.zip`；candidate为同字节硬链接。
- 143,419,755字节；SHA256 `14c86c59a37e80bb307477575aff9fde6014873da2f9ee97b867a6bc09ef1cec`。
- 原生程序SHA256 `473a0a3f0c982aa8273726e944860e31ebc50a24c5b9ba33b04b847810212494`。
- 实际解压应用 `.artifacts/release-ecad294/extracted/bundle/Hive Desktop.app`。
- 收据 `.artifacts/release-ecad294/receipt.json` 与 `.artifacts/current-package/receipt.json`。

## 内容和检查

纳入外部Pi取消失败回收/单次错误通知、Claude中断拒绝时等待所属CLI退出、导出预览正文/说明/选择、成员目录失败的对应后端修复/重读。具体根因及源验证见本日四份对应记录。

`cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` 与 runtime build通过。相对6148801仅原生程序、claude.js/external-pi.js及两份源码映射不同。最初组装允许列表漏列两份map，校验拒绝，修正精确允许列表后重新组装；失败记录保留，没有发布失败归档。

锁文件、patch、Cargo配置和生产依赖元数据精确不变；runtime package.json只有检查脚本变化，比较时只排除scripts字段，生产字段完整比较。保留原生产依赖，未声称新production deploy。112个runtime文件与当前dist匹配，解压28,863个内容/权限/链接条目与组装清单相同，29个链接均在包内。

包内Node执行六组检查通过：check-package、check-packaged-models、check-packaged-dispatches、check-packaged-codex、check-packaged-remote、check-packaged-claude。额外将原两个取消失败检查绑定到包内dist、用包内Node运行，真实JSONL子进程/官方SDK与真实Hive HTTP/SQLite验证取消失败回收、待处理输入原ID冷恢复、Claude显式继续各完成一次等；合成拒绝对端，无厂商模型推理。见 `gate-results.json` / `cancellation.log` / `check-cancellation.mjs`。没有为了过门槛改产品语义或扩展超时。

## 实际包窗口

两套独立自有状态副本，无HIVE_DESKTOP_NODE/RUNTIME源码覆盖，实际使用包内Node/runtime。

1. English/Dark20，13名停止成员。真实无效Codex路径导致目录错误；从错误进入对应配置，直接输入完整替换路径，Tab到可见Save、Return保存，Escape返回同成员/筛选/模型面板，显式Reload读取真实10模型目录；直接输入Terra搜索，仅对应模型可见。正常CmdQ、exit0。未Start或改选模型。
2. English/Dark16，合成导出历史5记录。窗口实际显示会话标题/固定快照范围、可读正文及10000字符预览说明；聚焦预览End滚至末尾。复制完整快照后退出弹层，粘到自有另一会话未发送草稿，正常CmdQ、exit0。冷读33,333字节逐字等于先前已验收的默认完整Markdown，不只复制预览；原会话未发送草稿精确保留，两条原生Pi历史逐字不变。
3. 包内runtime独立冷读：13成员与种子记录完全一致、原hive:codex/openai/gpt-6.1-terra launch不变，只有自有Codex程序路径按操作更新；两套SQLite messages/dispatches/dispatch_messages/agent_runs/workflow_runs均0。`cold.mjs` / `native-receipt.json`。

本批未新增真实模型请求。原Codex Terra真实团队/文件/图片/队列证据按原记录保留，不冒充本包重新跑过；本次后台已变化，因此额外执行上述包内取消失败与既有六组回归。

## 保留与边界

旧6148801的两个ZIP别名、收据归档至其release目录，旧可运行应用及验证状态保留。确认本批组装base无lsof引用后仅删重复base，回收538,427,392字节（约513.5MiB）；不删旧聊天工作目录或运行中的依赖。

未签名/未公证的macOS ARM64开发包。Windows源码目标检查已有通过，Windows原生环境缺失仍未验收。完整引导、所有后端真实厂商交互、全语言/主题/辅助技术矩阵和完整产品目标仍开放。不跑全量suite；本批覆盖变动适配器取消路径和包依赖/持久化关键链路，其他边界不由构建或六组检查代替。
