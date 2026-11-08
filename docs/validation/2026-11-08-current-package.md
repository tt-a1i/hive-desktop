# 当前 Mac 开发包

当前包纳入成员职责、界面字号和历史消息重测，以及 Claude 退出结算修复。源提交 `ca9eeb6abacf7fcfede5d418f3d2fe6e93f4b8b8`。

- ZIP：`.artifacts/hive-desktop-macos-arm64-current.zip`
- ZIP SHA256：`bd6616e7d2765b9ae40cf8a6b0d1bad8a85a5a6894bab27ef1be959106b04e81`
- 原生程序 SHA256：`9d29593ad0f5f6c50f9189989fb9bdd58f0b19995dd8196a0274f618005e9135`
- 未签名/未公证的 Mac arm64 开发包，Windows 原生与完整产品目标尚未完成。

current.zip 是固定位置，后续按一批变更更新，不为每次小改留一套应用/依赖树。上述哈希只对应当前批次，历史源码及验收记录独立保留。

## 最新包内验证

本次只替换 agents/claude.js 与 map。生产依赖/锁文件/patch/原生源码未变；runtime 包元数据除开发检查脚本外不变。独立解压后校验108个runtime与生产dist匹配，29969个未变归档条目逐一匹配，包括原生程序、Node、依赖和链接目标。没有重新复制一套生产依赖。

- check-packaged-claude.mjs：自带Node、生产runtime、官方SDK与合成CLI，真实Hive/SQLite/原生格式会话/spool，验证全局配置/其他会话/当前会话记录丢失与成功结果前退出四组；已确认输入不重复执行，失败输入与补充保持暂停，冷恢复和明确继续通过。无厂商推理，不冒认真Claude模型执行。
- check-package.mjs：包内Node、真实PTY、Hive agent-manager自然退出/停止、HTTP/SQLite工作区及后台关闭通过，无模型请求。
- check-desktop-startup.mjs：生产窗口读取实际数据并恢复项目/失效会话选择通过。脚本以信号清理其窗口，不将它计作正常UI退出或物理鼠标验收。

旧包在新增Claude退出检查中出现清理失败；源码基线另在完成ID断言失败。详细修复、校准和证据边界见[Claude退出结算](2026-11-08-claude-write-loss.md)。

## 未变原生界面的既有系统验证

相同哈希的原生程序在8a61711批次已通过系统鼠标/键盘：职责正文搜索、展开完整说明与复制入口；设置搜索进入外观、调整字号和恢复默认。使用该批解压包Node/runtime，无源码覆盖，owned进程70867正常零退出。为隔离同名窗口只修改该验证副本Info.plist和目录名，原ZIP不变。

本次Claude后台修复没有重复上述物理操作，不将上一批的系统操作冒认重新测过。源码/系统验收详见[成员职责](2026-11-08-member-responsibilities.md)与[字号](2026-11-08-interface-font-size.md)。

## 产物保留

检查结束后删除当前临时解压副本、小型依赖链接候选及旧包gate链接，保留日志、脚本和会话证据。ZIP保留current、settings-search、tasks-watcher三份；先前已核对哈希删除被替代的codex-settings ZIP。未删除会话/旧聊天目录或运行中依赖，.artifacts维持约3.4GiB。

当前收据 `.artifacts/current-package/receipt.json`；前批小型收据单独保留。Claude批次收据和日志 `.artifacts/claude-write-loss/`。不重复无变化后台的全部回归，不把源码、协议模拟、包内检查和正式发行混为一谈。
