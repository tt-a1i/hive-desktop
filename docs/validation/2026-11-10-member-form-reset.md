# 连续创建成员时清除已提交内容

2026-11-10；基线c36f7d5，当前开发包仍ecad294，本修复尚未入包。

## 问题与修复

实际创建成员成功、收起表单后，再打开“添加成员”仍带着上一位的名称和职责。用户继续添加时容易重复提交已存在的名称，得到409。改前在真实包窗口复现；`before-reopen.ax.txt`中两项仍为 Owned onboarding member / 完整两行职责。

成功完成创建及后端配置后，只清空仍与提交时原文相同的名称和职责；角色/后端沿用当前选择。失败、手动收起或请求期间已修改的输入不清除。沿用现有原生输入set_value、窗口句柄与请求workspace/generation检查，没有后台、schema或权限改动；唯一add_member调用同步传入当前窗口。名称与Textarea使用不同原生输入类型，初版数组循环未通过编译，已改为两项明确判断，不新增抽象。

## 验证

自有 `.artifacts/member-form-c36f7d5/` 从原成员修复状态复制，无用户数据修改。改前真实包建立第14位停止成员后复现；改后源码开发应用验证：

- English/Dark20、约593×507逻辑窗口，填写 Owned fresh member 与两行中文职责；正向Tab/Space选择Review和Codex、Tab到可见Add焦点、Return。真实HTTP创建成功，第15位停止成员出现，未启动/选模型。
- 返回顶部显露按钮后重开表单，名称与职责为空（`after-reopen.ax.txt`）。角色Review/后端Codex沿用。
- 再填写相同名称与不同两行职责，真实后台返回409；名称与完整职责保留。手动Hide form再打开仍精确保留（`failed-retained.ax.txt` / `hidden-retained.ax.txt`），不以成功路径清空失败草稿。
- 两个原生进程正常CmdQ/exit0。独立Node22真实Hive HTTP/SQLite冷读：先前14成员完整对象不变，只新增一位停止的Codex reviewer，职责逐字相等、launch args为空、pending0；重复尝试未多建成员，messages/dispatches/dispatch_messages/agent_runs/workflow_runs均0。`check-cold.mjs` / `cold-receipt.json`。
- macOS cargo build、Windows x86_64-pc-windows-msvc cargo check与git diff --check通过。纯原生UI行为不新增测试、不跑全量suite；已有真实后台路径如上，无模型请求。

工具边界：改前第一次中文typeText仅输入换行，改用paste后AX确认完整正文才提交；屏外Add的AX点击曾落到顶部返回区，改为先滚到顶部并截图确认再点击。未将这些尝试当作保存/产品缺陷证据。请求进行中改字段的判断已实现，未以受控延迟单独复演该时序；本轮不声称跨应用重开保留未提交成员表单。Windows原生/全主题语言/VoiceOver和完整产品目标继续开放。
