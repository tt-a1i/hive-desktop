# 欢迎页品牌与紧凑使用步骤

2026-11-10，源码基线e546e47后。继续用户指定T3 Code/Beluga/Cindy体验方向，检查实际已安装参考界面后，改进Hive欢迎页的品牌识别与信息层次。没有引入单独的演示页面或额外设置步骤。

## 参考与实际变化

本轮CUA只读查看T3 Code现有New thread草稿和Beluga尚未完成的欢迎界面，没有发送内容、关闭用户应用或推进其初始化。T3 AX实际呈现项目/模型/输入入口；其截图缩放异常，仅作布局信息参考，不作为像素级视觉证据。Beluga欢迎界面实际可见独立品牌标识、主标题/说明与明确主操作。这是当前产品界面的观察，不代表完整竞品功能验收。

Hive现有欢迎页只有HIVE文本，底部三段步骤纵向堆叠。现加入Hive原logo标识与“桌面工作台”副标题；宽窗保留36px主标题，窄窗改28px/38px行高；三步说明改为可换行的紧凑横向分栏。项目、配置菜单、四后端配置/真实元数据、错误与键盘显露逻辑沿用现有入口。没有增加新UI阶段或后台行为。

`apps/desktop/assets/hive-logo.png`直接复制已锁定 `@tt-a1i/hive@2.2.1/assets/logo.png`，SHA256 `530b2d330a451f93ddc13b5921a4f33bed8887fe1ffa01ac2b088671a93cd047`。上游NOTICE/LICENSE.BSL/TRADEMARK.md逐字节放在同目录hive-*文件；应用包已有runtime中的对应原通知仍保留。标识为编译时内嵌PNG，OnceLock复用同一个Arc<Image>，无每次render重复创建解码缓存身份或运行时源码路径依赖。

## 原生复验

证据 `.artifacts/welcome-brand-native/`，独立空桌面数据/空Pi目录、PI_OFFLINE=1，无用户凭据与模型调用。

- 英文浅色宽窗先检查14px，再用真实“恢复默认字号”改为16px；保留默认宽窗light-en16.png。logo清晰、标题/主按钮/四卡片/三栏步骤均在同一实际页面。
- 调整为1186×1014物理像素（约593×507逻辑），收起覆盖式导航，主标题缩小/换行，完整滚动到三栏步骤与外部程序入口，见narrow-en16-steps.png。
- 中文深色16px下，回车实际打开系统项目文件夹面板，Cancel返回；底部入口打开真实“外部程序”设置、Escape返回。没有点击版本探测、保存外部CLI或创建工作区。
- 首次在窄设置页对离屏字号按钮的点击没有实际生效。退出后读取preferences仍16，明确将该轮截图重命名为zh16；没有把点击请求算作20px成功。
- 进程退出后只修改本轮自有隔离preference为20，独立冷启动确认实际20px中文Dark。窄窗品牌/标题/主操作与三栏完整标题/正文、底部入口实际截图通过，见dark-zh20-narrow.png、dark-zh20-steps.png及cold-zh20.ax.txt。一次悬停tooltip覆盖步骤标题，用空白处点击移开后复取无遮挡截图，不将遮挡帧当最终截图。
- 两个原生句柄18823/98120均正常⌘Q/exit0。最终磁盘preference精确dark/20/zh；真实SQLite workspaces/workers/messages/agent_runs/dispatches/workflow_runs全部0，chats/index.json不存在。logo及三份通知与上游逐字节相同，receipt.json保留。

## 验证与边界

Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows MSVC目标 `cargo check --locked … --target x86_64-pc-windows-msvc --bin hive-desktop`、`git diff --check`通过。纯UI视觉与排版修改，未改后台、依赖或既有测试，不新增脆弱UI测试或运行无关全量suite。

实际覆盖English14/默认16与中文16/20、浅深色及列出的宽窄窗，不扩大为所有语言字号/主题组合、VoiceOver朗读、完整首次使用流程或Windows原生。窄设置页离屏点击不生效属于自动化可见性限制，未据此声称产品按钮损坏。当前ZIP/DMG/Applications仍db6fb20，本轮欢迎页与近期修复尚未打包；整体UIUX及完整目标继续推进。
