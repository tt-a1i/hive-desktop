# 原生访问恢复后的语言、导出与退出验证

源码c99037d（产品代码f26510f），Mac默认构建通过。原生访问已恢复，读取Finder后启动最新构建的独立`Hive Resume Audit.app`。复用已有合成历史隔离数据与缓存Node，以硬链接/资源链接运行源码runtime；不是便携包或真实模型执行。旧21544/54534已不在进程表，退出原因未获证据。

## 实际证据

- 设置点击中文→English，聊天导航/操作/输入提示变为英文；原中文项目名、成员名、正文与`DO_NOT_EXPORT_DRAFT · 保留未发送草稿`不变。
- 英文导出默认关闭思考/工具。点击复制与系统保存，再将剪贴板粘贴到另一空白测试会话草稿，落盘逐字核对一致：11,576字符，SHA256 `6095efbaac24b4aa75721a7e91849d498f6acccb583ba7fecbabc60377ab7a5e`，含1200段长正文及END_EXPORT_BODY，不含另一会话、隐藏上下文、注入包装、原草稿、未选思考/工具。核对后原生清空临时草稿，应用移除空草稿文件。
- 实际保存文件名为`native-english-body.md出验证`；自动化未完整替换原名称，指定文件名操作不算通过，未归因为产品保存缺陷。
- 拖到1184×1104截图像素（Retina）后，收起导航可访问聊天。稳定画面确认英文聊天控件、导出说明/选项/预览/复制/保存可见，两个选项可打开。不是完整窄窗或字号矩阵。
- 实际Cmd+Q，自有原生39724返回0；日志有shutdown成功回复，Node40261退出。
- 同源码冷启动，英文偏好与原会话/草稿恢复。中文“语言”搜索可打开英文Appearance；中文通过原生粘贴输入，不代表物理输入法验证。选择浅色、切回中文、返回聊天，原草稿仍一致。
- 第二次点击窗口关闭，原生41457返回0，shutdown成功回复，进程表无本轮原生/runtime残留；未用SIGTERM代替用户退出。
- 两次日志无chat.prompt，真实SQLite messages为0；无厂商请求或全局模型/账号改动。

## 失败与边界

首次fixture遗漏Resources/runtime工作目录，后台报No such file；补齐源码资源链接后，点击“重新启动后台”恢复，未改生产fallback。捕捉偶发ScreenCaptureKit -3811及缩放过渡帧，采用后续控件树和稳定截图确认。预览翻页键尝试未取得充分画面证据，不算通过。

UI保存失败重试、全页面语言切换、选区/undo/输入法、完整键盘/主题/窄窗矩阵、Windows原生及便携候选包GUI仍未完成。Pi重登和Claude真实模型边界不变，#16/#17及完整目标不关闭。

收据在`.artifacts/native-resume/{receipt.json,export-copy-receipt.json,native.log,exit.json,reopen.log,reopen-exit.json}`。本轮没有产品/测试改动、新ZIP或依赖树副本；运行Mac构建、原生操作、文件逐字与SQLite只读检查，未跑全量测试。

## 原生主题修复与真实保存失败重试

上一轮稳定截图显示：浅色内容区上方仍是深色原生标题栏。追到`appearance::apply`只调用组件Theme；安装的GPUI明确提供`App::set_window_appearance`，组件Theme::change自身不设置原生外观。现于同一apply入口同步Light/Dark，System传None清除应用覆盖；仍在偏好成功保存后才应用，不改持久化格式、字号或用户系统设置。其他平台该GPUI接口为no-op，Windows仍须实际验收。

修复构建在实际Mac窗口恢复浅色时，标题栏也变浅；切深色后两者同为深色。再切浅色→跟随系统，标题栏和内容回到系统深色，界面仍可操作，无强制固定浅色遗留。构建文件SHA256为`ab996829fda1bb3a950c98c6d292954b1ea017432c2c429c080dcd88fb0b23e6`；基线4620b98加appearance.rs本次修改。

导出失败使用隔离项目中新建的`native-save-retry.md`（内容OWNED_IMMUTABLE_ORIGINAL），仅该文件设置macOS用户immutable标志。实际系统保存选中该精确文件名/项目目录并确认替换，原子persist真实返回Operation not permitted（os error 1）；原文件及保护标志均断言不变。截图显示“保存失败，快照仍保留”及原始诊断，选项仍关闭、复制/保存按钮可用。解除该测试文件标志后，不关闭快照，重新系统保存并确认替换成功。保存文件与同一快照复制后粘贴的落盘草稿逐字一致：11,323字符，1200段及尾部保留，SHA256为`f66ef5bdcbf74818422ef24a1b97825de4902113bdf6542d7124b1505ef11777`；原聊天草稿/隐藏上下文/另一会话/未选过程均排除。临时粘贴草稿随后通过界面清空，回到原会话并Cmd+Q，原生53897正常exit0、shutdown成功，无自有后台残留，测试文件保护标志已清除。

