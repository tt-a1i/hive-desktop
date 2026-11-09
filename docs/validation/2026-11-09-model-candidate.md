# 模型配置修复候选包

2026-11-09。候选源提交 `2b3c07e10144d6af567f63a254fd7d116bf9e057`，纳入固定保存/错误栏、模型字段和能力/接口焦点显露、标签与手动滚动、新增必填ID初始焦点。完整Hive Desktop目标仍在实施。

- ZIP：`.artifacts/hive-desktop-macos-arm64-candidate.zip`，171,839,922字节（约164MiB）。
- ZIP SHA256：`a3e41d9e17baef853d10bd68f9fea6961fcd8100454a50d93724f3c91eff3e5c`。
- 原生程序 SHA256：`64a9fc79d4ab48b8e7ea5a98a272e596f4d05befe46b837ebd324c5555f65d73`。
- 未签名/未公证Mac arm64便携开发包，非正式安装器。current仍为1acd805，未推广候选。

## 构建与完整性

基线dd0230a候选，核对原ZIP/程序哈希及源码差异：packages/runtime/src、依赖清单/锁文件/patch、Cargo依赖均无变化。仅更新归档中的原生程序，复用已核验冻结依赖，没有执行新的production deploy。

独立ditto解压后，全部112个runtime文件与当前生产dist逐一匹配；28,833个未改普通文件、29个内部链接及31,570个AppleDouble元数据条目保持。ZIP testzip通过，链接实际解析在应用内。UI运行后再次核对程序及112个runtime文件，没有把开发目录symlink混成包内验证。Info.plist和应用身份未作修改。

## 本轮实际运行

使用包内Node执行：

- `scripts/check-package.mjs --bundle <bundle>`：真实PTY、Hive agent-manager自然退出/停止、HTTP/SQLite工作区和shutdown通过。
- `scripts/check-packaged-models.mjs --bundle <bundle>`：四组实际SDK/JSONC配置、高级字段/冲突、覆盖继承/编辑/移除、生产JSONL与常驻会话目录刷新检查通过；子进程确实从包内runtime启动。

两命令exit0。无模型请求，不把此轮写成Terra或Claude真实团队重验。之前dd0230a的真实Codex停止/保留队列/另一连接与成员阅读等证据按其原时间和范围保留，见[上一候选](2026-11-09-stop-candidate.md)，本轮runtime文件相同不等于重跑那些流程。

## 实际原生配置操作

从独立解压包启动，环境无HIVE_DESKTOP_NODE/HIVE_DESKTOP_RUNTIME覆盖。进程19845的子进程19854命令实际指向此包的Node和runtime/dist/main.js。采用新的空桌面状态目录与独立合成Pi配置，不创建工作区/启动Agent，不读取真实用户认证。配置含31个长名模型、一个此前新增的本地接口和两条覆盖；本轮只修改隔离副本。

- 1184×1104截图像素窗口，模型名称、上下文、输出、思考/图片按钮随Tab显露，标签和焦点环可见；固定保存/关闭始终可见。
- 非法Token保存后错误在固定栏和AX可读，配置字节未变。改为24576和中文名称PACKAGED_CONFIG_PROOF · 中文，但先退出不保存；草稿各值落盘，配置仍逐字节不变。
- 58227完整新进程重开，表单/原值恢复，恢复提示与固定保存可见。实际保存成功返回列表。逐对象断言只有目标模型name/contextWindow变化，其他模型、接口、headers与覆盖保持，当前路径草稿清除。
- 同一进程打开新增提供商，ID自动聚焦；正常Tab可到首模型字段且标签完整。鼠标滚至列表底部，覆盖两组“继承内置”按钮和焦点可见；新增模型同样先聚焦ID。没有激活覆盖修改或保存空的新表单。
- 66783再次完整重开，没有恢复旧草稿；重新编辑目标模型，中文名称及24576读回正确，保存禁用，无未保存状态。
- 三个窗口均Cmd+Q exit0。退出收据/已记录PID与进程表核对，解压路径无进程引用；lsof +D返回1且stdout/stderr均空，之后才删除解压树。

实际截图/输入/滚动在本聊天CUA记录。收据与日志：`.artifacts/model-candidate/receipt.json`、`gui-processes.json`、三次package-*-process/exit.json、`gui-draft-before-reopen.json`、`gui-models-before.json`、两组gate日志、cleanup*.json/log。

## 保留与边界

候选在通过上述验证后原子替换唯一candidate槽位；保存dd0230a的旧收据并标记ZIP已被取代，不额外保留完整旧候选或解压依赖。current哈希仍为`f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`，.artifacts约3.9GiB。没有删除其他聊天工作目录或会话记录。

本轮没有改产品代码/测试，也未重复无变化后台的全量suite；前轮源码Mac构建/Windows类型检查见[配置滚动](2026-11-09-model-editor-scroll.md)。英文17字号、新增实际保存、覆盖实际写盘等源程序证据仍按该记录的范围保留，本轮包内只重复上述路径。

Windows原生/发行、VoiceOver、完整字号/输入法/所有页面、真实远程网关与工作流等目标仍未完成；#20保留剩余矩阵，不用此候选宣称完整交付。最新索引`.artifacts/current-package/candidate-receipt.json`。

后续：此2b3c07e候选已被[7c28580汇报修复候选](2026-11-09-report-candidate.md)取代，旧收据保留、旧ZIP未另存；本页配置证据按其原版本/范围保留。
