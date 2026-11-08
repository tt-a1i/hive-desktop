# 当前 Mac 开发包

本批纳入成员职责查看/复制/搜索，以及界面字号、历史消息缩放与行高重测。源提交 `8a6171119f64fc03762edd6b0af4aab93c4fd7d5`。

- ZIP：`.artifacts/hive-desktop-macos-arm64-current.zip`
- ZIP SHA256：`c0fe07fd268e20e439b3bc7054d69a41fa7869d8684b739079ca5e61438c38a7`
- 原生程序 SHA256：`9d29593ad0f5f6c50f9189989fb9bdd58f0b19995dd8196a0274f618005e9135`
- 未签名/未公证的 Mac arm64 开发包，Windows 原生与完整产品目标尚未完成。

current.zip 是当前包的固定位置，后续按一批变更更新，不为每次小改留一套应用/依赖树。上述哈希只对应本批次，历史源码及验收记录独立保留。

## 包内验证

与设置搜索包比较，runtime 源码、锁文件、patch、包元数据和 Rust 依赖未变；仅替换生产原生程序。独立解压校验程序、108 个 runtime 文件、29970 个未变归档条目，包括 Node、依赖及链接目标，均一致。中间暂存包在发现普通历史固定字号后被最终包替代，不作为当前包。

- check-package.mjs：包内 Node、真实 PTY、Hive agent-manager 自然退出/停止、HTTP/SQLite 工作区及后台关闭通过，无模型请求。
- check-desktop-startup.mjs：生产窗口读取实际数据，恢复项目与失效会话选择通过。脚本以信号清理其窗口，不将它计作正常 UI 退出。
- 最终包另经系统鼠标/键盘：职责正文搜索仅命中目标成员、展开说明与复制入口；设置搜索进入外观，16→17 立即生效并恢复默认。使用包内 Node/runtime，无源码路径覆盖，owned 进程 70867 正常零退出。已有 owned 字号历史包含明确标注的合成文本，不冒认真模型输出。

为避免同名应用冲突，只改解压验证副本的 Info.plist 身份/名称和应用文件夹名；原 ZIP 不变，退出后再次核对 ZIP 与程序哈希。

## 产物保留

结束后删除临时解压副本，保留收据、脚本、日志和会话数据。ZIP 保留 current、settings-search、tasks-watcher 三份；核对哈希后删除已被替代的 codex-settings ZIP，未删除会话/旧聊天目录或运行中依赖。清理后 .artifacts 约 3.4 GiB。

收据和日志：`.artifacts/current-package/`。源码/系统验收详见 [成员职责](2026-11-08-member-responsibilities.md) 与 [界面字号](2026-11-08-interface-font-size.md)。未重复无变化后台的全部回归，不把源码、包内检查和正式发行混为一谈。
