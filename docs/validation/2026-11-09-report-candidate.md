# Codex汇报回收修复候选包

2026-11-09。候选源 `7c28580b64062328fabf33e6bdb23c6c76ddb415`，纳入#21的主机工具原生确认后回收，不改变直接用户Stop。完整目标#1仍在实施。

- ZIP：`.artifacts/hive-desktop-macos-arm64-candidate.zip`，171,840,492字节（约164MiB）。
- ZIP SHA256：`e15a7e71dc170afe59b06c824250c99e21cb2d42abcb6730eb396a3fb6733974`。
- 原生程序SHA256：`64a9fc79d4ab48b8e7ea5a98a272e596f4d05befe46b837ebd324c5555f65d73`，与2b3c07e相同。
- Mac arm64未签名/未公证便携开发包，非正式安装器。current仍1acd805，未推广候选。

## 归档核验

以2b3c07e候选为基线，核对ZIP/程序及源码。只替换chats.js/map、agents/codex.js/map，以及runtime/package.json的scripts（将新增回归加入check:codex）。包内pnpm部署后的依赖/optionalDependencies等元数据原样保留；与原manifest逐键比较，除scripts外全部相同。未重新production deploy，不把脚本元数据更新写成依赖不变的整份manifest字节相同。

独立ditto解压、ZIP testzip通过；112个runtime文件与当前dist全部匹配，28,829个未改普通文件、29内部链接/解析目标、31,570个AppleDouble元数据逐项保持。原生程序未替换，UI沿用前候选。运行后再次核对程序/112个runtime和manifest，再更新唯一candidate槽位。

## 本轮包内检查

均使用包内Node和runtime，命令exit0：

- `scripts/check-package.mjs --bundle <bundle>`：真实PTY、Hive agent-manager自然/停止、HTTP/SQLite工作区与shutdown，无模型请求。
- `scripts/check-packaged-queue.mjs --bundle <bundle>`：内置Pi/原始外部Pi的实际SDK/Hive/SQLite、FIFO、附件、停止保留和暂停冷恢复；本地回环合成响应，不是厂商推理。
- `checks/codex-tool-retirement.ts`经既有TypeScript转译方式绑定到包内codex.js和typebox，再用包内Node跑四分支：回收先获得实际工具确认、直接Stop立即signal、无确认有界且不伪造成功、断线拒绝。真实JSONL子进程的合成对端，不冒称原样Codex模型。
- 另从包内desktop-runtime.js启动真实workflow agent()。自定义角色固定hive:codex及openai/gpt-6.1-terra，使用原始Codex0.166.0；模型自己读input.json、计算、写JSON、读消息并team report。driver只启动生产API工作流，没有代report。

真实模型run `994d7918-e5ab-4cb4-91f2-3e682a69985d`完成，唯一dispatch `4e041d84-da24-464b-b484-a59040135a23` reported、产物sum31/count3，输入保持。原生report status completed、success true、成功空输出符合原CLI，线程/模型核对正确、runtime正常close。不是只根据workflow completed判绿。

独立新runtime从同一数据读回运行/派单/同一Codex线程和原生工具成功，恢复脚本exit0，没有模型请求。主控队列暂停，所以不宣称模型主控编写/启动了本次工作流，也不把driver触发当用户从聊天框启动。

## 实际原生窗口

从独立解压包打开上述真实数据，环境没有HIVE_DESKTOP_NODE/HIVE_DESKTOP_RUNTIME覆盖。12530的实际子进程路径指向包内Node/runtime，记录了8个owned进程。

Worker会话显示两组共6个已完成工具，展开最后一组，report的当前派单UUID与真实收据相同，标记“完成”。工作流页面显示同一运行已完成、sum31/count3汇报、关联派单与2条日志；产物预览读取该工作区实际JSON。没有点击“继续处理”、没有GUI chat.prompt，不新增GUI推理。

原生Cmd+Q exit0，8个已记录owned PID均不在进程表；解压路径无进程引用，lsof +D返回1且stdout/stderr空，然后才删除解压树。没有改变应用标识或用源码runtime替代包内运行。

## 收据与保留

`.artifacts/report-candidate/receipt.json`、四组gate日志、`live/receipt.json`/events/run.log、live恢复记录、`gui-processes.json`、package-ui进程/退出、cleanup记录；截图在本聊天CUA。当前索引`.artifacts/current-package/candidate-receipt.json`。

通过后原子替换唯一candidate，旧2b3c07e收据标明ZIP已被取代并保留；不新留完整依赖/ZIP副本。.artifacts仍约3.9GiB。current哈希`f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`不变，工作区/会话/模型产物证据保留。

本轮仅打包补验和记录，无新产品/测试修改、不重跑全量suite。源程序定向回归及真实后台Stop证据见[#21源码记录](2026-11-09-workflow-codex-report.md)；配置窄窗/新增/草稿等UI证据仍按[2b3c07e候选](2026-11-09-model-candidate.md)的原范围保留，不冒称本轮逐页重跑。

Windows原生/发行、其他后端真实模型回收、完整字号/VoiceOver/输入法/所有页面，以及模型主控编写工作流、工作流删除/跨连接/大历史等仍未完整验收。完整桌面目标未完成，不因#21已进包推广current。
