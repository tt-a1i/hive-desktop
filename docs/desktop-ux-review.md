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

## 2026-11-21 第五批

本轮从 Hive Desktop `06bb654` 开始。T3 Code `667d8a1`、Cindy `48acd5f`、Beluga `45a484b` 和 Hive 上游 `18861b8` 仍是各自默认分支。

- **从欢迎页开启无项目对话。** 选择 Agent 后可直接开始。Hive 在桌面数据目录内创建或复用可见的 Scratch 工作区，移除工作区时保留目录与文件。同一次启动因回包失败而重试，仍返回同一条会话。已有项目时沿用普通会话入口。 [T3 Code 空状态](https://github.com/pingdotgg/t3code/blob/667d8a120aa72e8c37795c9e391596acb141170b/apps/web/src/components/NoProjectsHero.tsx#L15-L54)
- **从文件树打开当前工作树的精确 Diff。** 改动标记提供打开入口。Hive 重新读取当前范围的 Git 状态，为目标文件打开所属页，只有暂存改动时进入暂存比较。路径缺失或身份有歧义时显示错误。点击文件行仍可预览。 [Beluga 文件行操作](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L242-L252)
- **收件箱关闭时显示新事项。** 用户未查看、未暂缓的待答复协作问题及窗口已知未读会话会点亮入口。后台读取轻量身份摘要，打开收件箱时隐藏提示。用项目筛选查看时，只确认该范围已呈现的事项。 [Cindy 收件箱入口行为](https://github.com/makecindy/cindy/blob/48acd5fff52ffcd9892a8dc9759e8fe5fa31d706/docs/product-rules/task-inbox.md#L3-L6)

- macOS 使用零工作区和受控 External Pi 接通欢迎页。一次创建请求的 ID 与返回和打开的会话一致。发送前没有提交提示。发送后对端在 Scratch 写出测试文件，冷启动恢复了原工作区、会话和对话内容。运行时回归另验了并发重试、工作区回包丢失和移除后保留文件。证据在 `.artifacts/scratch-first-chat-qa/receipt.md`。
- macOS 在收件箱关闭时向 Atlas 加入新问题，入口显示提示。按保存的 Beacon 项目筛选查看后，Atlas 的提示保留。重开应用并查看全部项目后，关闭收件箱清除了提示。证据在 `.artifacts/inbox-badge-qa/receipt.md`。
- macOS 从文件树分别打开项目目录和工作树 A、B 的同名 `docs/nested/note.txt`，三个 Diff 各显示本范围的内容。A 从文件浏览弹窗发起，另两个使用停靠面板。在 112 条 Git 改动的工作树中，也能打开第二页的 `page-109.txt`，并自动选择暂存比较。证据在 `.artifacts/tree-to-diff-qa/receipt.md` 和 `paging-receipt.md`。
- 194 项桌面 Rust 测试通过，1 项原有忽略。TypeScript 类型检查、运行时构建和 Windows x64 MSVC 交叉检查通过。macOS 验收使用独立测试项目和协议对端，未调用模型服务。Windows 原生界面待复验。

## 2026-11-21 第六批

本轮从 Hive Desktop `02f9d63` 开始。参考默认分支为 T3 Code `667d8a1`、Cindy `79d93b1`、Beluga `45a484b`，Hive 上游仍是 `18861b8`。

- **新建 Scratch 对话各有自己的执行目录。** 欢迎页和普通新建入口都分配独立目录，Agent、文件浏览和自动终端跟随会话。明确选择工作区终端时仍进入 Scratch 根目录。旧会话继续使用原目录，移除工作区保留文件。目录缺失或被替换时会提示错误。 [T3 Code 无项目对话](https://github.com/pingdotgg/t3code/blob/667d8a120aa72e8c37795c9e391596acb141170b/docs/user/thread-sidebar.md#L19-L33)
- **长文件可展开到更大的阅读窗口。** 点开路径或正文命中的预览后可以展开，原行位置或当前页一并带入。展开后可翻页或回到命中行，关闭后回到原筛选、选中文件、页码和滚动位置。 [Beluga 文件阅读](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L104-L116)
- **单独查看跨项目的新事项。** 收件箱内可打开新事项列表，再返回保存的项目和阅读位置。事项进入当前窗口的可见区域才会确认看过，在新事项列表中仍可继续处理它。 [Cindy 视图规则](https://github.com/makecindy/cindy/blob/79d93b160b97f7617f02a8821bfde976fa02f4b5/docs/product-rules/task-inbox.md#L14-L23)

- macOS 在同一 Scratch 工作区新建两段 External Pi 会话。发送前没有提交提示。两段会话各自写入同名文件，读回的内容不同。退出重开后，同一个工作区里的两段历史分别恢复。另用运行时验证旧目录、并发和冷启动重试、附件范围、目录被替换及移除工作区后保留文件。证据在 `.artifacts/scratch-two-qa/desktop-receipt.json` 和 `.artifacts/scratch-conversations-native/runtime-product-check.json`。
- macOS 从两段 Scratch 对话分别打开自动终端，用 `pwd -P` 验证各自的执行目录。切到工作区终端后，`pwd -P` 返回 Scratch 根目录。证据在 `.artifacts/scratch-two-qa/terminal-receipt.json`。
- macOS 从 700 行文件的第 350 行正文命中展开阅读，翻到第 513 行开头的一页，回到命中位置，再返回保留查询和预览的文件面板。切到工作树后，同名文件显示工作树自己的内容。每个文件只读一次，展开、翻页和返回均沿用已加载内容。另在 900 像素窄窗中从文件树打开、展开预览并返回，选中文件和滚动位置保持。证据在 `.artifacts/content-preview-qa/receipt.md`。
- macOS 在 Beacon 项目收件箱滚动到较早的事项，关闭期间向 Atlas 加入新问题。查看新事项时能打开准确的 Atlas 派单，返回时恢复 Beacon 筛选，原事项按钮的垂直坐标相同。证据在 `.artifacts/inbox-review-qa/receipt.md`。
- 197 项桌面 Rust 测试通过，1 项原有忽略。运行时构建、TypeScript 检查、Scratch 和会话终端专项回归、Windows x64 MSVC 交叉类型检查均通过。macOS 实窗使用隔离测试数据和受控协议对端。Windows 原生交互待复验。

## 2026-11-21 第七批

本轮从 Hive Desktop `d45caa7` 开始。参考默认分支为 T3 Code `c813e7d`、Cindy `79d93b1`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **把 Scratch 文件夹添加为项目。** 从拥有独立目录的会话菜单添加并命名，Hive 在原路径登记项目。项目入口可以打开原会话，原来的 Agent 身份、历史、附件和协作账本仍归原工作区。回包中断后可用同一入口核对结果。移除项目保留原会话和磁盘文件，仍有链接时不能移除 Scratch 工作区。 [T3 Code 转为项目](https://github.com/pingdotgg/t3code/blob/c813e7db513aa165863d83f230f0989e3ca845eb/docs/user/thread-sidebar.md#L43-L47)
- **在编辑器打开正在预览的文件。** 紧凑或展开预览都能通过图标和名称选择已安装的编辑器。Hive 核对当前会话目录和选中文件，再把准确的绝对路径交给编辑器。请求期间切换选中项或工作树会取消这次打开。 [Beluga 预览行操作](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L104-L113)
- **将事项暂缓到指定日期和时间。** 工作区收件箱和跨项目收件箱共用日期与时间选择器。保存前显示本地唤回时间和时区偏移，新输入可以提前唤回。明天同一时间按日历计算，提示因夏令时而不存在的本地钟点。 [Cindy 暂缓规则](https://github.com/makecindy/cindy/blob/79d93b160b97f7617f02a8821bfde976fa02f4b5/docs/product-rules/task-inbox.md#L75-L89)

- macOS 在两段 Scratch 对话中选择 Alpha，输入项目名称并用 Return 确认，只提交一次请求。项目路径与 Alpha 原执行目录一致，能从项目打开原会话。重启后两段对话仍各有原来的原生会话 ID、历史和文件。运行时同时验证回包丢失后恢复、附件血缘和删除保护。证据在 `.artifacts/scratch-promote-qa` 和 `.artifacts/competitive-ux/seventh/scratch-promotion.log`。
- macOS 从项目、工作树 A 的紧凑预览和工作树 B 的展开预览选择 Zed。操作系统确认 Zed 实际读取了三处带中文、空格和 `&` 的同名文件。菜单有对应 Zed 的无障碍名称。运行时专项另验证缺失文件和符号链接会被拒绝。证据在 `.artifacts/editor-open-qa/receipt.md`。
- macOS 收件箱先取消自定义暂缓，确认没有写入。将 Beacon 问题改为当地 11:37 后保存，磁盘记录与所选时刻一致。冷启动后仍在暂缓区，新问题到来时原事项提前恢复。从等待 Agent 区段将 Atlas 的事项暂缓到 15:42，保存的时间与辅助技术读到的日期、时间和时区一致。证据在 `.artifacts/inbox-custom-qa`。
- 203 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 通过交叉类型检查，Windows 原生交互待复验。TypeScript 检查、运行时构建、项目文件和工作树引用、Scratch 创建和转项目、工作区删除专项通过。

## 2026-11-21 第八批

本轮从 Hive Desktop `2e0ef71` 开始。参考默认分支为 T3 Code `c813e7d`、Cindy `79d93b1`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **从全局新建入口选择新对话的项目。** 多个项目时可以按名称或路径搜索，选择后再创建。关闭选择器可以回到原位置继续写草稿。`Cmd/Ctrl+Shift+N` 或按住 Shift 点击新建可直接用当前项目。还可以在专用 Scratch 文件夹开始，不必先选已有项目。 [T3 Code 新对话](https://github.com/pingdotgg/t3code/blob/c813e7db513aa165863d83f230f0989e3ca845eb/docs/user/thread-sidebar.md#L8-L13) · [快捷键](https://github.com/pingdotgg/t3code/blob/c813e7db513aa165863d83f230f0989e3ca845eb/packages/shared/src/keybindings.ts#L108-L112)
- **在暂缓列表查看和调整唤回时间。** 两个收件箱都显示当前时区的唤回时刻或「无限期」。重新打开自定义调整时带入原时刻。菜单增加 3 小时、今晚 18:00、明天 09:00 和下周一 09:00。过了今晚的时刻就收起该项。事项提前唤回或已经改期时，旧操作不可覆盖新状态。 [Cindy 暂缓规则](https://github.com/makecindy/cindy/blob/79d93b160b97f7617f02a8821bfde976fa02f4b5/docs/product-rules/task-inbox.md#L75-L89)
- **用键盘展开文件预览并返回。** 在文件浏览区按 `Cmd/Ctrl+Enter` 展开当前已读入的文件，按 Esc 回到原位置。停靠面板和窄窗的文件弹层共用这套操作。在搜索框和聊天输入区可继续使用原有快捷键。 [Beluga 预览快捷键](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L110-L113)

- macOS 从 Alpha 按 `Cmd+N`，按 Esc 后直接继续打字，原草稿和附件仍在。在选择器按路径找到 Beta 后用 Return 确认。将实际创建回包延迟四秒，界面在等待期间留在 Alpha。收到回包后打开新 Beta 会话，首次持久化导航就使用它的准确 ID。返回 Alpha 时原草稿和附件哈希一致。另验 `Cmd+Shift+N` 直接在当前 Beta 新建。证据在 `.artifacts/chooser-eighth-qa`。
- macOS 使用全新数据目录，从欢迎页按 `Cmd+N`，屏幕阅读器可读出「不使用项目」，按 Return 创建并打开同一条 Scratch 会话。证据在 `.artifacts/chooser-scratch-eighth-qa/receipt.json`。
- macOS 跨项目收件箱实测用「明天 09:00」保存，列表显示该时刻。打开自定义调整时带入 09:00，调整到 12:43 后核对磁盘上的同一事项和精确时间。重新打开调整，插入同一派单的新消息后，旧表单保存时保留错误提示，事项保持唤回。证据在 `.artifacts/snooze-eighth-qa`。
- macOS 串联验证停靠和 900 像素窄窗。搜索框内按 `Cmd+Enter` 保留搜索，选中文件后按相同快捷键展开第 350 行命中。Esc 返回后仍有原查询、命中行和文件预览。窄窗中 Esc 只关闭展开阅读层，返回原文件树和预览。宽窗返回后不点鼠标，再按相同快捷键可以重新展开，验证了焦点恢复。证据在 `.artifacts/reader-keys-qa/receipt.md`。
- 复测最初对照 Maka 发现的旧消息定位问题。对 10,000 条消息请求最近一页和第 50 条附近，各返回 120 条。定位页保留向前和向后游标。现有分页专项还验证了两向完整遍历。证据在 `.artifacts/competitive-ux/eighth/old-history-anchor.json` 和 `history-window.log`。 [Maka Rust 分支的游标查询](https://github.com/apache/maka/blob/871e747340088c98d69584eb8bdd5b9b3df84bd3/plugins/agent/state/src/session_history/source_tree/queries.rs#L83-L104)
- 209 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 交叉类型检查、TypeScript 检查、Scratch 和历史分页专项通过。Windows 原生交互待实机复验。

## 2026-11-21 第九批

本轮从 Hive Desktop `e8380e0` 开始。参考默认分支为 T3 Code `c813e7d`、Cindy `4e41737`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **将未发送的草稿复制到另一个项目。** 在编辑区的项目菜单选择目标，文字和附件进入目标项目的一条新对话，原草稿与目标项目的已有草稿都保留。项目文件引用按原附件的字节复制为普通文件。复制本身不发送消息。回包中断后可以恢复同一条复制。 [T3 Code 草稿换项目](https://github.com/pingdotgg/t3code/blob/c813e7db513aa165863d83f230f0989e3ca845eb/docs/user/thread-sidebar.md#L25-L33)
- **当场撤销收件箱里的暂缓操作。** 工作区和跨项目收件箱在暂缓、改期或取消暂缓后提供八秒撤销入口。撤销恢复同一派单的原唤回时间。如果该派单已收到新输入或已有更新的操作，保留新状态。 [T3 Code 操作撤销](https://github.com/pingdotgg/t3code/blob/c813e7db513aa165863d83f230f0989e3ca845eb/docs/user/thread-sidebar.md#L65-L69) · [Cindy 收件箱行操作](https://github.com/makecindy/cindy/blob/4e4173743fc2c3cee83d0c4ae2f3aae79a4f2df4/docs/product-rules/task-inbox.md#L59-L63)
- **在文件面板阅读 Markdown。** 通过文件树或路径打开时默认显示排版。正文搜索从准确的源码行打开。紧凑和展开的预览都能在排版和源码之间切换，保留源码阅读位置。文件超过 64 KiB 或 1,000 行，或连续非空行超过 8 KiB 时，显示源码。预览禁用嵌入图片，点击 HTTP(S) 链接可用系统浏览器打开。 [Beluga Markdown 预览](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L97-L118)

- macOS 从 Alpha 的空会话复制带文字、普通文件和项目文件快照的草稿。回包延迟四秒时仍留在 Alpha，回复后打开准确的 Beta 新会话。返回后原草稿与附件仍在，Beta 的既有草稿也保留。另一次在后台完成后、回包前退出，重开时恢复同一个会话 ID 和附件 ID，前台仍留在 Alpha。两次复制均从实际附件存储逐字节核对。证据在 `.artifacts/draft-copy-qa`。
- macOS 跨项目收件箱实测暂缓后撤销，原问题恢复可见。取消暂缓后撤销，原提醒文件的字节一致。再次取消暂缓后，在该派单加入新问题，立即撤销会提示状态已变化，保留新问题。证据在 `.artifacts/inbox-undo-qa`。
- macOS 打开包含标题和代码块的 Markdown，正文搜索跳到第 350 行。在展开阅读中切换排版与源码，关闭后保留原查询和命中。在 900 像素窄窗打开同名文件，排版和源码都来自所选工作树。另用单段接近 19 KiB 的样例复验，打开时直接进入源码。链接目标由单元测试覆盖，实际浏览器跳转未单独实测。证据在 `.artifacts/markdown-reader-qa`。
- 217 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 MSVC 交叉类型检查、运行时构建和文件、图片、项目文件引用、Scratch 与工作区移除专项均通过。复制专项覆盖逐字节复制、并发重试、会话写入失败后的冷启动恢复，以及无 Agent 调用。运行时证据在 `.artifacts/draft-copy-runtime`。Windows 原生交互待实机复验。

## 2026-11-21 第十批

本轮从 Hive Desktop `170b004` 开始。参考默认分支为 T3 Code `068332c`、Cindy `4e41737`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **把文件直接拖到侧栏会话。** 普通行和置顶行都可接收桌面文件。Hive 确认目标会话和项目后，按拖入顺序将文件追加到其草稿。目标原有文字和附件保留，使用原有数量与类型限制。在打开回包前切走会取消这次拖入，上传开始后切走仍保存到已确认的目标。拖入本身不发送消息。 [T3 Code 拖附件到对话](https://github.com/pingdotgg/t3code/blob/068332c603ce5ca40ad5a96a09cffa34ebd05dff/docs/user/thread-sidebar.md#L72-L75)
- **按唤回时间查看暂缓事项。** 工作区和跨项目收件箱都把最早到期的提醒排在前面，最后显示无限期提醒。相同时间沿用原有优先级和更新顺序。跨项目收件箱保留正在操作的行，释放焦点或菜单后再应用新顺序。 [Cindy 暂缓排序](https://github.com/makecindy/cindy/blob/4e4173743fc2c3cee83d0c4ae2f3aae79a4f2df4/apps/desktop/src/renderer/features/cc-agent/sidebar/TaskInbox.tsx#L124-L128)
- **用方向键连续预览文件。** 在文件树用上、下键移动后，停留 120 毫秒即预览选中文件，Return 仍立即打开。切出文件树、搜索或切换工作树时取消之前的预览任务。打开目录和错误重试仍由显式操作触发。 [Beluga 键盘预览](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L270-L283)

- macOS 用独立 AppKit 测试窗把文本文件和 PNG 从操作系统拖到 Beta 会话，Alpha 当时处于前台。目标确认回包延迟四秒，首个上传在回包后发出。两份文件只写入 Beta，逐字节等于源文件。Alpha 草稿、Beta 已有文字和附件都保留。再次拖入并在目标回包前切回 Alpha，没有产生新上传。全程没有发送消息。证据在 `.artifacts/sidebar-drop-qa`。
- macOS 跨项目收件箱按顺序显示 Atlas 较早提醒、Beacon 较晚提醒和 Beacon 无限期提醒。把 Beacon 较晚提醒改为 15 分钟后，移开指针才更新顺序。撤销后恢复原顺序和时间。两处收件箱共用的专项测试同时覆盖到期唤回。证据在 `.artifacts/snooze-order-qa`。
- macOS 在项目文件树以 25 毫秒间隔连按三次向下键，后台只读取最终选中的文件。切到工作树后重复操作，后台只读取该工作树的文件，显示与项目根目录不同的内容。预览完成后按 Return 没有重复读取。证据在 `.artifacts/keyboard-debounce-qa`。
- 220 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 MSVC 交叉类型检查、TypeScript 检查、编译后的文件和图片附件协议专项、跨项目复制草稿回归均通过。Windows 原生交互待实机复验。

## 2026-11-21 第十一批

本轮从 Hive Desktop `b12fe72` 开始。参考默认分支为 T3 Code `068332c`、Cindy `55e3ad8`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **暂存正在写的消息。** 输入区按 `Cmd/Ctrl+S` 把文字与附件一起暂存，空输入区可恢复一条草稿，有多条时可用时间和文件名称选择。编辑区也有暂存入口。每个会话或项目草稿最多保存 20 条，暂存和恢复可跨重启，不发送消息。 [T3 Code 暂存草稿](https://github.com/pingdotgg/t3code/blob/068332c603ce5ca40ad5a96a09cffa34ebd05dff/docs/user/composer.md#L194-L203)
- **第一次打开跨项目收件箱时说明怎么用。** 同一窗口先介绍需要关注、暂缓和新事项，按 Esc 或开始使用即可看到列表，焦点回到收件箱帮助。完成状态保存在本机，帮助入口可随时再看。 [Cindy 首次收件箱介绍](https://github.com/makecindy/cindy/blob/55e3ad864ccaa5547adb28a3d16b71aa6bc30d7c/docs/product-rules/task-inbox.md#L8-L15)
- **从文件预览直接添加到草稿。** 停靠面板和展开阅读都有入口。附件绑定当前预览的会话执行目录、文件路径和内容，追加到当前草稿。文件变化时提示重新读取。窄窗在接受添加后回到编辑区，宽窗停靠面板可继续使用。 [Beluga 在聊天引用](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/views/FilesView.tsx#L95-L117)

- macOS 在 Alpha 用 Cmd+S 暂存两条未发送消息，其中一条带项目文件快照。冷启动后从列表选回第一条，核对原文字、附件归属和底层文件字节，另一条仍在暂存列表。Beta 的暂存列表为空。定向存储测试覆盖已有编辑内容和写入失败。证据在 `.artifacts/stash-eleven-qa`。
- macOS 用全新数据目录打开跨项目收件箱，首次显示引导，Esc 留在收件箱。关闭重开和冷启动后直接看到列表，仍可通过帮助重看。重看后按 Esc，再按空格或 Return 可打开帮助。引导内按 Return 开始使用，并把焦点交回帮助入口。证据在 `.artifacts/inbox-intro-qa`。
- macOS 在停靠面板打开工作树的同名文件，修改磁盘内容后添加会提示重新读取，且没有新附件。重新读取后成功添加，核对附件来自准确的工作树，字节与重新显示的内容一致。停靠面板保持打开。900 像素窄窗从展开预览添加后回到编辑区，原文字和已有附件都保留。证据在 `.artifacts/preview-attach-qa`。
- 225 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 MSVC 交叉类型检查、TypeScript 检查、编译后的附件协议专项及项目文件和工作树回归通过。Windows 原生交互待实机复验。

## 2026-11-21 第十二批

本轮从 Hive Desktop `a7034d1` 开始。参考默认分支为 T3 Code `068332c`、Cindy `9602463`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **复制会话引用，并用快捷命令打开。** 从标题栏或侧栏会话菜单复制，在快捷命令中选择「打开会话引用」。引用只含项目和会话标识，按标识查找；改名或归档后仍可使用。等待期间关闭输入面板或切走时，旧回包不会改变视图。 [T3 Code 查找和引用会话](https://github.com/pingdotgg/t3code/blob/068332c603ce5ca40ad5a96a09cffa34ebd05dff/docs/user/thread-sidebar.md#L235-L242)
- **重启后保留收件箱筛选。** 跨项目收件箱保存所选项目，以及「等待 Agent」和「已暂缓」的展开状态。后台确认项目已移除后回到全部项目，保留所选区段。 [Cindy 保留收件箱筛选](https://github.com/makecindy/cindy/blob/96024635d21ad07772501610c933f4d34536d09f/docs/product-rules/task-inbox.md#L17-L24)
- **在文件管理器中选中当前文件。** 停靠面板和展开阅读都提供入口。二进制文件和超过预览限制的文件可以从停靠面板定位。Hive 核对工作树与选中文件后交给系统文件管理器，切换工作树、文件或关闭阅读窗口会取消尚未完成的定位。 [Beluga 定位本地文件](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/views/FilesView.tsx#L95-L117)

- macOS 从两个项目的同名会话中复制 Beta 的引用，再改名并冷启动。通过快捷命令打开，后台按两个标识定位到已改名的 Beta。返回 Alpha 后草稿文字和原附件字节完整。另用 Return 进入引用输入框，提交后在延迟回包前按 Esc 并返回 Alpha，旧结果没有触发再次打开。证据在 `.artifacts/reference-twelve-qa`。
- macOS 选择 Atlas 并展开「等待 Agent」和「已暂缓」，重启后确认 Atlas 筛选和暂缓事项可见。移除测试项目后再次启动，收件箱回到全部项目。两次重启保留了两处展开设置，等待区段状态通过持久化记录核对。证据在 `.artifacts/inbox-layout-qa`。
- macOS 从停靠面板定位项目与独立工作树里超过预览限制的同名二进制文件。Finder 实际选中各自的路径，两处文件名均含中文、空格和 `&`。从展开预览也定位了准确的工作树文本文件。延迟四秒解析二进制文件期间切到另一工作树，Finder 仍选中文本文件。证据在 `.artifacts/finder-reveal-qa`。
- 232 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 MSVC 交叉类型检查、TypeScript 检查，编译后的精确会话引用、会话归档和项目文件协议回归通过。Windows 原生交互待实机复验。

## 2026-11-21 第十三批

本轮从 Hive Desktop `8ae6dfb` 开始。参考默认分支为 T3 Code `068332c`、Cindy `9602463`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **发送后继续撰写下一条草稿。** 空闲的普通项目会话可用编辑区按钮或 `Cmd/Ctrl+Alt+Enter` 发送并开始新草稿。原会话接收回复，新草稿沿用项目和 Agent 配置，并在发送第一条消息时连接。运行时在送出前再次核对原会话是否空闲，避免把消息误排入队列。 [T3 Code 后台发送](https://github.com/pingdotgg/t3code/blob/068332c603ce5ca40ad5a96a09cffa34ebd05dff/docs/user/thread-sidebar.md#L49-L54)
- **按成员筛选收件箱。** 跨项目收件箱可独立选择项目和成员，成员选项同时显示项目名。筛选按两个标识核对，问题也能按实际发问者找到。重启恢复两项选择，「新事项」临时显示所有项目和成员。 [Cindy 组合筛选](https://github.com/makecindy/cindy/blob/96024635d21ad07772501610c933f4d34536d09f/docs/product-rules/task-inbox.md#L20-L23)
- **复制正在预览文件的路径。** 停靠面板和展开阅读都可复制绝对路径或相对于当前项目或工作树的路径。绝对路径沿用已验证的文件定位；切换选中文件或工作树时取消尚未完成的复制。 [Beluga 复制文件路径](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/views/FilesView.tsx#L95-L117)

- macOS 用编辑区按钮发送合成消息时，本地模拟进程只收到原会话的一次请求。回复到达时，新会话仍显示独立草稿，冷启动后恢复草稿。证据在 `.artifacts/send-fresh-controlled-qa`。首次快捷键验收的 Agent 配置误指向本机 Codex，发出一条合成消息和一个合成附件。之后重新生成隔离配置，并核对本地模拟进程的启动记录后复测。
- macOS 在 Atlas 与 Beacon 的两个同名 Reviewer 之间切换，分别只显示所属项目的问题。「新事项」同时显示两边的问题，返回时恢复筛选。冷启动后项目和成员筛选仍在，选不相容的项目后回到全部成员。证据在 `.artifacts/inbox-member-qa/01-atlas-intersection.ax` 至 `08-incompatible-project.ax`。
- macOS 在停靠面板复制项目文件的相对和绝对路径，并在展开阅读复制独立工作树里的同名文件。路径内的中文、空格和 `&` 逐字吻合。延迟四秒解析工作树 A 的绝对路径期间切到 B，旧回包没有改写剪贴板。证据在 `.artifacts/copy-path-qa/receipt.md`。
- 237 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 MSVC 交叉类型检查、TypeScript 检查、编译后的 Codex 浏览和延迟连接协议、Hive HTTP/SQLite 和项目文件工作树专项通过。协议另验证原会话忙碌时，新草稿不能接走原 Agent。Windows 原生交互待实机复验。

## 2026-11-21 第十四批

本轮从 Hive Desktop `86ccfe0` 开始。参考默认分支为 T3 Code `e668e61`、Cindy `9602463`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **归档后当场撤销。** 归档成功后有五秒撤销入口。撤销核对工作区、会话和该次归档版本，并保留原置顶顺序。若归档前正在查看该会话，并且之后仍停留在项目起始页，撤销后回到原会话。 [T3 Code 归档撤销](https://github.com/pingdotgg/t3code/blob/e668e6141faef533509e1d38d0f984bb38431d3f/docs/user/thread-sidebar.md#L65-L70)
- **从收件箱标为已读。** 项目收件箱和跨项目收件箱的普通未读会话可直接标为已读。操作按工作区和会话标识定位。审批、待处理汇报以及投递失败继续展示；该会话有新回复时重新进入收件箱。 [Cindy 已读操作](https://github.com/makecindy/cindy/blob/96024635d21ad07772501610c933f4d34536d09f/docs/product-rules/task-inbox.md#L94-L98)
- **按目录分组和折叠 Git 改动。** 列表切换方式在重启后保留。方向键只经过可见文件，折叠全部目录后关闭文件预览并停用单文件暂存。选定文件或从文件面板跳转 Diff 时，按实际工作树和完整路径定位。 [Beluga 分组选择](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ChangesTab.tsx#L275-L295)

- macOS 归档两个项目的同名会话之一，撤销后回到准确的会话并恢复三个置顶会话的原顺序。再次归档后切到另一会话，撤销后当前选择保持。等六秒后，撤销按钮消失，归档仍然生效。证据在 `.artifacts/archive-undo-qa`。
- macOS 跨项目收件箱从两个同名未读会话中标记 Beacon，会话切换记录没有新增，当前草稿、Atlas 未读项和 Beacon 的待答复协作问题仍在。下一条受控 Beacon 回复重新出现于「新事项」。在 Atlas 的项目收件箱标记成员会话时，待汇报派单仍在，返回后原草稿文字完整。证据在 `.artifacts/inbox-read-qa`。
- macOS 折叠全部 Git 目录后，按键没有读取旧 Diff。重启后仍使用分组。在工作树 A 折叠夹在中间的顶层目录，方向键和回车在 `api/settings.ts` 与 `ui/settings.ts` 间往返，两个 Diff 来自 A。通过文件面板再进入折叠目录的 Diff 时也展开准确目录。项目和工作树的 Git 索引保持原样。证据在 `.artifacts/grouped-diff-qa`。
- 243 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 MSVC 交叉类型检查、TypeScript 检查、会话归档与精确回滚、Git 工作树读操作与索引操作协议回归通过。Windows 原生交互待实机复验。

## 2026-11-21 第十五批

本轮从 Hive Desktop `7df4c8b` 开始。参考默认分支为 T3 Code `e668e61`、Cindy `9602463`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **从会话菜单丢弃草稿。** 清除所选会话的未发送文字和附件，五秒内可以撤销。撤销核对准确的会话和该次丢弃，保留后来写入的新草稿。历史、已暂存的其他草稿及发送失败的记录继续保存。 [T3 Code 草稿撤销](https://github.com/pingdotgg/t3code/blob/e668e6141faef533509e1d38d0f984bb38431d3f/docs/user/thread-sidebar.md#L65-L70)
- **暂缓普通未读会话。** 项目与跨项目收件箱沿用已有暂缓时间和撤销。保存的会话标识和回复版本跨重启有效。同一会话有新的可见回复或暂缓到期时，事项回到收件箱，直到打开或标为已读。 [Cindy 暂缓会话](https://github.com/makecindy/cindy/blob/96024635d21ad07772501610c933f4d34536d09f/docs/product-rules/task-inbox.md#L84-L93)
- **在 Git 改动里查看当前文件。** 「当前文件」按选中的项目或工作树重新读取，通过原有文件预览显示文本和 Markdown。切回 Diff 保留文件及暂存比较范围。已删除文件仍可查看 Diff。 [Beluga 文件与改动切换](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ChangesTab.tsx#L234-L254)

- macOS 在同名会话中丢弃 Alpha 的草稿并撤销，找回 Alpha 原文和准确上传的附件，Beta 的草稿仍在。重新丢弃后键入新文字，旧撤销保留新草稿；超时后撤销入口消失。证据在 `.artifacts/discard-draft-qa`。
- macOS 暂缓 Beacon 的未读会话一小时，冷启动和 Atlas 的新回复都保留暂缓；Beacon 的下一条回复将其唤回，待答复协作问题一直可见。Atlas 的项目收件箱另验证了暂缓和撤销，以及冷启动处理已到期的测试提醒。到期事项先以未读显示，标为已读后移除，待汇报派单仍在。证据在 `.artifacts/snooze-unread-qa`。
- macOS 从暂存 Diff 切到「当前文件」，看到同一路径更新后的 Markdown 内容，切回仍显示原暂存比较。外部修改后再次切入可读取更新的内容。独立工作树的同名文件显示工作树独有标题，选中删除文件后回到 Diff。核对了项目和两个工作树的暂存内容。证据在 `.artifacts/git-current-file-qa`。
- 247 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 MSVC 交叉类型检查、TypeScript 检查、受控 ACP 回复游标冷恢复、会话归档和项目文件工作树专项通过。Windows 原生交互待实机复验。

## 2026-11-21 第十六批

本轮从 Hive Desktop `3e50d9c` 开始。参考默认分支为 T3 Code `f0a2aa1`、Cindy `9602463`、Beluga `45a484b`，Hive 上游为 `18861b8`。

- **手动排列置顶会话。** 可以拖动置顶行左侧手柄，也可以从会话菜单上移或下移。Hive 以准确的项目和会话标识保存顺序，读取新回复和重启后仍按手动顺序展示。归档后撤销可以回到原位置。 [T3 Code 手动排列](https://github.com/pingdotgg/t3code/blob/f0a2aa10f5165413e863db9f1502dc4bb6fa42e8/docs/user/thread-sidebar.md#L98-L105)
- **本地计时唤回到期的暂缓事项。** 项目和跨项目收件箱按本机时间处理已保存的暂缓。即使后台回包延迟，到期事项仍会回到对应列表。跨项目收件箱会先保留正在用鼠标或键盘操作的行。 [Cindy 到期处理](https://github.com/makecindy/cindy/blob/96024635d21ad07772501610c933f4d34536d09f/docs/product-rules/task-inbox.md#L140-L146)
- **显示文件中的每一条正文命中。** 同一文件的每个命中行都能独立打开，按路径和行号排序，单页最多 100 条。切换会话执行目录会按该项目或工作树重新搜索。 [Beluga 正文命中](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ContentSearch.tsx#L192-L225)

- macOS 使用会话菜单移动 Alpha 项目的同名会话，再拖动 Beta 项目的同名会话到置顶区首位。四条置顶在重启和归档撤销后顺序一致，当前草稿保留。证据在 `.artifacts/pin-order-qa`。
- macOS 跨项目收件箱拦住会话列表与协作事项回包后，Beacon 的待答复问题先到期并返回，Atlas 的普通未读会话等到自己的截止时间才返回。单独打开 Atlas 项目收件箱，拦住协作事项回包后也能看到到期的未读会话回来。证据在 `.artifacts/snooze-deadlines-qa`。
- macOS 从同一文件的正文结果打开第 3、350、620 行。切到独立工作树后，用方向键和回车打开该工作树的第 350 行。只在项目目录出现的词组在工作树中没有命中，切回项目后恢复三条结果。另在同一文件的 130 条命中中翻到第 2 页，打开第 103 行。证据在 `.artifacts/file-search-jump-qa`。
- 250 项桌面 Rust 测试通过，1 项原有忽略。Windows x64 MSVC 交叉类型检查、TypeScript 检查、置顶顺序与归档协议，以及项目和工作树文件正文专项通过。Windows 原生交互待实机复验。

## 2026-11-21 第十七批

本轮从 Hive Desktop `94aa2c9` 开始，继续对照 T3 Code `f0a2aa1`、Cindy `9602463`、Beluga `45a484b`。

- **折叠正在工作的会话。** 侧栏按项目显示后台执行数，默认折叠。当前会话、置顶、待回应、失败和有未发送草稿的会话保持可见。回复结束后会话回到常规列表。项目的展开状态跨重启保留，普通搜索可以找到折叠区里的会话。 [T3 Code 工作区段](https://github.com/pingdotgg/t3code/blob/f0a2aa10f5165413e863db9f1502dc4bb6fa42e8/docs/user/thread-sidebar.md#L134-L149)
- **从收件箱归档已完成的会话。** 项目和跨项目收件箱的可归档未读会话提供行尾操作。启用归档确认时显示目标项目和会话名。提交会核对会话、项目及回复版本。成功归档后可立即撤销。 [Cindy 收件箱归档](https://github.com/makecindy/cindy/blob/96024635d21ad07772501610c933f4d34536d09f/docs/product-rules/task-inbox.md#L48-L72)
- **文件搜索支持区分大小写。** 文件名和正文共用 `Aa` 开关，默认关闭，在当前窗口保留选择。切换时保留查询并回到第一页。匹配仍认可等价的 Unicode 写法，结果打开原路径和原行号。 [Beluga 大小写开关](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ContentSearch.tsx#L114-L125)

- macOS 用本地模拟 Codex 对端让一条未置顶会话保持执行，验证折叠、展开、搜索以及结束后回到普通列表。当前会话和置顶入口保留；实际展开选择写入侧栏偏好文件。证据在 `.artifacts/working-sidebar-qa`。
- macOS 在跨项目收件箱从两条同名未读中选中 Beacon。取消确认时保留两条。确认后只移走 Beacon，在收件箱底部点撤销即恢复。Atlas 的同名会话和当前草稿一直保留。项目收件箱也验证了停止成员的已完成历史会话可归档并撤销。证据在 `.artifacts/archive-unread-qa`。
- macOS 验证文件名大小写，以及正文 `Café`、`café` 和采用分解字符的 `Café`。在第二页切换开关会回到第一页。打开命中后预览定位到项目或工作树的原始行。证据在 `.artifacts/case-search-qa`。
- 254 项桌面 Rust 测试通过，1 项原有忽略。TypeScript、Windows x64 MSVC 交叉检查、归档和回复版本协议、置顶、项目及工作树文件搜索专项均通过。Windows 原生交互待实机复验。

## 2026-11-21 第十八批

本轮从 Hive Desktop `d0ac7d6` 开始，参考 T3 Code `6cbda0c`、Cindy `e035080`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **在通用设置中关闭工作会话折叠。** 默认开启。关闭后，正在工作的会话重新进入侧栏常规列表，上一条和下一条会话也包含这些会话。关闭状态在本机重启后保留，再开启时沿用每个项目原有的展开选择。 [T3 Code 的设备选择](https://github.com/pingdotgg/t3code/blob/6cbda0cd0aa982b7cbf22d9790bd74e68b2d3dbe/docs/user/thread-sidebar.md#L134-L149)
- **在跨项目收件箱查看和恢复已处理会话。** 末尾的已处理区默认折叠，首次展开才加载，每页 20 条。项目和成员筛选先于分页。可以打开准确的归档会话阅读历史，或从列表直接恢复，保留当前草稿。 [Cindy 已处理区段](https://github.com/makecindy/cindy/blob/e0350808b88301fc6355cdb7be731c1dd086ddbd/docs/product-rules/task-inbox.md#L52-L59)
- **在正文搜索结果里显示原文片段。** 同一文件的不同命中行可直接区分。片段最多 192 个 UTF-8 字节，能准确映射的原文范围会高亮。点击或按回车仍打开选中的路径和行号。文件名搜索沿用原有结果行。 [Beluga 正文结果](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ContentSearch.tsx#L192-L225)

- macOS 用本地模拟对端保持一条会话正在执行，从通用设置关闭折叠。忙碌会话回到常规列表，保存的项目展开记录保留。冷启动后开关仍关闭，再次开启后复验 Alpha 按原偏好展开。证据在 `.artifacts/working-setting-qa`。
- macOS 在已处理区关闭时没有产生归档查询。展开及加载更多分别请求 0 和 20 的偏移，每次最多 20 条。恢复第二页同名的 Beacon 时，Atlas 仍归档，Alpha 草稿保留。再筛选 Atlas 成员，查询携带其工作区及成员标识，打开归档会话时保持归档状态。证据在 `.artifacts/completed-inbox-qa`。
- macOS 在正文结果中读取到同一文件第 3、350 行含表情和分解重音字符的不同原文，点击打开第 350 行。长行尾部的命中带省略号，切到工作树后打开该工作树的第 620 行。独立的分页样例也打开了第 103 行。证据取自原生无障碍树和实际预览，见 `.artifacts/search-snippet-qa`。
- 257 项桌面 Rust 测试通过，1 项原有忽略。TypeScript、Windows x64 MSVC 交叉检查、归档分页协议、项目及工作树文件搜索专项均通过。130 条长行的专项核对了分解重音字符的高亮范围和分页，首屏 100 条响应约 16 KB。Windows 原生交互待实机复验。

## 2026-11-21 第十九批

本轮从 Hive Desktop `f9026a1` 开始。参考 T3 Code `338352d`、Cindy `e035080`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **用快捷键在当前侧栏范围内切换。** 上一条、下一条会话按当前项目筛选、可见区段和顺序切换。展开的全局置顶仍可跨项目，相同会话只遍历一次。侧栏正在显示搜索结果时，用方向键和回车从结果中打开。 [T3 Code 的可见会话顺序](https://github.com/pingdotgg/t3code/blob/338352dee2f43079482e054e8229a2ba055641d9/apps/web/src/components/Sidebar.tsx#L533-L561)
- **按 Agent 后端筛选跨项目收件箱。** 后端和项目、成员可以组合。待处理与已处理都应用所选范围，已处理在分页前筛选。派单以原执行会话归类，缺少可信来源仍可在全部后端查看。「新事项」暂时跨筛选范围，返回后恢复原选择。设置保存在本机。 [Cindy 组合筛选](https://github.com/makecindy/cindy/blob/e0350808b88301fc6355cdb7be731c1dd086ddbd/docs/product-rules/task-inbox.md#L19-L23)
- **按文件折叠正文命中。** 文件头显示完整相对路径及本页命中数。可以点击文件头，也可以聚焦结果列表，用左右键折叠或展开。上下键遍历可见行。已打开的预览保持原位置。同一查询的折叠可跨页沿用，换查询或工作树时重置。 [Beluga 文件分组与键盘操作](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ContentSearch.tsx#L135-L165)

- macOS 筛到 Alpha 后用下一条会话快捷键依次访问可见的全局置顶和 Alpha 普通会话。同名会话按标识打开，重复项只访问一次。收起全局置顶后只循环 Alpha 的三条会话。内联搜索有查询时按同一快捷键未打开其他会话，原草稿保留。证据在 `.artifacts/visible-navigation-qa`。
- macOS 跨项目收件箱显示两条同名的 Pi 和 Codex 未读。切到 Codex 后只剩准确的未读会话，并保留来源为 Codex 的 Atlas 派单。缺少来源的 Beacon 派单在全部后端可见。已处理的 Codex 归档排在 23 条更新的 Pi 归档之后，仍出现在筛选首屏。按成员和后端组合时请求包含工作区、成员和后端标识，重启保持选择。证据在 `.artifacts/backend-inbox-qa`。
- macOS 折叠 `one/same.txt` 时预览仍停在第 3 行。翻到第二页仍折叠，另一个目录的 `two/same.txt` 独立展开。原生结果区用右键展开、左键折叠，搜索框用方向键和回车打开 `two/same.txt` 的第 3 行。切换到独立工作树后恢复默认展开并显示该工作树的原文。证据在 `.artifacts/grouped-search-qa`。
- 261 项桌面 Rust 测试通过，1 项原有忽略。TypeScript、Windows x64 MSVC 交叉检查、归档分页、派单来源和工作树文件搜索专项均通过。派单来源检查在冷启动和成员从 Pi 换到 Codex 后，仍确认旧派单归原 Pi 会话。Windows 原生交互待实机复验。

## 2026-11-21 第二十批

本轮从 Hive Desktop `367c940` 开始。参考 T3 Code `338352d`、Cindy `e035080`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **在当前独立工作树开启新会话。** 聊天目录菜单和 Git 分支面板都提供入口。两条会话使用同一个工作树，分别保存会话、草稿和历史。打开新会话时延迟连接 Agent。目录归属和路径会重新校验，尚有会话引用时禁止清理。共享期间，Hive 会阻止分支切换和检查点文件恢复。 [T3 Code 在现有工作树新建](https://github.com/pingdotgg/t3code/blob/338352dee2f43079482e054e8229a2ba055641d9/docs/user/thread-sidebar.md#L8-L13)
- **恢复已处理会话时留在原位置。** 确认后移除目标行，已加载的相邻会话留在窗口。刷新完成时重算下一页边界，保持当前可见会话的位置。切换项目、成员或后端时从该范围的第一页加载。 [Cindy 恢复后的列表补位](https://github.com/makecindy/cindy/blob/e0350808b88301fc6355cdb7be731c1dd086ddbd/docs/product-rules/task-inbox.md#L57-L63)
- **正文搜索支持整词匹配。** `ab` 默认关闭，可和 `Aa` 同时使用。字母、数字、下划线和组合字符参与 Unicode 词边界判断。切换时保留查询和现有预览，结果回到第一页。筛选和原文片段只标记符合相同边界的匹配。 [Beluga 整词筛选](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ContentSearch.tsx#L114-L125)

- macOS 从聊天目录菜单在工作树 A 新建会话，新旧草稿分别保存。新会话预览打开 A 独有的文件内容，重启后恢复新会话及它的草稿。原所有者的清理对话框仍提示其他会话正在使用这个目录，确认按钮不可用。证据在 `.artifacts/whole-word-qa`。
- 独立运行时复验使用本地模拟 Codex 对端。重启后台后，新会话在原工作树创建了自己的原生对话，首次请求保留 Hive 协作上下文且没有带入原会话的消息。把工作树根目录和其子目录分别登记为项目后，两条分支切换请求都被阻止，工作树仍处于原分支。证据在 `.artifacts/shared-worktree-runtime-audit`。
- macOS 从已处理的第二页恢复 Beacon 一条记录。阻断刷新回包期间相邻两条会话和编辑草稿保留。回包后继续加载，剩余 47 条归档各出现一次。另将一次旧回包留到切换 Atlas 后释放，结果仍只在 Atlas 范围。补验滚动条拖到列表底部后恢复屏内一项，相邻会话在请求前、拦截期和回包后均保持在同一位置。证据在 `.artifacts/completed-depth-qa`。
- macOS 验证了 `cat`、`scatter cat`、带表情的 `cat`、下划线、CJK 相邻字母及 `C++`。整词与区分大小写一起开启后，`CAT` 只返回大写命中。第二页切换整词回到第一页，原预览保留，命中打开准确的行。证据在 `.artifacts/whole-word-qa`。
- 263 项 Rust 测试通过，1 项原有忽略。TypeScript、Windows x64 MSVC 交叉检查、工作树、终端、归档分页、Codex 冷启动与历史回退专项通过。Windows 原生交互待实机复验。

## 2026-11-21 第二十一批

本轮从 Hive Desktop `284806e` 开始。参考 T3 Code `ad4d0f0`、Cindy `e035080`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **解除会话对共用工作树的引用。** 从会话菜单确认后，历史、草稿和已保存的附件仍可阅读，草稿可以选择和复制。进行中的会话或仍开着专属终端的会话须先结束使用。所有引用都解除后，原会话可重新检查工作树是否能清理。 [T3 Code 管理已结束工作占用的工作树空间](https://github.com/pingdotgg/t3code/blob/ad4d0f0659b0b147591724999b0e2f1d59d4b42e/docs/user/thread-sidebar.md#L163-L182)
- **跨项目收件箱按最近活跃时间筛选。** 提供全部时间、最近 1、3、7、30 天，和项目、成员、后端一起使用。已处理先按时间筛选再分页，同一次分页使用同一截止时间。没有活动时间的事项仍可从全部时间和新事项查看。 [Cindy 最近活跃菜单](https://github.com/makecindy/cindy/blob/e0350808b88301fc6355cdb7be731c1dd086ddbd/apps/desktop/src/renderer/features/cc-agent/sidebar/SidebarFilterPopover.tsx#L129-L135)
- **文件正文支持正则搜索。** `.*` 可与大小写和整词匹配一起使用，结果保留原文件行号和高亮。`^$` 可以跳到真实空行。正则在独立 Worker 中处理，切换条件或工作树能取消前一轮，超时后可以继续搜索。 [Beluga 正则筛选](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/src/core/workspace-files.ts#L163-L182)

- macOS 从会话菜单分别解除一个普通会话和一个归档会话的引用。解除第一个后清理仍被阻止。解除第二个后，清理对话框移除了临时工作树，项目目录仍在。重启后从只读输入框复制了原草稿，打字后内容保持，原附件仍可见。证据在 `.artifacts/worktree-release-qa`。
- 运行时用真实 Git 工作树和终端验证活动引用拒绝解除、最后一个引用解除后才能清理、已保存附件的字节保持，以及物理清理和重启后的 Claude 历史分页与搜索。相同原生会话 ID 但不同原目录的历史会被拒绝。使用了本地夹具，无模型请求。证据在 `.artifacts/batch21-runtime-worktree-release.log`。
- macOS 在两个同名会话中，最近 1 天保留两小时前刚收到回复的会话，排除两天前的记录。已处理翻过第 20 条时两次请求使用完全相同的截止时间，共返回 24 条。实际组合 Atlas 项目、reviewer 成员、Pi 后端与最近 1 天，重启后仍保留。证据在 `.artifacts/activity-inbox-qa`。
- macOS 对同一个表达式切换字面和正则搜索，在第 2 页打开原文件第 103 行。输入 `[` 后核对无障碍树中的错误文案，改回有效表达式再切到工作树 A，打开 `item-777` 命中并读到该工作树独有的内容。`^$` 在结果中列出真实空行。证据在 `.artifacts/regex-search-qa`。
- 编译后的运行时搜索 45,000 个小文件，4 秒返回超时，同一后台进程随即响应下一次请求。专项覆盖取消、Unicode 高亮、CRLF 空行及工作树范围。收件箱专项覆盖回复时间跨重启保留、同一截止时间翻页，以及旧分页凭证与新截止时间不匹配时拒绝续页。
- 265 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时构建和 Windows x64 MSVC 交叉检查通过。运行时归档包含已编译的正则 Worker。Windows 原生交互待实机复验。

## 2026-11-21 第二十二批

本轮从 Hive Desktop `4019128` 开始。参考 T3 Code `0702c6f`、Cindy `e035080`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **在独立工作树发送后开始新草稿。** 普通独立工作树会话可使用「发送并开始新草稿」。原消息被接收后，Hive 从原工作树记录的基点建立新目录和分支，使用同一后端与配置。新草稿在发送自己的第一条消息前保持被动。准备失败时可以重试同一个草稿请求。 [T3 Code 后台发送](https://github.com/pingdotgg/t3code/blob/0702c6fdcabd70ca6c5a765bc744a24989979540/docs/user/thread-sidebar.md#L49-L54)
- **一次清除收件箱筛选。** 跨项目收件箱有筛选生效时显示「清除筛选」，一次清空项目、成员、Agent 后端和最近活跃时间。已处理展开时加载全部范围的第一页，焦点回到项目筛选。 [Cindy 收件箱筛选重置](https://github.com/makecindy/cindy/blob/e0350808b88301fc6355cdb7be731c1dd086ddbd/docs/product-rules/task-inbox.md#L19-L24)
- **文件名搜索支持正则。** 搜索 `^src/.*\.test\.ts$` 可限制目录和文件扩展名。文件名与正文共用 `.*` 和大小写开关，搜索仍以当前项目或独立工作树为范围。 [Beluga 名称匹配整个相对路径](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/src/core/workspace-files.ts#L377-L387)

- macOS 按 `Cmd+Option+Enter` 将原消息发送给本地模拟 Codex 对端一次。原项目根目录已提交 B，原工作树已提交 C 并留有未提交文件。新草稿的 Git HEAD 和文件状态仍与最初的 A 一致，分支和目录都独立，Agent 保持被动。输入新草稿后退出，重启恢复同一条草稿。证据在 `.artifacts/fresh-independent-qa`。
- macOS 模拟新工作树已创建但首个成功回包丢失。从界面重试时使用同一个请求标识，打开已创建的草稿。索引仅有一条目标记录，原提示词在模拟对端只出现一次。证据在 `.artifacts/fresh-retry-qa`。
- 运行时使用真实 Git 验证新草稿从原始基点创建。请求并发重试只创建一份。模拟工作树已创建但首次索引写入失败，冷启动后用同一请求恢复原分支与目录。也覆盖源目录缺失、已存在目标的重试与延迟启动 Agent。证据在 `.artifacts/batch22-fresh-worktree.log`。
- macOS 从四项同时筛选的状态进入新事项并返回，原筛选保留。先拦住已处理第 2 页的回包，再清除筛选。已处理区保持展开、草稿仍在，只发起了全部范围的首屏查询。释放旧回包后结果保持在全部范围。重启后四项仍为全部。证据在 `.artifacts/reset-filters-qa`。
- macOS 在文件名模式搜索 `^src/.*\.test\.ts$`，到第 2 页打开编号为 103 的文件并读到项目目录中的标记。切换到独立工作树时查询保留并回到第一页。输入 `[` 可从无障碍树读到明确的语法错误。也复验了从结果区用键盘打开文件。证据在 `.artifacts/regex-filename-qa`。
- 文件名运行时专项覆盖路径分页、大小写、原始 Unicode、带空格和换行的相对路径、无效正则、取消以及工作树隔离。编译后的 JSONL 接口独立验证了翻页和取消。
- 267 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时构建、工作树及初始化、Git 远端、项目文件专项和 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机复验。

## 2026-11-21 第二十三批

本轮从 Hive Desktop `362939e` 开始。参考 T3 Code `90c028c`、Cindy `e035080`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **后台发送后保留源会话已选模型。** 项目根和独立工作树的后续草稿读取同一工作区、后端和配置中的已保存选择。打开空草稿即可看到模型，启动后确认后端使用该模型再接收首次输入。模型失效时保留草稿，并提供「使用配置默认模型」。 [T3 Code 新对话的模型选择](https://github.com/pingdotgg/t3code/blob/90c028c6fc4a9ecd3af048cb52432e819af6487c/docs/user/thread-sidebar.md#L6-L13)
- **窄窗收件箱集中显示筛选。** 一个原生菜单显示生效数量和四项当前值，项目、成员、后端和最近活跃仍可组合。改窗宽时筛选继续生效，大窗口提供直接入口，清除后焦点回到当前布局的筛选按钮。 [Cindy 筛选摘要](https://github.com/makecindy/cindy/blob/e0350808b88301fc6355cdb7be731c1dd086ddbd/apps/desktop/src/renderer/features/cc-agent/sidebar/SidebarFilterPopover.tsx#L545-L565)
- **调整文件列表和预览的上下高度。** 停靠面板和窄窗弹层都提供分隔条，可拖动或用上下方向键调整，按 Home 或双击恢复。比例保存到本机设置，宽窗的左右布局沿用原来的方式。 [Beluga 文件区垂直分隔条](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L393-L405)

- 运行时用受控 Codex 对端分别验证项目根和独立工作树的新草稿冷启动后，在首次实际协议请求中使用手选的非默认模型。另验证了 Pi、Pi RPC、Claude 的选模和 Claude 模型菜单连接后再冷启的失效恢复。失效模型无法以其他模型发出原输入，显式切回默认模型后才会按配置发送。全程无模型网络请求。证据在 `.artifacts/batch23-fresh-draft-model.log`。
- macOS 在独立工作树会话手动选择 Fixture Alternate。后台发送后的草稿立即显示该模型，重启后相同模型和输入仍在。另一个隔离夹具中先让原会话结束，再提交后续草稿。受控 Codex 对端以 `fixture-alternate` 和新工作树目录启动线程，收到一次后继输入。证据在 `.artifacts/fresh-model-qa` 和 `.artifacts/fresh-model-finish-qa`。
- 复验中发现 Codex 的模型列表移除已选模型、但线程启动回显原模型时，旧路径仍会发送。修正后先核对完整模型目录：缺失时既不创建线程也不发送输入，模型在第 2 页时仍正常。Codex 专项通过，证据在 `.artifacts/batch23-missing-model-catalog.log`。
- macOS 在本地模拟对端从目录移除继承模型后，发送失败直接在输入区显示「使用配置默认模型」，原草稿完整。切回默认模型后重新发送，受控对端按配置默认模型启动。证据在 `.artifacts/fresh-model-missing-repeat-qa` 和 `.artifacts/fresh-model-inline-qa`。
- macOS 在 640×480 窄窗设置四项筛选，菜单保留长项目名和成员名。将同一个窗口拉宽到 1220 后，四项出现在各自的直接入口。再次缩窄，从原生菜单用方向键和 Return 清除，读屏可见的生效数量回到 0。证据在 `.artifacts/compact-inbox-qa`。
- macOS 在停靠面板打开 600 行文件，用上方向键和鼠标拖动调整列表与预览，固定的左右宽度保持。重启后再次打开同文件，恢复到保存的约 36% 比例。Home 恢复默认比例。在 640×600 窄窗弹层，按下方向键扩大列表，再按 Home 回到精确的原位置。证据在 `.artifacts/file-split-qa`。
- 269 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时构建、工作树与桌面视图、Pi 模型、Codex 专项和 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机复验。

## 2026-11-21 第二十四批

本轮从 Hive Desktop `9e6d723` 开始。参考 T3 Code `2713570`、Cindy `02fb09b`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **后续 Codex 草稿保留协作模式。** 项目根和独立工作树的后续草稿沿用「默认协作」或「计划」。模型和模式各自保存，访问权限由新会话配置决定。首次发送前核对后端公布的选项和实际生效的模式。模式不再可用时保留输入，可单独切回配置默认工作模式。 [T3 Code 新对话沿用模式](https://github.com/pingdotgg/t3code/blob/271357040676ea578f134493169df4b27ee080f6/docs/user/thread-sidebar.md#L8-L13)
- **从收件箱标题直接打开事项。** 项目收件箱、跨项目收件箱和已处理行使用整块标题与摘要作为打开区域，支持点击和键盘。右侧的暂缓、已读、归档和恢复保持为独立动作。 [Cindy 收件箱行导航](https://github.com/makecindy/cindy/blob/02fb09bf9fbe787ed863bbca285408232589dd49/apps/desktop/src/renderer/features/cc-agent/sidebar/SessionItem.tsx#L948-L986)
- **一次折叠文件树的目录。** 文件树可见时可折叠当前项目或工作树的全部文件夹。选中的隐藏文件回到可见的祖先，焦点返回文件树。已打开的预览和阅读位置保留，延迟结果可以更新缓存，同时保持目录折叠。 [Beluga 全部折叠入口](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L320-L330)

- 受控 Codex 运行时验证项目根的新草稿用「计划」、工作树的新草稿用「默认协作」启动。确认首轮本机协议使用准确模式，目录缺失和回包不匹配时保留输入。另验证模式和模型独立恢复，Claude 权限模式沿用原配置逻辑。证据在 `.artifacts/batch24-fresh-draft-work-mode.log` 和 `.artifacts/batch24-session-modes.log`。
- macOS 从独立工作树以「计划」发送并打开新草稿。冷开显示原模式和输入。隔离测试后台退出后从界面重新连接，同一个后继草稿仍可发送。受控对端先确认计划设置，再用计划模式接收首轮输入。首次退出的触发原因未确认。证据在 `.artifacts/fresh-mode-qa`。
- macOS 从受控对端的可用列表移除「计划」后，后继输入留在草稿中，发送未到达对端。输入区直接显示「使用配置默认工作模式」，点击后原文保留。证据在 `.artifacts/fresh-mode-missing-repeat-qa`。
- macOS 在跨项目收件箱从左侧打开两个同名会话中的 Beacon，并打开 Beacon 的确切派单和问题。点击 Atlas 同名会话右侧的已读动作没有发出打开会话请求。在项目收件箱聚焦左区后按 Return，打开准确的 Atlas 会话。从已处理的左侧标题查看会话后，磁盘记录仍为归档。证据在 `.artifacts/inbox-row-activation-qa`。
- macOS 在工作树 A 同时展开 `docs/specs` 和 `src/components`，打开 600 行文件预览后批量折叠。子行消失，预览和分隔条位置保持。单独重新打开 `docs` 时，`specs` 仍折叠。另在工作树 B 展开 `docs`，切到 A 全部折叠后再返回 B，B 的 `specs` 行仍可见。证据在 `.artifacts/collapse-tree-qa`。
- 271 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时构建、工作树、会话模式、桌面视图和 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机复验。

## 2026-11-21 第二十五批

本轮从 Hive Desktop `26f2d99` 开始。参考 T3 Code `3925267`、Cindy `90e373d`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **后续草稿保留同一模型的推理强度。** Codex 和 Claude 的项目根及独立工作树草稿继承已确认、与模型匹配的强度。后端缺少该档位或实际应用其他值时，首轮发送会暂停，原输入保留。可以在输入区单独恢复配置默认强度，已有模型和工作模式仍生效。 [T3 Code 模型选项与默认值](https://github.com/pingdotgg/t3code/blob/392526768806ab7b36854ad9a401955c4bec6a85/docs/user/composer.md#L121-L127)
- **重开跨项目收件箱时回到附近事项。** 第一条可见事项因已读等原因移出列表后，选用仍在当前范围内的下一条可见事项，恢复它原来的屏幕偏移。候选记录包含项目和事项标识，并遵守列表滚动范围。 [Cindy 相邻行补位](https://github.com/makecindy/cindy/blob/90e373d5d6561a9e40bf944e70e89a018522dbeb/docs/product-rules/task-inbox.md#L47-L51)
- **从当前文件预览查看 Diff。** 停靠面板、窄窗文件弹层和展开阅读器都提供入口。打开时重新核对当前工作树的文件状态，仅有暂存改动就进入暂存比较。干净文件隐藏该入口。 [Beluga 预览改动入口](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L90-L113)

- 运行时验证项目根和独立工作树的首次实际协议请求使用重启后恢复的档位，服务端删去档位或应用了其他值时原输入保留。另核对档位重置与模型、模式独立，以及工作树创建中断后的恢复。Codex 和 Claude 通过本地受控对端验证，全程无模型推理请求。证据在 `.artifacts/batch25-fresh-draft-reasoning.log` 和 `.artifacts/batch25-native-reasoning-controls.log`。
- macOS 从使用「高」推理和「计划」模式的会话新开草稿，即时与冷开均显示两个已选项。本地受控对端确认设置，再以「高」档接收新草稿的首轮输入。证据在 `.artifacts/fresh-effort-qa`。
- macOS 在后继草稿冷开前从受控对端移除「高」档。首轮发送没有到达对端，输入区显示不可用提示。点击「使用默认推理强度」后，原文、模型和「计划」模式仍在。证据在 `.artifacts/fresh-effort-missing-qa`。
- macOS 用 32 条未读会话验证从首屏打开、改为已读并返回，项目筛选保留，下一条会话出现在原行位置。Rust 回归覆盖首个及第二个候选移除、跨项目同标识和列表重排时的偏移恢复。本次实窗的合成滚轮和拖动没有移动视口，滚到中段之后的恢复待手动复验。证据在 `.artifacts/inbox-next-qa`。
- macOS 在工作树 A 搜索到 `docs/nested/note.txt`，从停靠预览打开准确的暂存改动。在 640×600 窄窗，从展开阅读器进入 Git 比较，文件弹层随交接关闭，暂存内容仍来自 A。工作树 B 中同路径的干净文件未出现 Diff 入口。证据在 `.artifacts/search-diff-qa`。
- 273 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时构建、工作树、会话标识、桌面视图、原生推理控制协议和 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机复验。

## 2026-11-21 第二十六批

本轮从 Hive Desktop `9e5be1f` 开始。参考 T3 Code `6f5293f`、Cindy `90e373d`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **两个桌面收件箱都能看到第四条及之后的等待汇报派单。** 桌面发现汇总的等待条数超出明细时，从同一个工作区的派单账本补齐。成员取自派单，后端取自该次执行的来源。沿用超过或达到 10 分钟的已提交状态。 [Hive 上游明细限制](https://github.com/tt-a1i/hive/blob/18861b80379bff0d658623a3aa6f1454159d6925/src/server/action-center-summary.ts#L98-L112) · [Cindy 完整分组](https://github.com/makecindy/cindy/blob/90e373d5d6561a9e40bf944e70e89a018522dbeb/apps/desktop/src/renderer/features/cc-agent/sidebar/TaskInbox.tsx#L186-L249)
- **只引用选中的助手回复。** 选中一段文字并右键选择「引用选中的文字」，在当前光标插入可编辑的 Markdown 引用，草稿前后的文字保留。草稿及发出的消息提供「引用原消息」，旧消息未加载时按项目、会话和消息标识打开附近历史。引用片段随文字保存，源消息无法打开时仍可阅读。 [T3 Code 选区及返回原文](https://github.com/pingdotgg/t3code/blob/6f5293fbdca476cddbc37466450983ba1dd2ada2/docs/user/composer.md#L129-L143)
- **收起文件预览时保留文件面板。** 停靠面板和窄窗可点关闭，预览或目录树聚焦时可按 Esc。列表查询、已加载页和选中行保留，Return 可再次打开。展开阅读器的 Esc 仍返回紧凑预览。 [Beluga 关闭预览](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L108-L114) · [Beluga Esc 行为](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L295-L310)

- 真实 Hive SQLite 和 JSONL 运行时验证授权 HTTP 预览仍只含前三条，桌面返回完整四条，第四条按其成员和实际 Codex 后端准确归类。验证时间门槛、新近、已汇报、已取消和不同项目的派单均排除，补充字段与原有明细一致。证据在 `.artifacts/competitive-ux/twenty-sixth/inbox-wire.log`。
- macOS 跨项目收件箱显示五条满足时间门槛的派单。按第四位成员和 Codex 筛选后只剩目标派单，暂缓和恢复记录绑定同一标识，打开后的派单详情也使用该标识。本次实窗覆盖跨项目入口，排除条件由运行时验证。证据在 `.artifacts/batch26-inbox-qa`。
- macOS 实际双击选择助手回复中的 `selected`，右键引用到 `Before` 和 `之后` 之间。冷开后文字和来源入口保留。从历史搜索跳到旧页，原回复移出已加载范围。再点击「引用原消息」，后台按准确标识请求附近记录，页上限为 120 条，原回复和草稿重现。使用合成 Pi 历史，未发送消息。证据在 `.artifacts/batch26-quote-qa`。
- macOS 在停靠面板与 640×600 窄窗用 Esc 关闭紧凑预览，再按 Return 从原选中行打开 `docs/nested/note.txt`。展开阅读器按 Esc 回到紧凑预览，点击关闭按钮后列表获得空间。实窗发现会话查找提前接收了停靠面板的 Esc，修正按键范围后两种布局均通过。证据在 `.artifacts/batch26-preview-qa`。
- 278 项 Rust 测试通过，1 项原有忽略。TypeScript、桌面构建、Hive 运行时、桌面视图、收件箱的生产通信入口和 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机复验。

## 2026-11-21 第二十七批

本轮从 Hive Desktop `1f46896` 开始。参考 T3 Code `ccabed0`、Cindy `90e373d`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **按派单账本唤回有新消息的暂缓事项。** 两个桌面收件箱检查同工作区、同派单的最新有效消息，打开时和关闭后的待办轮询都用这个查询。旧记录继续按暂缓时记下的时间判断。其他派单的消息以及已取消消息不会唤回该事项。 [Cindy 新输入唤回规则](https://github.com/makecindy/cindy/blob/90e373d5d6561a9e40bf944e70e89a018522dbeb/docs/product-rules/task-inbox.md#L79-L87) · [Hive 最近消息限制](https://github.com/tt-a1i/hive/blob/18861b80379bff0d658623a3aa6f1454159d6925/src/server/action-center-summary.ts#L190-L195)
- **把队首待发消息送到当前 Codex 回合。** 活动回合可接收时，合适的第一条待发消息提供「发送到当前回合」。待发列表有焦点时也可按 `Cmd+Shift+Enter` 或 `Ctrl+Shift+Enter`。操作沿用原消息标识和普通文件附件，保留输入框草稿及后续排队顺序。结果不确定时保留待核查记录，避免重启后自动重发。 [T3 Code 当前回合追加排队消息](https://github.com/pingdotgg/t3code/blob/ccabed086ad47294173769c77d2d44e55bb8bd0d/docs/user/composer.md#L76-L80)
- **文件面板可预览当前工作树的本地 PNG 和 JPEG。** 文件树和文件名搜索可在停靠面板、窄窗和展开阅读器打开图片。图片按 4 MiB 文件大小、8192 像素的单边尺寸和 900 万总像素限制读取及解码，文本预览沿用 1 MiB 限制。 [Beluga 文件读图](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/views/reader/FileReader.tsx#L89-L105)

- macOS 将派单 A 无限期暂缓，再给同项目 B 写入 13 条消息。A 继续暂缓。关闭收件箱后给 A 写入一条消息，再给 B 写入 13 条。重开收件箱时 A 已唤回。生产运行时另验证旧时间记录、其他工作区隔离、已取消消息和冷启动。证据在 `.artifacts/batch27-inbox-qa`。
- 受控 Codex 运行时验证精确的活动回合和队首消息标识、普通文件附件字节、并发只接受一次、后一条在下一回合发送，以及冷启动保留回执。全程使用本地协议对端，未调用模型推理。
- macOS 保持受控 Codex 第一回合运行，排入 A、B，并在输入框留下未发送的 C。只有 A 显示「发送到当前回合」，点击后对端只收到一次 A，B 仍在队列中，C 完整保留。释放第一回合后，对端用下一回合收到 B。证据在 `.artifacts/batch27-steer-qa`。
- 文件运行时和 Rust 测试验证了同名文件的工作树隔离、超过 1 MiB 的有效 PNG、PNG/JPEG 像素、格式错误和超限拒绝。旧文本读取入口仍返回原格式。
- macOS 的两个工作树按各自会话读取同一路径的 PNG，源文件哈希不同，停靠面板和窄窗展开均显示图片的无障碍控件。JPEG 同样可预览。损坏 PNG 的错误原先没有无障碍名称，修正后复验可读取「项目文件图片无法显示」。屏幕红蓝像素的区别由解码测试核对，实窗核对了图片控件及准确的文件请求。证据在 `.artifacts/batch27-images-qa`。
- 281 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时和桌面构建、桌面视图、队列与 Codex、项目文件与工作树引用，以及 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机复验。

## 2026-11-21 第二十八批

本轮从 Hive Desktop `52b0821` 开始。参考 T3 Code `ccabed0`、Cindy `90e373d`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **等待汇报的派单显示任务标题和成员。** 两个桌面收件箱按工作区和派单读取简短标题，优先使用标签，否则使用正文首行。重名或空标题带派单编号。打开、暂缓和唤回继续使用准确的派单标识。 [Cindy 任务行显示标题](https://github.com/makecindy/cindy/blob/90e373d5d6561a9e40bf944e70e89a018522dbeb/apps/desktop/src/renderer/features/cc-agent/sidebar/SessionItem.tsx#L1062-L1067)
- **从输入框开头编辑队尾待发消息。** macOS 按 `Option+↑`，Windows 按 `Alt+↑`。输入框有焦点、没有选区和输入法组合时，打开队尾消息原有的编辑框。保存沿用原消息标识、附件和队列位置，聊天草稿保留。 [T3 Code 编辑队尾快捷键](https://github.com/pingdotgg/t3code/blob/ccabed086ad47294173769c77d2d44e55bb8bd0d/docs/user/composer.md#L81-L84)
- **文件图片预览支持原尺寸和适应窗口。** 停靠面板、窄窗和展开阅读器都有按钮，也可双击图片切换。原尺寸按屏幕缩放显示，可以横向和纵向滚动。展开时带入当前模式和位置，选择另一张图片时从适应窗口开始。 [Beluga 图片缩放](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/views/reader/FileReader.tsx#L73-L107)

- 生产通信验证同成员同一提交时间、不同标题、重名、空标题和超过 96 个码点的标题。原 HTTP 摘要保持原格式。macOS 跨项目收件箱按标题打开展示 `Audit migration` 的派单、暂缓 `Review billing` 的派单，两个操作命中各自的派单标识。证据在 `.artifacts/batch28-inbox-qa`。
- macOS 用受控 Codex 排入 A、B，输入框保留草稿 C。光标在末尾按 `Option+↑` 时未打开编辑框，移到开头后打开 B。保存并释放前一回合后，对端依次收到 A 与修改后的 B，C 保留且未发送。证据在 `.artifacts/batch28-steer-qa`。
- macOS 打开 3200×1800 PNG，从适应窗口切到原尺寸，并在展开阅读器中保留模式。这段操作只发出一次文件预览请求。代码检查发现停靠面板的图片区域需要显式接收焦点，已补上焦点入口。证据在 `.artifacts/batch28-image-qa`。
- 补记第 28 批的屏幕观察。原生截图工具中，适应窗口显示四色角，原尺寸显示放大的左上红色区域。未保存截图，横向和纵向的实窗位移仍待复验。记录在 `.artifacts/batch28-image-qa/receipt.md`。
- Rust 专项覆盖输入框位置、选区、输入法状态和队尾不可编辑，以及图片的 1×、2×、1.25× 显示缩放和两个方向的滚动边界。实窗无障碍接口只提供图片控件和缩放操作，不能直接读取滚动偏移。
- 285 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时和桌面构建、Hive HTTP/SQLite、桌面视图和 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机复验。

## 2026-11-21 第二十九批

本轮从 Hive Desktop `566a86d` 开始。参考 T3 Code `c53ac23`、Cindy `90e373d`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **收件箱直达所选协作消息。** 项目和跨项目收件箱打开问题时，派单详情顶部显示完整的所选消息。读取使用准确的工作区、派单和消息标识，原历史和成员会话入口仍可用。 [Cindy 待答直达对话](https://github.com/makecindy/cindy/blob/90e373d5d6561a9e40bf944e70e89a018522dbeb/docs/product-rules/task-inbox.md#L89-L98) · [Hive 上游消息定位](https://github.com/tt-a1i/hive/blob/18861b80379bff0d658623a3aa6f1454159d6925/src/server/routes-dispatches.ts#L105-L136)
- **普通粘贴把长纯文本保存为附件。** 32 KiB 至 8 MiB 的纯文本保存为 `pasted-text.txt`，保留输入框的原文和选区。用 `Cmd/Ctrl+Shift+V` 可在原位置直接粘贴文本。保存中断可在原草稿重试或复制原文。不能开始保存附件时沿用直接粘贴行为。 [T3 Code 长文本粘贴](https://github.com/pingdotgg/t3code/blob/c53ac237a0af3d48229a509945edb0168fc92df1/docs/user/composer.md#L6-L13)
- **文件名搜索提供整词匹配。** 整词匹配针对完整相对路径，可与大小写和正则组合。换匹配条件时重算结果，返回的排序、分页和路径标识保持一致。 [Beluga 文件名搜索选项](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ContentSearch.tsx#L73-L85)

- 生产运行时以同一派单在 100 条旧消息后收到两个问题为例，验证原文读取准确，拒绝错工作区、错派单、已取消和找不到的消息。macOS 从项目和跨项目两个入口逐条打开两个问题，详情显示各自原文。证据在 `.artifacts/batch29-inbox-qa`。
- 附件运行时通过真实本地通信和附件存储验证 UTF‑8、CRLF 和 NUL 字节、大小上限、跨项目拒绝和冷恢复。Rust 专项覆盖粘贴阈值、图片兼容和附件保存中断后按草稿恢复。
- macOS 实测普通粘贴 64,800 字节的多语言日志，生成附件并保留原指令和选区。上传文件的 SHA‑256 与原文一致，冷开后仍有草稿和附件。`Cmd+Shift+V` 按原编辑器行为替换选区，附件数量保持为一。证据在 `.artifacts/batch29-paste-qa`。
- macOS 用隔离通信对端拒绝第一次粘贴附件请求，原草稿保留并显示重试。重试上传后，附件 SHA‑256 再次与原文一致。证据在 `.artifacts/batch29-paste-failure-qa`。
- 文件运行时验证整词、大小写和正则、Unicode 分词边界、分页及工作树范围。旧客户端省略整词选项时沿用原默认行为。
- macOS 在文件名中启用整词，`cat` 命中目录 `cat/README.md` 和文件 `docs/CAT-notes.md`，排除 `scatter` 与 `cat_id`，并返回分页结果。加上大小写匹配后排除大写文件。打开目录匹配核对了准确的项目文件内容。证据在 `.artifacts/batch29-files-qa`。
- 288 项 Rust 测试通过，1 项原有忽略。TypeScript、桌面和运行时构建、桌面视图、Hive 附件、项目文件、工作树引用和 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十批

本轮从 Hive Desktop `22445ba` 开始。参考 T3 Code `46b6262`、Cindy `90e373d`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **收件箱和后台提醒可读取第 101 条及之后的待答问题。** 项目和跨项目列表按工作区分批加载，关闭收件箱后的提醒也逐页读取。读取完整批次后才更新该工作区的待办，后续页失败时保留已有结果。旧 HTTP 和旧版桌面通信响应保留原格式。 [Hive 待办读取上限](https://github.com/tt-a1i/hive/blob/18861b80379bff0d658623a3aa6f1454159d6925/src/server/dispatch-message-store.ts#L115-L126) · [Cindy 完整分区计数](https://github.com/makecindy/cindy/blob/90e373d5d6561a9e40bf944e70e89a018522dbeb/apps/desktop/src/renderer/features/cc-agent/sidebar/TaskInbox.tsx#L181-L243)
- **在空输入框用方向键取回本会话的已发送文字。** `↑` 回看，`↓` 向后回到空白。只取回当前已加载、由用户实际发送的文本。编辑文字、切换会话或存在附件时，方向键使用原有编辑行为。跨行定位按显示换行判断。 [T3 Code 提示词历史](https://github.com/pingdotgg/t3code/blob/46b626263fcfaa13d09448272dd749c82c95023a/docs/user/composer.md#L152-L163)
- **文件名搜索高亮命中的相对路径片段。** 目录或文件名中的匹配位置沿用大小写、整词和正则设置，运行时只为当前页返回有限的 UTF‑8 范围。原路径、排序、键盘选择和打开方式保持一致。 [Beluga 文件名高亮](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L50-L65)

- 生产运行时验证第 101 条问题能从普通和紧凑待办第二页读取，且保留确切成员和后端。另核对分页边界、后到数据、工作区隔离、原 HTTP 和旧版响应。Rust 检查后续页失败时保留原有待办。
- macOS 先打开原有 100 条问题，再暂缓这些旧项以隔离提醒观察。关闭收件箱后，在同一工作区的另一派单新增第 101 条问题。入口显示新事项，打开后展示对应原文。通信记录显示关闭期间与打开后均读取了第二页。证据在 `.artifacts/batch30-inbox-qa`。
- macOS 用本地受控 Codex 对端依次发送 A 和带附件的 B，从空输入框按 `↑`、`↑`、`↓`、`↓`，依次看到 B、A、B、空白。输入框始终没有恢复附件，对端仅收到原来的两次发送。冷开后另输入草稿 C，按 `↑` 时文字仍为 C。证据在 `.artifacts/batch30-history-qa`。
- Rust 专项验证已发送文字的回看顺序、编辑后重置及消息来源。文件专项验证 Unicode、大小写和正则范围、当前页及原文件路径。
- macOS 搜索 `archive` 并开启整词。实际请求启用高亮范围，前两条结果分别返回目录和文件名的准确字节范围。用方向键和 Return 打开了第二条的正确文件。实窗截图未取得，颜色待截图复验。证据在 `.artifacts/batch30-files-qa`。
- 293 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时和桌面构建、桌面视图、项目文件、工作树引用及 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十一批

本轮从 Hive Desktop `7a08116` 开始。参考 T3 Code `21d7fea`、Cindy `6b05493`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **收件箱中的旧问题保留发问者、摘要和实际提问时间。** 两个桌面收件箱按待答问题本身读取发问者和正文首行，点击打开或暂缓继续使用准确的消息标识。摘要最多取 96 个 Unicode 码点并在较长时标记省略。后台待办数量检查沿用轻量的原请求。 [Cindy 任务行显示标题](https://github.com/makecindy/cindy/blob/6b05493ec138477f23828d76cc90e2a86f5f88aa/apps/desktop/src/renderer/features/cc-agent/sidebar/SessionItem.tsx#L1106-L1111)
- **用 `Shift+Enter` 续写 Markdown 列表。** 项目符号保留缩进，有序列表继续编号，待办的下一项从未勾选开始。在空项按键会退出列表。自动修改可以一次撤销。普通段落和围栏代码使用原有换行行为。 [T3 Code 列表续写](https://github.com/pingdotgg/t3code/blob/21d7feaff7edb445855c91e0d4526b9cce63d83a/docs/user/composer.md#L31-L36)
- **文件源码预览显示原始行号。** 正文搜索和翻页后从实际原始行开始编号。很长的原始行占用多个显示行时，编号只显示在它的开头。字号、字体和换行沿用正文布局，行号在可选源码外绘制。 [Beluga 源码行号](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/components/markdown/CodeLines.tsx#L34-L70)

- 生产运行时测试把两个旧问题放在 13 条较新普通消息之前。两个问题均从正确工作区读取摘要、提问者和时间，第 101 条也可从第二页读取。原 HTTP 及后台简版待办响应沿用原字段。另核对了 Unicode 截断。
- macOS 在同一工作区写入 14 条更新的普通消息后，跨项目收件箱仍按实际时间显示两个旧问题、正确成员和各自摘要。暂缓 A 写入 A 的消息标识，打开 B 读取 B 的原文。项目收件箱也显示 B 和已暂缓的 A。证据在 `.artifacts/batch31-inbox-qa`。
- macOS 实际按 `Shift+Enter` 续写含中文和 emoji 的有序项、待办和缩进项目符号。再次按键退出空项，`Cmd+Z` 一次恢复。代码围栏内的项目符号只换行。本次未发送消息。证据在 `.artifacts/batch31-composer-qa`。
- macOS 从正文搜索定位第 350 行。实窗截图中，折行后的剩余行没有重复编号，下一页的 513、514 等编号与正文对齐。展开阅读器后留在同一页。拖选复制两行得到与原始文件完全相同的 49 字节。截图已查看，未保存到本地。证据在 `.artifacts/batch31-source-qa`。
- Rust 专项验证待答问题按实际提问时间排序，以及列表的连续编号、缩进、任务框、UTF‑8 与 CRLF、插入位置和代码围栏。文件专项核对从第 350 行定位、后页第 513 行、空行，以及超长源码跨页时保留同一个行号。
- 300 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时和桌面构建、Hive HTTP 与 SQLite、桌面视图和 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十二批

本轮从 Hive Desktop `7af78fe` 开始。参考 T3 Code `21d7fea`、Cindy `6b05493`、Beluga `45a484b`。Hive 上游 `main` 为 `18861b8`。

- **收件箱各分区先显示 20 条。** 项目与跨项目收件箱保留完整计数，点击或用键盘激活「显示更多」逐次显示 20 条。打开已展开的条目后返回，恢复对应的展开范围和阅读位置。已处理会话沿用原有分页。 [Cindy 分区显示上限](https://github.com/makecindy/cindy/blob/6b05493ec138477f23828d76cc90e2a86f5f88aa/apps/desktop/src/renderer/features/cc-agent/sidebar/TaskInbox.tsx#L186-L248)
- **草稿可按实际行数选用发送键。** 新设置允许单行用 `Enter` 发送，窗口宽度造成的折行仍按单行处理。含实际换行时，`Enter` 继续编辑，`Cmd/Ctrl+Enter` 发送。多行草稿在忙碌时可加 `Shift` 执行另一种跟进方式，原有两种设置继续生效。 [T3 Code 发送键规则](https://github.com/pingdotgg/t3code/blob/21d7feaff7edb445855c91e0d4526b9cce63d83a/docs/user/keybindings.md#L14-L18) · [T3 Code 多行跟进](https://github.com/pingdotgg/t3code/blob/21d7feaff7edb445855c91e0d4526b9cce63d83a/docs/user/composer.md#L69-L74)
- **展开源码阅读器可关闭长行换行。** 关闭后可用横向滚动条或方向键读到长行结尾，保留当前文件、页码和原始行号。展开阅读器会记住设置，紧凑预览继续换行。 [Beluga 阅读器换行开关](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/views/reader/FileFocus.tsx#L193-L213)

- macOS 跨项目收件箱有 103 项需关注，首批只提供 20 项的操作入口，并提示还可显示 83 项。点击和按空格可逐次增加 20 条，等待和已处理入口排在首批行之后。项目与跨项目入口均实际滚动并打开第 21 条，返回后该行的坐标和展开范围一致。始终未显示的第 101 个问题仍能从「新事项」找到。证据在 `.artifacts/batch32-inbox-qa`。
- macOS 用本地受控 Codex 对端验证短单行和自动折行的单行均用 `Enter` 发送。一个新草稿输入 `Shift+Enter` 后立即按 `Enter`，两个换行完整保留，对端没有收到新请求。`Cmd+Enter` 发送这一草稿一次，冷开后设置文件保留所选模式。证据在 `.artifacts/batch32-composer-qa`。
- macOS 从正文搜索打开第 350 行。关闭换行后，正文宽度由 1,029 像素变为 3,710 像素。方向键移动了 1,200 像素，横向滚动事件到达右边界。冷开仍关闭换行，开启后回到同一命中与原宽度，紧凑预览继续换行。关闭换行时翻到第 513 行并跨行复制，结果与原文的 49 字节完全一致。滚动范围通过实窗无障碍坐标核验，未取得截图。证据在 `.artifacts/batch32-source-qa`。
- Rust 专项覆盖三个分区独立的展开上限、101 条待答时保留完整计数与较深的返回位置。另核对旧发送设置、多行按键分派、源码横向滚动边界及旧外观设置默认继续换行。
- 305 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时和桌面构建、桌面视图、收件箱通信及 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十三批

本轮从 Hive Desktop `7aadec0` 开始。参考 T3 Code `a1b9e1c`、Cindy `6b05493`、Beluga `eed1b7e`。Hive 上游 `main` 为 `18861b8`。

- **收件箱直达对应团队成员。** 项目与跨项目入口按工作区和成员 ID 核对目标，展开职责说明并将焦点移过去。团队搜索或状态筛选仅在挡住目标时清除。无法核对目标时在原收件箱提示。 [Cindy 伙伴收件箱规则](https://github.com/makecindy/cindy/blob/6b05493ec138477f23828d76cc90e2a86f5f88aa/docs/product-rules/task-inbox.md#L90-L106)
- **通过可自定义的快捷键调整当前会话的推理强度。** 提高和降低默认无键位。给动作绑定快捷键后，在已连接且空闲的会话输入框中直接切换档位，两端循环。按 Agent 当前模型的可用档位判断，保存 Agent 确认的结果。 [T3 Code 推理强度快捷键](https://github.com/pingdotgg/t3code/blob/a1b9e1cc8709d50190bf8e1f50e94257ce206525/docs/user/keybindings.md#L41-L44)
- **在展开的源码阅读器内查找。** 用 `Cmd/Ctrl+F` 搜索已加载的源码，显示匹配总数，用 `Enter` 或 `Shift+Enter` 前后切换。同一行上的命中可单独定位，关闭换行时也会滚到远处匹配。按 `Esc` 先回到源码，再返回文件面板。 [Beluga 文件内查找](https://github.com/beluga-labs/beluga/blob/eed1b7ea2ef127479f8fe8773e5516013a6e146f/packages/mac/src/components/markdown/DocFind.tsx#L22-L39)

- macOS 先设置挡住目标的团队搜索和「工作中」筛选，再从跨项目收件箱打开成员 2，看到 Atlas 的正确职责说明。随后从 Atlas 进入 Beacon 的同名成员，显示 Beacon 的正文。项目与跨项目入口跳转后按空格，只有目标职责说明收起，证明焦点在正确控件。重新打开两个收件箱后，目标行均返回原坐标。证据在 `.artifacts/batch33-inbox-qa`。
- macOS 在一个 740 行文件中用 `Cmd+F` 找到四处文本，包括同一行中的两处。`Enter` 和 `Shift+Enter` 前后跳转，只有最初读取文件的请求。关闭换行后跳到第 350 行的第 1,619 列，实窗坐标确认正文横向移动 13,726 像素。两次 `Esc` 依次关闭查找和阅读器，文件面板保留原搜索。证据在 `.artifacts/batch33-find-qa`。
- macOS 冷开后在快捷键设置中确认两个动作已绑定，绑定值来自隔离夹具配置。聚焦输入框后按 `Cmd+Shift+J`、`Cmd+Shift+J`、`Cmd+Shift+K`，受控 Codex 对端收到同一会话的三次设置更新，档位依次为高、低、高。另在草稿中间放置光标，切换档位后在同一位置插入字符，确认文字和光标保持。未发送提示。证据在 `.artifacts/batch33-effort-qa`。
- 专项测试覆盖同名成员与筛选、旧快捷键配置迁移和冲突、Unicode 匹配范围及跨页定位。受控 Codex 运行时核对两端循环、每次有效调整只写一次、另一会话设置保持、保存后端实际档位，以及忙碌、模型变化、失效视图和未连接时不写入。内置 Pi 的运行时测试核对原生会话记录只追加一次，且全局默认设置字节一致。
- 310 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时和桌面构建、Hive HTTP 与 SQLite、桌面视图、推理强度专项与 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十四批

本轮从 Hive Desktop `3ccf86a` 开始。参考 T3 Code `8d37736`、Cindy `6b05493`、Beluga `eed1b7e`。Hive 上游 `main` 为 `18861b8`。

- **项目收件箱可打开失败的工作流和原日志。** 当前项目的已结束顶层失败运行进入待关注区，按运行名称和 ID 区分。收件箱按结束时间读取最近 20 条失败，更多记录从工作流页面查看。打开一项只读取对应运行、派单与日志，可返回原收件箱。 [Cindy 失败回到待处理](https://github.com/makecindy/cindy/blob/6b05493ec138477f23828d76cc90e2a86f5f88aa/docs/product-rules/task-inbox.md#L30-L50) · [Hive 运行和日志接口](https://github.com/tt-a1i/hive/blob/18861b80379bff0d658623a3aa6f1454159d6925/src/server/routes-workflows.ts#L12-L82)
- **聚焦当前 Git 差异后可以查找。** `Cmd/Ctrl+F` 搜索当前文件已加载补丁的增删和上下文行，显示原文件或新文件的命中行号。`Enter` 和 `Shift+Enter` 前后切换，可揭开已加载的折叠上下文。切换文件或比较范围后清除旧结果。 [T3 Code 差异内查找](https://github.com/pingdotgg/t3code/blob/8d37736cddce74e7cc7ff965b26b1d3ea1fbcd7d/docs/user/keybindings.md#L66-L73)
- **展开阅读器可复制已加载的完整原文。** 源码和 Markdown 渲染视图的工具栏都提供入口。复制取自当前项目或工作树的预览快照，无需再次读取文件。保留当前页、查找位置和换行选项。 [Beluga 复制文件文本](https://github.com/beluga-labs/beluga/blob/eed1b7ea2ef127479f8fe8773e5516013a6e146f/packages/mac/src/views/reader/FileFocus.tsx#L195-L200)

- macOS 项目收件箱用不同的运行 ID 后缀区分当前项目的两次同名失败。打开 B 只请求 B 的运行详情、派单和日志，窗口显示 B 的原始日志。返回时两行恢复原坐标。证据在 `.artifacts/batch34-inbox-qa`。
- macOS 聚焦合并差异后用 `Cmd+F` 找到三行。`Enter` 从原文件第 2 行到新文件第 2 行，`Shift+Enter` 返回。并排视图跳到新文件第 3 行的上下文，命中时自动展开已折叠的文件。保持查找打开时切换比较范围或文件，旧查询均清除。`Esc` 关闭差异查找，回到聊天后 `Cmd+F` 仍打开会话查找。证据在 `.artifacts/batch34-diff-qa`。
- macOS 从两个工作树各自同名文件的第 620 行打开阅读器并复制。剪贴板分别得到 27,374 和 28,114 字节，与对应完整文件逐字节一致，包含 UTF-8 BOM、CRLF 和 emoji。阅读器保持在原页。Markdown 渲染视图复制出的 63 字节也与原文件完全一致。证据在 `.artifacts/batch34-copy-qa`。
- 生产运行时验证了 20 条上限、按结束时间排序、项目范围、排除未结束和子运行、区分两次同名失败、准确读取错误与日志，以及从原工作流列表查到更早运行。Rust 专项核对运行 ID 与项目匹配，并核对补丁增删两侧和上下文的行号，排除补丁元数据。
- 314 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时和桌面构建、桌面视图、Hive HTTP 与 SQLite、文件和 Git 工作区专项及 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十五批

本轮从 Hive Desktop `01ea31a` 开始。参考 T3 Code `f2ec853`、Cindy `1db8327`、Beluga `eed1b7e`。Hive 上游 `main` 为 `18861b8`。

- **跨项目收件箱和新事项提示可收到工作流失败。** 收件箱关闭时按批读取各项目的失败运行 ID。打开后显示所属项目及运行名称，可进入该项目的准确运行和原日志。返回收件箱时恢复原筛选与阅读位置。 [Cindy 失败分类与范围](https://github.com/makecindy/cindy/blob/1db8327d9cce5c66030c53662b93de4fbb166db6/docs/product-rules/task-inbox.md#L29-L50)
- **按记录的工作树分支查找会话。** 全局搜索、项目侧栏和归档搜索会匹配已保存的工作树分支并在结果上显示它。结果使用原会话和工作树标识定位。这里的分支是创建工作树时保存的记录。 [T3 Code 按分支查找](https://github.com/pingdotgg/t3code/blob/f2ec853fbbb6ed84f6c08aa156d3b7d9e9f7385f/docs/user/thread-sidebar.md#L224-L232)
- **展开图片支持逐档缩放。** 图片可在适应窗口、100% 与更大或更小的倍率间切换。100% 对应设备原像素。缩放以当前视野中心为锚点，靠近边缘时按可滚动范围调整。关闭展开阅读器后回到原紧凑预览。 [Beluga 图片缩放](https://github.com/beluga-labs/beluga/blob/eed1b7ea2ef127479f8fe8773e5516013a6e146f/packages/mac/src/views/reader/FileReader.tsx#L67-L107)

- macOS 关闭收件箱后，在 Beacon 写入同名失败运行，侧栏出现新事项提示。点击后只读取 Beacon 的指定运行、派单和原日志。返回保留「新事项」，离开后恢复 Atlas 筛选、原行坐标和原草稿。证据在 `.artifacts/batch35-inbox-qa`。
- macOS 在两个同名会话中，用 `Cmd+K` 搜索 `review/beta-481` 只显示 Beta，并标明分支。按 `Return` 发出准确的 Beta 会话打开请求。离线归档夹具中的会话后，归档页面仍可按同一分支找到它。另用侧栏的全局搜索从 Alpha 跳到 Beta，Beta 的输入框为空，点「后退」回到 Alpha 后原草稿仍在。证据在 `.artifacts/batch35-branch-qa`。
- macOS Retina 实窗将 3,200 × 1,800 像素的图片按适应窗口、75%、100%、150% 再回到适应窗口，返回后紧凑预览仍为适应窗口，期间只请求一次图片。临时布局记录确认 100% 是 1,600 × 900 逻辑像素。方向键平移再放大到 150% 时，视野中心对应图片上的同一点。证据在 `.artifacts/batch35-zoom-qa`。
- 运行时验收关闭后的提示每批最多查询 32 个项目，每个最多返回最近 20 个失败运行 ID。Rust 验证查询遗漏或无效只保留对应项目的上次结果，确认空结果只清除该项目。分支检索专项覆盖同名会话、两个真实工作树、项目过滤、归档及原工作树标识。缩放专项核对 100% 设备像素、从适应窗口进入档位、滚动后保持中心及两轴边缘裁限。
- 318 项 Rust 测试通过，1 项原有忽略。TypeScript、运行时和桌面构建、桌面视图、Hive HTTP 与 SQLite、会话搜索和归档及 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十六批

本轮从 Hive Desktop `8287e4e` 开始。参考 T3 Code `f2ec853`、Cindy `1db8327`、Beluga `eed1b7e`。Hive 上游 `main` 为 `18861b8`。

- **重启中断的工作流进入两个收件箱。** Hive 冷启动后，已中断的顶层运行会出现独立状态和原日志入口，关闭收件箱时也会计入新事项。手动停止、已完成及子运行仍可在工作流历史中查看。 [Cindy 保留中止轮次](https://github.com/makecindy/cindy/blob/1db8327d9cce5c66030c53662b93de4fbb166db6/docs/product-rules/archive-pauses-automations.md#L4-L15) · [Hive 冷启动恢复](https://github.com/tt-a1i/hive/blob/18861b80379bff0d658623a3aa6f1454159d6925/src/server/workflow-run-store.ts#L209-L215)
- **用方向键与 Enter 选模型。** 在会话模型搜索框里按上下键移动高亮并滚动到该行。松开 Enter 时按提供商和模型 ID 应用选择。输入法组合和按键长按使用独立处理，草稿和光标跟随原有焦点恢复流程。 [T3 Code 模型搜索](https://github.com/pingdotgg/t3code/blob/f2ec853fbbb6ed84f6c08aa156d3b7d9e9f7385f/apps/web/src/components/chat/ModelPickerContent.tsx#L941-L985)
- **Markdown 预览可打开当前工作树中的相对文件。** 例如 `./guides/setup%20guide.md` 按当前文档所在目录解析。紧凑和展开预览都可跟随链接，原查询保持。展开预览跟随链接后，把焦点交给文件面板中的目标。预览沿用工作树范围及符号链接检查。 [Beluga 相对链接解析](https://github.com/beluga-labs/beluga/blob/eed1b7ea2ef127479f8fe8773e5516013a6e146f/packages/mac/src/components/markdown/md-paths.ts#L77-L92)

- 运行时用实际父子工作流写入日志，在仍运行时强制结束隔离进程。冷重开后两条运行均标为中断，收件箱明细和提示只收录原父运行，原日志仍可读取。文件专项使用两个真实工作树核对相同路径、工作树切换及外部符号链接边界。
- macOS 跨项目收件箱与 Atlas 的项目收件箱都只显示隔离夹具中应关注的顶层中断运行。两处都能打开「已中断」的原详情和原日志。点击「返回项目收件箱」回到原行。手动停止、完成及中断的子运行均未显示。证据在 `.artifacts/batch36-inbox-native`。
- macOS 在模型框搜索 `Fixture`，方向键高亮第二项后用 Enter 选择。一次确认只为原会话设置一次 `openai / fixture-alternate`，没有发送草稿。返回后在原光标处输入能得到 `BEFOREx after`。恢复原文并冷重开，模型选择和草稿仍在。证据在 `.artifacts/batch36-model-native`。
- macOS 在工作树 A 的紧凑与展开预览中跟随含空格的链接，均显示 A 的内容并保留 `start.md` 查询。切到工作树 B 后同一链接显示 B 的内容。点击越界路径和 `javascript:` 链接后，预览与请求数保持原样。夹具初次因默认配置启动本机 Codex CLI，进程在隔离目录初始化时退出，没有发出模型请求。将夹具配置改为本地模拟进程后重做验收。证据在 `.artifacts/batch36-markdown-native`。
- 320 项 Rust 测试通过，1 项原有忽略。TypeScript、桌面运行时、真实 Hive HTTP 与 SQLite、文件预览及 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十七批

本轮从 Hive Desktop `ef3c9f5` 开始。参考 T3 Code `f2ec853`、Cindy `1db8327`、Beluga `eed1b7e`。Hive 上游 `main` 为 `18861b8`。

- **暂缓失败或中断的工作流。** 项目和跨项目收件箱都可为单个运行设暂缓时间。按工作区和运行 ID 保存，冷启动后仍可打开原日志。同名新运行独立显示。已经暂缓过的运行可按 ID 找回，手动取消或到期后重新进入列表。 [Cindy 错误行操作](https://github.com/makecindy/cindy/blob/1db8327d9cce5c66030c53662b93de4fbb166db6/apps/desktop/src/renderer/features/bots/TeammateInboxRow.tsx#L14-L46)
- **给项目文件夹里的普通新会话设置默认模型。** 在模型菜单里可把当前可用模型设为此项目和 Agent 配置版本的默认值，也可以重置为配置默认。只在新建时应用，已有会话保留各自的模型。 [T3 Code 项目模型](https://github.com/pingdotgg/t3code/blob/f2ec853fbbb6ed84f6c08aa156d3b7d9e9f7385f/apps/web/src/composerDraftStore.ts#L1214-L1233)
- **Markdown 目录链接跳到同一文档的标题。** 紧凑和展开预览共用已渲染文档的标题位置，可以定位 `#repeat-1` 这样的重复标题和 URL 编码的中文标题。目标缺失时保留当前阅读位置。 [Beluga 文档内链接](https://github.com/beluga-labs/beluga/blob/eed1b7ea2ef127479f8fe8773e5516013a6e146f/packages/mac/src/components/markdown/MarkdownDoc.tsx#L225-L228)

- 运行时专项核对工作区范围、最多 64 个精确运行 ID、冷启动后的原日志和近期列表以外的记录。模型专项以本地模拟 Codex 核对两个项目、配置版本、模型被移除后的恢复，以及发送后新草稿仍使用源会话的模型。Markdown 专项覆盖 ATX、Setext、重复标题、编码及代码围栏。
- macOS 跨项目收件箱暂缓一个工作流，同名运行继续显示。写入 21 条较新运行后冷重开，暂缓列表仍能打开原日志，取消暂缓后原运行重新显示。项目收件箱也通过暂缓及取消暂缓实窗验证。精确查询的参数和上限另由运行时专项核对。证据在 `.artifacts/batch37-native-receipt.md`。
- macOS 将 Alpha 的当前模型设为项目默认。普通新会话与冷重开后新建的会话都用 `fixture-alternate` 首次启动，使用同一 Agent 配置的 Beta 仍用配置默认。重置 Alpha 后新会话恢复配置默认，已打开过的会话仍保留 `fixture-alternate`。首次模拟进程因夹具缺少版本文件未连接，补齐后重测。证据在 `.artifacts/batch37-model-native`。
- macOS 在紧凑和展开预览点击 `#repeat-1` 和编码的 `#章節`。布局记录确认两处都实际滚到第二个同名标题和中文标题。不存在的标题与代码围栏中的标题均未触发滚动，目录跳转也没有再次读取文件。诊断记录只用于隔离验收，已从应用源码移除。证据在 `.artifacts/batch37-markdown-native/layout.log`。
- 323 项 Rust 测试通过，1 项原有忽略。TypeScript、桌面运行时、真实 Hive HTTP 与 SQLite、Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十八批

本轮从 Hive Desktop `fd88496` 开始。参考 T3 Code `b985057`、Cindy `1db8327`、Beluga `eed1b7e`。Hive 上游 `main` 为 `18861b8`。

- **跨重启保留工作流新事项的已读状态。** 跨项目收件箱沿用窗口和行可见性规则，按工作区与运行 ID 保存已读。原运行仍可从「需要关注」打开。同名新运行独立提示，暂缓后的唤回可以重新提示。 [Cindy 恢复已读状态](https://github.com/makecindy/cindy/blob/1db8327d9cce5c66030c53662b93de4fbb166db6/apps/desktop/src/renderer/features/bots/botReadState.ts#L38-L56)
- **用键盘翻看终端输出。** Shift+PageUp/PageDown 翻页，macOS 用 Cmd+Shift+Home/End 到首尾，Windows 用 Ctrl+Shift。右键里的「跳到最新输出」也可回到末尾。这些操作在普通终端只滚动当前标签。备用屏幕沿用既有按键路由。 [T3 Code 终端翻页](https://github.com/pingdotgg/t3code/blob/b985057d3f0ae8d368d73f98585aeb639daffbed/docs/user/terminal.md#L33-L40)
- **从 Markdown 链接打开另一文件的指定章节。** 例如 `./setup%20guide.md#linux` 只读取当前工作树里的目标，等解析完成后定位标题。找不到标题时打开文件顶部。紧凑和展开预览沿用文件面板原查询。切换工作树后，先前的读请求不能更新当前预览。 [Beluga 跨文档跳转](https://github.com/beluga-labs/beluga/blob/eed1b7ea2ef127479f8fe8773e5516013a6e146f/packages/mac/src/components/markdown/MarkdownDoc.tsx#L225-L256)

- 专项检查核对收件箱按原工作区和运行 ID 恢复，以及旧版本仍可读取暂缓记录。终端专项核对按键修饰键、首尾裁限以及备用屏幕和输入法的原有路由。文件专项核对编码路径、只读工作树范围和来源目录变化。
- macOS 在跨项目收件箱看到旧运行后冷重开，旧运行没有重新提示，仍可打开原日志。第二条同名运行单独出现在「新事项」。实窗暂缓旧运行后，保存数据清除该运行的已读标记并保留原 ID。到期后的重新提示由 Rust 专项核对。证据在 `.artifacts/batch38-inbox-native`。
- macOS 用两个带 200 行输出的终端标签验证翻页、首尾和右键回最新输出。普通终端的真实输入流只收到验收用的 `q`，备用屏幕仍收到了原 PageUp/PageDown 字节序列。布局诊断只用于隔离验收，已从应用源码移除。证据在 `.artifacts/batch38-markdown-native/app.log` 与 `.artifacts/competitive-ux/thirty-eighth/pty-native-bytes.log`。
- macOS 从工作树 A 的紧凑预览和 B 的展开预览点击相同的带空格链接。每次都只读取当前工作树的目标一次，并滚到各自不同位置的 `Linux` 标题。展开预览跟随后将目标显示在文件面板。缺失标题打开目标文件顶部。另将 A 的读回包延迟 7 秒，切换 B 并打开其源文件，A 回包没有触发滚动，随后 B 仍能打开自己的章节。证据在 `.artifacts/batch38-markdown-native`。
- 326 项桌面 Rust 测试和 9 项终端 Rust 测试通过，1 项桌面测试原有忽略。TypeScript、终端运行时、文件运行时及 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 2026-11-21 第三十九批

本轮从 Hive Desktop `604d3aa` 开始。参考 T3 Code `d984b71`、Cindy `1db8327`、Beluga `eed1b7e`。Hive 上游 `main` 为 `18861b8`。

- **收件箱可查看正在运行的顶层工作流。** 打开项目或跨项目收件箱，展开「等待 Agent」可进入具体运行和日志。每个项目最多读取 20 个运行中的记录。运行成功后从该区移除，失败或中断后原运行进入「需要关注」。新事项沿用原失败提醒规则。 [Cindy 收件箱运行中规则](https://github.com/makecindy/cindy/blob/1db8327d9cce5c66030c53662b93de4fbb166db6/docs/product-rules/task-inbox.md#L29-L46)
- **用键盘在工作区可见区域间切换焦点。** 普通聊天页面用 F6 和 Shift+F6 双向循环。命令面板也能聚焦上一个或下一个区域。已隐藏的区域跳过，弹窗保留焦点。 [T3 Code 焦点切换](https://github.com/pingdotgg/t3code/blob/d984b71385910bd575d4ed10ae6c0d1caf48f008/docs/user/keyboard-focus.md#L13-L21)
- **切回工作树时恢复文件预览。** 同一个窗口记住上次打开的紧凑预览。返回匹配的工作区和工作树后重新读取文件，并恢复之前的位置和显示方式。手动关闭预览后，切回工作树仍保持关闭。 [Beluga 按工作区显示预览](https://github.com/beluga-labs/beluga/blob/eed1b7ea2ef127479f8fe8773e5516013a6e146f/packages/mac/src/layout/panel/FilesTab.tsx#L181-L198)

- macOS 项目和跨项目收件箱显示运行中的根记录。跨项目列表可分别打开 Atlas、Beacon 同名运行的原日志，子运行保持在工作流历史。Atlas 完成后刷新原详情，再返回时记录从列表移除。Beacon 失败后，同一运行 ID 进入「需要关注」并触发新提示。证据在 `.artifacts/batch39-inbox-native`。
- macOS 通过 F6 和 Shift+F6 在输入框、历史、侧栏及文件查询间双向切换。回到输入框后从原光标位置继续输入，文件查询仍保留。关闭文件面板后按键跳过它，在展开的文件预览中按 F6 仍停在弹窗。终端 F6 路由由快捷键和终端专项覆盖。证据在 `.artifacts/batch39-files-native`。
- macOS 打开工作树 A 的同名文件并读到第 2 页。切到 B 后在磁盘修改 A，返回 A 时收到一次精确到 A 工作树的重新读取请求，预览恢复第 2 页并显示新内容。在 B 打开新预览会替换此前记录。点 X 后切走再回来，预览保持关闭。另在 A 发出读取后切到 B，A 的延迟回包没有打开旧预览。证据在 `.artifacts/batch39-files-native`。
- 运行时专项用真实顶层和子工作流核对独立上限、工作区范围、静默完成和同 ID 失败。332 项桌面 Rust 测试、9 项终端 Rust 测试通过，1 项桌面测试原有忽略。TypeScript、桌面运行时、文件运行时和 Windows x64 MSVC 交叉检查通过。Windows 原生交互待实机验收。

## 接下来优先检查

- **在欢迎页指明 Codex 登录的下一步。** 已安装但没有凭据时，当前「配置」打开的是程序输入框。验收从欢迎页聚焦账号检查，未登录时可复制正确引用路径的登录命令，返回欢迎页后能重新检查。使用隔离认证目录和路径带空格的模拟程序。 [T3 Code 欢迎页](https://github.com/pingdotgg/t3code/blob/d984b71385910bd575d4ed10ae6c0d1caf48f008/docs/user/welcome-page.md#L23-L46)
- **在跨项目收件箱筛选工作流记录。** 工作流占满列表时，当前只能逐个项目查问题和回复。验收开关随筛选保存，启用后隐藏工作流行及对应计数，保留回复、提问和「新事项」里的新失败。 [Cindy 隐藏自动化任务筛选](https://github.com/makecindy/cindy/blob/1db8327d9cce5c66030c53662b93de4fbb166db6/apps/desktop/src/renderer/features/cc-agent/sidebar/SidebarFilterPopover.tsx#L891-L898)
- **为展开的 Markdown 阅读器生成目录。** 阅读没有手写目录的长文时，目前只能滚动或查找。验收用键盘选中第二个同名标题和中文子标题，准确定位且无需再次读取文件。 [Beluga 生成目录](https://github.com/beluga-labs/beluga/blob/eed1b7ea2ef127479f8fe8773e5516013a6e146f/packages/mac/src/components/markdown/Outline.tsx#L56-L84)
