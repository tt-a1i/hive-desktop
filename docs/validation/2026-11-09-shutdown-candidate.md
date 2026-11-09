# 新依赖冻结后的1534986候选

2026-11-09，源码1534986f992fe011cef94706618bf2ee2acf40df。纳入工作流结果优先、更多导航、成员未结派单数/窄窗操作，以及运行中退出的interrupted语义与清理。

候选：`.artifacts/hive-desktop-macos-arm64-candidate.zip`，157,476,665字节（约150MiB）；ZIP SHA256 `e353c74c0a0343f651f41bfe039b47f9c644872f54888e7612277d54d0190529`；原生程序SHA256 `7bbceffa275433e2058844258a951e674dc71d0d9d9906ecdc5421c53f1f21ad`。

## 新冻结与独立解压

本次Hive补丁与lockfile变化，未复用旧包依赖。按portable-packaging文档执行`pnpm --filter @hive-desktop/runtime deploy --prod --frozen-lockfile --node-linker=hoisted`，然后`node scripts/package-desktop.mjs`。生产部署使用锁定的381个包；沿现有配置忽略genai/protobufjs的安装脚本，未扩大构建许可。官方Node22.23.3的darwin-arm64 archive SHA256核对为`23b25245dcfb9af7262f8ff142e9e2e0af025368117329e7a7458a51e5922f53`。

先将完整bundle写入新ZIP，再独立ditto解压。28,834个普通文件逐一匹配内容/权限，29个链接逐一匹配且均在应用内部；112个runtime dist文件与当前构建匹配；包内workflow-runner.js、d.ts及runtime-store.js与已应用Hive依赖匹配，补丁hash为`7c02ce83275d11a54b1fba8504e8006000ef989c8a05b07edd75c116a2bc7bd5`。不以本地运行代替包内依赖检查。

## 包内检查和实际窗口

以下四组全部exit0，参数均为独立解压目录：

- `node scripts/check-package.mjs --bundle …`：包内Node、真实PTY、Hive agent-manager正常退出/停止、HTTP/SQLite及关闭。
- `node scripts/check-packaged-dispatches.mjs --bundle …`：结算队列、Codex工具回收/同回合通知、空会话/显式输入恢复；新增workflow-shutdown覆盖双成员、嵌套、CPU脚本退出及cold interrupted，保留completed/failed/stopped。
- `node scripts/check-packaged-queue.mjs --bundle …`：真实内置/外部Pi SDK/Hive/SQLite队列、附件/FIFO/停止/冷读，模型侧为回环合成响应。
- `node scripts/check-packaged-remote.mjs --bundle …`：真实本地WebSocket、X25519/SAS、加密HTTP、配置键拒绝、撤销断网重试和冷恢复；未访问生产网关或授权用户设备。

实际运行独立解压后的应用，没有HIVE_DESKTOP_NODE/RUNTIME覆盖。使用隔离owned状态与本地JSONL peer，从原生聊天发起工作流→两个真实Worker进程与派单→追加team send；团队页显示2/1未结责任，中文窄窗整组操作可见。运行中Cmd+Q，PID5008正常exit0；包内Node/runtime独立cold读两段成员原生thread ID保持、pending0。SQLite另证三项dispatch均cancelled，临时成员回收。

再次启动同一包PID50521，通过“更多→工作流”实际打开已中断记录，原因/取消责任在来源参数之前，截图与AX文本保存。再次Cmd+Q退出0，所有owned native/peer进程已不在进程表。包内未做新厂商推理，不把本地peer冒称真实Codex/Terra团队；此前真实模型证据按各自源码/候选范围保留。

证据在`.artifacts/shutdown-candidate-1534986/`，包括deploy/package/freeze日志、四组gate日志、live截图/JSONL/状态/冷收据和receipt.json。lsof检查只有检查器自己的日志句柄，无应用/测试占用；清理本轮fresh deployed、原bundle、独立extracted，保留唯一canonical ZIP和轻量证据，不留额外依赖副本。ZIP普通归档方式未附AppleDouble资源元数据，大小变化不作为性能或优化基准。

## 验收边界

current仍1acd805，SHA256 `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`，未推广候选。未签名/未公证，不是正式安装器。用户明确当前没有Windows机器，先继续其他实施；本轮不新增Windows原生、Claude真实模型、Pi重新登录、生产远程、完整键盘/VoiceOver/输入法/大历史等结论。完整目标保持未完成。