曾拿另一份旧fixture的中文保存文件作golden，因会话/附件ID不同及本地化后的产物前缀标点不同断言失败；没有修改产品迁就旧文件，改为本次实际剪贴板与保存文件直接逐字核对。记录在save-retry-receipt.json，不将旧golden比较记为通过。

Mac构建、Windows默认MSVC交叉类型检查、既有appearance::tests两条及git diff --check通过。日志分别在`/tmp/hive-native-appearance-{build,windows,test}.log`；收据`.artifacts/native-resume/{appearance-save-receipt,save-failure,save-retry-receipt}.json`。无新增/删除测试；这次纯外观入口修复按原生走查和已有偏好回归验证，未跑无关全量测试、未请求模型或制作新ZIP。

新发现：保存失败/成功文字在画面可见，但当前AX树只返回开关/预览/按钮，没有这些提示文本；辅助技术错误通知仍需修复和实际验证。窄窗带长错误、完整键盘/输入法、Windows与候选包仍未验收。上述“保存失败重试未完成”已由本段补齐Mac普通窗口路径，不能推广到全部平台与可访问性矩阵。

## 导出提示可读取与窄窗固定页脚

修复上述AX遗漏：提示使用稳定节点及Status/Alert角色，完整已本地化提示作为aria_label；保留原始系统诊断。原生AX先读到中文复制/取消提示，最终布局又读到英文失败、保存路径、复制与取消提示。这里只证明AX可读取，未开启VoiceOver实测自动播报，不声称已完成屏幕阅读器体验。

同一1184×1104截图像素窗口复现错误换行后按钮底部裁切。复用现有Dialog footer，将预览范围说明、状态/错误和操作一起固定；新增私有ExportFooter视图持有同一个ConversationExport实体，观察原实体通知，保存忙碌/恢复状态仍来自原保存流程。快照、选项、文件写入与协议没有另建副本或改语义。

保留失败迭代：仅固定按钮时诊断末行仍裁切；禁止根容器收缩、尝试弹性预览都未取得完整诊断/滚动证据，均撤回。最终稳定截图确认两行完整EPERM诊断、预览范围说明和两个完整按钮同时可见，保存成功的长路径也完整换行；不是靠缩小字号或隐藏诊断通过。预览区域仍需另做完整滚动/键盘矩阵，本轮不将该项算通过。

实际保护同一隔离文件，保存失败后原文OWNED_A11Y_ORIGINAL及immutable标志不变；解除标志后通过固定页脚重试成功。文件与此前同一会话的英文导出逐字一致：11,576字符、1200段，SHA256 `6095efbaac24b4aa75721a7e91849d498f6acccb583ba7fecbabc60377ab7a5e`。原草稿保持，SQLite messages=0、日志无chat.prompt。所有本轮验证进程均以Cmd+Q退出0，最终6551及自有后台已退出，测试保护标志清除。

最终源码Mac构建、Windows默认MSVC交叉类型、既有export_preserves_selected_content一条及diff检查通过；未增删测试、未跑无关全量测试、未制作ZIP。日志`/tmp/hive-export-a11y-delivery-{build,windows,test}.log`。收据`.artifacts/native-resume/a11y-receipt.json`记录原生验证程序与最终构建两份哈希：原生走查后仅把私有类型ExportActions更名为ExportFooter并rustfmt，随后重新构建/检查。各中间版的退出与故障证据保留在a11y、a11y-footer、a11y-final、a11y-flex及a11y-fixed-status日志/收据，未将未通过的布局记为通过。

## 长预览键盘阅读与文件选区复制

源码75ae2b5的实际构建（SHA256 `705ab9453aa49332cf4727dc01200c48af24934cb96d898d9bd7cbb38a1ce109`）复用同一隔离应用与历史，无产品代码改动。点击导出预览后，End实际到长正文末段、Home返回标题/工作区，PageDown显示附件/Agent正文段，PageUp返回前段。截图不支持“正文被压成不可滚动”的猜测，因此未为该猜测修改共用text_reading组件。

在隔离项目创建原先不存在的review.txt：BEGIN、200行编号中文、END，共202行/10,318字节。从成员汇报产物入口打开当前磁盘文件，End显示第200行与END；Home→PageDown显示第26行起，PageUp回到BEGIN。鼠标选第1行并Cmd+C，粘贴到另一空白测试会话，落盘精确等于`READING-001 · 中文文件阅读验证 · line 1`，未混入完整文件或其他文字。临时草稿清空、原草稿保持，实际Cmd+Q退出0，shutdown成功，自有原生/runtime退出；SQLite messages=0、无chat.prompt或厂商请求。

滚轮仍未获可靠位移证据：CUA按页滚动画面不足以确认滚动，像素距离被工具直接拒绝（macOS只接受pages）；不记通过，也不据此归因产品bug。工具详情只查了共用组件调用，未做本轮原生走查；完整滚轮/键盘修饰键、输入法/字号/窄窗及Windows矩阵继续开放。

收据`.artifacts/native-resume/{reading-receipt.json,reading-selection.json,reading-before.log,reading-before-exit.json}`。仅添加隔离验证文件与记录，未改生产代码/测试、未重复无变更构建或跑全量测试、未新增ZIP。
