# 本机系统提醒：接线与设置验证（#25，进行中）

2026-11-10，源码基线9e61f41。发现固定Hive Web基线5d87d4b的NotificationProvider/NotificationSettingsButton已有本地通知，而桌面通知页只有Webhook；用户切到其他工作时需要盯着窗口。新增本机提醒接线以补迁移缺口，不能将本记录当作系统送达或完整通知功能验收。

## 源码行为

复用已锁定GPUI的set_app_identity、show_system_notification、on_system_notification_response；不新增依赖、Rust/Node后台服务、模型审批或远程通知通道。通知偏好为本机数据目录内desktop-notifications.json，默认false；临时文件写入成功再替换并更新内存，失败保留旧开关。系统授权/送达API没有可返回的权限状态，因此界面只说偏好保存/提交给系统，不宣称系统已授权或提醒已送达。

新序列事件通过现有agent_event去重入口后，分类回复结束、错误和pending交互；取消不当完成，失败后的settled不再发成功，恢复已有失败队列的静态state不当作新错误。只在未阅读该会话时发送，检查系统窗口实际active状态及窄窗侧栏覆盖；macOS active_window可能只是mainWindow，因此另核对is_window_active。内容为会话名称和通用提示，不包含回复、工具、错误原文。每会话固定tag沿OS接口替换；关闭偏好撤回当前进程记录的已发送tag。

点击会话提醒的处理器激活窗口，通过已有chat.list查找仍存在的目标，沿navigate_to_session切换并保留既有草稿逻辑；测试提醒只激活窗口。没有创建第二套会话导航或绕过Agent原生审批。后台断线/未就绪、应用退出后的冷点击、实际错误/交互/停止以及跨项目返回等仍须真实系统交互验证，不凭源码推断通过。

通知页同时使用现有Viewport/Reveal保证新增内容后Webhook输入和按钮可达；新增中英文文案、状态AX和搜索关键词。声音选项未迁移；当前GPUI通知API不提供应用内声音选择，本轮不声称具备Web七种音效或完整原生通知配置。

## 已验证

- 三个现有框架下的最小Rust检查：真实偏好缺省/保存/关闭重开/非法内容/真实替换写入失败且无临时残留；事件分类成功/失败/取消、pending交互、已有队列失败不误报。命令`cargo test --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop local_notifications_`，3通过。分类输入经真实PiEvent serde解析；这属于本地逻辑检查，不是假称模型或系统通知集成。
- 最终Mac debug build与Windows MSVC目标check均通过，git diff检查通过。首次测试用了GPUI同名test宏的通配导入，改为显式测试导入；另有一条合成请求写成native_request_id而契约是nativeRequestId，修正夹具。没有给产品加测试fallback，也未修改既有测试。
- 自有.artifacts/local-notifications-9e61f41，新建一工作区/两条Codex Terra空会话，仅用于设置与后续通知测试。没有发送消息、模型回合或创建Agent run。
- 原生中文20px浅色：默认off且Test禁用，开启后实际落盘true。把本轮自有偏好文件完整移到备份、原路径创建空目录，实际切换失败显示Is a directory，开关仍on。撤销故障并精确恢复备份，重试关闭成功且Test禁用。write-failure.png/AX、retry-off.ax.txt。
- 关闭应用后冷开仍off，cold-off.ax.txt。切English，20px约593×507窄窗，Test为正常内容宽度；正向Tab自动显露Webhook三操作及Reload焦点，实际Return重新读取空URL；反向返回搜索/开关，Space启用、Tab到可见Test，未按Test；Shift+Tab/Space关闭。en20-webhook-keyboard.png、en20-test-focus.png。
- 两个原生窗口Cmd+Q均exit0。settings-receipt.json：偏好false、English/light/20px，两条会话ID及Terra元数据保留，没有原生Codex线程；SQLite workspace1，messages/agent_runs/dispatches/workflow_runs均0。

原生验证壳最初缺Contents/Resources/runtime工作目录，Node启动ENOENT；补验证壳目录后用原生Restart恢复，没有重启仍活跃的未知进程或改生产定位语义。仅硬链接本项目debug程序并复用现有021c162包内Node/runtime，没有新依赖副本。

## 授权与未验收

原生操作工具要求OS权限单独确认，已异步请求用户允许本轮Hive Notification Audit申请macOS通知并点击Allow。尚未收到确认，未点击Test、未显示/接受系统授权、未发送任何系统通知。

因此OS送达、点击返回/草稿保留、真实Agent完成/错误/交互/取消、前台抑制与后台投递、授权拒绝/专注模式、冷点击和Windows原生均未验收；#25保持开放。当前安装版/ZIP/DMG仍021c162，不含本次新源码。测试数据的通知偏好最终关闭，未改默认用户配置或全局认证。完整目标继续推进。

## 后续：关闭提醒时的真实模型与冷恢复

源码 b90c18d，继续使用自有隔离目录，不触发OS授权。原生界面在“保留草稿 B”写入未发送的 NOTIFICATION_OFF_KEEP_B · 保留未发送草稿；切到“提醒目标 A”，确认 GPT-6.1 Terra / Medium，实际发送只回复 NOTIF_OFF_REPLY_OK、不调用工具或修改文件的验证请求。收到精确回复，0工具，pending归零；返回B恢复原草稿。该次原生窗口正常退出，exec返回0。

原始 Codex 线程 01a1c50b-063f-7d41-9756-14fdfc3e659b 的 JSONL 只有一次 task_started/task_complete，turn_context 为 gpt-6.1-terra、medium、自有项目路径，工具调用为空；live-off-model-receipt.json。独立包内 Node/runtime 调 chats.open 而非 resume：A助手正文精确匹配、pending=0、busy=false、完成投递ID保留，B历史空、pending=0、busy=false、原草稿逐字保留，本机通知false；live-off-cold-receipt.json。SQLite此时messages=1、agent_runs=1、dispatches/workflow_runs=0。这是后续实际模型阶段，前文settings-receipt.json的零回合/两空会话仍只描述较早设置验证现场。

另修复命名空会话仍显示欢迎页的问题，见[标题验证](2026-11-10-empty-session-title.md)；之后新增第三条空会话，亦未发送消息。提醒关闭的真实回复验证通过，不代表开启时的前台抑制、系统送达、点击返回、错误或交互路径已通过。OS授权问题仍待回复，未发送任何系统通知；#25与完整目标保持开放。安装包仍021c162。
