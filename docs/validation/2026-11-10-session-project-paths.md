# 同名项目的会话查找

2026-11-10，基线6bdf3a3。用户在两个不同目录都命名为Shared的项目中保存同名Release review会话，后端同为Pi；原搜索两项文字和AX名称完全相同，必须逐个打开才能确认项目，容易在错误项目继续工作。现在结果显示项目目录摘要、完整路径提示/AX身份，并支持按项目路径筛选；不做则同名项目仍无法安全直接选择。属于现有多项目导航的具体补缺。

## 改动

- Chats共享导航元数据直接附带Hive已有的规范化workspace.path，不新建存储或扫描文件；会话查找的多词匹配加入该字段。置顶元数据也自然带此路径，其排序和行为未改。
- Rust SessionHit增加默认空的workspace_path字段；新后台提供路径，旧后台缺字段时仍呈现原项目名，属于本地JSONL增加可选展示元数据。
- 搜索结果第二行显示项目名、末两级目录和后端；完整路径进入tooltip与辅助技术名称。中英文查找提示改为“项目路径”，归档搜索提示同样更新。会话选择仍按原workspace/session ID，不以显示路径做身份。

完整路径可用于查询；既有200字符查询上限和分页语义不变。末两级目录可能仍有重名，完整身份可从tooltip/AX读取，不宣称实现所有目录层级的最短唯一名称算法。

## 真实后台检查

现有 `checks/session-search.ts` 扩展：四后端命中携带真实路径，两个同名项目/同名会话由不同目录精确筛选，完整runtime重开仍按原ID命中，新增查询不初始化Agent。原成员改名、全局置顶、101条搜索分页/102置顶及工作区移除断言保留。

- 首先只扩展检查，旧代码在workspacePath为undefined处失败，search-red.log。
- 新代码第一次对路径检查暴露macOS /var→/private/var规范化差异；按Hive实际保存的规范路径修正测试目录为realpath，不在产品代码加入别名fallback。
- 最终search-final.log通过；现有session-archives.ts也通过，含四后端、真实SDK/SQLite、分页、归档保护和冷恢复，不依赖模型厂商。
- pnpm check、runtime build、最终Mac cargo build、Windows MSVC目标cargo check、git diff --check通过。未改Hive协议/schema/依赖，不跑无关全量suite；既有搜索检查保留并扩展，没有删测试。

## 原生窗口

证据 `.artifacts/search-project-paths/`，两个本轮自有项目分别alpha/Shared和beta/Shared，显示名均Shared、会话名均Release review。隔离Pi配置注册同ID Terra离线能力元数据，未请求推理；两份草稿分别为 `Draft alpha\n勿发送 🐝` 与 `Draft beta\n勿发送 🐝`。

1. 基线真实⌘K：两个按钮均读 `Open Release review · Shared · Pi`，截图/AX保留before-ambiguous。
2. 最终源码实际结果能看到 `Shared · beta/Shared · Pi` / `Shared · alpha/Shared · Pi`，AX与tooltip含各自完整路径，fixed-results.png。
3. 输入beta/Shared，只有Beta会话结果；Return后项目完整路径与Draft beta原两行实际出现，beta-path/opened-beta.ax.txt。
4. 再⌘K并缩到约593×507逻辑窗口：搜索、两条目录结果及底部操作均无裁切，fixed-narrow.png；Down→Return打开Alpha，实际完整路径和Draft alpha保持，opened-alpha.ax.txt。再次⌘K→Escape返回，未修改正文。
5. 两个原生句柄76021/94841均⌘Q/exit0。

独立新runtime冷读：两份草稿JSON与准备记录精确相同，原workspace/session ID、模型openai/gpt-6.1-terra与pending0/busyfalse保持；查询目录仍唯一命中对应会话；导航最终选择Alpha，两项目各自会话选择保持。历史没有user/assistant消息，SQLite工作区2，workers/messages/agent_runs/dispatches/workflow_runs均0，未产生Agent执行或模型请求。见check-cold.mjs、cold-receipt.json。

本轮实机范围English/浅色/16px与列出宽窄窗；其他语言字号主题、完整键盘/VoiceOver、长路径重名层级及Windows原生继续待验。当前ZIP/DMG/Applications仍db6fb20，本轮与近期源码修复尚未入包，完整目标保持开放。
