# 欢迎、成员历史与项目导航开发包

源码 `46373f6bb11a66d5bfdd658ad848545e8a2db13d` 的Mac ARM64优化release，2026-11-10更新current/candidate及Applications安装应用。纳入261111e图片读取反馈、7eccc10成员删除/筛选语义、e546e47已删除成员的启动配置校验、6bdf3a3欢迎品牌与46373f6路径查找。

## 真实包与检查

生产依赖沿用已核验db6fb20：Node、native模块、锁文件、pnpm patches、Cargo依赖不变。只替换原生程序、chats.js/map、hive-host.js/map及runtime package.json的开发检查scripts；生产依赖元数据保留。内嵌Hive原logo及已提交原通知来自同一上游版本；原Info.plist、ICNS/来源说明保持。

- 优化Rust release和runtime build通过；112个runtime文件与源码生产dist逐一匹配。
- 新ZIP解压28,865条文件/模式/哈希与29内部链接全部匹配，其他依赖条目不变。assembly.json / manifest.json。
- 六组包检查check-package、packaged-models、packaged-dispatches、packaged-codex、packaged-remote、packaged-claude全部exit0，实际使用解压包Node/runtime。覆盖实际PTY/Hive HTTP/SQLite、模型、派单队列/恢复、原生协议对端和远程链路，按各检查原范围解释，不冒称厂商推理。
- 将提交的deleted-member-session.ts和session-search.ts编译并绑定包内dist，执行时使用包内Node/SDK：已删除成员prompt/resume/model拒绝无孤立配置、历史/其他成员配置/冷恢复，以及四后端/规范项目路径/重名目录查询、101搜索分页/全局置顶等全部通过。update-gates.log。

没有引入假server或fetch mock。模型端点仅作本机意外请求检测，新增后台检查无厂商推理；旧模型请求证据不重记为本轮执行。

## 原生包内导航

从最终ZIP解压app启动，只有本轮自有HIVE_DESKTOP_DATA_DIR与空/合成PI目录隔离，没有HIVE_DESKTOP_NODE/RUNTIME源码覆盖。真实PID90599及子进程90607均来自本轮bundle资源。

实际⌘K→beta/Shared唯一命中→Return，项目完整路径与原Draft beta多行草稿一致；再⌘K→alpha/Shared→Return恢复原Draft alpha。截图/AX为packaged-beta/alpha.ax.txt。窗口⌘Q，句柄10499正常exit0、原生和Node PID消失。

另以包内模块启动独立新runtime冷读同一自有数据，两份草稿JSON、workspace/session ID、openai/gpt-6.1-terra选择、pending0/busyfalse、最终Alpha导航均保持；没有user/assistant历史，工作区2，workers/messages/agent_runs/dispatches/workflow_runs0。见check-search-cold.mjs、packaged-search-cold.json。

## DMG与Applications更新

1. 标准UDZO DMG实际生成、hdiutil verify通过，readonly/nobrowse挂载后28,865条app清单及Applications→/Applications链接匹配。
2. 更新前确认旧安装app与db6fb20清单精确相同且未运行；原包及可运行解压应用保留用于回退。Finder实际复制新app，经系统“较新项目替换较旧项目”提示执行替换，拷贝结束后再次核对Applications全部28,865条清单，而非以粘贴动作当更新完成。
3. 默认桌面数据工作区/消息/派单均0后，从Finder直接打开 `/Applications/Hive Desktop.app`，实际看到内嵌标识、工作台标题、四后端配置和紧凑三栏步骤，installed-welcome.png / .ax.txt。原生PID66535与子进程66539的Node/runtime均在安装目录。
4. ⌘Q后系统isRunning=false，两PID消失，默认工作区/消息/派单仍0。Finder启动没有可取exitCode，不写exit0。没有修改用户配置、登录或模型默认；映像已正常卸载。

本批只重新实测列出的包内路径/欢迎与后台检查，图片系统保存和成员删除UI按各源码阶段记录保留，不冒称所有界面矩阵均在本包重跑。前版原始Codex Terra Medium成员80/4记录保持历史证据，本轮没有新增厂商模型请求。

## 固定产物

- current/candidate同字节硬链接，ZIP143,856,220字节，SHA256 `4b0fe6ee7e6d0b73dda487ccd824c26dd563d025d86affc155cee77af7bc2eb1`。
- `.artifacts/release-46373f6/Hive-Desktop-macos-arm64.dmg`，224,492,510字节，SHA256 `03e7cd1ed711b8ec397f008974152ae9b44f0d860a6d7c656a173745cf2f5313`。
- 原生程序SHA256 `3ac9a11ab490a6ca680d1e2df84386a144ecbe228e0b2686a1b35097f3bf6a84`。
- 当前收据 `.artifacts/current-package/receipt.json`；本批完整证据 `.artifacts/release-46373f6/`。

未签名/未公证、未测试下载隔离的Gatekeeper或Windows原生。Windows源码检查按先前每次修改记录；本批不重复无改动Rust/协议全量suite。完整产品目标、厂商与UIUX完整矩阵继续开放。

## 清理

核验归档哈希并确认lsof无引用后，删除本轮重复base；已落后两版的3339ce1解压bundle同样只在ZIP校验及无引用后删除。ZIP/收据、原测试状态和旧聊天工作目录保留，上一db6fb20可运行解压app保留。合计回收1,077,329,920磁盘字节（约1.00GiB），精确记录见cleanup.json和superseded-cleanup.json；没有清理全局Cargo、用户会话或系统废纸篓。
