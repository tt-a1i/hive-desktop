# 远程阅读与计划确认优化开发包

2026-11-10，源码021c16283afff47a4ee1a043f0970cbd8002f097。Mac ARM64优化release，current/candidate ZIP、DMG和/Applications/Hive Desktop.app已更新到同一批。纳入0934dbf远程正文/显露、b13ea26审计事件名称、d1de2d6计划确认Cancel焦点与删除后导航恢复。

## 包内容与检查范围

- `cargo build --release --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`通过。
- 相对46373f6包清单，仅MacOS/hive-desktop一个文件变化；112个runtime输出、Node、生产依赖元数据、锁文件、图标/许可证保持。28,865个条目（路径/mode/SHA/29内部软链接）在组装、ZIP解压及安装目录逐项匹配。
- 重跑`node scripts/check-package.mjs --bundle .artifacts/release-021c162/extracted/bundle`通过：原生plist/图标、包内Node、真实PTY、Pi账号元数据、Hive HTTP/SQLite工作区及退出。
- 后台代码与依赖字节未变，未重复跑全部五组packaged-models/dispatches/codex/remote/claude；上一46373f6六组gate证据仍归原版本。本轮收据packageGates只有check-package，unchangedRuntimeReference明确rerunThisBatch=false；不把旧套件冒充重跑。
- DMG以UDZO生成，hdiutil verify通过；readonly/nobrowse挂载后app清单精确匹配，Applications指向/Applications。安装完成后已正常卸载。

没有新建/修改自动化测试或生产源码。Windows目标检查按最近源码修复的实证保留，本轮只有构建产物与文档，不重复无改动的全量suite。

## 新包原生交互

两个窗口都直接从最终ZIP解压app启动，只传自有数据和Pi目录/离线标记，不提供HIVE_DESKTOP_NODE/RUNTIME源码覆盖。实际原生/后台PID为45417/45496和93776/95094，进程命令均指向本包Contents资源，两个Cmd+Q均exit0。

- 自有远程数据：中文20px/约593×507窗口中，已撤销设备的既有历史、建立/关闭连接名称、未关联设备、完整UUID及原诊断仍可读；远程保持关闭，无新增配对/网关连接。packaged-remote.png/AX。
- 自有工作流数据：同样窄窗/中文20px/浅色，键盘到唯一暂停计划的删除入口，Return展开，下一布局帧显示完整说明和两操作，Cancel有焦点框；再次Return取消，计划保留、焦点恢复刷新入口。packaged-confirm.png、packaged-cancel.ax.txt。此次包内GUI只验展开/取消，没有再次删除唯一保留计划；最终按钮删除证据归源码阶段[计划删除](2026-11-10-workflow-delete.md)。
- 包内Node/runtime独立冷读：原peer计划、completed运行、结果和日志逐字段等于创建快照，总运行1，待投递为空。packaged-workflow-cold.json。没有调用chats.resume来人为唤醒队列。
- owned Codex原生JSONL的task_started/task_complete前后均1，本轮新增模型回合0；前一源码测试的通知回合仍归原记录，见no-new-model-turns.json。

## Finder更新与安装版

先确认旧安装版与46373f6清单一致且未运行；可运行旧包仍保留。Finder从挂载映像实际复制，经系统“较新项目替换较旧项目”提示选择Replace；拷贝完成后再次核对安装app全部28,865条清单。

默认应用数据的workspace/messages/dispatches均0后，从Finder打开安装app，显示原品牌欢迎、四Agent配置与步骤；进入Remote查看未登录/关闭/无设备状态，没有启用远程或更改配置。安装原生PID29542、Node29547，路径都在/Applications/Hive Desktop.app。Cmd+Q后系统无运行app、两PID消失，默认三表仍0。Finder启动无exec句柄，exitCode按null记录，不写exit0。native-install-receipt.json、installed-welcome.png及installed-remote.ax.txt保留。

## 固定产物

- ZIP143,843,647字节，SHA256 `4a04f30140a0f4824a16d71947a4e4258eeee1464576c9849c5ddf6cae3bd14b`；current/candidate为同字节硬链接。
- `.artifacts/release-021c162/Hive-Desktop-macos-arm64.dmg`，236,329,998字节，SHA256 `10c61af747b1d533ca3bae0320b63ee1ea78950ed1f7fdf0fa4626ea9196a339`。
- 原生程序SHA256 `b05b2a19bc218461ac52071aec424608de9cce40daf3af8b144b24688c3c4100`。
- 最新收据`.artifacts/current-package/{receipt,candidate-receipt}.json`，原始证据`.artifacts/release-021c162/`。

旧46373f6 ZIP已归档到其release目录，保留完整可运行解压app作为最近回退。本轮重复base在lsof无引用后移除；更早db6fb20的ZIP重新验SHA，28,865条清单完整核对且无打开句柄后，仅移除其extracted。二者文件分配块合计1,077,633,024字节（约1GiB，不保证APFS即时物理回收等于此值）。ZIP、原状态/项目/日志/脚本和旧聊天工作目录保留；证据cleanup.json、superseded-cleanup.json，旧收据bundlePresent=false。

这仍是未签名/未公证的开发包，没有验证下载隔离Gatekeeper、真实手机/生产网关、VoiceOver、完整UI/输入法/性能矩阵或Windows原生。完整产品目标与相关Issues保持开放。
