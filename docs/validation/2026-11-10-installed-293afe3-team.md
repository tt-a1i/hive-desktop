# 已安装293afe3：真实一对二协作与完整恢复

本轮直接使用 `/Applications/Hive Desktop.app`，原生程序SHA256仍匹配current收据的 `f0dfed19a0433f1025378c034627b9f66e93df92f3c5c6ac595f033d3ded2cab`。没有Node/runtime路径override；进程树确认Applications内后台与三个拥有的原始Codex子进程。CLI实际0.167.0，所有原生turn_context均为 `gpt-6.1-terra / medium`，没有修改用户全局配置或认证。

独立项目/状态/agent：`.artifacts/team-installed-293afe3`。生产包内后台创建隔离工作区与空会话；原生输入框粘贴768字符任务，磁盘草稿逐字核对后仅点击一次Send。主控使用Hive注册team工具创建proof-alpha/proof-beta，分别派单并处理显式汇报。三会话：

- 主控 `f93ba291-5470-4f4c-8e02-2664d7d23409`。
- alpha `068f25e6-05e7-4e7f-b245-8b5184d8fb02`。
- beta `bd8c93fb-a4e9-4614-b85a-ddc6cd7c2bd1`。

## 实际结果

两条dispatch均reported，来自本工作区主控，分别属于正确成员；两条report_outbox均投递到同一主控。alpha/beta文件分别为精确27/26字节、无结尾换行，内容独立断言通过。原生工具展开显示主控的只读Python同时断言两文件，输出`Both byte assertions passed`；原始Codex日志中的custom_tool_call及结构化output再次确认exit_code=0，保存main-native-assertion.json。唯一最终回复包含 `HIVE_TWO_WORKERS_COMPLETE` 及汇报/核验说明。

三个原生会话各1轮，共3真实模型回合，三个spool均空；两报告在原主控回合内完成，没有额外空回合。不能把一次用户Send、三个模型回合写成“只有三次厂商HTTP请求”，工具循环的实际HTTP请求数本轮未统计。

实际英文20px窗口查看协作概览的submitted→reported、主会话带成员来源的报告/完成状态、主控核验工具、alpha成员会话及返回主会话。保留未发送草稿`Installed 293afe3 · 未发送恢复草稿 🐝`；第一次Cmd+Q exit0，包内独立后台只读打开三会话。随后完整原生重开，报告、最终回复和精确草稿恢复，未点Send/resume，再Cmd+Q exit0。

baseline、after-navigation、after-cold及after-reopen均保存；后两次核对按会话ID逐字段保持原生历史/完成投递，派单/汇报/文件保持，所有初始拥有进程均不存在。索引集合的导出顺序在原生重开后变化，原始快照保留，不能声称JSON数组逐字相同；按ID比较每条会话记录完全一致。最终receipt及current/candidate/package三份收据已追加本批真实模型验证，旧的只读模型参考记录保留。

## 失败与边界

两个成员及主控有读取不存在`docs/implementation-plan.md`的工具失败（隔离项目位于Desktop目录下，继承项目指令）。README可读，指定文件操作、显式汇报和独立断言均成功；失败日志/模型答复保留，未修改产品代码掩盖它，也不声称所有工具调用成功。

协作概览初次加载时短暂将正在工作的成员标成Historical conversation，随后自动恢复正确名字；应继续追成员缓存与标签判定，不据此宣称完整动态UI验收。短暂报告processing画面本轮未捕获，不以最终completed画面替代动态状态证据。

本轮无生产代码/测试变动、不新增ZIP、不重复无关全量测试；真实安装包新建→协作→工具/汇报→成员往返→退出→包内冷读→原生重开为验证范围。其他后端混合协作、完整UI/辅助技术、Windows原生及正式发行仍开放，完整目标未完成。
