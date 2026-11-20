# 桌面操作体验持续对标

目标是让用户在实际项目里少绕路。每轮从三个参考产品中找出能复现的操作差距，先修容易误操作、丢失当前状态或反复多点的地方。记录同一操作在 Hive 的修复前后表现，以及实际验证的平台。任务见 [#57](https://github.com/tt-a1i/hive-desktop/issues/57)。

## 2026-11-21 第一批

Hive Desktop 从 `9555db1` 开始。Hive 上游 `main` 为 `18861b80379bff0d658623a3aa6f1454159d6925`，与已有依赖基线一致。上一轮验收见[记录](validation/2026-11-21-comprehensive-workflows.md)。

| 操作 | 当前改动 | 参考 |
| --- | --- | --- |
| 导入多个项目后进入指定项目 | 导入结果的每个成功项目都有打开入口。用户可以直接进入任一项目查看导入的会话。 | [T3 Code 欢迎页](https://github.com/pingdotgg/t3code/blob/ccc1feb15572c422fbdce26ba6077b91459cfd47/docs/user/welcome-page.md) |
| 只清理一个项目的待办 | 跨项目收件箱可筛选到指定项目。项目身份、暂缓状态和派单跳转都沿用原来的精确记录。 | [Cindy 收件箱规则](https://github.com/makecindy/cindy/blob/86c473f9ecc9a4deeb8180bc21deb0416411d828/docs/product-rules/task-inbox.md#L20-L23) |
| 同屏阅读长文件与聊天 | 通过分隔线拖曳或方向键调整文件面板宽度，双击或按 Home 恢复。当前窗口关闭再打开面板会恢复选定宽度。 | [Beluga 面板宽度](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/RightPanel.tsx)、[分隔线操作](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ResizeHandle.tsx) |
| 用键盘找同一会话里的旧消息 | 查找栏搜索尚未加载的历史。跳页时保留查询和结果，Enter 和 Shift+Enter 在结果之间移动。 | [T3 Code 会话内查找](https://github.com/pingdotgg/t3code/blob/ccc1feb15572c422fbdce26ba6077b91459cfd47/docs/user/keybindings.md) |

macOS 隔离窗口已经验证，两个项目结果中可直接打开第二个项目并查看它的原始会话。收件箱筛选到 Atlas 时只显示 Atlas 事项，返回全部项目后重现 Atlas 和 Beacon 两项目。文件面板通过键盘从 320 调到 344 像素，拖曳到 364 像素后关闭重开保持宽度，按 Home 回到 320 像素。收据在 `.artifacts/open-project-native`、`.artifacts/inbox-filter-native` 和 `.artifacts/dock-width-native`。

在 360 条消息的 Pi 会话里，用 Cmd+F 搜到两条未加载的消息。实窗用上、下结果按钮往返两个历史窗口，查询和值 `1/2`、`2/2` 保留。第三批复验通过，Return 前进，Shift+Return 返回，Esc 关闭查找栏并保留定位。整段只发出一次历史查询，每个窗口只返回 120 条，原始会话文件的哈希一致。上一轮事件注入把 Command 带到了普通 Return，改用独立事件源后按键正确。运行时检查覆盖 Codex 已存、已导入及实时会话，含身份、取消请求、可搜索字段和结果上限。实窗收据在 `.artifacts/competitive-ux/native-find-trace/receipt.json`。

## 2026-11-21 第二批

- **更快看出谁在等你。** 收件箱把需要关注、等待 Agent 和已暂缓的事项分开。等待 Agent 默认收起，有明确的条数。需要处理的错误和问题仍直接显示。打开已暂缓后能看到其中的等待事项。 [Cindy 收件箱规则](https://github.com/makecindy/cindy/blob/86c473f9ecc9a4deeb8180bc21deb0416411d828/docs/product-rules/task-inbox.md#L34-L52)
- **返回工作树时恢复展开的目录。** 同一个项目路径的会话共享展开记录，独立工作树和导入会话的子目录按原路径各自记忆。切回时重新读取目录，失败后可以重试。树的展开状态在当前窗口保存，文件预览仍由当前工作树决定。 [Beluga 文件面板](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L148-L185)
- **从长项目列表选出想导入的几个。** 先显示最近五个，打开全部项目后按名称或路径搜索。筛选变化会保留被隐藏的勾选项，导入前显示总数以及当前看不见的数量。 [T3 Code 项目推荐](https://github.com/pingdotgg/t3code/blob/ccc1feb15572c422fbdce26ba6077b91459cfd47/docs/user/welcome-page.md#L65-L83)

- macOS 收件箱实窗中，Beacon 的问题立即可见，Atlas 的等待汇报默认收起。展开后可继续暂缓 Atlas，查看暂缓时单独显示。切换到 Beacon 隐藏 Atlas 的暂缓项，点击问题打开准确派单。证据在 `.artifacts/inbox-sections-qa`。
- macOS 文件树实窗中，两个使用项目根目录的会话共享目录状态。A、B 两个独立工作树各自恢复展开的层级。收起 A 的上层目录后切走再回到 A，上层仍收起，主动展开后下层状态恢复。证据在 `.artifacts/tree-restore-qa`。
- macOS 欢迎页在七个项目中先显示最近五个。展开全部项目后，输入 Fjord 只显示匹配项目。对 Blue 和 Fjord 执行批量操作后，后台确认 Blue 的三段会话已经导入，Fjord 新增三段。证据在 `.artifacts/welcome-search-qa/receipt.md`。
- 176 项桌面 Rust 测试通过、1 项原有忽略。macOS 常规构建检查、Windows x64 MSVC 交叉类型检查、运行时类型检查和构建、历史搜索和项目文件专项回归均通过。Windows 原生交互未复验。

## 2026-11-21 第三批

本轮从 Hive Desktop `6b799c8` 开始。重新核对三个参考仓库的默认分支，T3 Code 为 `ccc1feb`，Cindy 为 `cf58205`，Beluga 为 `45a484b`。Hive 上游基线仍是 `18861b8`。

- **恢复未完成的项目导入。** 勾选后退出，重新打开欢迎页并查找时，从当前扫描结果按原路径恢复勾选。成功项目会退出待导入选择，部分成功或仍未发现的项目会留到下次。用户可以在当前筛选下仍看到刚提交的结果。 [T3 Code 导入恢复](https://github.com/pingdotgg/t3code/blob/ccc1feb15572c422fbdce26ba6077b91459cfd47/docs/user/welcome-page.md#L95-L103)
- **收件箱更新时保住正在操作的按钮。** 指针停在操作区、键盘聚焦操作按钮或打开暂缓菜单时，新事项等待本次操作结束，再进入优先级排序。打开时再次确认项目与目的地。 [Cindy 收件箱规则](https://github.com/makecindy/cindy/blob/cf58205bed0865c911ff052393d05d478d47c96b/docs/product-rules/task-inbox.md#L53-L57)
- **从 Diff 和聊天文件引用定位当前文件。** 文件面板在当前项目或工作树逐层加载并展开目录，自动翻页、滚动到文件并选中，按 Enter 可以打开预览。项目文件引用会核对来源再提供入口。删除的文件不提供定位，临时消失的文件有提示。 [Beluga 文件定位](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L186-L208)

- macOS 欢迎页实窗勾选 Blue 和 Amber 后按 Amber 筛选。在测试代理只返回 Blue 结果时，Blue 的打开入口仍显示，只保留 Amber 的路径。重启后重新扫描并直接导入，只提交 Amber 新生成的发现 ID。后台报告一条新会话、三条已导入。证据在 `.artifacts/welcome-persist-qa/receipt.md`。
- 原生欢迎页扫描完成后，将视野移到新结果。修复前项目勾选和导入按钮会落在固定页脚下方。修复后，同尺寸窗口可直接点第一个项目和导入按钮。如果结果已经可见，再扫描时位置保持，手动滚动也保留。证据在 `.artifacts/welcome-reveal-after`。
- macOS 收件箱的轮询已经读到 Atlas 的新问题时，鼠标指着的 Beacon 按钮保持原位。移开鼠标后，Atlas 在同一个收件箱中出现在 Beacon 上方。另一次实窗操作用原按钮打开了准确的 Beacon 派单。键盘焦点和暂缓菜单锁定有定向测试，未做这两项实窗复验。证据在 `.artifacts/inbox-poll-repeat/receipt.md`。
- macOS Diff 实窗从项目和工作树 A 分别定位同名文件，并通过 Enter 预览到各自不同的内容。定位工作树中的 `page-109.txt` 时，后台自动读完前 100 个条目再取下一页。聊天文件引用入口、改名／删除和窄窗交互未单独做实窗复验。证据在 `.artifacts/diff-reveal-qa/receipt.md`。
- 184 项桌面 Rust 测试通过，1 项原有忽略。项目文件、项目文件引用和工作树文件引用的运行时检查通过。Windows x64 MSVC 通过交叉类型检查，原生窗口待复验。欢迎页、收件箱和 Diff 实窗使用隔离项目和历史，没有发起模型请求。

## 2026-11-21 第四批

本轮从 Hive Desktop `e5ff84a` 开始。T3 Code 默认分支为 `667d8a1`，Cindy 为 `48acd5f`，Beluga 为 `45a484b`。Hive 上游基线仍是 `18861b8`。

- **临时切换本条消息的排队或插话。** 在当前回合运行时，macOS 按 `Cmd+Enter`，Windows 按 `Ctrl+Enter`，即可对这条消息使用与默认相反的方式。发送按钮沿用默认方式并提示快捷键。等待空闲设置仍会保留草稿。 [T3 Code 输入快捷键](https://github.com/pingdotgg/t3code/blob/667d8a120aa72e8c37795c9e391596acb141170b/docs/user/keybindings.md#L14-L24)
- **返回收件箱时回到正在看的事项。** 筛到指定项目，滚动到后面的事项并打开，再打开收件箱。恢复项目、展开区段和此前在视野顶部的事项。列表重排时按事项身份定位。原事项消失时回到本项目的顶部，项目移除时回到全部项目。视图状态在当前应用会话中保留。 [Cindy 收件箱视图规则](https://github.com/makecindy/cindy/blob/48acd5fff52ffcd9892a8dc9759e8fe5fa31d706/docs/product-rules/task-inbox.md#L14-L23)
- **直接从文件树看出当前工作树的改动。** 文件行标记新增、修改、改名或冲突，有改动后代的目录显示提示。文件面板打开、执行目录变化或手动刷新时重新读取。一次最多读取 500 条，有遗漏时提示。结果依据当前浏览范围，切换时清除上一工作树的标记。 [Beluga 文件树与改动](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L209-L224)

- macOS 聊天实窗用受控的 Codex 协议替身验证两种默认发送设置以及等待空闲设置。插话只进当前回合，排队内容在释放后进入下一回合，普通 Return 和 Shift+Return 在修饰键发送模式下会换行。等待空闲期间草稿保留，空闲后发送一次。中文输入法候选选定及 Windows 实体键盘待复验。证据在 `.artifacts/composer-busy-native/receipt.md`。
- macOS 收件箱实窗验证了 Beacon 项目筛选、打开派单及返回位置。关闭期间在列表前插入更晚的待处理问题，重开后仍看到同一条顶部事项，原派单按钮的垂直坐标保持一致。展开区段和原事项消失的分支未做实窗复验。证据在 `.artifacts/inbox-return-qa/receipt.md`。
- macOS 文件树实窗验证项目目录及工作树 A、B 分别显示各自的新增文件，关闭或展开父目录时仍显示改动提示。工作树 A 的 `note.txt` 显示修改，B 的同名文件不显示修改。编辑和还原 A 的文件后重新读取，修改标记随之出现和消失。证据在 `.artifacts/tree-badges-qa/receipt.md`。
- 189 项桌面 Rust 测试通过，1 项原有忽略。运行时 Git 读取、工作区绑定和消息队列的 RPC 检查通过。Windows x64 MSVC 通过交叉类型检查。

## 接下来优先检查

- **有就绪 Agent 时先开一段没有项目的会话。** 当前空白安装需要先选目录。T3 Code 从欢迎页进入空状态，再通过专用入口创建本机临时目录。用空项目和可用 Agent 验证 Hive 的首次发送、实际执行目录和冷启动恢复，并明确用户文件的保留规则。 [T3 Code 欢迎按钮](https://github.com/pingdotgg/t3code/blob/667d8a120aa72e8c37795c9e391596acb141170b/apps/web/src/components/onboarding/WelcomePage.tsx#L322-L335) · [T3 Code 空状态](https://github.com/pingdotgg/t3code/blob/667d8a120aa72e8c37795c9e391596acb141170b/apps/web/src/components/NoProjectsHero.tsx#L15-L54)
- **从有改动的文件直接打开 Diff。** 在两个工作树中各改同名文件，从文件树的改动标记打开 Diff，验证选中的是当时的工作树和确切路径。当前可从 Diff 定位到文件树。 [Beluga 文件行操作](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L242-L252)
- **收件箱入口提示有新的待处理事项。** 关闭收件箱，向另一项目送入需要用户答复的协作问题。检查入口能否提示新事项，打开后进入正确项目。当前必须先打开收件箱才会刷新列表。 [Cindy 收件箱入口行为](https://github.com/makecindy/cindy/blob/48acd5fff52ffcd9892a8dc9759e8fe5fa31d706/docs/product-rules/task-inbox.md#L3-L6)
